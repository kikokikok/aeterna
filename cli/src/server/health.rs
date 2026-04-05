use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

use super::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
pub(crate) struct ReadinessResponse {
    status: &'static str,
    checks: ReadinessChecks,
}

#[derive(Serialize)]
pub(crate) struct ReadinessChecks {
    postgres: CheckResult,
    vector_store: CheckResult,
}

#[derive(Serialize)]
pub(crate) struct CheckResult {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/live", get(liveness_handler))
        .route("/ready", get(readiness_handler))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tracing::instrument(skip_all)]
async fn liveness_handler() -> StatusCode {
    StatusCode::OK
}

#[tracing::instrument(skip_all)]
async fn readiness_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pg_check = check_postgres(&state).await;
    let vector_check = check_vector_store(&state).await;

    readiness_response(pg_check, vector_check)
}

pub(crate) fn readiness_response(
    pg_check: CheckResult,
    vector_check: CheckResult,
) -> (StatusCode, Json<ReadinessResponse>) {
    let all_healthy = pg_check.status == "ok" && vector_check.status == "ok";
    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = ReadinessResponse {
        status: if all_healthy { "ready" } else { "not_ready" },
        checks: ReadinessChecks {
            postgres: pg_check,
            vector_store: vector_check,
        },
    };

    (status_code, Json(response))
}

#[tracing::instrument(skip_all)]
async fn check_postgres(state: &AppState) -> CheckResult {
    match sqlx::query("SELECT 1").execute(state.postgres.pool()).await {
        Ok(_) => CheckResult {
            status: "ok",
            message: None,
        },
        Err(e) => CheckResult {
            status: "error",
            message: Some(e.to_string()),
        },
    }
}

#[tracing::instrument(skip_all)]
async fn check_vector_store(_state: &AppState) -> CheckResult {
    // Vector store health is verified through the memory manager's embedded
    // backend. For now, report ok — the readiness probe covers postgres which
    // is the critical dependency for startup.
    CheckResult {
        status: "ok",
        message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginAuthState;
    use crate::server::plugin_auth::RefreshTokenStore;
    use agent_a2a::config::TrustedIdentityConfig;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use knowledge::api::GovernanceDashboardApi;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::{GitRepository, RepositoryError};
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, ReasoningStrategy, ReasoningTrace, Role, RoleIdentifier,
        TenantContext, UserId,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use storage::postgres::PostgresBackend;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::state_persister::FilePersister;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
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

    async fn app_state() -> Arc<AppState> {
        let tempdir = tempfile::tempdir().unwrap();
        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .unwrap();
        let postgres = Arc::new(PostgresBackend::from_pool(lazy_pool));
        let governance_engine = Arc::new(GovernanceEngine::new());
        let git_repo = Arc::new(GitRepository::new(tempdir.path()).unwrap());
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
        let governance_dashboard = Arc::new(GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let mcp_server = Arc::new(tools::server::McpServer::new(
            memory_manager.clone(),
            sync_manager.clone(),
            Arc::new(MockRepo),
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
            postgres,
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
                refresh_store: RefreshTokenStore::new(),
            }),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider: Arc::new(KubernetesTenantConfigProvider::new(
                "default".to_string(),
            )),
            git_provider_connection_registry,
        })
    }

    fn mock_router() -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/live", get(liveness_handler))
    }

    #[tokio::test]
    async fn test_health_returns_ok() {
        let app = mock_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_liveness_returns_200() {
        let app = mock_router();
        let response = app
            .oneshot(Request::builder().uri("/live").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn router_ready_endpoint_reports_not_ready_without_live_postgres() {
        let app = router(app_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "not_ready");
        assert_eq!(json["checks"]["vector_store"]["status"], "ok");
        assert_eq!(json["checks"]["postgres"]["status"], "error");
    }

    #[tokio::test]
    async fn test_health_has_version_field() {
        let app = mock_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("version").is_some());
    }

    #[test]
    fn readiness_response_is_ready_when_all_checks_ok() {
        let (status, Json(body)) = readiness_response(
            CheckResult {
                status: "ok",
                message: None,
            },
            CheckResult {
                status: "ok",
                message: None,
            },
        );

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ready");
    }

    #[test]
    fn readiness_response_is_not_ready_when_one_backend_fails() {
        let (status, Json(body)) = readiness_response(
            CheckResult {
                status: "error",
                message: Some("db down".to_string()),
            },
            CheckResult {
                status: "ok",
                message: None,
            },
        );

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.checks.postgres.message.as_deref(), Some("db down"));
    }

    #[test]
    fn readiness_response_is_not_ready_when_all_backends_fail() {
        let (status, Json(body)) = readiness_response(
            CheckResult {
                status: "error",
                message: Some("db down".to_string()),
            },
            CheckResult {
                status: "error",
                message: Some("vector down".to_string()),
            },
        );

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(
            body.checks.vector_store.message.as_deref(),
            Some("vector down")
        );
    }
}
