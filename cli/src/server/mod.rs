pub mod admin_sync;
pub mod auth_middleware;
pub mod backup_api;
pub mod bootstrap;
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
use mk_core::types::{Role, RoleIdentifier, TenantContext, TenantId, UserId};
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

pub async fn authenticated_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let any_provider_configured =
        state.plugin_auth_state.config.enabled || state.k8s_auth_config.enabled;

    enum AuthResolution {
        Provider {
            provider: &'static str,
            subject: String,
        },
        DevHeaders {
            user_id: String,
            roles: Vec<RoleIdentifier>,
        },
    }

    let auth_resolution = if any_provider_configured {
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
                AuthResolution::Provider {
                    provider: PROVIDER_GITHUB,
                    subject: identity.github_login,
                }
            } else if state.k8s_auth_config.enabled {
                if let Some(identity) =
                    k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config).await
                {
                    AuthResolution::Provider {
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
                return Err(error_json(
                    StatusCode::UNAUTHORIZED,
                    "invalid_bearer_token",
                    "Bearer token was not accepted by any configured provider",
                ));
            }
        } else if let Some(identity) =
            k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config).await
        {
            AuthResolution::Provider {
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
        AuthResolution::DevHeaders {
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

    let (user_id_str, all_roles_root, tenant_ids_from_db) = match &auth_resolution {
        AuthResolution::Provider { provider, subject } => {
            let uid = state
                .postgres
                .resolve_user_id_by_idp(provider, subject)
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

            (uid, root_roles, Some(tenant_ids))
        }
        AuthResolution::DevHeaders { user_id, roles } => (user_id.clone(), roles.clone(), None),
    };

    // --- Resolve tenant ----------------------------------------------------------
    let explicit_tenant = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty());

    let tenant_id_str: String = match explicit_tenant {
        Some(t) => t.to_owned(),
        None => match &auth_resolution {
            AuthResolution::Provider { .. } => {
                let is_platform_admin = all_roles_root
                    .iter()
                    .any(|r| RoleIdentifier::from(Role::PlatformAdmin) == *r);
                if is_platform_admin {
                    return Err(error_json(
                        StatusCode::BAD_REQUEST,
                        "tenant_required",
                        "X-Tenant-ID header required for PlatformAdmin requests",
                    ));
                }
                let tenant_ids = tenant_ids_from_db.as_deref().unwrap_or(&[]);
                match tenant_ids {
                    [single] => single.clone(),
                    [] => {
                        return Err(error_json(
                            StatusCode::BAD_REQUEST,
                            "no_tenant",
                            "Authenticated user has no tenant assignment",
                        ));
                    }
                    _ => {
                        return Err(error_json(
                            StatusCode::BAD_REQUEST,
                            "ambiguous_tenant",
                            "User belongs to multiple tenants — provide X-Tenant-ID to select one",
                        ));
                    }
                }
            }
            AuthResolution::DevHeaders { .. } => state
                .plugin_auth_state
                .config
                .default_tenant_id
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        },
    };

    let tenant_id = TenantId::new(tenant_id_str.chars().take(100).collect()).ok_or_else(|| {
        error_json(
            StatusCode::BAD_REQUEST,
            "invalid_tenant_id",
            "Invalid X-Tenant-ID header",
        )
    })?;

    let roles: Vec<RoleIdentifier> = match &auth_resolution {
        AuthResolution::Provider { .. } => state
            .postgres
            .get_user_roles_for_auth(&user_id_str, tenant_id.as_str())
            .await
            .map_err(|err| {
                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "role_lookup_failed",
                    &err.to_string(),
                )
            })?,
        AuthResolution::DevHeaders { roles, .. } => roles.clone(),
    };

    let user_id = UserId::new(user_id_str).ok_or_else(|| {
        error_json(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid authenticated user identifier",
        )
    })?;

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

pub async fn authenticated_platform_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UserId, Vec<RoleIdentifier>), axum::response::Response> {
    let any_provider_configured =
        state.plugin_auth_state.config.enabled || state.k8s_auth_config.enabled;

    if any_provider_configured {
        let bearer_token = extract_bearer_token(headers).ok_or_else(|| {
            error_json(
                StatusCode::UNAUTHORIZED,
                "missing_bearer_token",
                "Authorization: Bearer token required",
            )
        })?;

        let (provider, subject) = if state.plugin_auth_state.config.enabled {
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
                (PROVIDER_GITHUB, identity.github_login)
            } else if state.k8s_auth_config.enabled {
                k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config)
                    .await
                    .map(|id| (PROVIDER_KUBERNETES, id.username))
                    .ok_or_else(|| {
                        error_json(
                            StatusCode::UNAUTHORIZED,
                            "invalid_bearer_token",
                            "Bearer token was not accepted by any configured provider",
                        )
                    })?
            } else {
                return Err(error_json(
                    StatusCode::UNAUTHORIZED,
                    "invalid_bearer_token",
                    "Bearer token was not accepted by any configured provider",
                ));
            }
        } else {
            k8s_auth::validate_k8s_bearer(bearer_token, &state.k8s_auth_config)
                .await
                .map(|id| (PROVIDER_KUBERNETES, id.username))
                .ok_or_else(|| {
                    error_json(
                        StatusCode::UNAUTHORIZED,
                        "invalid_bearer_token",
                        "Bearer token was not accepted by any configured provider",
                    )
                })?
        };

        let user_id_str = state
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

        let roles = state
            .postgres
            .get_user_roles_for_auth(&user_id_str, INSTANCE_SCOPE_TENANT_ID)
            .await
            .map_err(|err| {
                error_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "role_lookup_failed",
                    &err.to_string(),
                )
            })?;

        let user_id = UserId::new(user_id_str).ok_or_else(|| {
            error_json(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid authenticated user identifier",
            )
        })?;

        Ok((user_id, roles))
    } else {
        let user_id_str: String = headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .unwrap_or(SYSTEM_USER_ID)
            .chars()
            .take(100)
            .collect();
        let roles = headers
            .get("x-user-role")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(|s| vec![RoleIdentifier::from_str_flexible(s)])
            .unwrap_or_default();
        let user_id = UserId::new(user_id_str).ok_or_else(|| {
            error_json(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid X-User-ID header",
            )
        })?;
        Ok((user_id, roles))
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let authorization = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    authorization.strip_prefix("Bearer ").map(str::trim)
}
