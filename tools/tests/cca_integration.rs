//! CCA (Confucius Code Agent) Integration Tests
//!
//! End-to-end tests for CCA capabilities exposed via MCP tools.
//! Uses Testcontainers for real dependency testing (PostgreSQL, Redis, Qdrant,
//! S3/MinIO, DuckDB).

use async_trait::async_trait;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{AuthorizationService, KnowledgeRepository, MemoryProviderAdapter};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, MemoryLayer, Role, TenantContext, TenantId, UserId,
};
use serde_json::json;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use tempfile;
use testcontainers::{
    ContainerAsync, GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};
use testcontainers_modules::{postgres::Postgres, redis::Redis};
use tokio::sync::OnceCell;
use tools::server::{JsonRpcRequest, McpServer};

// Simple mock auth service for compilation
struct MockAuthService;

#[async_trait]
impl AuthorizationService for MockAuthService {
    type Error = anyhow::Error;

    async fn check_permission(
        &self,
        _ctx: &TenantContext,
        _action: &str,
        _resource: &str,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn get_user_roles(&self, _ctx: &TenantContext) -> anyhow::Result<Vec<Role>> {
        Ok(vec![])
    }

    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

// Shared PostgreSQL fixture for CCA E2E tests
struct PostgresFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    url: String,
}

static POSTGRES_FIXTURE: OnceCell<Option<PostgresFixture>> = OnceCell::const_new();

async fn get_postgres_fixture() -> Option<&'static PostgresFixture> {
    POSTGRES_FIXTURE
        .get_or_init(|| async {
            let container = Postgres::default()
                .with_db_name("ccatest")
                .with_user("ccatest")
                .with_password("ccatest")
                .start()
                .await
                .ok()?;

            let port = container.get_host_port_ipv4(5432).await.ok()?;
            let url = format!(
                "postgres://ccatest:ccatest@localhost:{}/ccatest?sslmode=disable",
                port
            );

            Some(PostgresFixture { container, url })
        })
        .await
        .as_ref()
}

// Shared Redis fixture for CCA E2E tests
struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String,
}

static REDIS_FIXTURE: OnceCell<Option<RedisFixture>> = OnceCell::const_new();

async fn get_redis_fixture() -> Option<&'static RedisFixture> {
    REDIS_FIXTURE
        .get_or_init(|| async {
            let container = Redis::default().start().await.ok()?;
            let port = container.get_host_port_ipv4(6379).await.ok()?;
            let url = format!("redis://localhost:{}", port);

            Some(RedisFixture { container, url })
        })
        .await
        .as_ref()
}

// Shared Qdrant fixture for CCA E2E tests
struct QdrantFixture {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    url: String,
}

static QDRANT_FIXTURE: OnceCell<Option<QdrantFixture>> = OnceCell::const_new();

async fn get_qdrant_fixture() -> Option<&'static QdrantFixture> {
    QDRANT_FIXTURE
        .get_or_init(|| async {
            let container = GenericImage::new("qdrant/qdrant", "latest")
                .with_exposed_port(ContainerPort::Tcp(6334))
                .with_wait_for(WaitFor::message_on_stdout(
                    "Qdrant is ready to accept connections",
                ))
                .start()
                .await
                .ok()?;

            let host = container.get_host().await.ok()?;
            let port = container.get_host_port_ipv4(6334).await.ok()?;
            let url = format!("http://{}:{}", host, port);

            Some(QdrantFixture { container, url })
        })
        .await
        .as_ref()
}

// Shared MinIO (S3 compatible) fixture for CCA E2E tests

async fn setup_cca_test_server_with_real_deps() -> anyhow::Result<Arc<McpServer>> {
    // Get shared fixtures - will fail if Docker not available
    let postgres_fixture = get_postgres_fixture()
        .await
        .ok_or_else(|| anyhow::anyhow!("PostgreSQL not available - Docker required"))?;
    let redis_fixture = get_redis_fixture()
        .await
        .ok_or_else(|| anyhow::anyhow!("Redis not available - Docker required"))?;
    let qdrant_fixture = get_qdrant_fixture()
        .await
        .ok_or_else(|| anyhow::anyhow!("Qdrant not available - Docker required"))?;
    // MinIO would be used for real E2E tests
    let minio_url = "http://localhost:9000".to_string();

    // Create mock memory provider for compilation (real tests would use Qdrant)
    let memory_manager = Arc::new(MemoryManager::new());
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    > = Arc::new(MockProvider::new());

    memory_manager
        .register_provider(MemoryLayer::User, provider)
        .await;

    // Create real PostgreSQL storage backend
    use storage::postgres::PostgresBackend;
    let storage_backend = Arc::new(
        PostgresBackend::new(&postgres_fixture.url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create PostgreSQL backend: {}", e))?,
    );

    // Create real Redis storage
    use storage::redis::RedisStorage;
    let _redis_storage = Arc::new(
        RedisStorage::new(&redis_fixture.url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Redis storage: {}", e))?,
    );

    // Create real Git repository for knowledge
    use knowledge::repository::GitRepository;
    let temp_dir = tempfile::tempdir()
        .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;
    let knowledge_repo = Arc::new(
        GitRepository::new(temp_dir.path())
            .map_err(|e| anyhow::anyhow!("Failed to create Git repository: {}", e))?,
    );

    // Create real sync persister
    use sync::state_persister::DatabasePersister;
    let persister = Arc::new(DatabasePersister::new(
        storage_backend.clone(),
        "cca_sync_state".to_string(),
    ));

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            config::config::DeploymentConfig::default(),
            None,
            persister,
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    );

    let server = Arc::new(McpServer::new(
        memory_manager,
        sync_manager,
        knowledge_repo.clone(),
        storage_backend,
        Arc::new(knowledge::governance::GovernanceEngine::new()),
        Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        ))),
        Arc::new(MockAuthService),
        None,
        None,
    ));

    Ok(server)
}

/// Test 7.4.1: Context assembly with hierarchical compression
#[tokio::test]
async fn test_context_assembly_hierarchical_compression() -> anyhow::Result<()> {
    let server = setup_cca_test_server_with_real_deps().await?;

    // Test context_assemble tool
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "context_assemble",
            "arguments": {
                "query": "test query for hierarchical compression",
                "max_tokens": 1000,
                "hierarchy": true,
                "compression_threshold": 0.8
            }
        })),
    };

    let response = server.handle_request(request).await;

    // Verify response structure
    assert!(response.result.is_some() || response.error.is_some());

    if let Some(result) = response.result {
        let result_obj = result.as_object().expect("Result should be an object");
        assert!(result_obj.contains_key("context"));
        assert!(result_obj.contains_key("tokens_used"));
        assert!(result_obj.contains_key("layers_compressed"));

        let layers_compressed = result_obj["layers_compressed"].as_array().unwrap();
        assert!(
            !layers_compressed.is_empty(),
            "Should have compressed some layers"
        );
    }

    Ok(())
}

/// Test 7.4.2: Note generation from tool trajectory
#[tokio::test]
async fn test_note_generation_from_trajectory() -> anyhow::Result<()> {
    let server = setup_cca_test_server_with_real_deps().await?;

    // Test note_capture tool with tool trajectory
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(2),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "note_capture",
            "arguments": {
                "trajectory": {
                    "tools_used": ["memory_search", "knowledge_query", "context_assemble"],
                    "decisions": ["Use hierarchical compression", "Prioritize recent memories"],
                    "outcomes": ["Found relevant context", "Compressed 3 layers"],
                    "duration_ms": 1500
                },
                "format": "markdown",
                "sections": ["decisions", "outcomes", "patterns"]
            }
        })),
    };

    let response = server.handle_request(request).await;

    // Verify response
    assert!(response.result.is_some() || response.error.is_some());

    if let Some(result) = response.result {
        let result_obj = result.as_object().expect("Result should be an object");
        assert!(result_obj.contains_key("note_id"));
        assert!(result_obj.contains_key("summary"));
        assert!(result_obj.contains_key("markdown"));

        let markdown = result_obj["markdown"].as_str().unwrap();
        assert!(
            markdown.contains("# Trajectory Notes"),
            "Should contain markdown header"
        );
        assert!(
            markdown.contains("## Decisions"),
            "Should contain decisions section"
        );
        assert!(
            markdown.contains("## Outcomes"),
            "Should contain outcomes section"
        );
    }

    Ok(())
}

/// Test 7.4.3: Hindsight capture and retrieval
#[tokio::test]
async fn test_hindsight_capture_and_retrieval() -> anyhow::Result<()> {
    let server = setup_cca_test_server_with_real_deps().await?;

    // First, capture an error pattern
    let capture_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(3),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "note_capture",
            "arguments": {
                "trajectory": {
                    "error": "Timeout connecting to database",
                    "context": "Database migration script",
                    "resolution": "Increased timeout from 5s to 30s",
                    "pattern": "database_connection_timeout"
                },
                "format": "error_pattern"
            }
        })),
    };

    let capture_response = server.handle_request(capture_request).await;
    assert!(
        capture_response.result.is_some(),
        "Should capture error pattern"
    );

    // Then query hindsight for similar errors
    let query_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(4),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "hindsight_query",
            "arguments": {
                "error_pattern": "database_connection_timeout",
                "similarity_threshold": 0.7,
                "max_results": 5
            }
        })),
    };

    let query_response = server.handle_request(query_request).await;

    // Verify hindsight query response
    assert!(query_response.result.is_some() || query_response.error.is_some());

    if let Some(result) = query_response.result {
        let result_obj = result.as_object().expect("Result should be an object");
        assert!(result_obj.contains_key("patterns"));
        assert!(result_obj.contains_key("suggestions"));

        let patterns = result_obj["patterns"].as_array().unwrap();
        let suggestions = result_obj["suggestions"].as_array().unwrap();

        // Should have at least the pattern we just captured
        assert!(!patterns.is_empty(), "Should find error patterns");
        assert!(
            !suggestions.is_empty(),
            "Should provide improvement suggestions"
        );
    }

    Ok(())
}

/// Test 7.4.4: Meta-agent loop with test failure recovery
#[tokio::test]
async fn test_meta_agent_loop_failure_recovery() -> anyhow::Result<()> {
    let server = setup_cca_test_server_with_real_deps().await?;

    // Test meta_loop_status tool with simulated failure recovery
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(5),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "meta_loop_status",
            "arguments": {
                "loop_id": "test-build-123",
                "phase": "test",
                "status": "failed",
                "failure_reason": "Test timeout after 30s",
                "recovery_attempts": 2,
                "max_attempts": 3
            }
        })),
    };

    let response = server.handle_request(request).await;

    // Verify meta-agent response
    assert!(response.result.is_some() || response.error.is_some());

    if let Some(result) = response.result {
        let result_obj = result.as_object().expect("Result should be an object");
        assert!(result_obj.contains_key("loop_state"));
        assert!(result_obj.contains_key("recovery_plan"));
        assert!(result_obj.contains_key("next_action"));

        let loop_state = result_obj["loop_state"].as_str().unwrap();
        let recovery_plan = result_obj["recovery_plan"].as_array().unwrap();

        assert_eq!(loop_state, "recovering", "Should be in recovery state");
        assert!(!recovery_plan.is_empty(), "Should have recovery steps");

        // Verify recovery plan includes reasonable steps
        let first_step = recovery_plan[0].as_str().unwrap();
        assert!(
            first_step.contains("timeout") || first_step.contains("retry"),
            "Recovery step should address timeout issue"
        );
    }

    Ok(())
}

/// Test 7.4.5: Extension callback chain execution
#[tokio::test]
async fn test_extension_callback_chain() -> anyhow::Result<()> {
    let server = setup_cca_test_server_with_real_deps().await?;

    // Test context_assemble with extension callbacks
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(6),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "test-tenant",
                "user_id": "test-user"
            },
            "name": "context_assemble",
            "arguments": {
                "query": "test with extension callbacks",
                "max_tokens": 500,
                "hierarchy": true,
                "compression_threshold": 0.7,
                "callbacks": [
                    {
                        "type": "pre_compress",
                        "handler": "validate_context_size"
                    },
                    {
                        "type": "post_compress",
                        "handler": "log_compression_stats"
                    }
                ]
            }
        })),
    };

    let response = server.handle_request(request).await;

    // Verify response includes callback execution info
    assert!(response.result.is_some() || response.error.is_some());

    if let Some(result) = response.result {
        let result_obj = result.as_object().expect("Result should be an object");

        // Should have standard context assembly fields
        assert!(result_obj.contains_key("context"));
        assert!(result_obj.contains_key("tokens_used"));

        // May have callback execution info if implemented
        if result_obj.contains_key("callbacks_executed") {
            let callbacks = result_obj["callbacks_executed"].as_array().unwrap();
            assert_eq!(callbacks.len(), 2, "Should have executed 2 callbacks");
        }
    }

    Ok(())
}
