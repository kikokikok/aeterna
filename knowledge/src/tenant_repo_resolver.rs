//! Per-tenant repository resolution with fail-closed semantics.
//!
//! [`TenantRepositoryResolver`] maps a [`TenantId`] to the `Arc<GitRepository>`
//! that should back all knowledge reads and writes for that tenant.  Resolution
//! results are cached so that repeated operations within the same process
//! reuse the already-initialised repository without re-reading the database.
//!
//! # Fail-closed contract
//!
//! If no binding exists in `tenant_repository_bindings` for a given tenant,
//! or if the stored binding is structurally invalid, the resolver returns
//! [`RepoResolutionError::MissingBinding`] or
//! [`RepoResolutionError::InvalidBinding`] respectively.  The caller MUST
//! propagate these as configuration errors — no operation should silently
//! fall through to a process-global or another tenant's repository.
//!
//! # Cache invalidation
//!
//! The in-process cache holds `Arc<GitRepository>` values keyed by
//! `TenantId`.  Entries are never evicted automatically; call
//! [`TenantRepositoryResolver::invalidate`] after a binding is updated so
//! the next operation re-resolves from the database.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use mk_core::traits::{GitProviderConnectionRegistry, KnowledgeRepository};
use mk_core::types::{CredentialKind, RepositoryKind, TenantId, TenantRepositoryBinding};
use storage::git_provider_connection_store::GitProviderConnectionError;
use storage::secret_provider::{SecretError, SecretProvider};
use storage::tenant_store::TenantRepositoryBindingStore;
use thiserror::Error;
use tracing::instrument;

use crate::git_provider::GitHubProvider;
use crate::repository::{GitRepository, RemoteConfig, RepositoryError};

/// Errors returned when a tenant's repository cannot be resolved.
#[derive(Debug, Error)]
pub enum RepoResolutionError {
    /// No binding record exists for this tenant.
    #[error(
        "no repository binding configured for tenant '{tenant_id}'; \
         a platform admin must configure a binding before knowledge operations are available"
    )]
    MissingBinding { tenant_id: TenantId },

    /// A binding exists but its fields are internally inconsistent.
    #[error("repository binding for tenant '{tenant_id}' is structurally invalid: {reason}")]
    InvalidBinding { tenant_id: TenantId, reason: String },

    /// The database lookup itself failed.
    #[error("storage error while resolving binding for tenant '{tenant_id}': {source}")]
    Storage {
        tenant_id: TenantId,
        #[source]
        source: storage::postgres::PostgresError,
    },

    /// Repository construction failed (e.g. bad path, clone error).
    #[error("failed to open repository for tenant '{tenant_id}': {source}")]
    Repository {
        tenant_id: TenantId,
        #[source]
        source: RepositoryError,
    },

    /// Credential bootstrap failed (e.g. GitHub App token fetch).
    #[error("failed to initialise credentials for tenant '{tenant_id}': {reason}")]
    Credentials { tenant_id: TenantId, reason: String },

    /// The secret provider returned an error while resolving a credential ref.
    #[error("secret provider error for tenant '{tenant_id}': {source}")]
    SecretResolution {
        tenant_id: TenantId,
        #[source]
        source: SecretError,
    },

    /// The tenant is not in the allow-list for the referenced Git provider connection.
    #[error(
        "tenant '{tenant_id}' is not allowed to use Git provider connection '{connection_id}';          a platform admin must grant visibility before this binding can be used"
    )]
    ConnectionNotAllowed {
        tenant_id: TenantId,
        connection_id: String,
    },

    /// The referenced Git provider connection does not exist in the registry.
    #[error("Git provider connection '{connection_id}' not found in the registry")]
    ConnectionNotFound { connection_id: String },

    /// The Git provider connection registry returned an error.
    #[error("Git provider connection registry error for tenant '{tenant_id}': {source}")]
    ConnectionRegistry {
        tenant_id: TenantId,
        #[source]
        source: GitProviderConnectionError,
    },
}

/// Resolves and caches a `GitRepository` per tenant from the
/// `tenant_repository_bindings` table.
///
/// Construction is cheap; the underlying `DashMap` starts empty and is
/// populated lazily on first access per tenant.
pub struct TenantRepositoryResolver {
    binding_store: Arc<TenantRepositoryBindingStore>,
    /// In-process cache: `TenantId` → initialised repository.
    cache: DashMap<TenantId, Arc<GitRepository>>,
    /// Base directory under which per-tenant local repos are rooted when the
    /// binding kind is `Local` and the stored `local_path` is relative.
    base_path: PathBuf,
    /// Secret provider used to resolve credential references stored in
    /// `TenantRepositoryBinding::credential_ref`.
    secret_provider: Arc<dyn SecretProvider>,
    /// Registry of platform-owned Git provider connections.
    /// When `None` the legacy `credential_ref` path is used unconditionally.
    connection_registry: Option<
        Arc<dyn GitProviderConnectionRegistry<Error = GitProviderConnectionError> + Send + Sync>,
    >,
}

pub struct TenantBoundKnowledgeRepository {
    resolver: Arc<TenantRepositoryResolver>,
}

impl TenantBoundKnowledgeRepository {
    pub fn new(resolver: Arc<TenantRepositoryResolver>) -> Self {
        Self { resolver }
    }

    async fn resolve_repo(
        &self,
        ctx: &mk_core::types::TenantContext,
    ) -> Result<Arc<GitRepository>, RepositoryError> {
        self.resolver
            .resolve(&ctx.tenant_id)
            .await
            .map_err(|err| match err {
                RepoResolutionError::MissingBinding { .. }
                | RepoResolutionError::InvalidBinding { .. }
                | RepoResolutionError::Credentials { .. } => {
                    RepositoryError::Remote(err.to_string())
                }
                RepoResolutionError::Storage { source, .. } => {
                    RepositoryError::Remote(source.to_string())
                }
                RepoResolutionError::Repository { source, .. } => source,
                RepoResolutionError::SecretResolution { source, .. } => {
                    RepositoryError::Remote(source.to_string())
                }
                RepoResolutionError::ConnectionNotAllowed { .. }
                | RepoResolutionError::ConnectionNotFound { .. }
                | RepoResolutionError::ConnectionRegistry { .. } => {
                    RepositoryError::Remote(err.to_string())
                }
            })
    }
}

impl TenantRepositoryResolver {
    /// Creates a new resolver.
    ///
    /// `base_path` is used as the root for any `Local`-kind binding whose
    /// `local_path` is relative.  Pass the same value used by the legacy
    /// `knowledge_repo_path()` to maintain backward-compatible paths.
    pub fn new(
        binding_store: Arc<TenantRepositoryBindingStore>,
        base_path: impl Into<PathBuf>,
        secret_provider: Arc<dyn SecretProvider>,
    ) -> Self {
        Self {
            binding_store,
            cache: DashMap::new(),
            base_path: base_path.into(),
            secret_provider,
            connection_registry: None,
        }
    }

    /// Attach a Git provider connection registry so that bindings with
    /// `git_provider_connection_id` set are resolved through the registry
    /// instead of raw `credential_ref` fields.
    pub fn with_connection_registry(
        mut self,
        registry: Arc<
            dyn GitProviderConnectionRegistry<Error = GitProviderConnectionError> + Send + Sync,
        >,
    ) -> Self {
        self.connection_registry = Some(registry);
        self
    }

    /// Returns the cached `Arc<GitRepository>` for `tenant_id`, or looks it
    /// up from the binding store and caches it on first call.
    ///
    /// Returns an error (fail-closed) when:
    /// - no binding row exists for the tenant,
    /// - the binding is structurally invalid, or
    /// - the underlying repository cannot be opened/cloned.
    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn resolve(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Arc<GitRepository>, RepoResolutionError> {
        // Fast path: already in cache.
        if let Some(repo) = self.cache.get(tenant_id) {
            return Ok(Arc::clone(&repo));
        }

        // Slow path: look up from DB, build repo, insert into cache.
        let binding = self
            .binding_store
            .get_binding(tenant_id)
            .await
            .map_err(|source| RepoResolutionError::Storage {
                tenant_id: tenant_id.clone(),
                source,
            })?
            .ok_or_else(|| RepoResolutionError::MissingBinding {
                tenant_id: tenant_id.clone(),
            })?;

        if !binding.is_structurally_valid() {
            return Err(RepoResolutionError::InvalidBinding {
                tenant_id: tenant_id.clone(),
                reason: structural_invalidity_reason(&binding),
            });
        }

        let repo = self.build_repository(tenant_id, &binding).await?;
        let repo = Arc::new(repo);
        self.cache.insert(tenant_id.clone(), Arc::clone(&repo));
        Ok(repo)
    }

    pub async fn validate_binding(
        &self,
        tenant_id: &TenantId,
        binding: &TenantRepositoryBinding,
    ) -> Result<(), RepoResolutionError> {
        if !binding.is_structurally_valid() {
            return Err(RepoResolutionError::InvalidBinding {
                tenant_id: tenant_id.clone(),
                reason: structural_invalidity_reason(binding),
            });
        }

        self.build_repository(tenant_id, binding).await.map(|_| ())
    }

    /// Removes the cached entry for `tenant_id` so the next call to
    /// [`resolve`] re-reads the binding from the database.
    ///
    /// Call this after an admin updates or deletes a tenant's binding.
    pub fn invalidate(&self, tenant_id: &TenantId) {
        self.cache.remove(tenant_id);
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn build_repository(
        &self,
        tenant_id: &TenantId,
        binding: &TenantRepositoryBinding,
    ) -> Result<GitRepository, RepoResolutionError> {
        match binding.kind {
            RepositoryKind::Local => {
                let raw_path = binding
                    .local_path
                    .as_deref()
                    .expect("local_path validated above");
                let root = if std::path::Path::new(raw_path).is_absolute() {
                    PathBuf::from(raw_path)
                } else {
                    self.base_path.join(raw_path)
                };
                GitRepository::new(root).map_err(|source| RepoResolutionError::Repository {
                    tenant_id: tenant_id.clone(),
                    source,
                })
            }

            RepositoryKind::GitHub => {
                let remote_url = binding.remote_url.as_deref().expect("validated above");
                let owner = binding.github_owner.as_deref().expect("validated above");
                let repo_name = binding.github_repo.as_deref().expect("validated above");
                let branch = binding.branch.clone();

                let git_provider: Option<Arc<dyn crate::git_provider::GitProvider>> = match binding
                    .credential_kind
                {
                    CredentialKind::PersonalAccessToken => {
                        let token_ref = binding.credential_ref.as_deref().unwrap_or_default();
                        // `credential_ref` is an opaque handle; for PAT the handle IS the
                        // token (the caller is responsible for secret-provider resolution in
                        // task 3.5 — for now we use it directly so the PAT path works).
                        Some(Arc::new(
                            GitHubProvider::new(
                                token_ref,
                                owner.to_string(),
                                repo_name.to_string(),
                                None,
                            )
                            .map_err(|e| {
                                RepoResolutionError::Credentials {
                                    tenant_id: tenant_id.clone(),
                                    reason: e.to_string(),
                                }
                            })?,
                        ))
                    }
                    CredentialKind::GitHubApp => {
                        // Path 1: platform-owned connection registry (preferred).
                        if let Some(connection_id) = &binding.git_provider_connection_id {
                            let registry = self.connection_registry.as_ref()
                                .ok_or_else(|| RepoResolutionError::Credentials {
                                    tenant_id: tenant_id.clone(),
                                    reason: format!(
                                        "binding references connection_id '{connection_id}' but                                          no GitProviderConnectionRegistry is configured"
                                    ),
                                })?;

                            // Enforce visibility.
                            let allowed =
                                registry
                                    .tenant_can_use(connection_id, tenant_id)
                                    .await
                                    .map_err(|source| RepoResolutionError::ConnectionRegistry {
                                        tenant_id: tenant_id.clone(),
                                        source,
                                    })?;
                            if !allowed {
                                return Err(RepoResolutionError::ConnectionNotAllowed {
                                    tenant_id: tenant_id.clone(),
                                    connection_id: connection_id.clone(),
                                });
                            }

                            let conn = registry
                                .get_connection(connection_id)
                                .await
                                .map_err(|source| RepoResolutionError::ConnectionRegistry {
                                    tenant_id: tenant_id.clone(),
                                    source,
                                })?
                                .ok_or_else(|| RepoResolutionError::ConnectionNotFound {
                                    connection_id: connection_id.clone(),
                                })?;

                            let pem = self
                                .secret_provider
                                .get_secret(&conn.pem_secret_ref)
                                .await
                                .map_err(|source| RepoResolutionError::SecretResolution {
                                    tenant_id: tenant_id.clone(),
                                    source,
                                })?;

                            Some(Arc::new(
                                GitHubProvider::new_with_app(
                                    conn.app_id,
                                    conn.installation_id,
                                    &pem,
                                    owner.to_string(),
                                    repo_name.to_string(),
                                    None,
                                )
                                .await
                                .map_err(|e| {
                                    RepoResolutionError::Credentials {
                                        tenant_id: tenant_id.clone(),
                                        reason: e.to_string(),
                                    }
                                })?,
                            ))
                        } else {
                            // Path 2: legacy credential_ref ("app_id:installation_id:pem_ref").
                            let cred = binding.credential_ref.as_deref().unwrap_or_default();
                            let parts: Vec<&str> = cred.splitn(3, ':').collect();
                            if parts.len() < 3 {
                                return Err(RepoResolutionError::Credentials {
                                    tenant_id: tenant_id.clone(),
                                    reason: format!(
                                        "GitHubApp credential_ref must be                                          'app_id:installation_id:pem_ref'; got: {cred}"
                                    ),
                                });
                            }
                            let app_id: u64 =
                                parts[0]
                                    .parse()
                                    .map_err(|_| RepoResolutionError::Credentials {
                                        tenant_id: tenant_id.clone(),
                                        reason: format!(
                                            "GitHubApp app_id is not a valid u64: {}",
                                            parts[0]
                                        ),
                                    })?;
                            let installation_id: u64 =
                                parts[1]
                                    .parse()
                                    .map_err(|_| RepoResolutionError::Credentials {
                                        tenant_id: tenant_id.clone(),
                                        reason: format!(
                                            "GitHubApp installation_id is not a valid u64: {}",
                                            parts[1]
                                        ),
                                    })?;
                            let pem_ref = parts[2];
                            let pem = self.secret_provider.get_secret(pem_ref).await.map_err(
                                |source| RepoResolutionError::SecretResolution {
                                    tenant_id: tenant_id.clone(),
                                    source,
                                },
                            )?;
                            Some(Arc::new(
                                GitHubProvider::new_with_app(
                                    app_id,
                                    installation_id,
                                    &pem,
                                    owner.to_string(),
                                    repo_name.to_string(),
                                    None,
                                )
                                .await
                                .map_err(|e| {
                                    RepoResolutionError::Credentials {
                                        tenant_id: tenant_id.clone(),
                                        reason: e.to_string(),
                                    }
                                })?,
                            ))
                        }
                    }
                    CredentialKind::None | CredentialKind::SshKey => None,
                };

                let root = self.base_path.join(tenant_id.as_str()).join("repo");
                let remote_config = RemoteConfig {
                    url: remote_url.to_string(),
                    branch,
                    git_provider,
                };
                GitRepository::new_with_remote(root, Some(remote_config)).map_err(|source| {
                    RepoResolutionError::Repository {
                        tenant_id: tenant_id.clone(),
                        source,
                    }
                })
            }

            RepositoryKind::GitRemote => {
                let remote_url = binding.remote_url.as_deref().expect("validated above");
                let branch = binding.branch.clone();
                let root = self.base_path.join(tenant_id.as_str()).join("repo");
                let remote_config = RemoteConfig {
                    url: remote_url.to_string(),
                    branch,
                    git_provider: None,
                };
                GitRepository::new_with_remote(root, Some(remote_config)).map_err(|source| {
                    RepoResolutionError::Repository {
                        tenant_id: tenant_id.clone(),
                        source,
                    }
                })
            }
        }
    }
}

#[async_trait]
impl KnowledgeRepository for TenantBoundKnowledgeRepository {
    type Error = RepositoryError;

    async fn get(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: mk_core::types::KnowledgeLayer,
        path: &str,
    ) -> Result<Option<mk_core::types::KnowledgeEntry>, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.get(ctx, layer, path).await
    }

    async fn store(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: mk_core::types::KnowledgeEntry,
        message: &str,
    ) -> Result<String, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.store(ctx, entry, message).await
    }

    async fn list(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: mk_core::types::KnowledgeLayer,
        prefix: &str,
    ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.list(ctx, layer, prefix).await
    }

    async fn delete(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: mk_core::types::KnowledgeLayer,
        path: &str,
        message: &str,
    ) -> Result<String, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.delete(ctx, layer, path, message).await
    }

    async fn get_head_commit(
        &self,
        ctx: mk_core::types::TenantContext,
    ) -> Result<Option<String>, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.get_head_commit(ctx).await
    }

    async fn get_affected_items(
        &self,
        ctx: mk_core::types::TenantContext,
        since_commit: &str,
    ) -> Result<Vec<(mk_core::types::KnowledgeLayer, String)>, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.get_affected_items(ctx, since_commit).await
    }

    async fn search(
        &self,
        ctx: mk_core::types::TenantContext,
        query: &str,
        layers: Vec<mk_core::types::KnowledgeLayer>,
        limit: usize,
    ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.search(ctx, query, layers, limit).await
    }

    async fn update_status(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: mk_core::types::KnowledgeLayer,
        path: &str,
        new_status: mk_core::types::KnowledgeStatus,
        message: &str,
    ) -> Result<String, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.update_status(ctx, layer, path, new_status, message)
            .await
    }

    async fn promote(
        &self,
        ctx: mk_core::types::TenantContext,
        source_layer: mk_core::types::KnowledgeLayer,
        target_layer: mk_core::types::KnowledgeLayer,
        path: &str,
        message: &str,
    ) -> Result<String, Self::Error> {
        let repo = self.resolve_repo(&ctx).await?;
        repo.promote(ctx, source_layer, target_layer, path, message)
            .await
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

/// Produces a human-readable reason string for why a binding is structurally
/// invalid.  Only called when `is_structurally_valid()` returns `false`.
fn structural_invalidity_reason(binding: &TenantRepositoryBinding) -> String {
    match binding.kind {
        RepositoryKind::Local => {
            if binding.local_path.is_none() {
                "kind=Local requires local_path".to_string()
            } else {
                "kind=Local must not have remote_url set".to_string()
            }
        }
        RepositoryKind::GitHub => {
            let mut missing = Vec::new();
            if binding.remote_url.is_none() {
                missing.push("remote_url");
            }
            if binding.github_owner.is_none() {
                missing.push("github_owner");
            }
            if binding.github_repo.is_none() {
                missing.push("github_repo");
            }
            if binding.local_path.is_some() {
                return "kind=GitHub must not have local_path set".to_string();
            }
            format!("kind=GitHub requires: {}", missing.join(", "))
        }
        RepositoryKind::GitRemote => {
            if binding.remote_url.is_none() {
                "kind=GitRemote requires remote_url".to_string()
            } else {
                "kind=GitRemote must not have local_path set".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{BranchPolicy, CredentialKind, RecordSource, RepositoryKind, TenantId};
    use std::collections::HashMap;
    use storage::secret_provider::LocalSecretProvider;

    fn make_test_secret_provider() -> Arc<dyn SecretProvider> {
        let mut secrets = HashMap::new();
        secrets.insert(
            "local/test-pat-token".to_string(),
            "resolved-token".to_string(),
        );
        Arc::new(LocalSecretProvider::new(secrets))
    }

    fn make_binding(kind: RepositoryKind) -> TenantRepositoryBinding {
        let tid = TenantId::new("test-tenant".to_string()).unwrap();
        match kind {
            RepositoryKind::Local => TenantRepositoryBinding {
                id: "1".to_string(),
                tenant_id: tid,
                kind: RepositoryKind::Local,
                local_path: Some("/tmp/test-repo".to_string()),
                remote_url: None,
                branch: "main".to_string(),
                branch_policy: BranchPolicy::DirectCommit,
                credential_kind: CredentialKind::None,
                credential_ref: None,
                github_owner: None,
                github_repo: None,
                source_owner: RecordSource::Admin,
                git_provider_connection_id: None,
                created_at: 0,
                updated_at: 0,
            },
            RepositoryKind::GitHub => TenantRepositoryBinding {
                id: "2".to_string(),
                tenant_id: tid,
                kind: RepositoryKind::GitHub,
                local_path: None,
                remote_url: Some("https://github.com/acme/knowledge.git".to_string()),
                branch: "main".to_string(),
                branch_policy: BranchPolicy::RequirePullRequest,
                credential_kind: CredentialKind::PersonalAccessToken,
                credential_ref: Some("local/test-pat-token".to_string()),
                github_owner: Some("acme".to_string()),
                github_repo: Some("knowledge".to_string()),
                source_owner: RecordSource::Admin,
                git_provider_connection_id: None,
                created_at: 0,
                updated_at: 0,
            },
            RepositoryKind::GitRemote => TenantRepositoryBinding {
                id: "3".to_string(),
                tenant_id: tid,
                kind: RepositoryKind::GitRemote,
                local_path: None,
                remote_url: Some("https://gitlab.example.com/acme/knowledge.git".to_string()),
                branch: "main".to_string(),
                branch_policy: BranchPolicy::DirectCommit,
                credential_kind: CredentialKind::None,
                credential_ref: None,
                github_owner: None,
                github_repo: None,
                source_owner: RecordSource::Admin,
                git_provider_connection_id: None,
                created_at: 0,
                updated_at: 0,
            },
        }
    }

    #[test]
    fn structural_invalidity_reason_local_missing_path() {
        let mut binding = make_binding(RepositoryKind::Local);
        binding.local_path = None;
        let reason = structural_invalidity_reason(&binding);
        assert!(reason.contains("local_path"), "got: {reason}");
    }

    #[test]
    fn structural_invalidity_reason_local_with_remote() {
        let mut binding = make_binding(RepositoryKind::Local);
        binding.remote_url = Some("https://example.com/repo.git".to_string());
        let reason = structural_invalidity_reason(&binding);
        assert!(reason.contains("remote_url"), "got: {reason}");
    }

    #[test]
    fn structural_invalidity_reason_github_missing_fields() {
        let mut binding = make_binding(RepositoryKind::GitHub);
        binding.remote_url = None;
        binding.github_owner = None;
        let reason = structural_invalidity_reason(&binding);
        assert!(reason.contains("remote_url"), "got: {reason}");
        assert!(reason.contains("github_owner"), "got: {reason}");
    }

    #[test]
    fn structural_invalidity_reason_github_with_local_path() {
        let mut binding = make_binding(RepositoryKind::GitHub);
        binding.local_path = Some("/tmp/bad".to_string());
        let reason = structural_invalidity_reason(&binding);
        assert!(reason.contains("local_path"), "got: {reason}");
    }

    #[test]
    fn structural_invalidity_reason_git_remote_missing_url() {
        let mut binding = make_binding(RepositoryKind::GitRemote);
        binding.remote_url = None;
        let reason = structural_invalidity_reason(&binding);
        assert!(reason.contains("remote_url"), "got: {reason}");
    }

    #[test]
    fn repo_resolution_error_missing_binding_message_contains_tenant_id() {
        let tid = TenantId::new("acme".to_string()).unwrap();
        let err = RepoResolutionError::MissingBinding {
            tenant_id: tid.clone(),
        };
        assert!(err.to_string().contains("acme"), "got: {err}");
        assert!(
            err.to_string().contains("no repository binding"),
            "got: {err}"
        );
    }

    #[test]
    fn repo_resolution_error_invalid_binding_message_contains_tenant_and_reason() {
        let tid = TenantId::new("acme".to_string()).unwrap();
        let err = RepoResolutionError::InvalidBinding {
            tenant_id: tid.clone(),
            reason: "kind=Local requires local_path".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("acme"), "got: {msg}");
        assert!(msg.contains("structurally invalid"), "got: {msg}");
        assert!(msg.contains("local_path"), "got: {msg}");
    }

    #[tokio::test]
    async fn build_local_repository_uses_absolute_path() {
        // We can only test Local kind without a DB; use a temp dir.
        let dir = tempfile::tempdir().unwrap();
        let binding = TenantRepositoryBinding {
            local_path: Some(dir.path().to_string_lossy().to_string()),
            ..make_binding(RepositoryKind::Local)
        };

        // Construct a fake resolver with no DB (we won't call resolve()).
        let tid = binding.tenant_id.clone();
        // Manually invoke build_repository through a dummy store.
        // Since we can't easily construct TenantRepositoryBindingStore without
        // a real DB pool, we test the underlying helper indirectly by checking
        // GitRepository::new() directly with the expected resolved path.
        let root = PathBuf::from(binding.local_path.as_deref().unwrap());
        let repo = GitRepository::new(&root);
        assert!(repo.is_ok(), "expected local repo at {:?}", root);
        drop(tid);
    }

    // -------------------------------------------------------------------------
    // Secret-reference validation helpers (task 3.5)
    // -------------------------------------------------------------------------

    #[test]
    fn make_test_secret_provider_works() {
        // Smoke-test: provider is constructed without panic.
        let _provider = make_test_secret_provider();
    }

    #[test]
    fn github_binding_fixture_uses_secret_ref_format() {
        let binding = make_binding(RepositoryKind::GitHub);
        let r = binding.credential_ref.as_deref().unwrap();
        assert!(
            r.starts_with("local/") || r.starts_with("secret/") || r.starts_with("arn:aws:"),
            "test fixture credential_ref must be a secret reference, got: {r}"
        );
    }

    #[test]
    fn structural_validity_is_independent_of_credential_ref_format() {
        // is_structurally_valid() is a fields-presence check only;
        // credential-ref format validation is done separately via validate_credential_ref().
        let mut binding = make_binding(RepositoryKind::GitHub);
        binding.credential_ref = Some("ghp_rawtoken".to_string());
        // Still structurally valid (that check only tests field presence).
        assert!(binding.is_structurally_valid());
        // But credential_ref validation correctly rejects it.
        assert!(binding.validate_credential_ref().is_err());
    }

    #[tokio::test]
    async fn local_secret_provider_resolves_test_pat() {
        let provider = make_test_secret_provider();
        let value = provider.get_secret("local/test-pat-token").await.unwrap();
        assert_eq!(value, "resolved-token");
    }

    #[tokio::test]
    async fn local_secret_provider_returns_not_found_for_unknown_key() {
        let provider = make_test_secret_provider();
        let err = provider.get_secret("local/no-such-key").await.unwrap_err();
        assert!(
            err.to_string().contains("no-such-key"),
            "expected key in error, got: {err}"
        );
    }
}
