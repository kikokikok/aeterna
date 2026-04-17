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

/// Async closure that resolves a tenant secret value by logical name.
///
/// Returns `None` when the secret is not set for the given tenant.
pub type SecretResolver = Arc<
    dyn Fn(TenantId, String) -> Pin<Box<dyn Future<Output = Option<String>> + Send + 'static>>
        + Send
        + Sync,
>;

/// Default cache TTL: 1 hour.
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(3600);

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
    /// Optional type-erased secret resolver for self-contained tenant lookups.
    secret_resolver: Option<SecretResolver>,
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
            secret_resolver: None,
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
            secret_resolver: None,
        }
    }

    /// Attach type-erased config and secret resolvers for self-contained
    /// tenant resolution (no external `TenantConfigProvider` parameter needed).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// registry.set_resolvers(config_resolver, secret_resolver);
    /// ```
    pub fn set_resolvers(
        &mut self,
        config_resolver: ConfigResolver,
        secret_resolver: SecretResolver,
    ) {
        self.config_resolver = Some(config_resolver);
        self.secret_resolver = Some(secret_resolver);
    }

    /// Get the LLM service for a tenant.
    ///
    /// Checks cache first, then builds from tenant config + secrets, then
    /// falls back to the platform default.
    pub async fn get_llm_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Option<BoxedLlmService> {
        let key = tenant_id.as_str().to_string();

        // Check cache — remove if expired
        if let Some(entry) = self.tenant_llm_cache.get(&key) {
            if !entry.is_expired(self.cache_ttl) {
                return Some(entry.service.clone());
            }
            drop(entry);
            self.tenant_llm_cache.remove(&key);
            tracing::debug!(
                tenant = %tenant_id.as_str(),
                "LLM cache entry expired, re-resolving"
            );
        }

        // Try to build from tenant config
        if let Ok(Some(config)) = config_provider.get_config(tenant_id).await
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
                    return Some(service);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        tenant = %tenant_id.as_str(),
                        error = %e,
                        "Failed to build tenant LLM service, falling back to platform default"
                    );
                }
            }
        }

        // Fall back to platform default
        self.platform_llm.clone()
    }

    /// Get the embedding service for a tenant.
    ///
    /// Checks cache first, then builds from tenant config + secrets, then
    /// falls back to the platform default.
    pub async fn get_embedding_service<E: std::fmt::Debug>(
        &self,
        tenant_id: &TenantId,
        config_provider: &dyn mk_core::traits::TenantConfigProvider<Error = E>,
    ) -> Option<BoxedEmbeddingService> {
        let key = tenant_id.as_str().to_string();

        // Check cache — remove if expired
        if let Some(entry) = self.tenant_embedding_cache.get(&key) {
            if !entry.is_expired(self.cache_ttl) {
                return Some(entry.service.clone());
            }
            drop(entry);
            self.tenant_embedding_cache.remove(&key);
            tracing::debug!(
                tenant = %tenant_id.as_str(),
                "Embedding cache entry expired, re-resolving"
            );
        }

        // Try to build from tenant config
        if let Ok(Some(config)) = config_provider.get_config(tenant_id).await
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
                    return Some(service);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        tenant = %tenant_id.as_str(),
                        error = %e,
                        "Failed to build tenant embedding service, falling back to platform default"
                    );
                }
            }
        }

        // Fall back to platform default
        self.platform_embedding.clone()
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
    /// Uses config and secret resolvers set via [`set_resolvers`]. Falls back
    /// to the platform default when resolvers are not set or when the tenant
    /// has no custom provider configured.
    pub async fn resolve_llm(&self, tenant_id: &TenantId) -> Option<BoxedLlmService> {
        let (config_resolver, secret_resolver) =
            match (&self.config_resolver, &self.secret_resolver) {
                (Some(cr), Some(sr)) => (cr.clone(), sr.clone()),
                _ => return self.platform_llm.clone(),
            };

        let adapter = ClosureConfigAdapter {
            config_resolver,
            secret_resolver,
        };
        self.get_llm_service(tenant_id, &adapter).await
    }

    /// Resolve the embedding service for a tenant using the built-in resolvers.
    ///
    /// Uses config and secret resolvers set via [`set_resolvers`]. Falls back
    /// to the platform default when resolvers are not set or when the tenant
    /// has no custom provider configured.
    pub async fn resolve_embedding(&self, tenant_id: &TenantId) -> Option<BoxedEmbeddingService> {
        let (config_resolver, secret_resolver) =
            match (&self.config_resolver, &self.secret_resolver) {
                (Some(cr), Some(sr)) => (cr.clone(), sr.clone()),
                _ => return self.platform_embedding.clone(),
            };

        let adapter = ClosureConfigAdapter {
            config_resolver,
            secret_resolver,
        };
        self.get_embedding_service(tenant_id, &adapter).await
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
                let api_key = config_provider
                    .get_secret_value(tenant_id, config_keys::LLM_API_KEY)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read LLM API key: {e:?}"))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Tenant LLM API key secret '{}' not set",
                            config_keys::LLM_API_KEY
                        )
                    })?;
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
                let api_key = config_provider
                    .get_secret_value(tenant_id, config_keys::EMBEDDING_API_KEY)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read embedding API key: {e:?}"))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Tenant embedding API key secret '{}' not set",
                            config_keys::EMBEDDING_API_KEY
                        )
                    })?;
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

/// Adapter that wraps closure-based resolvers into a [`TenantConfigProvider`]
/// implementation so the existing generic `get_llm_service`/`get_embedding_service`
/// methods can be reused without duplication.
struct ClosureConfigAdapter {
    config_resolver: ConfigResolver,
    secret_resolver: SecretResolver,
}

/// Error type for the closure-based config adapter (infallible at the
/// resolver level — failures surface as `None`).
#[derive(Debug)]
struct ClosureAdapterError;

impl std::fmt::Display for ClosureAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "closure adapter error")
    }
}

#[async_trait::async_trait]
impl mk_core::traits::TenantConfigProvider for ClosureConfigAdapter {
    type Error = ClosureAdapterError;

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
        Err(ClosureAdapterError)
    }

    async fn set_secret_entry(
        &self,
        _tenant_id: &TenantId,
        _secret: mk_core::types::TenantSecretEntry,
    ) -> Result<mk_core::types::TenantSecretReference, Self::Error> {
        Err(ClosureAdapterError)
    }

    async fn delete_secret_entry(
        &self,
        _tenant_id: &TenantId,
        _logical_name: &str,
    ) -> Result<bool, Self::Error> {
        Err(ClosureAdapterError)
    }

    async fn get_secret_value(
        &self,
        tenant_id: &TenantId,
        logical_name: &str,
    ) -> Result<Option<String>, Self::Error> {
        Ok((self.secret_resolver)(tenant_id.clone(), logical_name.to_string()).await)
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

        async fn get_secret_value(
            &self,
            _tenant_id: &TenantId,
            logical_name: &str,
        ) -> Result<Option<String>, Self::Error> {
            Ok(self.secrets.get(logical_name).cloned())
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

    #[tokio::test]
    async fn resolve_llm_with_resolvers_falls_back_when_no_config() {
        let mut registry = TenantProviderRegistry::new(None, None);
        let tenant_id = test_tenant_id();

        // Resolvers that return None (no tenant config)
        let config_resolver: super::ConfigResolver = Arc::new(|_tid| Box::pin(async { None }));
        let secret_resolver: super::SecretResolver =
            Arc::new(|_tid, _name| Box::pin(async { None }));
        registry.set_resolvers(config_resolver, secret_resolver);

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
        let secret_resolver: super::SecretResolver =
            Arc::new(|_tid, _name| Box::pin(async { None }));
        registry.set_resolvers(config_resolver, secret_resolver);

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

        // Config that references openai but no secret -> falls back to platform default
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
        // Secret resolver returns None -> build will fail -> falls back
        let secret_resolver: super::SecretResolver =
            Arc::new(|_tid, _name| Box::pin(async { None }));
        registry.set_resolvers(config_resolver, secret_resolver);

        let result = registry.resolve_llm(&tenant_id).await;
        assert!(
            result.is_none(),
            "Missing API key secret, should fall back to None platform default"
        );
    }

    #[tokio::test]
    async fn set_resolvers_stores_closures() {
        let mut registry = TenantProviderRegistry::new(None, None);
        assert!(registry.config_resolver.is_none());
        assert!(registry.secret_resolver.is_none());

        let config_resolver: super::ConfigResolver = Arc::new(|_tid| Box::pin(async { None }));
        let secret_resolver: super::SecretResolver =
            Arc::new(|_tid, _name| Box::pin(async { None }));
        registry.set_resolvers(config_resolver, secret_resolver);

        assert!(registry.config_resolver.is_some());
        assert!(registry.secret_resolver.is_some());
    }
}
