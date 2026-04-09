pub mod admin_sync;
pub mod auth_middleware;
pub mod bootstrap;
pub mod govern_api;
pub mod health;
pub mod knowledge_api;
pub mod mcp_transport;
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
use axum::http::{HeaderMap, StatusCode};
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
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::GitProviderConnectionRegistry;
use mk_core::traits::{AuthorizationService, EventPublisher, KnowledgeRepository};
use mk_core::types::{RoleIdentifier, TenantContext, TenantId, UserId};
use serde_json::json;
use storage::events::EventError;
use storage::git_provider_connection_store::GitProviderConnectionError;
use storage::governance::GovernanceStorage;
use storage::graph_duckdb::DuckDbGraphStore;
use storage::postgres::PostgresBackend;
use storage::tenant_config_provider::KubernetesTenantConfigProvider;
use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
use tools::server::McpServer;

use plugin_auth::RefreshTokenStore;

/// Plugin-auth runtime state: configuration + in-process token store.
///
/// Held as `Arc<PluginAuthState>` inside `AppState` so handlers can access it
/// via `State<Arc<AppState>>` without an extra extraction layer.
pub struct PluginAuthState {
    pub config: config::PluginAuthConfig,
    pub postgres: Option<Arc<PostgresBackend>>,
    /// Single-use refresh-token store (rotated on every refresh).
    pub refresh_store: RefreshTokenStore,
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
    pub idp_config: Option<Arc<IdpSyncConfig>>,
    pub idp_sync_service: Option<Arc<IdpSyncService>>,
    pub idp_client: Option<Arc<dyn IdpClient>>,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub tenant_store: Arc<TenantStore>,
    pub tenant_repository_binding_store: Arc<TenantRepositoryBindingStore>,
    pub tenant_repo_resolver: Arc<TenantRepositoryResolver>,
    pub tenant_config_provider: Arc<KubernetesTenantConfigProvider>,
    /// Registry of platform-owned Git provider connections (task 3.4).
    pub git_provider_connection_registry:
        Arc<dyn GitProviderConnectionRegistry<Error = GitProviderConnectionError> + Send + Sync>,
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

/// Extracts an authenticated `TenantContext` from request headers.
///
/// - `X-Tenant-ID` header → `TenantContext::tenant_id`
/// - `X-User-ID` header (or plugin bearer JWT sub) → `TenantContext::user_id`
/// - `X-User-Role` header (optional) → `TenantContext::roles`
/// - `X-Target-Tenant-ID` header (optional, PlatformAdmin ops) → `TenantContext::target_tenant_id`
///
/// When plugin-auth is enabled the bearer token is validated; otherwise the
/// raw header values are trusted (development / service-to-service mode).
pub async fn authenticated_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    // --- Resolve user identity ---------------------------------------------------
    let (user_id_str, roles): (String, Vec<RoleIdentifier>) =
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
            let identity =
                plugin_auth::validate_plugin_bearer(headers, secret).ok_or_else(|| {
                    error_json(
                        StatusCode::UNAUTHORIZED,
                        "invalid_plugin_token",
                        "Valid plugin bearer token required",
                    )
                })?;

            let roles = lookup_roles_for_idp_subject(
                state.postgres.as_ref(),
                &identity.github_login,
                &identity.tenant_id,
            )
            .await
            .map_err(|err| {
                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "role_lookup_failed",
                    &err.to_string(),
                )
            })?;

            (identity.github_login, roles)
        } else {
            (
                headers
                    .get("x-user-id")
                    .and_then(|v| v.to_str().ok())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("system")
                    .chars()
                    .take(100)
                    .collect(),
                headers
                    .get("x-user-role")
                    .and_then(|v| v.to_str().ok())
                    .filter(|s| !s.is_empty())
                    .map(|s| vec![RoleIdentifier::from_str_flexible(s)])
                    .unwrap_or_default(),
            )
        };

    let user_id = UserId::new(user_id_str).ok_or_else(|| {
        error_json(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid X-User-ID header",
        )
    })?;

    // --- Resolve tenant ----------------------------------------------------------
    let tenant_id_str = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    let tenant_id = TenantId::new(tenant_id_str.chars().take(100).collect()).ok_or_else(|| {
        error_json(
            StatusCode::BAD_REQUEST,
            "invalid_tenant_id",
            "Invalid X-Tenant-ID header",
        )
    })?;

    // --- Optional roles ----------------------------------------------------------
    // --- Optional PlatformAdmin target tenant ------------------------------------
    let target_tenant_id: Option<TenantId> = headers
        .get("x-target-tenant-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .and_then(|s| TenantId::new(s.chars().take(100).collect()));

    Ok(TenantContext {
        tenant_id,
        user_id,
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
