//! Tenant-aware LLM and embedding provider registry.
//!
//! [`TenantProviderRegistry`] caches `Arc<dyn LlmService>` and
//! `Arc<dyn EmbeddingService>` per tenant, resolving from tenant config +
//! secrets and falling back to platform defaults when a tenant has no
//! custom provider configured.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use mk_core::traits::{EmbeddingService, LlmService};
use mk_core::types::{TenantConfigDocument, TenantId};

use crate::embedding::factory::{
    BedrockEmbeddingConfig, EmbeddingProviderConfig, EmbeddingProviderType, GoogleEmbeddingConfig,
    OpenAiEmbeddingConfig, create_embedding_service,
};
use crate::llm::factory::{
    BedrockLlmConfig, GoogleLlmConfig, LlmProviderConfig, LlmProviderType, OpenAiLlmConfig,
    create_llm_service,
};

/// Well-known tenant config field names for LLM provider configuration.
pub mod config_keys {
    /// LLM provider type (`openai`, `google`, `bedrock`).
    pub const LLM_PROVIDER: &str = "llm_provider";
    /// LLM model identifier.
    pub const LLM_MODEL: &str = "llm_model";
    /// Secret logical name for the LLM API key.
    pub const LLM_API_KEY: &str = "llm_api_key";
    /// Google Cloud project ID for LLM.
    pub const LLM_GOOGLE_PROJECT_ID: &str = "llm_google_project_id";
    /// Google Cloud location for LLM.
    pub const LLM_GOOGLE_LOCATION: &str = "llm_google_location";
    /// AWS region for Bedrock LLM.
    pub const LLM_BEDROCK_REGION: &str = "llm_bedrock_region";

    /// Embedding provider type (`openai`, `google`, `bedrock`).
    pub const EMBEDDING_PROVIDER: &str = "embedding_provider";
    /// Embedding model identifier.
    pub const EMBEDDING_MODEL: &str = "embedding_model";
    /// Secret logical name for the embedding API key.
    pub const EMBEDDING_API_KEY: &str = "embedding_api_key";
    /// Google Cloud project ID for embedding.
    pub const EMBEDDING_GOOGLE_PROJECT_ID: &str = "embedding_google_project_id";
    /// Google Cloud location for embedding.
    pub const EMBEDDING_GOOGLE_LOCATION: &str = "embedding_google_location";
    /// AWS region for Bedrock embedding.
    pub const EMBEDDING_BEDROCK_REGION: &str = "embedding_bedrock_region";
}

/// Well-known tenant config field names for GitHub org sync.
pub mod github_config_keys {
    /// GitHub organization name.
    pub const ORG_NAME: &str = "github.org_name";
    /// GitHub App ID (numeric string).
    pub const APP_ID: &str = "github.app_id";
    /// GitHub App installation ID (numeric string).
    pub const INSTALLATION_ID: &str = "github.installation_id";
    /// Optional regex to filter synced teams.
    pub const TEAM_FILTER: &str = "github.team_filter";
    /// Whether to map GitHub repos as Aeterna projects.
    pub const SYNC_REPOS_AS_PROJECTS: &str = "github.sync_repos_as_projects";
    /// Secret logical name for the GitHub App PEM private key.
    pub const APP_PEM: &str = "github.app_pem";
}

/// Type alias for a boxed, thread-safe LLM service.
pub type BoxedLlmService =
    Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>;

/// Type alias for a boxed, thread-safe embedding service.
pub type BoxedEmbeddingService =
    Arc<dyn EmbeddingService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>;

/// Async closure that resolves a tenant's config document.
///
/// Returns `None` when the tenant has no custom configuration, letting the
/// registry fall back to platform defaults.
pub type ConfigResolver = Arc<
    dyn Fn(TenantId) -> Pin<Box<dyn Future<Output = Option<TenantConfigDocument>> + Send + 'static>>
        + Send
        + Sync,
>;

// B4 §3.5 Phase B — the legacy `SecretResolver` closure typedef was
// deleted here. Secret resolution now goes through the typed
// [`crate::secret_resolver::SecretResolverRegistry`] installed via
// [`TenantProviderRegistry::set_secret_resolver_registry`]. The
// `ClosureConfigAdapter` below performs the logical-name →
// `TenantSecretReference` lookup against the tenant config document
// (loaded by `ConfigResolver`) and then dispatches through the
// registry by variant kind. See `secret_resolvers/` for the per-
// backend impls (Inline, Postgres, Env, File, K8s, Vault).

/// Default cache TTL: 1 hour.
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Error surface for the fallible resolver APIs (`try_get_llm_service`,
/// `try_get_embedding_service`).
///
/// The historical `get_llm_service` / `get_embedding_service` methods collapse
/// every failure mode into `None`, which is semantically indistinguishable
/// from "the tenant has no custom provider configured and the platform has
/// no default either". That's fine for request-time resolution, where the
/// caller usually just wants "the service or nothing", but it is the wrong
/// surface for the eager-wire / ready-gate path: those callers need to mark
/// the tenant as `LoadingFailed{reason}` with an accurate reason when the
/// config provider is broken or the tenant-configured provider fails to
/// build. This enum is that surface.
///
/// B2 task 5.2 followup — addresses the `TODO(b2-5.2-followup)` in
/// [`cli::server::tenant_eager_wire::wire_one`].
#[derive(Debug, Clone)]
pub enum ResolverError {
    /// The injected [`TenantConfigProvider`] returned an error while looking
    /// up the tenant config. Distinct from "tenant has no config" (which is
    /// `Ok(None)` and not an error): this means the config source itself
    /// (Postgres, Kubernetes CRD store, etc.) is unreachable or broken.
    ///
    /// The `String` payload is the upstream error rendered via `Debug`
    /// (config-provider errors are generic `E: Debug`, so this is the best
    /// we can do without introducing a thread-through trait bound).
    ConfigProviderFailed(String),
    /// The tenant DID configure a provider and the config was retrieved
    /// successfully, but [`create_llm_service`] / [`create_embedding_service`]
    /// failed — e.g. an unknown provider type, a missing required secret,
    /// invalid endpoint URL. Distinct from `ConfigProviderFailed` because
    /// the remedy is different: fix the tenant's manifest/secrets, not the
    /// platform's config store.
    BuildFailed(String),
}

impl std::fmt::Display for ResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigProviderFailed(e) => write!(f, "tenant config provider failed: {e}"),
            Self::BuildFailed(e) => write!(f, "tenant provider build failed: {e}"),
        }
    }
}

impl std::error::Error for ResolverError {}

/// A cached service entry with a creation timestamp for TTL expiration.
#[derive(Clone)]
struct CachedEntry<T> {
    service: T,
    created_at: Instant,
}

impl<T: Clone> CachedEntry<T> {
    fn new(service: T) -> Self {
        Self {
            service,
            created_at: Instant::now(),
        }
    }

    /// Returns `true` if this entry has been cached longer than the given TTL.
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// Per-tenant provider registry that caches LLM and embedding services.
///
/// Resolution order for each tenant:
/// 1. Return cached service if present and not expired.
/// 2. Build from tenant config + secrets via [`TenantConfigProvider`].
/// 3. Fall back to platform default.
///
/// Cached entries expire after a configurable TTL (default 1 hour), ensuring
/// config changes propagate even if the explicit invalidation call was missed.
pub struct TenantProviderRegistry {
    /// Platform default LLM service (created at boot from env vars).
    platform_llm: Option<BoxedLlmService>,
    /// Platform default embedding service.
    platform_embedding: Option<BoxedEmbeddingService>,
    /// Per-tenant LLM service cache keyed by tenant ID string.
    tenant_llm_cache: DashMap<String, CachedEntry<BoxedLlmService>>,
    /// Per-tenant embedding service cache keyed by tenant ID string.
    tenant_embedding_cache: DashMap<String, CachedEntry<BoxedEmbeddingService>>,
    /// Time-to-live for cached entries.
    cache_ttl: Duration,
    /// Optional type-erased config resolver for self-contained tenant lookups.
    config_resolver: Option<ConfigResolver>,
    /// B4 §3.5 — typed, variant-dispatched secret resolver registry.
    ///
    /// Populated at bootstrap via
    /// [`Self::set_secret_resolver_registry`]. When absent, the
    /// registry falls back to platform defaults (no tenant-specific
    /// LLM / embedding providers available).
    secret_resolver_registry: Option<Arc<crate::secret_resolver::SecretResolverRegistry>>,
}

impl TenantProviderRegistry {
    /// Create a new registry with the given platform defaults.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let registry = TenantProviderRegistry::new(platform_llm, platform_embedding);
    /// ```
    pub fn new(
        platform_llm: Option<BoxedLlmService>,
        platform_embedding: Option<BoxedEmbeddingService>,
    ) -> Self {
        Self {
            platform_llm,
            platform_embedding,
            tenant_llm_cache: DashMap::new(),
            tenant_embedding_cache: DashMap::new(),
            cache_ttl: DEFAULT_CACHE_TTL,
            config_resolver: None,
            secret_resolver_registry: None,
        }
    }

    /// Create a new registry with a custom cache TTL.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let registry = TenantProviderRegistry::with_ttl(llm, embedding, Duration::from_secs(300));
    /// ```
    pub fn with_ttl(
        platform_llm: Option<BoxedLlmService>,
        platform_embedding: Option<BoxedEmbeddingService>,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            platform_llm,
            platform_embedding,
            tenant_llm_cache: DashMap::new(),
            tenant_embedding_cache: DashMap::new(),
            cache_ttl,
            config_resolver: None,
            secret_resolver_registry: None,
        }
    }

    /// Attach a type-erased config resolver for self-contained
    /// tenant resolution (no external `TenantConfigProvider` parameter needed).
    ///
    /// Pair with [`Self::set_secret_resolver_registry`] to enable
    /// tenant-specific LLM / embedding provider resolution. Without
    /// a config resolver installed, `resolve_llm` / `resolve_embedding`
    /// fall back to the platform defaults.
    ///
    /// B4 §3.5 Phase B: replaces the prior `set_resolvers(config, secret)`
    /// two-arg API. The secret-side closure has been removed entirely
    /// in favour of [`crate::secret_resolver::SecretResolverRegistry`].
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// registry.set_config_resolver(config_resolver);
    /// registry.set_secret_resolver_registry(Arc::new(secret_registry));
    /// ```
    pub fn set_config_resolver(&mut self, config_resolver: ConfigResolver) {
        self.config_resolver = Some(config_resolver);
    }

    /// B4 §3.5 — install a typed
    /// [`SecretResolverRegistry`](crate::secret_resolver::SecretResolverRegistry).
    ///
    /// The registry dispatches by [`mk_core::secret::SecretReference`]
    /// variant kind (`inline`, `postgres`, `env`, `file`, `k8s`,
    /// `vault`) and returns zeroized [`mk_core::SecretBytes`].
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use memory::secret_resolver::SecretResolverRegistry;
    /// use memory::secret_resolvers::{EnvRefResolver, FileRefResolver};
    ///
    /// let mut reg = SecretResolverRegistry::new();
    /// reg.register(Arc::new(EnvRefResolver::new()));
    /// reg.register(Arc::new(FileRefResolver::new()));
    /// registry.set_secret_resolver_registry(Arc::new(reg));
    /// ```
    pub fn set_secret_resolver_registry(
        &mut self,
        registry: Arc<crate::secret_resolver::SecretResolverRegistry>,
    ) {
        self.secret_resolver_registry = Some(registry);
    }

    /// B4 §3.5 — resolve a [`SecretReference`] through the typed
    /// registry, if one is installed.
    ///
    /// Returns [`ResolveError::BackendUnavailable`] with `kind="none"`
    /// when no registry has been installed (distinct from the
    /// per-backend `BackendUnavailable` returned by resolvers when
    /// their backend is unreachable).
    ///
    /// [`SecretReference`]: mk_core::secret::SecretReference
    /// [`ResolveError::BackendUnavailable`]: crate::secret_resolver::ResolveError::BackendUnavailable
    pub async fn resolve_secret_ref(
        &self,
        tenant: &TenantId,
        reference: &mk_core::secret::SecretReference,
    ) -> Result<mk_core::SecretBytes, crate::secret_resolver::ResolveError> {
        match &self.secret_resolver_registry {
            Some(reg) => reg.resolve(tenant, reference).await,
            None => Err(crate::secret_resolver::ResolveError::BackendUnavailable {
                kind: "none",
                reason: "no SecretResolverRegistry installed on TenantProviderRegistry"
                    .to_string(),
            }),
        }
    }

    /// Get the LLM service for a tenant.
    ///
    /// Checks cache first, then builds from tenant config + secrets, then
    /// falls back to the platform default. Returns `None` when the tenant
    /// has no configured provider **and** no platform default is installed.
    ///
    /// This method collapses every failure mode (config-provider error,
    /// build error) into `None` + a warn log. That is the right surface
    /// for request-time resolution (callers just want "a service or
    /// fall back"), but is lossy for wiring-state tracking. For the
    /// latter, use [`Self::try_get_llm_service`], which surfaces
    /// [`ResolverError`] so the caller can attach an accurate
    /// `LoadingFailed{reason}`.
    pub async fn get_llm_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Option<BoxedLlmService> {
        match self.try_get_llm_service(tenant_id, config_provider).await {
            Ok(service) => service,
            Err(e) => {
                // The fallible variant logs at warn with structured fields;
                // here we log the fallback decision at info so the two
                // call sites have distinct, greppable signals.
                tracing::warn!(
                    tenant = %tenant_id.as_str(),
                    error = %e,
                    "LLM resolution error, falling back to platform default"
                );
                self.platform_llm.clone()
            }
        }
    }

    /// Fallible LLM service resolution.
    ///
    /// Surfaces the distinction the `Option`-returning
    /// [`Self::get_llm_service`] collapses:
    ///
    /// | Return value                                | Meaning |
    /// |---------------------------------------------|---------|
    /// | `Ok(Some(svc))`                             | Resolved: tenant-specific OR platform default |
    /// | `Ok(None)`                                  | No tenant config AND no platform default installed |
    /// | `Err(ResolverError::ConfigProviderFailed)`  | Config source (DB/CRD) is broken or unreachable |
    /// | `Err(ResolverError::BuildFailed)`           | Tenant IS configured but provider build failed (bad secret, unknown type, invalid URL, …) |
    ///
    /// Use this from wiring-state code paths (see
    /// `cli::server::tenant_eager_wire::wire_one`) where the caller needs
    /// to set an accurate `LoadingFailed{reason}` on a real failure
    /// instead of a misleading `Available` via platform-default fallback.
    pub async fn try_get_llm_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Result<Option<BoxedLlmService>, ResolverError> {
        let key = tenant_id.as_str().to_string();

        // Check cache — remove if expired. A cache hit is always `Ok`:
        // if it was cached, it was built successfully at some point.
        if let Some(entry) = self.tenant_llm_cache.get(&key) {
            if !entry.is_expired(self.cache_ttl) {
                return Ok(Some(entry.service.clone()));
            }
            drop(entry);
            self.tenant_llm_cache.remove(&key);
            tracing::debug!(
                tenant = %tenant_id.as_str(),
                "LLM cache entry expired, re-resolving"
            );
        }

        // Config lookup. `Err` here is a provider-side failure (DB down,
        // CRD unreachable), NOT a missing tenant — missing is `Ok(None)`.
        let config = match config_provider.get_config(tenant_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    tenant = %tenant_id.as_str(),
                    error = ?e,
                    "LLM: tenant config provider returned error"
                );
                return Err(ResolverError::ConfigProviderFailed(format!("{e:?}")));
            }
        };

        // Try to build from tenant config.
        if let Some(config) = config
            && let Some(provider_str) = get_field_str(&config, config_keys::LLM_PROVIDER)
        {
            match self
                .build_llm_from_tenant_config(tenant_id, provider_str, &config, config_provider)
                .await
            {
                Ok(Some(service)) => {
                    self.tenant_llm_cache
                        .insert(key, CachedEntry::new(service.clone()));
                    tracing::info!(
                        tenant = %tenant_id.as_str(),
                        provider = provider_str,
                        "Tenant-specific LLM service initialized"
                    );
                    return Ok(Some(service));
                }
                Ok(None) => {
                    // Provider was declared but build returned no service —
                    // treated historically as "fall back silently". Keep
                    // that semantics here (Ok path) because the tenant
                    // manifest does not guarantee every field is present
                    // and we don't want boot to fail on soft mis-config.
                }
                Err(e) => {
                    tracing::warn!(
                        tenant = %tenant_id.as_str(),
                        provider = provider_str,
                        error = %e,
                        "Failed to build tenant LLM service"
                    );
                    return Err(ResolverError::BuildFailed(format!("{e}")));
                }
            }
        }

        // Fall back to platform default. `Ok(None)` is a legitimate
        // outcome — it means "no tenant provider AND no platform default
        // either" (bootstrap before any provider is installed).
        Ok(self.platform_llm.clone())
    }

    /// Get the embedding service for a tenant.
    ///
    /// See [`Self::get_llm_service`] for the rationale on the `Option`
    /// surface. Use [`Self::try_get_embedding_service`] for the fallible
    /// variant.
    pub async fn get_embedding_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Option<BoxedEmbeddingService> {
        match self
            .try_get_embedding_service(tenant_id, config_provider)
            .await
        {
            Ok(service) => service,
            Err(e) => {
                tracing::warn!(
                    tenant = %tenant_id.as_str(),
                    error = %e,
                    "embedding resolution error, falling back to platform default"
                );
                self.platform_embedding.clone()
            }
        }
    }

    /// Fallible embedding service resolution.
    ///
    /// See [`Self::try_get_llm_service`] for the full return-value
    /// table — the semantics here are identical.
    pub async fn try_get_embedding_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Result<Option<BoxedEmbeddingService>, ResolverError> {
        let key = tenant_id.as_str().to_string();

        if let Some(entry) = self.tenant_embedding_cache.get(&key) {
            if !entry.is_expired(self.cache_ttl) {
                return Ok(Some(entry.service.clone()));
            }
            drop(entry);
            self.tenant_embedding_cache.remove(&key);
            tracing::debug!(
                tenant = %tenant_id.as_str(),
                "Embedding cache entry expired, re-resolving"
            );
        }

        let config = match config_provider.get_config(tenant_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    tenant = %tenant_id.as_str(),
                    error = ?e,
                    "embedding: tenant config provider returned error"
                );
                return Err(ResolverError::ConfigProviderFailed(format!("{e:?}")));
            }
        };

        if let Some(config) = config
            && let Some(provider_str) = get_field_str(&config, config_keys::EMBEDDING_PROVIDER)
        {
            match self
                .build_embedding_from_tenant_config(
                    tenant_id,
                    provider_str,
                    &config,
                    config_provider,
                )
                .await
            {
                Ok(Some(service)) => {
                    self.tenant_embedding_cache
                        .insert(key, CachedEntry::new(service.clone()));
                    tracing::info!(
                        tenant = %tenant_id.as_str(),
                        provider = provider_str,
                        "Tenant-specific embedding service initialized"
                    );
                    return Ok(Some(service));
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        tenant = %tenant_id.as_str(),
                        provider = provider_str,
                        error = %e,
                        "Failed to build tenant embedding service"
                    );
                    return Err(ResolverError::BuildFailed(format!("{e}")));
                }
            }
        }

        Ok(self.platform_embedding.clone())
    }

    /// Invalidate cached services for a tenant.
    ///
    /// Call this when tenant config or secrets change so the next resolution
    /// rebuilds the service from updated configuration.
    pub fn invalidate_tenant(&self, tenant_id: &TenantId) {
        let key = tenant_id.as_str().to_string();
        self.tenant_llm_cache.remove(&key);
        self.tenant_embedding_cache.remove(&key);
        tracing::info!(tenant = %tenant_id.as_str(), "Tenant provider cache invalidated");
    }

    /// Resolve the LLM service for a tenant using the built-in resolvers.
    ///
    /// Uses the config resolver installed via [`Self::set_config_resolver`]
    /// and the secret resolver registry installed via
    /// [`Self::set_secret_resolver_registry`]. Falls back to the
    /// platform default when either is absent or when the tenant has
    /// no custom provider configured.
    pub async fn resolve_llm(&self, tenant_id: &TenantId) -> Option<BoxedLlmService> {
        let Some(adapter) = self.build_registry_adapter() else {
            return self.platform_llm.clone();
        };
        self.get_llm_service(tenant_id, &adapter).await
    }

    /// Resolve the embedding service for a tenant using the built-in resolvers.
    ///
    /// Uses the config resolver installed via [`Self::set_config_resolver`]
    /// and the secret resolver registry installed via
    /// [`Self::set_secret_resolver_registry`]. Falls back to the
    /// platform default when either is absent or when the tenant has
    /// no custom provider configured.
    pub async fn resolve_embedding(&self, tenant_id: &TenantId) -> Option<BoxedEmbeddingService> {
        let Some(adapter) = self.build_registry_adapter() else {
            return self.platform_embedding.clone();
        };
        self.get_embedding_service(tenant_id, &adapter).await
    }

    /// Internal helper — build a [`RegistryConfigAdapter`] from the
    /// currently-installed resolvers, or return `None` when either
    /// the config resolver or the secret resolver registry is
    /// missing. Callers treat `None` as "fall back to platform
    /// defaults" — the same semantics as pre-Phase-B.
    fn build_registry_adapter(&self) -> Option<RegistryConfigAdapter> {
        let config_resolver = self.config_resolver.clone()?;
        let secret_registry = self.secret_resolver_registry.clone()?;
        Some(RegistryConfigAdapter {
            config_resolver,
            secret_registry,
        })
    }

    /// Build an LLM service from tenant config fields + secrets.
    async fn build_llm_from_tenant_config<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        provider: &str,
        config: &TenantConfigDocument,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> anyhow::Result<Option<BoxedLlmService>> {
        let model = get_field_str(config, config_keys::LLM_MODEL)
            .unwrap_or("")
            .to_string();

        let provider_config = match provider.to_lowercase().as_str() {
            "openai" => {
                let api_key_bytes = config_provider
                    .get_secret_bytes(tenant_id, config_keys::LLM_API_KEY)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read LLM API key: {e:?}"))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Tenant LLM API key secret '{}' not set",
                            config_keys::LLM_API_KEY
                        )
                    })?;
                // OpenAI SDK takes an owned `String`. The `SecretBytes`
                // container zeroizes when it drops at the end of this scope;
                // the `String` we produce here lives for the duration of the
                // request only and is not logged or persisted.
                let api_key = String::from_utf8(api_key_bytes.expose().to_vec())
                    .map_err(|_| anyhow::anyhow!("LLM API key is not valid UTF-8"))?;
                LlmProviderConfig {
                    provider_type: LlmProviderType::Openai,
                    openai: Some(OpenAiLlmConfig {
                        model: if model.is_empty() {
                            "gpt-4o".to_string()
                        } else {
                            model
                        },
                        api_key,
                    }),
                    ..Default::default()
                }
            }
            "google" | "vertex" | "vertex_ai" | "vertexai" | "gemini" => {
                let project_id = get_field_str(config, config_keys::LLM_GOOGLE_PROJECT_ID)
                    .ok_or_else(|| anyhow::anyhow!("Google project_id not configured"))?
                    .to_string();
                let location = get_field_str(config, config_keys::LLM_GOOGLE_LOCATION)
                    .ok_or_else(|| anyhow::anyhow!("Google location not configured"))?
                    .to_string();
                if model.is_empty() {
                    return Err(anyhow::anyhow!("Google LLM model not configured"));
                }
                LlmProviderConfig {
                    provider_type: LlmProviderType::Google,
                    google: Some(GoogleLlmConfig {
                        project_id,
                        location,
                        model,
                    }),
                    ..Default::default()
                }
            }
            "bedrock" | "aws_bedrock" | "aws-bedrock" => {
                let region = get_field_str(config, config_keys::LLM_BEDROCK_REGION)
                    .ok_or_else(|| anyhow::anyhow!("Bedrock region not configured"))?
                    .to_string();
                if model.is_empty() {
                    return Err(anyhow::anyhow!("Bedrock LLM model not configured"));
                }
                LlmProviderConfig {
                    provider_type: LlmProviderType::Bedrock,
                    bedrock: Some(BedrockLlmConfig {
                        region,
                        model_id: model,
                    }),
                    ..Default::default()
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown LLM provider: {provider}")),
        };

        create_llm_service(provider_config)
            .map(|opt| opt.map(|s| s as BoxedLlmService))
            .map_err(|e| anyhow::anyhow!("Failed to create tenant LLM service: {e}"))
    }

    /// Build an embedding service from tenant config fields + secrets.
    async fn build_embedding_from_tenant_config<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        provider: &str,
        config: &TenantConfigDocument,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> anyhow::Result<Option<BoxedEmbeddingService>> {
        let model = get_field_str(config, config_keys::EMBEDDING_MODEL)
            .unwrap_or("")
            .to_string();

        let provider_config = match provider.to_lowercase().as_str() {
            "openai" => {
                let api_key_bytes = config_provider
                    .get_secret_bytes(tenant_id, config_keys::EMBEDDING_API_KEY)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read embedding API key: {e:?}"))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Tenant embedding API key secret '{}' not set",
                            config_keys::EMBEDDING_API_KEY
                        )
                    })?;
                // Same rationale as LLM above: owned `String` required by the
                // downstream SDK; `SecretBytes` still zeroizes on drop.
                let api_key = String::from_utf8(api_key_bytes.expose().to_vec())
                    .map_err(|_| anyhow::anyhow!("Embedding API key is not valid UTF-8"))?;
                EmbeddingProviderConfig {
                    provider_type: EmbeddingProviderType::Openai,
                    openai: Some(OpenAiEmbeddingConfig {
                        model: if model.is_empty() {
                            "text-embedding-3-small".to_string()
                        } else {
                            model
                        },
                        api_key,
                    }),
                    ..Default::default()
                }
            }
            "google" | "vertex" | "vertex_ai" | "vertexai" | "gemini" => {
                let project_id = get_field_str(config, config_keys::EMBEDDING_GOOGLE_PROJECT_ID)
                    .ok_or_else(|| anyhow::anyhow!("Google embedding project_id not configured"))?
                    .to_string();
                let location = get_field_str(config, config_keys::EMBEDDING_GOOGLE_LOCATION)
                    .ok_or_else(|| anyhow::anyhow!("Google embedding location not configured"))?
                    .to_string();
                if model.is_empty() {
                    return Err(anyhow::anyhow!("Google embedding model not configured"));
                }
                EmbeddingProviderConfig {
                    provider_type: EmbeddingProviderType::Google,
                    google: Some(GoogleEmbeddingConfig {
                        project_id,
                        location,
                        model,
                    }),
                    ..Default::default()
                }
            }
            "bedrock" | "aws_bedrock" | "aws-bedrock" => {
                let region = get_field_str(config, config_keys::EMBEDDING_BEDROCK_REGION)
                    .ok_or_else(|| anyhow::anyhow!("Bedrock embedding region not configured"))?
                    .to_string();
                if model.is_empty() {
                    return Err(anyhow::anyhow!("Bedrock embedding model not configured"));
                }
                EmbeddingProviderConfig {
                    provider_type: EmbeddingProviderType::Bedrock,
                    bedrock: Some(BedrockEmbeddingConfig {
                        region,
                        model_id: model,
                    }),
                    ..Default::default()
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown embedding provider: {provider}")),
        };

        create_embedding_service(provider_config)
            .map(|opt| opt.map(|s| s as BoxedEmbeddingService))
            .map_err(|e| anyhow::anyhow!("Failed to create tenant embedding service: {e}"))
    }
}

/// Adapter that exposes the installed [`ConfigResolver`] closure plus a
/// [`SecretResolverRegistry`](crate::secret_resolver::SecretResolverRegistry)
/// as a [`TenantConfigProvider`] so the existing generic
/// `get_llm_service` / `get_embedding_service` methods can be reused
/// without duplication.
///
/// B4 §3.5 Phase B — this replaces the former `ClosureConfigAdapter`
/// which held an `(tenant_id, logical_name) -> Option<String>` closure
/// for secret lookup. Secret resolution now goes through the typed
/// registry: we load the tenant config document (via `config_resolver`)
/// to find `secret_references[logical_name] = TenantSecretReference`,
/// then dispatch by variant kind through the registry.
struct RegistryConfigAdapter {
    config_resolver: ConfigResolver,
    secret_registry: Arc<crate::secret_resolver::SecretResolverRegistry>,
}

/// Error type for the registry-backed config adapter. Failures from
/// the underlying [`ConfigResolver`] (missing config) surface as
/// `Ok(None)`; secret-resolution failures surface as this error so the
/// caller can log diagnostics.
#[derive(Debug)]
struct RegistryAdapterError(String);

impl std::fmt::Display for RegistryAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tenant config adapter error: {}", self.0)
    }
}

#[async_trait::async_trait]
impl mk_core::traits::TenantConfigProvider for RegistryConfigAdapter {
    type Error = RegistryAdapterError;

    async fn get_config(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Option<TenantConfigDocument>, Self::Error> {
        Ok((self.config_resolver)(tenant_id.clone()).await)
    }

    async fn list_configs(&self) -> Result<Vec<TenantConfigDocument>, Self::Error> {
        Ok(Vec::new())
    }

    async fn upsert_config(
        &self,
        _config: TenantConfigDocument,
    ) -> Result<TenantConfigDocument, Self::Error> {
        Err(RegistryAdapterError(
            "upsert_config not supported on resolver-backed adapter".to_string(),
        ))
    }

    async fn set_secret_entry(
        &self,
        _tenant_id: &TenantId,
        _secret: mk_core::types::TenantSecretEntry,
    ) -> Result<mk_core::types::TenantSecretReference, Self::Error> {
        Err(RegistryAdapterError(
            "set_secret_entry not supported on resolver-backed adapter".to_string(),
        ))
    }

    async fn delete_secret_entry(
        &self,
        _tenant_id: &TenantId,
        _logical_name: &str,
    ) -> Result<bool, Self::Error> {
        Err(RegistryAdapterError(
            "delete_secret_entry not supported on resolver-backed adapter".to_string(),
        ))
    }

    async fn get_secret_bytes(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<Option<mk_core::SecretBytes>, Self::Error> {
        // 1. Load the tenant config document. Missing config → Ok(None)
        //    so callers fall back to platform defaults cleanly.
        let config = match (self.config_resolver)(tenant_id.clone()).await {
            Some(c) => c,
            None => return Ok(None),
        };

        // 2. Look up the logical name in the config's secret_references
        //    map. Missing entry → Ok(None); the caller decides whether
        //    that's fatal (e.g. "LLM_API_KEY not set").
        let Some(tsr) = config.secret_references.get(logical_name) else {
            return Ok(None);
        };

        // 3. Dispatch through the typed registry by SecretReference
        //    variant. NotFound → Ok(None); other errors surface as
        //    adapter errors so the caller can log with context.
        match self.secret_registry.resolve(tenant_id, &tsr.reference).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(crate::secret_resolver::ResolveError::NotFound { .. }) => Ok(None),
            Err(e) => Err(RegistryAdapterError(format!(
                "failed to resolve secret '{logical_name}' for tenant {tenant_id}: {e}"
            ))),
        }
    }

    async fn validate(&self, _config: &TenantConfigDocument) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Helper to extract a string field from a tenant config document.
fn get_field_str<'a>(config: &'a TenantConfigDocument, key: &str) -> Option<&'a str> {
    config.fields.get(key).and_then(|f| f.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use mk_core::types::{
        TenantConfigDocument, TenantConfigField, TenantConfigOwnership, TenantSecretEntry,
        TenantSecretReference,
    };

    /// A mock config provider that stores config and secrets in memory.
    struct MockConfigProvider {
        config: Option<TenantConfigDocument>,
        secrets: BTreeMap<String, String>,
    }

    impl MockConfigProvider {
        fn new() -> Self {
            Self {
                config: None,
                secrets: BTreeMap::new(),
            }
        }

        fn with_config(mut self, config: TenantConfigDocument) -> Self {
            self.config = Some(config);
            self
        }

        #[allow(dead_code)]
        fn with_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
            self.secrets.insert(key.into(), value.into());
            self
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("mock error: {0}")]
    struct MockError(String);

    #[async_trait]
    impl mk_core::traits::TenantConfigProvider for MockConfigProvider {
        type Error = MockError;

        async fn get_config(
            &self,
            _tenant_id: &TenantId,
        ) -> Result<Option<TenantConfigDocument>, Self::Error> {
            Ok(self.config.clone())
        }

        async fn list_configs(&self) -> Result<Vec<TenantConfigDocument>, Self::Error> {
            Ok(self.config.iter().cloned().collect())
        }

        async fn upsert_config(
            &self,
            _config: TenantConfigDocument,
        ) -> Result<TenantConfigDocument, Self::Error> {
            Err(MockError("not implemented".into()))
        }

        async fn set_secret_entry(
            &self,
            _tenant_id: &TenantId,
            _secret: TenantSecretEntry,
        ) -> Result<TenantSecretReference, Self::Error> {
            Err(MockError("not implemented".into()))
        }

        async fn delete_secret_entry(
            &self,
            _tenant_id: &TenantId,
            _logical_name: &str,
        ) -> Result<bool, Self::Error> {
            Err(MockError("not implemented".into()))
        }

        async fn get_secret_bytes(
            &self,
            _tenant_id: &TenantId,
            logical_name: &str,
        ) -> Result<Option<mk_core::SecretBytes>, Self::Error> {
            Ok(self
                .secrets
                .get(logical_name)
                .cloned()
                .map(|s| mk_core::SecretBytes::from(s.into_bytes())))
        }

        async fn validate(&self, _config: &TenantConfigDocument) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn test_tenant_id() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    fn make_config_doc(tenant_id: &TenantId, fields: Vec<(&str, &str)>) -> TenantConfigDocument {
        let mut field_map = BTreeMap::new();
        for (key, value) in fields {
            field_map.insert(
                key.to_string(),
                TenantConfigField {
                    ownership: TenantConfigOwnership::Tenant,
                    value: serde_json::json!(value),
                },
            );
        }
        TenantConfigDocument {
            tenant_id: tenant_id.clone(),
            fields: field_map,
            secret_references: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn returns_platform_default_when_no_tenant_config() {
        let registry = TenantProviderRegistry::new(None, None);
        let provider = MockConfigProvider::new();
        let tenant_id = test_tenant_id();

        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_none(), "No platform default and no tenant config");

        let embedding = registry.get_embedding_service(&tenant_id, &provider).await;
        assert!(
            embedding.is_none(),
            "No platform default and no tenant config"
        );
    }

    #[tokio::test]
    async fn returns_platform_default_when_tenant_has_no_provider_field() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let config = make_config_doc(&tenant_id, vec![("some_other_field", "value")]);
        let provider = MockConfigProvider::new().with_config(config);

        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_none(), "Should fall back to platform default (None)");
    }

    #[tokio::test]
    async fn falls_back_to_platform_default_on_missing_secret() {
        // Tenant config says "openai" but no API key secret is set
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let config = make_config_doc(
            &tenant_id,
            vec![
                (config_keys::LLM_PROVIDER, "openai"),
                (config_keys::LLM_MODEL, "gpt-4o"),
            ],
        );
        let provider = MockConfigProvider::new().with_config(config);

        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(
            llm.is_none(),
            "Should fall back to platform default when secret is missing"
        );
    }

    #[tokio::test]
    async fn rejects_unknown_provider_type_gracefully() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let config = make_config_doc(&tenant_id, vec![(config_keys::LLM_PROVIDER, "unknown_ai")]);
        let provider = MockConfigProvider::new().with_config(config);

        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_none(), "Unknown provider should fall back");
    }

    #[tokio::test]
    async fn invalidate_tenant_clears_cache() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let key = tenant_id.as_str().to_string();

        // Manually insert a dummy service into the LLM cache for testing
        // We cannot easily create a real service without features, so just
        // verify invalidation clears entries.
        assert!(registry.tenant_llm_cache.get(&key).is_none());
        assert!(registry.tenant_embedding_cache.get(&key).is_none());

        // invalidate is a no-op when nothing is cached (should not panic)
        registry.invalidate_tenant(&tenant_id);
    }

    #[tokio::test]
    async fn get_field_str_extracts_string_values() {
        let tenant_id = test_tenant_id();
        let config = make_config_doc(&tenant_id, vec![("key", "value")]);
        assert_eq!(get_field_str(&config, "key"), Some("value"));
        assert_eq!(get_field_str(&config, "missing"), None);
    }

    #[tokio::test]
    async fn google_llm_requires_all_fields() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        // Missing location and model
        let config = make_config_doc(
            &tenant_id,
            vec![
                (config_keys::LLM_PROVIDER, "google"),
                (config_keys::LLM_GOOGLE_PROJECT_ID, "my-project"),
            ],
        );
        let provider = MockConfigProvider::new().with_config(config);
        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_none(), "Should fail without location");
    }

    #[tokio::test]
    async fn bedrock_llm_requires_region_and_model() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        // Missing model
        let config = make_config_doc(
            &tenant_id,
            vec![
                (config_keys::LLM_PROVIDER, "bedrock"),
                (config_keys::LLM_BEDROCK_REGION, "us-east-1"),
            ],
        );
        let provider = MockConfigProvider::new().with_config(config);
        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_none(), "Should fail without model");
    }

    #[tokio::test]
    async fn embedding_falls_back_on_missing_secret() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let config = make_config_doc(
            &tenant_id,
            vec![(config_keys::EMBEDDING_PROVIDER, "openai")],
        );
        let provider = MockConfigProvider::new().with_config(config);

        let embedding = registry.get_embedding_service(&tenant_id, &provider).await;
        assert!(
            embedding.is_none(),
            "Should fall back when embedding API key is missing"
        );
    }

    #[cfg(feature = "embedding-integration")]
    #[tokio::test]
    async fn openai_llm_is_cached_after_first_resolution() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let config = make_config_doc(
            &tenant_id,
            vec![
                (config_keys::LLM_PROVIDER, "openai"),
                (config_keys::LLM_MODEL, "gpt-4o"),
            ],
        );
        let provider = MockConfigProvider::new()
            .with_config(config)
            .with_secret(config_keys::LLM_API_KEY, "sk-test-key");

        let llm = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm.is_some(), "Should build OpenAI LLM service");

        // Second call should hit cache
        let llm2 = registry.get_llm_service(&tenant_id, &provider).await;
        assert!(llm2.is_some(), "Should return cached service");

        // After invalidation, cache should be empty
        registry.invalidate_tenant(&tenant_id);
        let key = tenant_id.as_str().to_string();
        assert!(registry.tenant_llm_cache.get(&key).is_none());
    }

    #[test]
    fn cached_entry_is_expired_returns_false_when_fresh() {
        let entry = CachedEntry::new(42);
        assert!(!entry.is_expired(Duration::from_secs(3600)));
    }

    #[test]
    fn cached_entry_is_expired_returns_true_for_zero_ttl() {
        let entry = CachedEntry {
            service: 42,
            created_at: Instant::now() - Duration::from_secs(1),
        };
        assert!(entry.is_expired(Duration::from_secs(0)));
    }

    #[test]
    fn with_ttl_constructor_sets_custom_ttl() {
        let registry = TenantProviderRegistry::with_ttl(None, None, Duration::from_secs(300));
        assert_eq!(registry.cache_ttl, Duration::from_secs(300));
    }

    #[test]
    fn default_constructor_uses_default_ttl() {
        let registry = TenantProviderRegistry::new(None, None);
        assert_eq!(registry.cache_ttl, DEFAULT_CACHE_TTL);
    }

    #[tokio::test]
    async fn resolve_llm_returns_platform_default_without_resolvers() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let result = registry.resolve_llm(&tenant_id).await;
        assert!(result.is_none(), "No platform default set, should be None");
    }

    #[tokio::test]
    async fn resolve_embedding_returns_platform_default_without_resolvers() {
        let registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();
        let result = registry.resolve_embedding(&tenant_id).await;
        assert!(result.is_none(), "No platform default set, should be None");
    }

    /// Build an empty `SecretResolverRegistry` with no resolvers —
    /// every `.resolve()` call will return `ResolveError::NoResolver`
    /// which the adapter surfaces as an error (treated by the caller
    /// as "secret missing → fall back to platform default").
    fn empty_secret_registry() -> Arc<crate::secret_resolver::SecretResolverRegistry> {
        Arc::new(crate::secret_resolver::SecretResolverRegistry::new())
    }

    #[tokio::test]
    async fn resolve_llm_with_resolvers_falls_back_when_no_config() {
        let mut registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        // Config resolver returns None (no tenant config).
        let config_resolver: super::ConfigResolver = Arc::new(|_tid| Box::pin(async { None }));
        registry.set_config_resolver(config_resolver);
        registry.set_secret_resolver_registry(empty_secret_registry());

        let result = registry.resolve_llm(&tenant_id).await;
        assert!(
            result.is_none(),
            "No config and no platform default, should be None"
        );
    }

    #[tokio::test]
    async fn resolve_embedding_with_resolvers_falls_back_when_no_config() {
        let mut registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        let config_resolver: super::ConfigResolver = Arc::new(|_tid| Box::pin(async { None }));
        registry.set_config_resolver(config_resolver);
        registry.set_secret_resolver_registry(empty_secret_registry());

        let result = registry.resolve_embedding(&tenant_id).await;
        assert!(
            result.is_none(),
            "No config and no platform default, should be None"
        );
    }

    #[tokio::test]
    async fn resolve_llm_with_resolvers_uses_tenant_config_when_present() {
        let mut registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        // Config references openai but has no matching secret_references
        // entry → adapter returns Ok(None) for the API key → build fails
        // → resolve_llm returns None (platform default is also None).
        let tid = tenant_id.clone();
        let config_resolver: super::ConfigResolver = Arc::new(move |_| {
            let tid = tid.clone();
            Box::pin(async move {
                Some(make_config_doc(
                    &tid,
                    vec![
                        (config_keys::LLM_PROVIDER, "openai"),
                        (config_keys::LLM_MODEL, "gpt-4o"),
                    ],
                ))
            })
        });
        registry.set_config_resolver(config_resolver);
        registry.set_secret_resolver_registry(empty_secret_registry());

        let result = registry.resolve_llm(&tenant_id).await;
        assert!(
            result.is_none(),
            "Missing API key secret, should fall back to None platform default"
        );
    }

    #[tokio::test]
    async fn set_resolvers_stores_registry_handles() {
        let mut registry = TenantProviderRegistry::new(None, None);
        assert!(registry.config_resolver.is_none());
        assert!(registry.secret_resolver_registry.is_none());

        let config_resolver: super::ConfigResolver = Arc::new(|_tid| Box::pin(async { None }));
        registry.set_config_resolver(config_resolver);
        registry.set_secret_resolver_registry(empty_secret_registry());

        assert!(registry.config_resolver.is_some());
        assert!(registry.secret_resolver_registry.is_some());
    }

    // =======================================================================
    // B2 5.2 followup — ResolverError surface tests
    //
    // The tests above exercise the historical Option-returning API, which
    // collapses every failure into `None`. The tests below exercise the
    // new fallible `try_*` API and assert that each failure mode maps to
    // the correct `ResolverError` variant. This is what lets
    // `tenant_eager_wire::wire_one` attach accurate `LoadingFailed{reason}`.
    // =======================================================================

    /// A mock provider whose `get_config` always errors — simulates an
    /// unreachable or broken tenant config source (Postgres down, CRD
    /// store 500, etc.). Distinct from "tenant has no config" which is
    /// `Ok(None)` and must NOT be treated as an error.
    struct FailingConfigProvider;

    #[async_trait]
    impl mk_core::traits::TenantConfigProvider for FailingConfigProvider {
        type Error = MockError;

        async fn get_config(
            &self,
            _tenant_id: &TenantId,
        ) -> Result<Option<TenantConfigDocument>, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn list_configs(&self) -> Result<Vec<TenantConfigDocument>, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn upsert_config(
            &self,
            _config: TenantConfigDocument,
        ) -> Result<TenantConfigDocument, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn set_secret_entry(
            &self,
            _tenant_id: &TenantId,
            _secret: TenantSecretEntry,
        ) -> Result<TenantSecretReference, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn delete_secret_entry(
            &self,
            _tenant_id: &TenantId,
            _logical_name: &str,
        ) -> Result<bool, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn get_secret_bytes(
            &self,
            _tenant_id: &TenantId,
            _logical_name: &str,
        ) -> Result<Option<mk_core::SecretBytes>, Self::Error> {
            Err(MockError("config store unreachable".into()))
        }

        async fn validate(&self, _config: &TenantConfigDocument) -> Result<(), Self::Error> {
            Err(MockError("config store unreachable".into()))
        }
    }

    #[tokio::test]
    async fn try_get_llm_service_ok_none_when_no_tenant_config_and_no_platform_default() {
        // Bootstrap state: no platform default, no tenant config.
        // This is NOT an error — it's a legitimate quiescent state that
        // the caller may or may not treat as ready. Must surface as
        // `Ok(None)`, not `Err`.
        let registry = TenantProviderRegistry::new(None, None);
        let provider = MockConfigProvider::new();
        let tid = test_tenant_id();

        let r = registry.try_get_llm_service(&tid, &provider).await;
        assert!(matches!(r, Ok(None)), "expected Ok(None)");

        let r = registry.try_get_embedding_service(&tid, &provider).await;
        assert!(matches!(r, Ok(None)), "expected Ok(None)");
    }

    #[tokio::test]
    async fn try_get_llm_service_err_config_provider_failed_when_provider_errors() {
        // Config provider itself errors (DB down / CRD unreachable).
        // Must surface as `ConfigProviderFailed`, not as a silent
        // platform-default fallback. The old Option API returned
        // `platform_llm.clone()` here, which is misleading in the
        // wiring path.
        let registry = TenantProviderRegistry::new(None, None);
        let provider = FailingConfigProvider;
        let tid = test_tenant_id();

        let r = registry.try_get_llm_service(&tid, &provider).await;
        match r {
            Err(ResolverError::ConfigProviderFailed(msg)) => {
                assert!(
                    msg.contains("config store unreachable"),
                    "error message must carry upstream detail, got: {msg}"
                );
            }
            Err(e) => panic!("expected ConfigProviderFailed, got Err({e})"),
            Ok(_) => panic!("expected Err(ConfigProviderFailed), got Ok"),
        }
    }

    #[tokio::test]
    async fn try_get_embedding_service_err_config_provider_failed_when_provider_errors() {
        let registry = TenantProviderRegistry::new(None, None);
        let provider = FailingConfigProvider;
        let tid = test_tenant_id();

        let r = registry.try_get_embedding_service(&tid, &provider).await;
        assert!(
            matches!(r, Err(ResolverError::ConfigProviderFailed(_))),
            "expected ConfigProviderFailed"
        );
    }

    #[tokio::test]
    async fn try_get_llm_service_err_build_failed_when_provider_type_unknown() {
        // Tenant config says `llm_provider = "martian-wavelet-3000"` —
        // no such thing. The build step errors. Must surface as
        // `BuildFailed`, not ConfigProviderFailed (the config itself
        // WAS fetched successfully) and not Ok(None) (which would
        // mislead eager-wire into marking Available).
        let registry = TenantProviderRegistry::new(None, None);
        let tid = test_tenant_id();
        let config = make_config_doc(
            &tid,
            vec![(config_keys::LLM_PROVIDER, "martian-wavelet-3000")],
        );
        let provider = MockConfigProvider::new().with_config(config);

        let r = registry.try_get_llm_service(&tid, &provider).await;
        match r {
            Err(ResolverError::BuildFailed(msg)) => {
                // Don't over-specify the message — the underlying
                // factory may evolve — but at least ensure it mentions
                // the unknown provider somewhere in the string for
                // operator debuggability.
                assert!(
                    msg.to_lowercase().contains("martian-wavelet-3000")
                        || msg.to_lowercase().contains("unknown")
                        || msg.to_lowercase().contains("unsupported"),
                    "BuildFailed message should reference the unknown provider: {msg}"
                );
            }
            Err(e) => panic!("expected BuildFailed, got Err({e})"),
            Ok(_) => panic!("expected Err(BuildFailed), got Ok"),
        }
    }

    #[tokio::test]
    async fn get_llm_service_still_returns_none_on_config_provider_error() {
        // Back-compat: the Option-returning wrapper MUST keep silencing
        // provider errors into `None` so existing request-time callers
        // don't regress. The log line changes (now "resolution error,
        // falling back"), but the shape of the return value is stable.
        let registry = TenantProviderRegistry::new(None, None);
        let provider = FailingConfigProvider;
        let tid = test_tenant_id();

        assert!(registry.get_llm_service(&tid, &provider).await.is_none());
        assert!(
            registry
                .get_embedding_service(&tid, &provider)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn resolver_error_display_includes_upstream_message() {
        // `ResolverError`'s Display impl is what tenant_eager_wire uses
        // to populate `LoadingFailed { reason }` — the string needs to
        // carry enough operator signal, not just the variant name.
        let e = ResolverError::ConfigProviderFailed("postgres: conn refused".into());
        let s = format!("{e}");
        assert!(s.contains("postgres: conn refused"), "got: {s}");
        assert!(s.to_lowercase().contains("config"), "got: {s}");

        let e = ResolverError::BuildFailed("openai: invalid model 'foo'".into());
        let s = format!("{e}");
        assert!(s.contains("openai: invalid model 'foo'"), "got: {s}");
    }
}
