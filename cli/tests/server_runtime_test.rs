use std::collections::HashMap;
use std::sync::Arc;

use aeterna::server::{AppState, health, metrics, router};
use agent_a2a::config::TrustedIdentityConfig;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use knowledge::api::GovernanceDashboardApi;
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::{GitRepository, RepositoryError};
use memory::manager::MemoryManager;
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::{AuthorizationService, KnowledgeRepository};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, ReasoningStrategy,
    ReasoningTrace, Role, TenantContext, UserId,
};
use serde_json::json;
use storage::postgres::PostgresBackend;
use sync::bridge::SyncManager;
use sync::state_persister::FilePersister;
use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
use tempfile::TempDir;
use testing::postgres;
use tools::server::McpServer;
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

    async fn get_user_roles(&self, _ctx: &TenantContext) -> Result<Vec<Role>, Self::Error> {
        Ok(vec![Role::Developer])
    }

    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct MockRepo {
    items: tokio::sync::RwLock<HashMap<(KnowledgeLayer, String), KnowledgeEntry>>,
}

impl MockRepo {
    fn new() -> Self {
        Self {
            items: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl KnowledgeRepository for MockRepo {
    type Error = RepositoryError;

    async fn get(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        path: &str,
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        Ok(self
            .items
            .read()
            .await
            .get(&(layer, path.to_string()))
            .cloned())
    }

    async fn store(
        &self,
        _ctx: TenantContext,
        entry: KnowledgeEntry,
        _message: &str,
    ) -> Result<String, Self::Error> {
        self.items
            .write()
            .await
            .insert((entry.layer, entry.path.clone()), entry);
        Ok("mock-commit".to_string())
    }

    async fn list(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        prefix: &str,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(self
            .items
            .read()
            .await
            .iter()
            .filter(|((entry_layer, path), _)| *entry_layer == layer && path.starts_with(prefix))
            .map(|(_, value)| value.clone())
            .collect())
    }

    async fn delete(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        path: &str,
        _message: &str,
    ) -> Result<String, Self::Error> {
        self.items.write().await.remove(&(layer, path.to_string()));
        Ok("mock-commit".to_string())
    }

    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
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
        query: &str,
        layers: Vec<KnowledgeLayer>,
        limit: usize,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(self
            .items
            .read()
            .await
            .values()
            .filter(|entry| layers.contains(&entry.layer) && entry.content.contains(query))
            .take(limit)
            .cloned()
            .collect())
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

fn sample_entry(path: &str) -> KnowledgeEntry {
    KnowledgeEntry {
        path: path.to_string(),
        content: "sample content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        status: KnowledgeStatus::Draft,
        summaries: HashMap::new(),
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: 0,
    }
}

async fn test_app_state() -> Option<(Arc<AppState>, TempDir)> {
    let tempdir = tempfile::tempdir().unwrap();
    let repo = Arc::new(MockRepo::new());
    repo.store(
        TenantContext::new(
            mk_core::types::TenantId::new("default".to_string()).unwrap(),
            mk_core::types::UserId::new("system".to_string()).unwrap(),
        ),
        sample_entry("specs/example.md"),
        "seed",
    )
    .await
    .unwrap();

    let fixture = postgres().await?;
    let postgres = Arc::new(PostgresBackend::new(fixture.url()).await.ok()?);
    postgres.initialize_schema().await.ok()?;
    let governance_engine = Arc::new(GovernanceEngine::new());
    let git_repo = Arc::new(GitRepository::new(tempdir.path()).unwrap());
    let knowledge_manager = Arc::new(KnowledgeManager::new(git_repo, governance_engine.clone()));
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
    let dashboard = Arc::new(GovernanceDashboardApi::new(
        governance_engine.clone(),
        postgres.clone(),
        config::config::DeploymentConfig::default(),
    ));
    let mcp_server = Arc::new(McpServer::new(
        memory_manager.clone(),
        sync_manager.clone(),
        repo.clone(),
        postgres.clone(),
        governance_engine.clone(),
        Arc::new(TestNoopReasoner),
        auth_service.clone(),
        None,
        None,
        None,
    ));
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);

    Some((
        Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres,
            memory_manager,
            knowledge_manager,
            knowledge_repository: repo,
            governance_engine,
            governance_dashboard: dashboard,
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
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
        }),
        tempdir,
    ))
}

#[tokio::test]
async fn server_health_route_returns_ok() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);
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
}

#[tokio::test]
async fn server_metrics_route_returns_prometheus_content_type() {
    let handle = metrics::create_recorder();
    let app = metrics::router(handle);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("text/plain")
    );
}

#[tokio::test]
async fn server_ready_route_returns_503_with_unavailable_backend() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = health::router(state);
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
}

#[tokio::test]
async fn server_mcp_sse_route_returns_ok() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/mcp/sse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn server_mcp_message_route_returns_json_rpc_response() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let expected_tool_names: std::collections::HashSet<String> = state
        .mcp_server
        .list_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    let app = router::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/message")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
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

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);
    let tool_names: std::collections::HashSet<String> = json["result"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(ToOwned::to_owned))
        .collect();
    assert_eq!(tool_names, expected_tool_names);
    assert!(tool_names.contains("knowledge_list"));
    assert!(tool_names.contains("sync_status"));
}

#[tokio::test]
async fn server_mcp_message_route_can_call_registered_tool() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/message")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/call",
                        "params": {
                            "name": "knowledge_list",
                            "tenantContext": {
                                "tenantId": "default",
                                "userId": "system"
                            },
                            "arguments": {
                                "layer": "project",
                                "prefix": "specs/"
                            }
                        }
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 2);
    assert_eq!(json["result"]["success"], true);
    let entries = json["result"]["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert_eq!(entries[0]["path"], "specs/example.md");
}

#[tokio::test]
async fn server_a2a_route_is_mounted() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/a2a/.well-known/agent.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn shutdown_channel_propagates_true_signal() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let mut rx = state.shutdown_tx.subscribe();
    state.shutdown_tx.send(true).unwrap();
    rx.changed().await.unwrap();
    assert!(*rx.borrow());
}

#[tokio::test]
async fn sync_push_stores_entries() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let push_body = json!({
        "entries": [
            {
                "id": "test-mem-1",
                "content": "Test memory content",
                "layer": "project",
                "tags": ["test"],
                "metadata": null,
                "importance": 0.5,
                "created_at": "2025-01-15T10:30:00Z",
                "updated_at": "2025-01-15T10:30:00Z",
                "deleted_at": null
            }
        ],
        "device_id": "test-device"
    });

    let push_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/push")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(serde_json::to_vec(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(push_response.status(), StatusCode::OK);
    let push_bytes = axum::body::to_bytes(push_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let push_json: serde_json::Value = serde_json::from_slice(&push_bytes).unwrap();
    assert!(!push_json["cursor"].as_str().unwrap().is_empty());
    assert!(push_json["conflicts"].as_array().unwrap().is_empty());

    let pull_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sync/pull?since_cursor=0&layers=project")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);
    let pull_bytes = axum::body::to_bytes(pull_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let pull_json: serde_json::Value = serde_json::from_slice(&pull_bytes).unwrap();
    let entries = pull_json["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|entry| {
        entry["id"] == "test-mem-1"
            && entry["content"] == "Test memory content"
            && entry["layer"] == "project"
    }));
}

#[tokio::test]
async fn sync_pull_returns_seeded_data() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let push_body = json!({
        "entries": [
            {
                "id": "test-mem-project",
                "content": "Project memory",
                "layer": "project",
                "tags": ["project"],
                "metadata": null,
                "importance": 0.7,
                "created_at": "2025-01-15T10:30:00Z",
                "updated_at": "2025-01-15T10:30:00Z",
                "deleted_at": null
            },
            {
                "id": "test-mem-team",
                "content": "Team memory",
                "layer": "team",
                "tags": ["team"],
                "metadata": null,
                "importance": 0.6,
                "created_at": "2025-01-15T10:31:00Z",
                "updated_at": "2025-01-15T10:31:00Z",
                "deleted_at": null
            }
        ],
        "device_id": "test-device"
    });

    let push_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/push")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(serde_json::to_vec(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(push_response.status(), StatusCode::OK);

    let pull_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sync/pull?since_cursor=0&layers=project")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);
    let pull_bytes = axum::body::to_bytes(pull_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let pull_json: serde_json::Value = serde_json::from_slice(&pull_bytes).unwrap();

    let entries = pull_json["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], "test-mem-project");
    assert_eq!(entries[0]["layer"], "project");
    assert!(entries.iter().all(|entry| entry["layer"] == "project"));

    let cursor = pull_json["cursor"].as_str().unwrap();
    assert!(!cursor.is_empty());
    assert_ne!(cursor, "0");
}

#[tokio::test]
async fn sync_endpoints_reject_unauthenticated() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let push_body = json!({
        "entries": [
            {
                "id": "test-mem-unauth",
                "content": "Test memory content",
                "layer": "project",
                "tags": ["test"],
                "metadata": null,
                "importance": 0.5,
                "created_at": "2025-01-15T10:30:00Z",
                "updated_at": "2025-01-15T10:30:00Z",
                "deleted_at": null
            }
        ],
        "device_id": "test-device"
    });

    let push_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/push")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&push_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(push_response.status(), StatusCode::UNAUTHORIZED);
    let push_bytes = axum::body::to_bytes(push_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let push_json: serde_json::Value = serde_json::from_slice(&push_bytes).unwrap();
    assert_eq!(
        push_json,
        json!({
            "error": "auth_required",
            "message": "Bearer token required"
        })
    );

    let pull_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sync/pull?since_cursor=0&layers=project")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pull_response.status(), StatusCode::UNAUTHORIZED);
    let pull_bytes = axum::body::to_bytes(pull_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let pull_json: serde_json::Value = serde_json::from_slice(&pull_bytes).unwrap();
    assert_eq!(
        pull_json,
        json!({
            "error": "auth_required",
            "message": "Bearer token required"
        })
    );
}
