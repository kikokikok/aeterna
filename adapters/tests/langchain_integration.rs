use adapters::ecosystem::EcosystemAdapter;
use adapters::langchain::LangChainAdapter;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, MemoryProviderAdapter};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryLayer, TenantContext};
use serde_json::json;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use tools::server::McpServer;

struct MockRepo;

#[async_trait::async_trait]
impl KnowledgeRepository for MockRepo {
    type Error = knowledge::repository::RepositoryError;

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
        Ok("hash".to_string())
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
        Ok("hash".to_string())
    }

    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
        Ok(Some("head".to_string()))
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

struct MockPersister;

#[async_trait::async_trait]
impl sync::state_persister::SyncStatePersister for MockPersister {
    async fn load(
        &self,
        _tenant_id: &mk_core::types::TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }
    async fn save(
        &self,
        _tenant_id: &mk_core::types::TenantId,
        _state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn setup_server() -> Arc<McpServer> {
    let memory_manager = Arc::new(MemoryManager::new());
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    > = Arc::new(MockProvider::new());
    memory_manager
        .register_provider(MemoryLayer::User, provider)
        .await;

    let repo = Arc::new(MockRepo);
    let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
    let deployment_config = config::config::DeploymentConfig::default();
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            repo.clone(),
            governance.clone(),
            deployment_config,
            None,
            Arc::new(MockPersister),
            None,
        )
        .await
        .unwrap(),
    );

    let auth_service = Arc::new(MockAuthService);

    let mock_reasoner = Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
        memory::llm::mock::MockLlmService::new(),
    )));

    Arc::new(McpServer::new(
        memory_manager,
        sync_manager,
        repo,
        Arc::new(MockStorageBackend),
        governance,
        mock_reasoner,
        auth_service,
        None,
        None,
    ))
}

struct MockStorageBackend;
#[async_trait::async_trait]
impl mk_core::traits::StorageBackend for MockStorageBackend {
    type Error = storage::postgres::PostgresError;
    async fn store(
        &self,
        _ctx: mk_core::types::TenantContext,
        _key: &str,
        _value: &[u8],
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn retrieve(
        &self,
        _ctx: mk_core::types::TenantContext,
        _key: &str,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }
    async fn delete(
        &self,
        _ctx: mk_core::types::TenantContext,
        _key: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn exists(
        &self,
        _ctx: mk_core::types::TenantContext,
        _key: &str,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
    async fn get_ancestors(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(vec![])
    }
    async fn get_descendants(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(vec![])
    }
    async fn get_unit_policies(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
        Ok(vec![])
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
        Ok(vec![])
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
        Ok(vec![])
    }
    async fn create_suppression(
        &self,
        _suppression: mk_core::types::DriftSuppression,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn list_suppressions(
        &self,
        _ctx: mk_core::types::TenantContext,
        _project_id: &str,
    ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
        Ok(vec![])
    }
    async fn delete_suppression(
        &self,
        _ctx: mk_core::types::TenantContext,
        _suppression_id: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_drift_config(
        &self,
        _ctx: mk_core::types::TenantContext,
        _project_id: &str,
    ) -> Result<Option<mk_core::types::DriftConfig>, Self::Error> {
        Ok(None)
    }
    async fn save_drift_config(
        &self,
        _config: mk_core::types::DriftConfig,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn persist_event(
        &self,
        _event: mk_core::types::PersistentEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_pending_events(
        &self,
        _ctx: mk_core::types::TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(vec![])
    }
    async fn update_event_status(
        &self,
        _event_id: &str,
        _status: mk_core::types::EventStatus,
        _error: Option<String>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_dead_letter_events(
        &self,
        _ctx: mk_core::types::TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(vec![])
    }
    async fn check_idempotency(
        &self,
        _consumer_group: &str,
        _idempotency_key: &str,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
    async fn record_consumer_state(
        &self,
        _state: mk_core::types::ConsumerState,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_event_metrics(
        &self,
        _ctx: mk_core::types::TenantContext,
        _period_start: i64,
        _period_end: i64,
    ) -> Result<Vec<mk_core::types::EventDeliveryMetrics>, Self::Error> {
        Ok(vec![])
    }
    async fn record_event_metrics(
        &self,
        _metrics: mk_core::types::EventDeliveryMetrics,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct MockAuthService;
#[async_trait::async_trait]
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

#[tokio::test]
async fn test_langchain_adapter_tool_conversion() {
    // GIVEN
    let server = setup_server().await;
    let adapter = LangChainAdapter::new(server);

    // WHEN
    let tools = adapter.to_langchain_tools();

    // THEN
    assert!(!tools.is_empty());
    let memory_add = tools.iter().find(|t| t["name"] == "memory_add").unwrap();
    assert_eq!(
        memory_add["description"],
        "Store a piece of information in memory for future reference."
    );
    assert!(memory_add["parameters"].is_object());
    assert_eq!(memory_add["parameters"]["additionalProperties"], false);
    assert_eq!(
        memory_add["parameters"]["$schema"],
        "http://json-schema.org/draft-07/schema#"
    );
}

#[tokio::test]
async fn test_langchain_adapter_request_handling() {
    // GIVEN
    let server = setup_server().await;
    let lc_adapter = LangChainAdapter::new(server);

    let request = json!({
        "tenantContext": {
            "tenant_id": "test_tenant",
            "user_id": "test_user"
        },
        "name": "memory_add",
        "arguments": {
            "content": "test content",
            "layer": "user"
        }
    });

    // WHEN
    let response = lc_adapter.handle_mcp_request(request).await.unwrap();

    // THEN
    assert_eq!(response["jsonrpc"], "2.0");
    if let Some(err) = response.get("error") {
        panic!("Tool call failed: {}", err);
    }
    assert!(response["result"].is_object());
    assert!(response["result"]["success"].as_bool().unwrap());
}
