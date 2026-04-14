use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use dashmap::DashMap;
use idp_sync::config::GitHubConfig;
use idp_sync::sync::SyncReport;
use memory::provider_registry::github_config_keys;
use mk_core::traits::TenantConfigProvider;
use mk_core::types::{Role, TenantId};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{AppState, authenticated_platform_context, authenticated_tenant_context};

static SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static TENANT_SYNC_LOCKS: LazyLock<DashMap<String, ()>> = LazyLock::new(DashMap::new);

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/sync/github", post(handle_github_sync))
        .route(
            "/admin/tenants/{tenant}/sync/github",
            post(sync_tenant_github),
        )
        .with_state(state)
}

#[derive(Serialize)]
struct TenantSyncResult {
    tenant_id: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<SyncReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct TenantSyncResultsResponse {
    results: Vec<TenantSyncResult>,
}

#[tracing::instrument(skip_all)]
async fn handle_github_sync(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let (_user_id, roles) = match authenticated_platform_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    if !roles
        .iter()
        .any(|r| *r == mk_core::types::Role::PlatformAdmin.into())
    {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        );
    }

    if SYNC_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "sync_in_progress",
                "message": "A GitHub organization sync is already running"
            })),
        )
            .into_response();
    }

    let result = run_sync(&state).await;

    SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);

    match result {
        Ok(results) => (StatusCode::OK, Json(json!(results))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "sync_failed",
                "message": format!("{err:?}")
            })),
        )
            .into_response(),
    }
}

#[tracing::instrument(skip_all)]
async fn sync_tenant_github(
    State(state): State<Arc<AppState>>,
    Path(tenant): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    let tenant_record = match find_tenant_by_selector(state.postgres.pool(), &tenant).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                "Tenant not found",
            );
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "tenant_lookup_failed",
                    "message": format!("{err:?}")
                })),
            )
                .into_response();
        }
    };

    let same_tenant = ctx.tenant_id.as_str() == tenant_record.0.to_string()
        || ctx.tenant_id.as_str() == tenant_record.1;
    if !ctx.has_known_role(&Role::PlatformAdmin)
        && !(ctx.has_known_role(&Role::TenantAdmin) && same_tenant)
    {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin or same-tenant TenantAdmin role required",
        );
    }

    let tenant_lock_key = tenant_record.0.to_string();
    if TENANT_SYNC_LOCKS.contains_key(&tenant_lock_key) {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "sync_in_progress",
                "message": "A GitHub organization sync is already running for this tenant"
            })),
        )
            .into_response();
    }
    TENANT_SYNC_LOCKS.insert(tenant_lock_key.clone(), ());

    let result = async {
        initialize_sync_schema(&state).await?;
        let github_config = build_github_config_for_tenant(&tenant_record.0, &state)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        run_sync_for_tenant(tenant_record.0, github_config, &state).await
    }
    .await;

    TENANT_SYNC_LOCKS.remove(&tenant_lock_key);

    match result {
        Ok(report) => (StatusCode::OK, Json(json!(report))).into_response(),
        Err(err) => {
            let status = if err.to_string().contains("AETERNA_GITHUB_")
                || err.to_string().contains("GitHub tenant config")
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            (
                status,
                Json(json!({
                    "error": "sync_failed",
                    "message": err.to_string()
                })),
            )
                .into_response()
        }
    }
}

#[tracing::instrument(skip_all)]
async fn run_sync(state: &Arc<AppState>) -> anyhow::Result<TenantSyncResultsResponse> {
    initialize_sync_schema(state).await?;

    let configs = state
        .tenant_config_provider
        .list_configs()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to list tenant configs: {err}"))?;

    let tenant_configs: Vec<_> = configs
        .into_iter()
        .filter(|config| config.fields.contains_key(github_config_keys::ORG_NAME))
        .collect();

    if tenant_configs.is_empty() {
        let github_config = build_github_config()?;
        let tenant_id = resolve_tenant_id(state).await?;
        let report = run_sync_for_tenant(tenant_id, github_config, state).await?;
        return Ok(TenantSyncResultsResponse {
            results: vec![TenantSyncResult {
                tenant_id: "env-default".to_string(),
                status: "ok",
                report: Some(report),
                error: None,
            }],
        });
    }

    let mut results = Vec::with_capacity(tenant_configs.len());
    for config in tenant_configs {
        let tenant_id = config.tenant_id.as_str().to_string();
        let parsed_tenant_id = match Uuid::parse_str(config.tenant_id.as_str()) {
            Ok(tenant_id) => tenant_id,
            Err(err) => {
                results.push(TenantSyncResult {
                    tenant_id,
                    status: "error",
                    report: None,
                    error: Some(format!("Invalid tenant UUID in config: {err}")),
                });
                continue;
            }
        };

        let tenant_lock_key = parsed_tenant_id.to_string();
        if TENANT_SYNC_LOCKS.contains_key(&tenant_lock_key) {
            results.push(TenantSyncResult {
                tenant_id,
                status: "error",
                report: None,
                error: Some(
                    "A GitHub organization sync is already running for this tenant".to_string(),
                ),
            });
            continue;
        }
        TENANT_SYNC_LOCKS.insert(tenant_lock_key.clone(), ());

        let tenant_result = async {
            let github_config = build_github_config_for_tenant(&parsed_tenant_id, state).await?;
            run_sync_for_tenant(parsed_tenant_id, github_config, state).await
        }
        .await;

        TENANT_SYNC_LOCKS.remove(&tenant_lock_key);

        match tenant_result {
            Ok(report) => results.push(TenantSyncResult {
                tenant_id,
                status: "ok",
                report: Some(report),
                error: None,
            }),
            Err(err) => results.push(TenantSyncResult {
                tenant_id,
                status: "error",
                report: None,
                error: Some(err.to_string()),
            }),
        }
    }

    Ok(TenantSyncResultsResponse { results })
}

async fn initialize_sync_schema(state: &Arc<AppState>) -> anyhow::Result<()> {
    idp_sync::github::initialize_github_sync_schema(state.postgres.pool())
        .await
        .map_err(|e| anyhow::anyhow!("Schema init failed: {e:?}"))
}

#[tracing::instrument(skip_all)]
async fn run_sync_for_tenant(
    tenant_id: Uuid,
    github_config: GitHubConfig,
    state: &Arc<AppState>,
) -> anyhow::Result<SyncReport> {
    tracing::info!(
        org = %github_config.org_name,
        tenant_id = %tenant_id,
        "Starting GitHub organization sync"
    );

    let report =
        idp_sync::github::run_github_sync(&github_config, state.postgres.pool(), tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("GitHub sync failed: {e:?}"))?;

    if let Err(e) =
        idp_sync::github::bridge_sync_to_governance(state.postgres.pool(), tenant_id).await
    {
        tracing::warn!(error = ?e, "Governance bridge failed (non-fatal)");
    }

    tracing::info!(
        users_created = report.users_created,
        users_updated = report.users_updated,
        groups_synced = report.groups_synced,
        memberships_added = report.memberships_added,
        "GitHub organization sync completed"
    );

    Ok(report)
}

fn build_github_config() -> anyhow::Result<GitHubConfig> {
    build_github_config_from_env()
}

fn build_github_config_with_lookup<F>(lookup: F) -> anyhow::Result<GitHubConfig>
where
    F: Fn(&str) -> Option<String>,
{
    let org_name = lookup("AETERNA_GITHUB_ORG_NAME").ok_or_else(|| {
        anyhow::anyhow!("AETERNA_GITHUB_ORG_NAME is required for GitHub org sync")
    })?;

    let app_id: u64 = lookup("AETERNA_GITHUB_APP_ID")
        .ok_or_else(|| anyhow::anyhow!("AETERNA_GITHUB_APP_ID is required"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_APP_ID must be a number"))?;

    let installation_id: u64 = lookup("AETERNA_GITHUB_INSTALLATION_ID")
        .ok_or_else(|| anyhow::anyhow!("AETERNA_GITHUB_INSTALLATION_ID is required"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_INSTALLATION_ID must be a number"))?;

    let private_key_pem = lookup("AETERNA_GITHUB_APP_PEM")
        .ok_or_else(|| anyhow::anyhow!("AETERNA_GITHUB_APP_PEM is required"))?;

    let team_filter = lookup("AETERNA_GITHUB_TEAM_FILTER").filter(|value| !value.trim().is_empty());
    let sync_repos_as_projects = lookup("AETERNA_GITHUB_SYNC_REPOS_AS_PROJECTS")
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1"))
        .unwrap_or(false);

    Ok(GitHubConfig {
        org_name,
        app_id,
        installation_id,
        private_key_pem,
        team_filter,
        sync_repos_as_projects,
        api_base_url: None,
    })
}

pub(crate) fn build_github_config_from_env() -> anyhow::Result<GitHubConfig> {
    build_github_config_with_lookup(|key| std::env::var(key).ok())
}

async fn build_github_config_for_tenant_with_lookup<F>(
    tenant_id: &Uuid,
    state: &Arc<AppState>,
    lookup: F,
) -> anyhow::Result<GitHubConfig>
where
    F: Fn(&str) -> Option<String>,
{
    let tenant_id_typed = TenantId::new(tenant_id.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid tenant UUID for config lookup: {tenant_id}"))?;

    let config = state
        .tenant_config_provider
        .get_config(&tenant_id_typed)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to read tenant config for {tenant_id}: {err}"))?;

    if let Some(config) = config {
        let org_name = config
            .fields
            .get(github_config_keys::ORG_NAME)
            .and_then(|field| value_as_string(&field.value));
        let app_id_raw = config
            .fields
            .get(github_config_keys::APP_ID)
            .and_then(|field| value_as_string(&field.value));
        let installation_id_raw = config
            .fields
            .get(github_config_keys::INSTALLATION_ID)
            .and_then(|field| value_as_string(&field.value));
        let private_key_pem = state
            .tenant_config_provider
            .get_secret_value(&tenant_id_typed, github_config_keys::APP_PEM)
            .await
            .map_err(|err| anyhow::anyhow!("Failed to read tenant secret for {tenant_id}: {err}"))?
            .filter(|value| !value.trim().is_empty());

        if let (
            Some(org_name),
            Some(app_id_raw),
            Some(installation_id_raw),
            Some(private_key_pem),
        ) = (org_name, app_id_raw, installation_id_raw, private_key_pem)
        {
            let app_id = app_id_raw.parse().map_err(|_| {
                anyhow::anyhow!(
                    "GitHub tenant config '{}' must be a number",
                    github_config_keys::APP_ID
                )
            })?;
            let installation_id = installation_id_raw.parse().map_err(|_| {
                anyhow::anyhow!(
                    "GitHub tenant config '{}' must be a number",
                    github_config_keys::INSTALLATION_ID
                )
            })?;

            let team_filter = config
                .fields
                .get(github_config_keys::TEAM_FILTER)
                .and_then(|field| value_as_string(&field.value))
                .filter(|value| !value.trim().is_empty());
            let sync_repos_as_projects = config
                .fields
                .get(github_config_keys::SYNC_REPOS_AS_PROJECTS)
                .and_then(|field| value_as_bool(&field.value))
                .unwrap_or(false);

            return Ok(GitHubConfig {
                org_name,
                app_id,
                installation_id,
                private_key_pem,
                team_filter,
                sync_repos_as_projects,
                api_base_url: None,
            });
        }
    }

    build_github_config_with_lookup(lookup)
}

pub(crate) async fn build_github_config_for_tenant(
    tenant_id: &Uuid,
    state: &Arc<AppState>,
) -> anyhow::Result<GitHubConfig> {
    build_github_config_for_tenant_with_lookup(tenant_id, state, |key| std::env::var(key).ok())
        .await
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        Value::Number(value) => value.as_u64().map(|number| number != 0),
        _ => None,
    }
}

#[tracing::instrument(skip_all)]
async fn resolve_tenant_id(state: &Arc<AppState>) -> anyhow::Result<Uuid> {
    resolve_tenant_id_from_pool(state.postgres.pool()).await
}

async fn find_tenant_by_selector(
    pool: &sqlx::PgPool,
    tenant: &str,
) -> anyhow::Result<Option<(Uuid, String)>> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM tenants WHERE name = $1 OR id::text = $1 LIMIT 1")
            .bind(tenant)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

pub(crate) async fn resolve_tenant_id_from_pool(pool: &sqlx::PgPool) -> anyhow::Result<Uuid> {
    let tenant_str =
        std::env::var(crate::env_vars::AETERNA_TENANT_ID).unwrap_or_else(|_| "default".to_string());
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM tenants WHERE name = $1 OR id::text = $1 LIMIT 1")
            .bind(&tenant_str)
            .fetch_optional(pool)
            .await?;

    match row {
        Some((id,)) => Ok(id),
        None => {
            tracing::info!(tenant = %tenant_str, "Tenant not found, creating default");
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO tenants (id, name, created_at) VALUES ($1, $2, NOW()) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id")
                .bind(id)
                .bind(&tenant_str)
                .execute(pool)
                .await?;
            Ok(id)
        }
    }
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginAuthState;
    use crate::server::plugin_auth::{RefreshTokenStore, RefreshTokenStoreBackend};
    use agent_a2a::config::TrustedIdentityConfig;
    use async_trait::async_trait;
    use knowledge::api::GovernanceDashboardApi;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::{GitRepository, RepositoryError};
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, ReasoningStrategy, ReasoningTrace, RoleIdentifier,
        TenantContext, UserId,
    };
    use serial_test::serial;
    use std::collections::HashMap;
    use storage::postgres::PostgresBackend;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::state_persister::FilePersister;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};

    struct MockAuth;

    #[async_trait]
    impl AuthorizationService for MockAuth {
        type Error = anyhow::Error;

        async fn check_permission(
            &self,
            _ctx: &TenantContext,
            _action: &str,
            _resource: &str,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }

        async fn get_user_roles(
            &self,
            _ctx: &TenantContext,
        ) -> Result<Vec<RoleIdentifier>, Self::Error> {
            Ok(vec![Role::Developer.into()])
        }

        async fn assign_role(
            &self,
            _ctx: &TenantContext,
            _user_id: &UserId,
            _role: RoleIdentifier,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_role(
            &self,
            _ctx: &TenantContext,
            _user_id: &UserId,
            _role: RoleIdentifier,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct MockRepo;

    #[async_trait]
    impl KnowledgeRepository for MockRepo {
        type Error = RepositoryError;

        async fn get(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
        ) -> Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }

        async fn store(
            &self,
            _ctx: TenantContext,
            _entry: KnowledgeEntry,
            _message: &str,
        ) -> Result<String, Self::Error> {
            Ok("mock-commit".to_string())
        }

        async fn list(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _prefix: &str,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(vec![])
        }

        async fn delete(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
            _message: &str,
        ) -> Result<String, Self::Error> {
            Ok("mock-delete".to_string())
        }

        async fn get_head_commit(
            &self,
            _ctx: TenantContext,
        ) -> Result<Option<String>, Self::Error> {
            Ok(Some("mock-commit".to_string()))
        }

        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _since_commit: &str,
        ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(vec![])
        }

        async fn search(
            &self,
            _ctx: TenantContext,
            _query: &str,
            _layers: Vec<KnowledgeLayer>,
            _limit: usize,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(vec![])
        }

        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    struct TestNoopReasoner;

    #[async_trait]
    impl memory::reasoning::ReflectiveReasoner for TestNoopReasoner {
        async fn reason(
            &self,
            query: &str,
            _context_summary: Option<&str>,
        ) -> anyhow::Result<ReasoningTrace> {
            let now = chrono::Utc::now();
            Ok(ReasoningTrace {
                strategy: ReasoningStrategy::SemanticOnly,
                thought_process: "test noop".to_string(),
                refined_query: Some(query.to_string()),
                start_time: now,
                end_time: now,
                timed_out: false,
                duration_ms: 0,
                metadata: HashMap::new(),
            })
        }
    }

    struct MockTokenValidator;

    #[async_trait]
    impl TokenValidator for MockTokenValidator {
        async fn validate(&self, token: &str) -> WsResult<AuthToken> {
            Ok(AuthToken {
                user_id: token.to_string(),
                tenant_id: "default".to_string(),
                permissions: vec![],
                expires_at: 0,
            })
        }
    }

    async fn test_app_state(
        tenant_config_provider: Arc<KubernetesTenantConfigProvider>,
    ) -> Arc<AppState> {
        let tempdir = tempfile::tempdir().unwrap();
        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .unwrap();
        let postgres = Arc::new(PostgresBackend::from_pool(lazy_pool));
        let governance_engine = Arc::new(GovernanceEngine::new());
        let git_repo = Arc::new(GitRepository::new(tempdir.path()).unwrap());
        let knowledge_manager = Arc::new(KnowledgeManager::new(
            git_repo.clone(),
            governance_engine.clone(),
        ));
        let memory_manager = Arc::new(MemoryManager::new());
        let sync_manager = Arc::new(
            SyncManager::new(
                memory_manager.clone(),
                knowledge_manager.clone(),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(FilePersister::new(std::env::temp_dir())),
                None,
            )
            .await
            .unwrap(),
        );
        let auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync> =
            Arc::new(MockAuth);
        let governance_dashboard = Arc::new(GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let mcp_server = Arc::new(tools::server::McpServer::new(
            memory_manager.clone(),
            sync_manager.clone(),
            knowledge_manager.clone(),
            git_repo.clone(),
            postgres.clone(),
            governance_engine.clone(),
            Arc::new(TestNoopReasoner),
            auth_service.clone(),
            None,
            None,
            None,
        ));
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        let tenant_store = Arc::new(TenantStore::new(postgres.pool().clone()));
        let tenant_repository_binding_store =
            Arc::new(TenantRepositoryBindingStore::new(postgres.pool().clone()));
        let git_provider_connection_registry = Arc::new(
            storage::git_provider_connection_store::InMemoryGitProviderConnectionStore::new(),
        );
        let tenant_repo_resolver = Arc::new(
            TenantRepositoryResolver::new(
                tenant_repository_binding_store.clone(),
                std::env::temp_dir(),
                Arc::new(LocalSecretProvider::new(HashMap::new())),
            )
            .with_connection_registry(git_provider_connection_registry.clone()),
        );

        Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres: postgres.clone(),
            memory_manager,
            knowledge_manager,
            knowledge_repository: Arc::new(MockRepo),
            governance_engine,
            governance_dashboard,
            auth_service,
            mcp_server,
            sync_manager,
            git_provider: None,
            webhook_secret: None,
            event_publisher: None,
            graph_store: None,
            governance_storage: None,
            reasoner: None,
            ws_server: Arc::new(WsServer::new(Arc::new(MockTokenValidator))),
            a2a_config: Arc::new(agent_a2a::Config::default()),
            a2a_auth_state: Arc::new(agent_a2a::AuthState {
                api_key: None,
                jwt_secret: None,
                enabled: false,
                trusted_identity: TrustedIdentityConfig::default(),
            }),
            plugin_auth_state: Arc::new(PluginAuthState {
                config: config::PluginAuthConfig::default(),
                postgres: Some(postgres.clone()),
                refresh_store: RefreshTokenStoreBackend::InMemory(RefreshTokenStore::new()),
                oauth_state_store: super::plugin_auth::OAuthStateStore::new(),
            }),
            k8s_auth_config: config::KubernetesAuthConfig::default(),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider,
            provider_registry: Arc::new(memory::provider_registry::TenantProviderRegistry::new(
                None, None,
            )),
            git_provider_connection_registry,
            redis_conn: None,
        })
    }

    #[test]
    fn sync_guard_prevents_concurrent_execution() {
        SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
        assert!(
            SYNC_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        );
        assert!(
            SYNC_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        );
        SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
    }

    #[test]
    fn error_response_helper_produces_correct_json() {
        let resp = error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        );
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[serial]
    async fn build_github_config_for_tenant_falls_back_to_env() {
        let tenant_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let provider = Arc::new(KubernetesTenantConfigProvider::new("default".to_string()));
        let state = test_app_state(provider).await;

        let config = build_github_config_for_tenant_with_lookup(&tenant_id, &state, |key| {
            let value = match key {
                "AETERNA_GITHUB_ORG_NAME" => Some("env-org".to_string()),
                "AETERNA_GITHUB_APP_ID" => Some("123".to_string()),
                "AETERNA_GITHUB_INSTALLATION_ID" => Some("456".to_string()),
                "AETERNA_GITHUB_APP_PEM" => Some("pem-from-env".to_string()),
                "AETERNA_GITHUB_TEAM_FILTER" => Some("^platform-".to_string()),
                "AETERNA_GITHUB_SYNC_REPOS_AS_PROJECTS" => Some("true".to_string()),
                _ => None,
            };
            value
        })
        .await
        .unwrap();

        assert_eq!(config.org_name, "env-org");
        assert_eq!(config.app_id, 123);
        assert_eq!(config.installation_id, 456);
        assert_eq!(config.private_key_pem, "pem-from-env");
        assert_eq!(config.team_filter.as_deref(), Some("^platform-"));
        assert!(config.sync_repos_as_projects);
    }
}
