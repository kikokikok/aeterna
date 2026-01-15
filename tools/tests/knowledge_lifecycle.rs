use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, MemoryProviderAdapter, StorageBackend};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, MemoryLayer};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state_persister::DatabasePersister;
use testcontainers::runners::AsyncRunner;
use tokio::sync::RwLock;
use tools::server::{JsonRpcRequest, McpServer};

struct MockStorage {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StorageBackend for MockStorage {
    type Error = std::io::Error;

    async fn store(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
        value: &[u8],
    ) -> Result<(), Self::Error> {
        self.data
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn retrieve(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn delete(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<(), Self::Error> {
        self.data.write().await.remove(key);
        Ok(())
    }

    async fn exists(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<bool, Self::Error> {
        Ok(self.data.read().await.contains_key(key))
    }

    async fn get_ancestors(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
    }

    async fn get_descendants(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
    }

    async fn get_unit_policies(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
        Ok(Vec::new())
    }

    async fn create_unit(
        &self,
        _unit: &mk_core::types::OrganizationalUnit,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn add_unit_policy(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _unit_id: &str,
        _policy: &mk_core::types::Policy,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn assign_role(
        &self,
        _user_id: &mk_core::types::UserId,
        _tenant_id: &mk_core::types::TenantId,
        _unit_id: &str,
        _role: mk_core::types::Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _user_id: &mk_core::types::UserId,
        _tenant_id: &mk_core::types::TenantId,
        _unit_id: &str,
        _role: mk_core::types::Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn store_drift_result(
        &self,
        _result: mk_core::types::DriftResult,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_latest_drift_result(
        &self,
        _ctx: mk_core::types::TenantContext,
        _project_id: &str,
    ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
        Ok(None)
    }

    async fn list_all_units(&self) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
    }

    async fn record_job_status(
        &self,
        _job_name: &str,
        _tenant_id: &str,
        _status: &str,
        _message: Option<&str>,
        _started_at: i64,
        _finished_at: Option<i64>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_governance_events(
        &self,
        _ctx: mk_core::types::TenantContext,
        _since_timestamp: i64,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
        Ok(Vec::new())
    }
}

struct MockAuthService;
#[async_trait]
impl mk_core::traits::AuthorizationService for MockAuthService {
    type Error = anyhow::Error;
    async fn check_permission(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _action: &str,
        _resource: &str,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
    async fn get_user_roles(
        &self,
        _ctx: &mk_core::types::TenantContext,
    ) -> anyhow::Result<Vec<mk_core::types::Role>> {
        Ok(vec![])
    }
    async fn assign_role(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _user_id: &mk_core::types::UserId,
        _role: mk_core::types::Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn remove_role(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _user_id: &mk_core::types::UserId,
        _role: mk_core::types::Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

async fn setup_postgres_container() -> Result<
    (
        testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
        String,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let container = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("testdb")
        .with_user("testuser")
        .with_password("testpass")
        .start()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let connection_url = format!(
        "postgres://testuser:testpass@localhost:{}/testdb?sslmode=disable",
        container
            .get_host_port_ipv4(5432)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
    );

    Ok((container, connection_url))
}

#[tokio::test]
async fn test_knowledge_lifecycle_integration() -> anyhow::Result<()> {
    let (_container, connection_url) = setup_postgres_container()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().join("repo");
    let repo = Arc::new(GitRepository::new(&repo_path)?);

    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = MockProvider::new();
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    > = Arc::new(mock_provider);
    memory_manager
        .register_provider(MemoryLayer::Project, provider)
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(DatabasePersister::new(storage, "sync_key".to_string()));

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            config::config::DeploymentConfig::default(),
            None,
            persister,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?,
    );

    let server = McpServer::new(
        memory_manager,
        sync_manager,
        repo.clone(),
        Arc::new(
            storage::postgres::PostgresBackend::new(&connection_url)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ),
        Arc::new(knowledge::governance::GovernanceEngine::new()),
        Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        ))),
        Arc::new(MockAuthService),
        None,
        None,
    );

    // GIVEN a knowledge entry is stored in the repository
    let entry = KnowledgeEntry {
        path: "specs/auth.md".to_string(),
        content: "Auth spec content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        summaries: HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);

    repo.store(ctx.clone(), entry, "add auth spec").await?;

    // WHEN we query knowledge via MCP tool
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "name": "knowledge_query",
            "arguments": {
                "query": "Auth",
                "layers": ["project"]
            }
        })),
    };

    let response = server.handle_request(request).await;

    // THEN the query should return the entry
    assert!(
        response.error.is_none(),
        "Response should not have error: {:?}",
        response.error
    );
    let result = response.result.unwrap();
    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["results"]["keyword"].as_array().unwrap().len(), 1);

    // WHEN we fetch the specific entry via MCP tool
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(2),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "name": "knowledge_get",
            "arguments": {
                "path": "specs/auth.md",
                "layer": "project"
            }
        })),
    };

    let response = server.handle_request(request).await;

    // THEN the entry content should match
    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["entry"]["content"], "Auth spec content");

    Ok(())
}
