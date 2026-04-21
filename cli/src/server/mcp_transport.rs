use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream};
use std::convert::Infallible;
use std::sync::Arc;
use tools::server::{JsonRpcRequest, McpServer};

use super::AppState;
use super::plugin_auth::validate_plugin_bearer;

/// Combined state for MCP transport: MCP server + app config for auth enforcement.
#[derive(Clone)]
pub(super) struct McpTransportState {
    pub server: Arc<McpServer>,
    pub app: Arc<AppState>,
}

pub fn router(mcp_server: Arc<McpServer>, app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sse", get(handle_sse))
        .route("/message", post(handle_message))
        .with_state(McpTransportState {
            server: mcp_server,
            app: app_state,
        })
}

#[tracing::instrument(skip_all)]
async fn handle_sse() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::once(async {
        Ok::<_, Infallible>(
            Event::default()
                .event("endpoint")
                .data(serde_json::json!({"endpoint": "/mcp/message"}).to_string()),
        )
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[tracing::instrument(skip_all, fields(method = %request.method))]
async fn handle_message(
    State(state): State<McpTransportState>,
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    // Extract authenticated caller tenant from plugin bearer token (if plugin auth enabled).
    // When auth is enabled and a valid bearer is present, pass the tenant as a constraint
    // so the dispatcher can reject payloads that assert a broader scope.
    let caller_tenant: Option<String> = if state.app.plugin_auth_state.config.enabled {
        state
            .app
            .plugin_auth_state
            .config
            .jwt_secret
            .as_deref()
            .and_then(|secret| validate_plugin_bearer(&headers, secret))
            .map(|identity| identity.tenant_id)
    } else {
        None
    };

    let response = state
        .server
        .handle_request_with_caller(request, caller_tenant.as_deref())
        .await;
    Json(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginAuthState;
    use crate::server::plugin_auth::{RefreshTokenStore, RefreshTokenStoreBackend};
    use agent_a2a::config::TrustedIdentityConfig;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use knowledge::api::GovernanceDashboardApi;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::RepositoryError;
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, ReasoningStrategy, ReasoningTrace, Role, RoleIdentifier,
        TenantContext, UserId,
    };
    use std::collections::HashMap;
    use storage::git_provider_connection_store::InMemoryGitProviderConnectionStore;
    use storage::postgres::PostgresBackend;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::state_persister::FilePersister;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
    use tempfile::TempDir;
    use tower::ServiceExt;

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
            Ok("mock-commit".to_string())
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
    impl ReflectiveReasoner for TestNoopReasoner {
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

    async fn test_mcp_server() -> (Arc<McpServer>, Arc<AppState>, TempDir) {
        let tempdir = tempfile::tempdir().unwrap();
        let repo = Arc::new(MockRepo);
        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .unwrap();
        let postgres = Arc::new(PostgresBackend::from_pool(lazy_pool));
        let governance_engine = Arc::new(GovernanceEngine::new());
        let git_repo = Arc::new(knowledge::repository::GitRepository::new(tempdir.path()).unwrap());
        let knowledge_manager =
            Arc::new(KnowledgeManager::new(git_repo, governance_engine.clone()));
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
        let _dashboard = Arc::new(GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let _ws_server = Arc::new(WsServer::new(Arc::new(MockTokenValidator)));
        let _a2a_auth = Arc::new(agent_a2a::AuthState {
            api_key: None,
            jwt_secret: None,
            enabled: false,
            trusted_identity: TrustedIdentityConfig::default(),
        });

        let server = Arc::new(McpServer::new(
            memory_manager.clone(),
            sync_manager.clone(),
            knowledge_manager.clone(),
            repo,
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
        let git_provider_connection_registry = Arc::new(InMemoryGitProviderConnectionStore::new());
        let tenant_repo_resolver = Arc::new(
            TenantRepositoryResolver::new(
                tenant_repository_binding_store.clone(),
                std::env::temp_dir(),
                Arc::new(LocalSecretProvider::new(std::collections::HashMap::new())),
            )
            .with_connection_registry(git_provider_connection_registry.clone()),
        );
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            Arc::new(knowledge::repository::GitRepository::new(tempdir.path()).unwrap()),
            governance_engine.clone(),
        ));
        let app_state = Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres: postgres.clone(),
            memory_manager: memory_manager.clone(),
            knowledge_manager,
            knowledge_repository: Arc::new(MockRepo),
            governance_engine: governance_engine.clone(),
            governance_dashboard: Arc::new(knowledge::api::GovernanceDashboardApi::new(
                governance_engine,
                postgres.clone(),
                config::config::DeploymentConfig::default(),
            )),
            auth_service,
            mcp_server: server.clone(),
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
            }),
            k8s_auth_config: config::KubernetesAuthConfig::default(),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider: Arc::new(
                KubernetesTenantConfigProvider::new_in_memory_for_tests("default".to_string()),
            ),
            provider_registry: Arc::new(memory::provider_registry::TenantProviderRegistry::new(
                None, None,
            )),
            git_provider_connection_registry,
            redis_conn: None,
            redis_url: None,
            tenant_runtime_state: std::sync::Arc::new(
                crate::server::tenant_runtime_state::TenantRuntimeRegistry::new(),
            ),
        });

        (server, app_state, tempdir)
    }

    #[tokio::test]
    async fn sse_endpoint_returns_ok() {
        let app = Router::new().route("/sse", get(handle_sse));
        let response = app
            .oneshot(Request::builder().uri("/sse").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.contains("text/event-stream"));
    }

    #[tokio::test]
    async fn message_endpoint_returns_json_rpc_tools_list() {
        let (server, app_state, _tmp) = test_mcp_server().await;
        let expected_tools = server.list_tools().len();
        let app = router(server, app_state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/message")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "tools/list"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"].as_array().unwrap().len(), expected_tools);
    }
}
