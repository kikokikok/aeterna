use std::collections::HashMap;
use std::sync::Arc;

use aeterna::server::plugin_auth::{
    PluginTokenClaims, RefreshTokenStore, RefreshTokenStoreBackend,
};
use aeterna::server::{AppState, PluginAuthState, health, metrics, router};
use agent_a2a::config::TrustedIdentityConfig;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use jsonwebtoken::{EncodingKey, Header, encode};
use knowledge::api::GovernanceDashboardApi;
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::{GitRepository, RepositoryError};
use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
use memory::manager::MemoryManager;
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::{AuthorizationService, KnowledgeRepository};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, ReasoningStrategy,
    ReasoningTrace, Role, RoleIdentifier, TenantContext, TenantId, UserId,
};
use serde_json::json;
use storage::governance::GovernanceStorage;
use storage::postgres::PostgresBackend;
use storage::secret_provider::LocalSecretProvider;
use storage::tenant_config_provider::KubernetesTenantConfigProvider;
use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
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

async fn test_app_state_with_plugin_auth(
    plugin_auth_config: config::PluginAuthConfig,
) -> Option<(Arc<AppState>, TempDir)> {
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
    let dashboard = Arc::new(GovernanceDashboardApi::new(
        governance_engine.clone(),
        postgres.clone(),
        config::config::DeploymentConfig::default(),
    ));
    let mcp_server = Arc::new(McpServer::new(
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
    let git_provider_connection_registry =
        Arc::new(storage::git_provider_connection_store::InMemoryGitProviderConnectionStore::new());
    let tenant_repo_resolver = Arc::new(
        TenantRepositoryResolver::new(
            tenant_repository_binding_store.clone(),
            std::env::temp_dir(),
            Arc::new(LocalSecretProvider::new(HashMap::new())),
        )
        .with_connection_registry(git_provider_connection_registry.clone()),
    );

    Some((
        Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres: postgres.clone(),
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
            governance_storage: Some(Arc::new(GovernanceStorage::new(postgres.pool().clone()))),
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
                config: plugin_auth_config,
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
            tenant_config_provider: Arc::new(KubernetesTenantConfigProvider::new(
                "default".to_string(),
            )),
            provider_registry: Arc::new(memory::provider_registry::TenantProviderRegistry::new(
                None, None,
            )),
            git_provider_connection_registry,
            redis_conn: None,
        }),
        tempdir,
    ))
}

async fn test_app_state() -> Option<(Arc<AppState>, TempDir)> {
    test_app_state_with_plugin_auth(config::PluginAuthConfig::default()).await
}

async fn seed_company_unit(state: &Arc<AppState>, tenant_id: &TenantId) -> String {
    let unit_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    state
        .postgres
        .create_unit(&mk_core::types::OrganizationalUnit {
            id: unit_id.clone(),
            name: "Acme".to_string(),
            unit_type: mk_core::types::UnitType::Company,
            parent_id: None,
            tenant_id: tenant_id.clone(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            source_owner: mk_core::types::RecordSource::Admin,
        })
        .await
        .unwrap();
    unit_id
}

fn mint_test_plugin_bearer(secret: &str, tenant_id: &str, github_login: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &PluginTokenClaims {
            sub: github_login.to_string(),
            idp_provider: "github".to_string(),
            tenant_id: tenant_id.to_string(),
            iss: "aeterna-test".to_string(),
            aud: vec![PluginTokenClaims::AUDIENCE.to_string()],
            iat: now,
            exp: now + 3600,
            jti: "test-jti".to_string(),
            github_id: 42,
            email: Some(format!("{github_login}@example.com")),
            kind: PluginTokenClaims::KIND.to_string(),
        },
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
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

#[tokio::test]
async fn session_start_rejects_invalid_plugin_bearer_when_plugin_auth_enabled() {
    let secret = "super-secret-test-key-at-least-32-chars";
    let Some((state, _tmp)) = test_app_state_with_plugin_auth(config::PluginAuthConfig {
        enabled: true,
        jwt_secret: Some(secret.to_string()),
        ..Default::default()
    })
    .await
    else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, "Bearer not-a-jwt")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project": "auth-test",
                        "directory": "/tmp/auth-test"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "invalid_plugin_token");
}

#[tokio::test]
async fn session_start_accepts_valid_plugin_bearer_and_returns_github_login() {
    let secret = "super-secret-test-key-at-least-32-chars";
    let Some((state, _tmp)) = test_app_state_with_plugin_auth(config::PluginAuthConfig {
        enabled: true,
        jwt_secret: Some(secret.to_string()),
        ..Default::default()
    })
    .await
    else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);
    let token = mint_test_plugin_bearer(secret, "tenant-7", "octocat");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "project": "auth-test",
                        "directory": "/tmp/auth-test"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["userId"], "octocat");
    assert_eq!(json["project"], "auth-test");
    assert!(!json["sessionId"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn sync_push_rejects_invalid_plugin_bearer_when_plugin_auth_enabled() {
    let secret = "super-secret-test-key-at-least-32-chars";
    let Some((state, _tmp)) = test_app_state_with_plugin_auth(config::PluginAuthConfig {
        enabled: true,
        jwt_secret: Some(secret.to_string()),
        ..Default::default()
    })
    .await
    else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sync/push")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, "Bearer not-a-jwt")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "entries": [],
                        "device_id": "test-device"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "invalid_plugin_token");
}

#[tokio::test]
async fn plugin_auth_bootstrap_returns_service_unavailable_when_disabled() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/plugin/bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "provider": "github",
                        "github_access_token": "gho_test"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn plugin_auth_refresh_rejects_invalid_refresh_token() {
    let secret = "super-secret-test-key-at-least-32-chars";
    let Some((state, _tmp)) = test_app_state_with_plugin_auth(config::PluginAuthConfig {
        enabled: true,
        jwt_secret: Some(secret.to_string()),
        ..Default::default()
    })
    .await
    else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/plugin/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "refresh_token": "missing-refresh-token"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "invalid_refresh_token");
}

#[tokio::test]
async fn tenant_admin_route_group_is_mounted_for_org_team_user_and_govern() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    for path in [
        "/api/v1/org",
        "/api/v1/team",
        "/api/v1/user/test-user/roles",
        "/api/v1/govern/roles",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "route should be mounted: {path}"
        );
    }
}

#[tokio::test]
async fn user_role_revoke_fails_closed_when_assignment_scope_is_ambiguous() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let user_id = UserId::new("test-user".to_string()).unwrap();

    let company_unit_id = "11111111-1111-1111-1111-111111111111".to_string();
    let org_unit_id = "22222222-2222-2222-2222-222222222222".to_string();
    let team_unit_id = "33333333-3333-3333-3333-333333333333".to_string();
    let now = chrono::Utc::now().timestamp();

    state
        .postgres
        .create_unit(&mk_core::types::OrganizationalUnit {
            id: company_unit_id.clone(),
            name: "Acme".to_string(),
            unit_type: mk_core::types::UnitType::Company,
            parent_id: None,
            tenant_id: tenant_id.clone(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            source_owner: mk_core::types::RecordSource::Admin,
        })
        .await
        .unwrap();
    state
        .postgres
        .create_unit(&mk_core::types::OrganizationalUnit {
            id: org_unit_id.clone(),
            name: "Platform".to_string(),
            unit_type: mk_core::types::UnitType::Organization,
            parent_id: Some(company_unit_id.clone()),
            tenant_id: tenant_id.clone(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            source_owner: mk_core::types::RecordSource::Admin,
        })
        .await
        .unwrap();
    state
        .postgres
        .create_unit(&mk_core::types::OrganizationalUnit {
            id: team_unit_id.clone(),
            name: "API".to_string(),
            unit_type: mk_core::types::UnitType::Team,
            parent_id: Some(org_unit_id.clone()),
            tenant_id: tenant_id.clone(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            source_owner: mk_core::types::RecordSource::Admin,
        })
        .await
        .unwrap();
    state
        .postgres
        .assign_role(&user_id, &tenant_id, &org_unit_id, Role::Developer.into())
        .await
        .unwrap();
    state
        .postgres
        .assign_role(&user_id, &tenant_id, &team_unit_id, Role::Developer.into())
        .await
        .unwrap();

    let secret = "super-secret-test-key-at-least-32-chars";
    let token = mint_test_plugin_bearer(secret, "default", "admin-user");
    let mut state = (*state).clone();
    state.plugin_auth_state = Arc::new(PluginAuthState {
        config: config::PluginAuthConfig {
            enabled: true,
            jwt_secret: Some(secret.to_string()),
            ..Default::default()
        },
        postgres: Some(state.postgres.clone()),
        refresh_store: RefreshTokenStoreBackend::InMemory(RefreshTokenStore::new()),
    });
    let app = router::build_router(Arc::new(state));

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/user/test-user/roles/developer")
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"], "ambiguous_role_assignment");
    assert_eq!(json["assignments"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn tenant_admin_hierarchy_role_crud_via_rest_api() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/hierarchy")
                .header("content-type", "application/json")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Platform",
                        "unit_type": "organization",
                        "parent_id": null,
                        "metadata": {},
                        "source_owner": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let unit_id = create_json["unit"]["id"].as_str().unwrap().to_string();
    assert_eq!(create_json["unit"]["unitType"], "organization");

    let assign_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/hierarchy/{unit_id}/members"))
                .header("content-type", "application/json")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "user_id": "tenant-member",
                        "role": "developer"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign_response.status(), StatusCode::CREATED);
    let assign_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(assign_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(assign_json["membership"]["userId"], "tenant-member");
    assert_eq!(assign_json["membership"]["role"], "developer");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/admin/hierarchy/{unit_id}/members"))
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        list_json["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|member| {
                member["user_id"] == "tenant-member" && member["role"] == "developer"
            })
    );

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/admin/hierarchy/{unit_id}/members/tenant-member/roles/developer"
                ))
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoke_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(revoke_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(revoke_json["success"], true);

    let list_after_revoke = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/admin/hierarchy/{unit_id}/members"))
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_after_revoke.status(), StatusCode::OK);
    let list_after_revoke_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(list_after_revoke.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        !list_after_revoke_json["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|member| member["user_id"] == "tenant-member")
    );
}

#[tokio::test]
async fn tenant_admin_hierarchy_rejects_platform_admin_role_assignment() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/hierarchy")
                .header("content-type", "application/json")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Security",
                        "unit_type": "organization",
                        "parent_id": null,
                        "metadata": {},
                        "source_owner": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let unit_id = create_json["unit"]["id"].as_str().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/hierarchy/{unit_id}/members"))
                .header("content-type", "application/json")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "user_id": "tenant-member",
                        "role": "platformAdmin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json["error"], "invalid_role_assignment");
}

#[tokio::test]
async fn govern_roles_assign_and_revoke_via_rest_api() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let _company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);
    let principal = "11111111-2222-3333-4444-555555555555";

    let assign_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/roles")
                .header("content-type", "application/json")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "principal": principal,
                        "principalType": "user",
                        "role": "architect",
                        "scope": "company"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign_response.status(), StatusCode::OK);
    let assign_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(assign_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(assign_json["principal"], principal);
    assert_eq!(assign_json["principalType"], "user");
    assert_eq!(assign_json["role"], "architect");
    assert_eq!(assign_json["scope"], "company");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/govern/roles")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(list_json.as_array().unwrap().iter().any(|entry| {
        entry["principal"] == principal
            && entry["role"] == "architect"
            && entry["scope"] == "company"
    }));

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/govern/roles/{principal}/architect"))
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoke_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(revoke_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(revoke_json["success"], true);
    assert_eq!(revoke_json["principal"], principal);
    assert_eq!(revoke_json["role"], "architect");

    let list_after_revoke = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/govern/roles")
                .header("x-user-id", "tenant-admin")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_after_revoke.status(), StatusCode::OK);
    let list_after_revoke_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(list_after_revoke.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        !list_after_revoke_json
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["principal"] == principal && entry["role"] == "architect" })
    );
}

#[tokio::test]
async fn tenant_lifecycle_crud_via_rest_api() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/tenants")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "slug": "acme-runtime",
                        "name": "Acme Runtime"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(create.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(create_json["tenant"]["slug"], "acme-runtime");

    let show = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/tenants/acme-runtime")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(show.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/admin/tenants/acme-runtime")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Acme Runtime Updated"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let update_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(update.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(update_json["tenant"]["name"], "Acme Runtime Updated");

    let deactivate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/tenants/acme-runtime/deactivate")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deactivate.status(), StatusCode::OK);
    let deactivate_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(deactivate.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(deactivate_json["tenant"]["status"], "inactive");
}

#[tokio::test]
async fn tenant_repo_binding_set_and_show() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    state
        .tenant_store
        .create_tenant("binding-tenant", "Binding Tenant")
        .await
        .unwrap();
    let app = router::build_router(state);

    let set_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/admin/tenants/binding-tenant/repository-binding")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "kind": "local",
                        "local_path": "/tmp/binding-tenant",
                        "branch": "main",
                        "branch_policy": "directCommit",
                        "credential_kind": "none",
                        "credential_ref": null,
                        "github_owner": null,
                        "github_repo": null,
                        "source_owner": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(set_response.status(), StatusCode::OK);
    let set_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(set_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(set_json["binding"]["kind"], "local");
    assert_eq!(set_json["binding"]["sourceOwner"], "admin");

    let show_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/tenants/binding-tenant/repository-binding")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(show_response.status(), StatusCode::OK);
    let show_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(show_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(show_json["binding"]["localPath"], "/tmp/binding-tenant");
    assert_eq!(show_json["binding"]["branchPolicy"], "directCommit");
    assert_eq!(show_json["binding"]["sourceOwner"], "admin");
}

#[tokio::test]
async fn tenant_repo_binding_validate_and_missing_binding_behave_honestly() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    state
        .tenant_store
        .create_tenant("validate-tenant", "Validate Tenant")
        .await
        .unwrap();
    let app = router::build_router(state);

    let missing_show = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/tenants/validate-tenant/repository-binding")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_show.status(), StatusCode::NOT_FOUND);
    let missing_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(missing_show.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(missing_json["error"], "binding_not_found");

    let validate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/tenants/validate-tenant/repository-binding/validate")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "kind": "local",
                        "local_path": "/tmp/validate-tenant",
                        "branch": "main",
                        "branch_policy": "directCommit",
                        "credential_kind": "none",
                        "credential_ref": null,
                        "github_owner": null,
                        "github_repo": null,
                        "source_owner": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate_response.status(), StatusCode::OK);
    let validate_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(validate_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(validate_json["valid"], true);
    assert_eq!(validate_json["binding"]["sourceOwner"], "admin");
}

#[tokio::test]
async fn tenant_repo_binding_rejects_invalid_credential_ref() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    state
        .tenant_store
        .create_tenant("invalid-cred", "Invalid Cred")
        .await
        .unwrap();
    let app = router::build_router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/admin/tenants/invalid-cred/repository-binding")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "kind": "github",
                        "local_path": null,
                        "remote_url": "https://github.com/acme/knowledge.git",
                        "branch": "main",
                        "branch_policy": "requirePullRequest",
                        "credential_kind": "githubApp",
                        "credential_ref": "raw-secret-material",
                        "github_owner": "acme",
                        "github_repo": "knowledge",
                        "source_owner": "admin"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json["error"], "invalid_credential_ref");
}

#[tokio::test]
async fn tenant_api_requires_platform_admin_role() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/tenants")
                .header("content-type", "application/json")
                .header("x-user-id", "developer-user")
                .header("x-user-role", "developer")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "slug": "forbidden-tenant",
                        "name": "Forbidden Tenant"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json["error"], "forbidden");
    assert_eq!(json["message"], "PlatformAdmin role required");
}

#[tokio::test]
async fn git_provider_connection_lifecycle_and_binding_visibility_work() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant = state
        .tenant_store
        .create_tenant("conn-tenant", "Connection Tenant")
        .await
        .unwrap();
    let app = router::build_router(state.clone());

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/git-provider-connections")
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Shared GitHub App",
                        "providerKind": "githubApp",
                        "appId": 12345,
                        "installationId": 67890,
                        "pemSecretRef": "local/github-app-pem",
                        "webhookSecretRef": "local/github-webhook"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let connection_id = create_json["connection"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(create_json["connection"]["pemSecretRef"], "[redacted]");

    let pregrant_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/admin/tenants/{}/git-provider-connections",
                    tenant.slug
                ))
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pregrant_list.status(), StatusCode::OK);
    let pregrant_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(pregrant_list.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(pregrant_json["connections"].as_array().unwrap().len(), 0);

    let forbidden_binding = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/v1/admin/tenants/{}/repository-binding",
                    tenant.slug
                ))
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "kind": "github",
                        "remoteUrl": "https://github.com/acme/knowledge.git",
                        "branch": "main",
                        "branchPolicy": "requirePullRequest",
                        "credentialKind": "githubApp",
                        "credentialRef": null,
                        "githubOwner": "acme",
                        "githubRepo": "knowledge",
                        "sourceOwner": "admin",
                        "gitProviderConnectionId": connection_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden_binding.status(), StatusCode::FORBIDDEN);

    let grant_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/admin/git-provider-connections/{}/tenants/{}",
                    connection_id, tenant.slug
                ))
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grant_response.status(), StatusCode::OK);

    let visible_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/admin/tenants/{}/git-provider-connections",
                    tenant.slug
                ))
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(visible_list.status(), StatusCode::OK);
    let visible_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(visible_list.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(visible_json["connections"].as_array().unwrap().len(), 1);
    assert_eq!(visible_json["connections"][0]["id"], connection_id);

    let allowed_binding = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/v1/admin/tenants/{}/repository-binding",
                    tenant.slug
                ))
                .header("content-type", "application/json")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "kind": "github",
                        "remoteUrl": "https://github.com/acme/knowledge.git",
                        "branch": "main",
                        "branchPolicy": "requirePullRequest",
                        "credentialKind": "githubApp",
                        "credentialRef": null,
                        "githubOwner": "acme",
                        "githubRepo": "knowledge",
                        "sourceOwner": "admin",
                        "gitProviderConnectionId": connection_id
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_binding.status(), StatusCode::OK);
    let allowed_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(allowed_binding.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        allowed_json["binding"]["gitProviderConnectionId"],
        connection_id
    );

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/admin/git-provider-connections/{}/tenants/{}",
                    connection_id, tenant.slug
                ))
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_response.status(), StatusCode::OK);

    let post_revoke_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/admin/tenants/{}/git-provider-connections",
                    tenant.slug
                ))
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_revoke_list.status(), StatusCode::OK);
    let post_revoke_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(post_revoke_list.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(post_revoke_json["connections"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn admin_permissions_matrix_and_effective_endpoints_return_expected_shapes() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let matrix_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/permissions/matrix")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(matrix_response.status(), StatusCode::OK);
    let matrix_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(matrix_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matrix_json["matrix"]["platformAdmin"].is_array());
    assert!(matrix_json["matrix"]["admin"].is_array());
    assert!(matrix_json["matrix"]["architect"].is_array());
    assert!(matrix_json["matrix"]["techLead"].is_array());
    assert!(matrix_json["matrix"]["developer"].is_array());
    assert!(matrix_json["matrix"]["viewer"].is_array());
    assert!(matrix_json["actions"].as_array().unwrap().len() > 5);

    let effective_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/permissions/effective?user_id=alice&role=developer")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(effective_response.status(), StatusCode::OK);
    let effective_json: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(effective_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(effective_json["userId"], "alice");
    assert!(!effective_json["roles"].as_array().unwrap().is_empty());
    assert!(effective_json["granted"].as_array().is_some());
    assert!(effective_json["denied"].as_array().is_some());
}

// ─── Test 1: tenant list and hierarchy read operations ────────────────────────

#[tokio::test]
async fn tenant_list_and_hierarchy_read_operations() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // list_tenants – platform_admin
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/tenants")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    assert!(body["tenants"].is_array());

    // list_tenants – forbidden without platform_admin
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/tenants")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // list_hierarchy_units (no filter) – returns company unit
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/hierarchy")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    let units = body["units"].as_array().unwrap();
    assert!(!units.is_empty());
    let ids: Vec<_> = units.iter().filter_map(|u| u["id"].as_str()).collect();
    assert!(ids.contains(&company_id.as_str()));

    // show_hierarchy_unit
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/admin/hierarchy/{company_id}"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["unit"]["id"], company_id.as_str());

    // show_hierarchy_unit – not found
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/hierarchy/00000000-0000-0000-0000-000000000000")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ─── Test 2: hierarchy update, ancestors/descendants, user memberships ────────

#[tokio::test]
async fn hierarchy_update_ancestors_descendants_memberships() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let company_id = seed_company_unit(&state, &tenant_id).await;

    // Create a child org unit via HTTP so it gets seeded in postgres
    let app = router::build_router(state.clone());
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/hierarchy")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Engineering Org",
                        "unitType": "organization",
                        "parentId": company_id,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let org_unit_id = body["unit"]["id"].as_str().unwrap().to_string();

    // update_hierarchy_unit – rename the org
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/admin/hierarchy/{org_unit_id}"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "name": "Platform Engineering" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["unit"]["name"], "Platform Engineering");

    // update_hierarchy_unit – not found
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/admin/hierarchy/00000000-0000-0000-0000-000000000001")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "name": "Ghost" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // list_hierarchy_ancestors of org (parent = company)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/admin/hierarchy/{org_unit_id}/ancestors"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    let ancestor_ids: Vec<_> = body["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["id"].as_str())
        .collect();
    assert!(ancestor_ids.contains(&company_id.as_str()));

    // list_hierarchy_descendants of company (should include org)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/admin/hierarchy/{company_id}/descendants"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    let desc_ids: Vec<_> = body["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["id"].as_str())
        .collect();
    assert!(desc_ids.contains(&org_unit_id.as_str()));

    // assign a member then list_user_memberships
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/hierarchy/{org_unit_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "userId": "alice", "role": "developer" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/memberships?user_id=alice")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["userId"], "alice");
    let memberships = body["memberships"].as_array().unwrap();
    assert!(!memberships.is_empty());
    assert!(
        memberships
            .iter()
            .any(|m| m["unit_id"].as_str() == Some(org_unit_id.as_str()))
    );

    // add_domain_mapping
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/tenants/default/domain-mappings")
                .header("x-user-id", "platform-admin")
                .header("x-user-role", "platform_admin")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "domain": "example.com" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // 201 on success or 400 if tenant row not found — either way the handler ran
    assert!(
        resp.status() == StatusCode::CREATED || resp.status() == StatusCode::BAD_REQUEST,
        "unexpected status: {}",
        resp.status()
    );
}

// ─── Test 3: org CRUD and member lifecycle ────────────────────────────────────

#[tokio::test]
async fn org_crud_and_member_lifecycle() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // list_orgs – initially empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/org")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // create_org with invalid company_id
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/org")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Ghost Org",
                        "companyId": "00000000-0000-0000-0000-000000000099",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // create_org – success
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/org")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Platform Engineering",
                        "companyId": company_id,
                        "description": "Core infra org",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let org_id = body["id"].as_str().unwrap().to_string();

    // show_org
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/org/{org_id}"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["id"], org_id.as_str());
    assert_eq!(body["name"], "Platform Engineering");

    // show_org – not found
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/org/00000000-0000-0000-0000-000000000000")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // list_members – empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/org/{org_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // add_member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/org/{org_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "userId": "alice", "role": "developer" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // list_members – alice present
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/org/{org_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|m| m["userId"].as_str() == Some("alice"))
    );

    // set_member_role
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/org/{org_id}/members/alice/role"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "role": "tech_lead" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // remove_member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/org/{org_id}/members/alice"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── Test 4: team CRUD and member lifecycle ───────────────────────────────────

#[tokio::test]
async fn team_crud_and_member_lifecycle() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state.clone());

    // Create parent org first
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/org")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Platform Org",
                        "companyId": company_id,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let org_body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let org_id = org_body["id"].as_str().unwrap().to_string();

    // list_teams – initially empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/team")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // create_team – invalid org
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/team")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Ghost Team",
                        "orgId": "00000000-0000-0000-0000-000000000099",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // create_team – success
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/team")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "API Team",
                        "orgId": org_id,
                        "description": "Core API team",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let team_id = body["id"].as_str().unwrap().to_string();

    // show_team
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/team/{team_id}"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["id"], team_id.as_str());
    assert_eq!(body["name"], "API Team");

    // show_team – not found
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/team/00000000-0000-0000-0000-000000000000")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // list_members – empty
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/team/{team_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // add_member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/team/{team_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "userId": "bob", "role": "developer" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // list_members – bob present
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/team/{team_id}/members"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|m| m["userId"].as_str() == Some("bob"))
    );

    // set_member_role
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/team/{team_id}/members/bob/role"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "role": "tech_lead" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // list_teams with org filter
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/team?org={org_id}"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"].as_str() == Some(team_id.as_str()))
    );

    // remove_member
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/team/{team_id}/members/bob"))
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── Test 5: user roles grant/revoke (no users table needed) ─────────────────

#[tokio::test]
async fn user_roles_grant_revoke_and_unsupported_schema() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // list_user_roles – no roles yet (just ensures handler runs; no users table needed)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/user/alice/roles")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // grant_user_role – scope "company" (needs company unit)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/user/alice/roles")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "role": "developer", "scope": "company" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["userId"], "alice");
    assert_eq!(body["role"], "developer");
    assert_eq!(body["unitId"], company_id.as_str());

    // list_user_roles – alice now has developer role
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/user/alice/roles")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().unwrap().iter().any(|r| {
        r["role"].as_str() == Some("developer")
            && r["unit_id"].as_str() == Some(company_id.as_str())
    }));

    // revoke_user_role – success
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/user/alice/roles/developer")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["success"], true);

    // revoke_user_role – 404 when no role to revoke
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/user/alice/roles/developer")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // list_users – returns 503 service_unavailable when no users table exists
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/user")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["error"], "user_schema_unsupported");

    // show_user – also returns 503 without users table
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/user/alice")
                .header("x-user-id", "alice")
                .header("x-user-role", "developer")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    // invite_user – also returns 503 without users table
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/user/invite")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "email": "new@example.com" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

// ─── Test 6: govern status, config, audit and pending ────────────────────────

#[tokio::test]
async fn govern_status_config_audit_and_pending() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let _company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // govern/status
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/status")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body["config"].is_object());
    assert!(body["metrics"]["pendingRequests"].is_number());

    // govern/config GET
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/config")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let config_before: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(config_before["approvalMode"].is_string());

    // govern/config PUT – update min_approvers
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/govern/config")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "minApprovers": 2 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["config"]["minApprovers"], 2);

    // govern/config PUT – invalid approval_mode
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/govern/config")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "approvalMode": "not_a_real_mode" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // govern/audit GET
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/audit")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().is_some());

    // govern/audit GET – invalid since param
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/audit?since=notadate")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // govern/pending GET
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/pending")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body.as_array().is_some());

    // govern/pending GET – invalid request type
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/pending?type=not_a_type")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // govern/approve – request not found (404 path)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/approve/00000000-0000-0000-0000-000000000000")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "comment": "lgtm" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // 404 or 400 depending on whether uuid is recognised
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::BAD_REQUEST,
        "unexpected status: {}",
        resp.status()
    );

    // govern/reject – reason required validation
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/reject/00000000-0000-0000-0000-000000000000")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&json!({})).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["error"], "reason_required");
}

// ─── Test: GET /api/v1/admin/stats ───────────────────────────────────────────

#[tokio::test]
async fn admin_stats_returns_counts() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let _company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/stats")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        body["tenantCount"].is_number(),
        "tenantCount must be a number"
    );
    assert!(body["userCount"].is_number(), "userCount must be a number");
    assert!(
        body["memoryCount"].is_number(),
        "memoryCount must be a number"
    );
    assert!(
        body["knowledgeCount"].is_number(),
        "knowledgeCount must be a number"
    );
}

// ─── Test: GET + POST /api/v1/govern/policies ────────────────────────────────

#[tokio::test]
async fn govern_policies_list_and_create() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let _company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // GET /govern/policies – empty list initially
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/govern/policies")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body["policies"].is_array(), "policies must be an array");

    // POST /govern/policies – create a new policy
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/policies")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "test-policy",
                        "description": "integration test policy",
                        "layer": "company",
                        "mode": "optional"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(body["id"].is_string(), "id must be a string");
    assert_eq!(body["name"], "test-policy");

    // POST /govern/policies – invalid layer returns 422
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/policies")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "name": "bad", "layer": "unknown_layer" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["error"], "invalid_layer");
}

// ─── Test: POST /api/v1/memory/search is reachable ───────────────────────────

#[tokio::test]
async fn memory_search_route_is_mounted() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let app = router::build_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/memory/search")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "query": "test", "limit": 5 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // The route must exist – 404 is the only unacceptable status
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "POST /api/v1/memory/search must be mounted"
    );
}

// ─── Test: govern/approve and govern/reject path order ───────────────────────

#[tokio::test]
async fn govern_approve_reject_path_order() {
    let Some((state, _tmp)) = test_app_state().await else {
        eprintln!("Skipping server runtime test: Docker not available");
        return;
    };
    let tenant_id = TenantId::new("default".to_string()).unwrap();
    let _company_id = seed_company_unit(&state, &tenant_id).await;
    let app = router::build_router(state);

    // /govern/approve/{id} must be routable (not 404 / 405)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/approve/00000000-0000-0000-0000-000000000001")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "comment": "lgtm" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);

    // /govern/reject/{id} must be routable (not 404 / 405)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/govern/reject/00000000-0000-0000-0000-000000000001")
                .header("x-user-id", "admin-user")
                .header("x-user-role", "admin")
                .header("x-tenant-id", "default")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "reason": "not good" })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}
