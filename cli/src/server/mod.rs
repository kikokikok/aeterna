pub mod admin_sync;
pub mod auth_middleware;
pub mod backup_api;
pub mod bootstrap;
pub mod context;
pub mod govern_api;
pub mod health;
pub mod k8s_auth;
pub mod knowledge_api;
pub mod lifecycle;
pub mod lifecycle_api;
pub mod mcp_transport;
pub mod memory_api;
pub mod metrics;
pub mod org_api;
pub mod plugin_auth;
pub mod project_api;
pub mod role_grants;
pub mod router;
pub mod sessions;
pub mod sync;
pub mod team_api;
pub mod tenant_api;
pub mod user_api;
pub mod webhooks;

use std::sync::Arc;

use ::sync::bridge::SyncManager;
use ::sync::websocket::WsServer;
use agent_a2a::{AuthState as A2aAuthState, Config as A2aConfig};
use axum::Json;
use axum::http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use axum::response::IntoResponse;
use idp_sync::config::IdpSyncConfig;
use idp_sync::{IdpClient, IdpSyncService};
use knowledge::api::GovernanceDashboardApi;
use knowledge::git_provider::GitProvider;
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::RepositoryError;
use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
use memory::manager::MemoryManager;
use memory::provider_registry::TenantProviderRegistry;
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::GitProviderConnectionRegistry;
use mk_core::traits::{AuthorizationService, EventPublisher, KnowledgeRepository};
use mk_core::types::INSTANCE_SCOPE_TENANT_ID;
use mk_core::types::SYSTEM_USER_ID;
use mk_core::types::{PROVIDER_GITHUB, PROVIDER_KUBERNETES};
use mk_core::types::{RoleIdentifier, TenantContext, UserId};
use serde_json::json;
use storage::events::EventError;
use storage::git_provider_connection_store::GitProviderConnectionError;
use storage::governance::GovernanceStorage;
use storage::graph_duckdb::DuckDbGraphStore;
use storage::postgres::PostgresBackend;
use storage::tenant_config_provider::KubernetesTenantConfigProvider;
use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
use tools::server::McpServer;

use plugin_auth::RefreshTokenStoreBackend;

pub struct PluginAuthState {
    pub config: config::PluginAuthConfig,
    pub postgres: Option<Arc<PostgresBackend>>,
    pub refresh_store: RefreshTokenStoreBackend,
}

impl std::fmt::Debug for PluginAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginAuthState")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub postgres: Arc<PostgresBackend>,
    pub memory_manager: Arc<MemoryManager>,
    pub knowledge_manager: Arc<KnowledgeManager>,
    pub knowledge_repository: Arc<dyn KnowledgeRepository<Error = RepositoryError> + Send + Sync>,
    pub governance_engine: Arc<GovernanceEngine>,
    pub governance_dashboard: Arc<GovernanceDashboardApi>,
    pub auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync>,
    pub mcp_server: Arc<McpServer>,
    pub sync_manager: Arc<SyncManager>,
    pub git_provider: Option<Arc<dyn GitProvider>>,
    pub webhook_secret: Option<String>,
    pub event_publisher: Option<Arc<dyn EventPublisher<Error = EventError> + Send + Sync>>,
    pub graph_store: Option<Arc<DuckDbGraphStore>>,
    pub governance_storage: Option<Arc<GovernanceStorage>>,
    pub reasoner: Option<Arc<dyn ReflectiveReasoner>>,
    pub ws_server: Arc<WsServer>,
    pub a2a_config: Arc<A2aConfig>,
    pub a2a_auth_state: Arc<A2aAuthState>,
    pub plugin_auth_state: Arc<PluginAuthState>,
    pub k8s_auth_config: config::KubernetesAuthConfig,
    pub idp_config: Option<Arc<IdpSyncConfig>>,
    pub idp_sync_service: Option<Arc<IdpSyncService>>,
    pub idp_client: Option<Arc<dyn IdpClient>>,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub tenant_store: Arc<TenantStore>,
    pub tenant_repository_binding_store: Arc<TenantRepositoryBindingStore>,
    pub tenant_repo_resolver: Arc<TenantRepositoryResolver>,
    pub tenant_config_provider: Arc<KubernetesTenantConfigProvider>,
    /// Per-tenant LLM/embedding provider registry with caching.
    pub provider_registry: Arc<TenantProviderRegistry>,
    /// Registry of platform-owned Git provider connections (task 3.4).
    pub git_provider_connection_registry:
        Arc<dyn GitProviderConnectionRegistry<Error = GitProviderConnectionError> + Send + Sync>,
    /// Optional Redis connection manager for shared state stores (HA mode).
    ///
    /// When present, backup job stores, dead-letter queue, remediation store,
    /// and lifecycle distributed locks use Redis instead of in-memory state.
    pub redis_conn: Option<Arc<redis::aio::ConnectionManager>>,
}

// ---------------------------------------------------------------------------
// Auth / context extraction helpers used by tenant and admin API handlers
// ---------------------------------------------------------------------------

fn error_json(status: StatusCode, code: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({ "error": code, "message": message }))).into_response()
}

pub(crate) async fn lookup_roles_for_idp_subject(
    postgres: &PostgresBackend,
    idp_subject: &str,
    tenant_id: &str,
) -> Result<Vec<RoleIdentifier>, storage::postgres::PostgresError> {
    let Some(user_id) = postgres.resolve_user_id_by_idp_subject(idp_subject).await? else {
        return Ok(Vec::new());
    };

    postgres.get_user_roles_for_auth(&user_id, tenant_id).await
}

pub(crate) async fn lookup_roles_for_idp(
    postgres: &PostgresBackend,
    idp_provider: &str,
    idp_subject: &str,
    tenant_id: &str,
) -> Result<Vec<RoleIdentifier>, storage::postgres::PostgresError> {
    let Some(user_id) = postgres
        .resolve_user_id_by_idp(idp_provider, idp_subject)
        .await?
    else {
        return Ok(Vec::new());
    };
    postgres.get_user_roles_for_auth(&user_id, tenant_id).await
}

/// Extract the authenticated identity without resolving a tenant.
///
/// Returns `(user_id, instance_scope_roles, known_tenant_ids)` where the
/// roles are evaluated at `INSTANCE_SCOPE_TENANT_ID` (so PlatformAdmin is
/// visible) and `known_tenant_ids` is the distinct set of tenants the user
/// has any role in. For dev-header auth mode, `known_tenant_ids` is empty
/// and the caller must resolve tenant via configured defaults.
///
/// Added for OpenSpec change `refactor-platform-admin-impersonation` (#44)
/// to support the new `context::request_context` resolver without
/// duplicating the multi-provider auth dance.
pub(crate) async fn resolve_identity(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UserId, Vec<RoleIdentifier>, Vec<String>), axum::response::Response> {
    let any_provider_configured =
        state.plugin_auth_state.config.enabled || state.k8s_auth_config.enabled;

    enum Resolution {
        Provider {
            provider: &'static str,
            subject: String,
        },
        DevHeaders {
            user_id: String,
            roles: Vec<RoleIdentifier>,
        },
    }

    let resolution = if any_provider_configured {
        let bearer_token = extract_bearer_token(headers).ok_or_else(|| {
            error_json(
                StatusCode::UNAUTHORIZED,
                "missing_bearer_token",
                "Authorization: Bearer token required",
            )
        })?;

        if state.plugin_auth_state.config.enabled {
            let secret = state
                .plugin_auth_state
                .config
                .jwt_secret
                .as_deref()
                .ok_or_else(|| {
                    error_json(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "configuration_error",
                        "Plugin auth JWT secret is not configured",
                    )
                })?;
            if let Some(identity) = plugin_auth::validate_plugin_bearer(headers, secret) {
                Resolution::Provider {
                    provider: PROVIDER_GITHUB,
                    subject: identity.github_login,
                }
            } else if state.k8s_auth_config.enabled
                && let Some(identity) =
                    k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config).await
            {
                Resolution::Provider {
                    provider: PROVIDER_KUBERNETES,
                    subject: identity.username,
                }
            } else {
                return Err(error_json(
                    StatusCode::UNAUTHORIZED,
                    "invalid_bearer_token",
                    "Bearer token was not accepted by any configured provider",
                ));
            }
        } else if let Some(identity) =
            k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config).await
        {
            Resolution::Provider {
                provider: PROVIDER_KUBERNETES,
                subject: identity.username,
            }
        } else {
            return Err(error_json(
                StatusCode::UNAUTHORIZED,
                "invalid_bearer_token",
                "Bearer token was not accepted by any configured provider",
            ));
        }
    } else {
        Resolution::DevHeaders {
            user_id: headers
                .get("x-user-id")
                .and_then(|v| v.to_str().ok())
                .filter(|s| !s.is_empty())
                .unwrap_or(SYSTEM_USER_ID)
                .chars()
                .take(100)
                .collect(),
            roles: headers
                .get("x-user-role")
                .and_then(|v| v.to_str().ok())
                .filter(|s| !s.is_empty())
                .map(|s| vec![RoleIdentifier::from_str_flexible(s)])
                .unwrap_or_default(),
        }
    };

    let (user_id_str, instance_roles, tenant_ids) = match resolution {
        Resolution::Provider { provider, subject } => {
            let uid = state
                .postgres
                .resolve_user_id_by_idp(provider, &subject)
                .await
                .map_err(|err| {
                    error_json(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "identity_lookup_failed",
                        &err.to_string(),
                    )
                })?
                .ok_or_else(|| {
                    error_json(
                        StatusCode::UNAUTHORIZED,
                        "identity_not_provisioned",
                        "Authenticated identity is not provisioned",
                    )
                })?;

            let root_roles = state
                .postgres
                .get_user_roles_for_auth(&uid, INSTANCE_SCOPE_TENANT_ID)
                .await
                .map_err(|err| {
                    error_json(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "role_lookup_failed",
                        &err.to_string(),
                    )
                })?;

            let tenant_ids = state
                .postgres
                .get_user_tenant_ids(&uid)
                .await
                .map_err(|err| {
                    error_json(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "tenant_lookup_failed",
                        &err.to_string(),
                    )
                })?;

            (uid, root_roles, tenant_ids)
        }
        Resolution::DevHeaders { user_id, roles } => (user_id, roles, Vec::new()),
    };

    let user_id = UserId::new(user_id_str).ok_or_else(|| {
        error_json(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid authenticated user identifier",
        )
    })?;

    Ok((user_id, instance_roles, tenant_ids))
}

/// Legacy helper: returns a `TenantContext` for the authenticated request.
///
/// **Since #44.c this is a thin shim over [`context::request_context`]** —
/// every call site automatically picks up the new 4-step resolution chain
/// (`X-Tenant-ID` → `users.default_tenant_id` → auto-select → error) and
/// the structured `select_tenant` error body (opt out with
/// `Accept-Error-Legacy: true`).
///
/// The external signature is preserved so the ~40 existing handlers need
/// no changes. Future work (#44.d) will migrate handlers that need to
/// distinguish "PlatformAdmin without target tenant" (a case `TenantContext`
/// cannot express) to `RequestContext` directly.
///
/// Behavioural contract preserved from the pre-#44.c implementation:
/// - `roles` are **tenant-scoped** (re-fetched at the resolved tenant),
///   matching what handlers expect for RBAC checks.
/// - `target_tenant_id` is still read from the legacy `X-Target-Tenant-Id`
///   header for cross-tenant listing handlers that haven't migrated to
///   `ListTenantScope` yet.
/// - Dev-headers mode falls back to `plugin_auth_state.config.default_tenant_id`
///   (or `"default"`) when no `X-Tenant-ID` is provided, preserving the
///   previous dev UX.
pub async fn authenticated_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let any_provider_configured =
        state.plugin_auth_state.config.enabled || state.k8s_auth_config.enabled;

    // Dev-headers mode fallback: when no auth provider is configured and
    // the request doesn't carry `X-Tenant-ID`, inject the configured
    // default so `request_context` has something to resolve. Matches the
    // pre-#44.c dev UX.
    let injected_headers_owned;
    let effective_headers: &HeaderMap =
        if !any_provider_configured && !headers.contains_key("x-tenant-id") {
            let fallback = state
                .plugin_auth_state
                .config
                .default_tenant_id
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let mut h = headers.clone();
            if let Ok(v) = axum::http::HeaderValue::from_str(&fallback) {
                h.insert("x-tenant-id", v);
            }
            injected_headers_owned = h;
            &injected_headers_owned
        } else {
            headers
        };

    let ctx = context::request_context(state, effective_headers).await?;
    let tenant = ctx.require_target_tenant(effective_headers)?.clone();

    // Tenant-scoped roles: RequestContext carries *instance-scope* roles
    // (so PlatformAdmin is always visible); legacy handlers expect
    // TenantContext.roles to be evaluated at the resolved tenant.
    let roles = if any_provider_configured {
        state
            .postgres
            .get_user_roles_for_auth(ctx.user_id.as_str(), tenant.id.as_str())
            .await
            .map_err(|err| {
                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "role_lookup_failed",
                    &err.to_string(),
                )
            })?
    } else {
        // Dev-headers: trust the X-User-Role header as-is (already parsed
        // into `ctx.roles` by `resolve_identity`).
        ctx.roles.clone()
    };

    // #44.d §8 — X-Target-Tenant-Id is deprecated in favor of ?tenant=<slug>.
    // Delegate to the shared extractor so the compat::warn log line is
    // emitted uniformly from every entry point that still honors the header.
    let target_tenant_id = auth_middleware::extract_deprecated_target_tenant(headers);

    Ok(TenantContext {
        tenant_id: tenant.id,
        user_id: ctx.user_id,
        agent_id: None,
        roles,
        target_tenant_id,
    })
}

/// Like [`authenticated_tenant_context`] but also resolves the tenant from
/// the knowledge repository binding store when `X-Target-Tenant-ID` is absent.
///
/// Currently this is a thin wrapper; future iterations may perform async
/// tenant resolution (e.g. from a verified domain mapping).
pub async fn tenant_scoped_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    authenticated_tenant_context(state, headers).await
}

/// Legacy helper: returns `(user_id, instance_scope_roles)` for a request.
///
/// Used by platform-level endpoints (lifecycle, global admin) that do a
/// local `PlatformAdmin` role check and don't care about tenant resolution.
///
/// Since #44.c this is a thin wrapper over [`resolve_identity`] — the
/// pre-existing ~125 lines of auth-provider dance have been removed in
/// favour of the single extracted helper.
pub async fn authenticated_platform_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UserId, Vec<RoleIdentifier>), axum::response::Response> {
    let (user_id, roles, _tenant_ids) = resolve_identity(state, headers).await?;
    Ok((user_id, roles))
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let authorization = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    authorization.strip_prefix("Bearer ").map(str::trim)
}
