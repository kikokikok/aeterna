use crate::ecosystem::EcosystemAdapter;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use tools::server::McpServer;

pub struct LangChainAdapter {
    server: Arc<McpServer>
}

impl LangChainAdapter {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub fn to_langchain_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .map(|tool| {
                let mut schema = tool.input_schema.clone();
                if let Some(obj) = schema.as_object_mut() {
                    obj.insert(
                        "$schema".to_string(),
                        json!("http://json-schema.org/draft-07/schema#")
                    );
                    obj.insert("additionalProperties".to_string(), json!(false));
                }

                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": schema,
                })
            })
            .collect()
    }
}

#[async_trait]
impl EcosystemAdapter for LangChainAdapter {
    fn name(&self) -> &str {
        "langchain"
    }

    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value> {
        let name = request["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
        let arguments = request["arguments"].clone();
        let tenant_context = request["tenantContext"].clone();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
                "tenantContext": tenant_context
            }
        });

        let response = self
            .server
            .handle_request(serde_json::from_value(mcp_request)?)
            .await;
        Ok(serde_json::to_value(response)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory::manager::MemoryManager;
    use sync::bridge::SyncManager;

    async fn setup_server() -> McpServer {
        let memory_manager = Arc::new(MemoryManager::new());
        let repo = Arc::new(knowledge::repository::GitRepository::new_mock());
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            repo.clone(),
            governance.clone()
        ));

        let auth_service = Arc::new(MockAuthService);
        let deployment_config = config::config::DeploymentConfig::default();
        let sync_manager = Arc::new(
            SyncManager::new(
                memory_manager.clone(),
                knowledge_manager,
                deployment_config,
                None,
                Arc::new(MockPersister),
                None
            )
            .await
            .unwrap()
        );

        McpServer::new(
            memory_manager.clone(),
            sync_manager,
            repo,
            Arc::new(MockStorageBackend),
            governance,
            Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
                memory::llm::mock::MockLlmService::new()
            ))),
            auth_service,
            None,
            None
        )
    }

    struct MockStorageBackend;
    #[async_trait::async_trait]
    impl mk_core::traits::StorageBackend for MockStorageBackend {
        type Error = storage::postgres::PostgresError;
        async fn store(
            &self,
            _ctx: mk_core::types::TenantContext,
            _key: &str,
            _value: &[u8]
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn retrieve(
            &self,
            _ctx: mk_core::types::TenantContext,
            _key: &str
        ) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }
        async fn delete(
            &self,
            _ctx: mk_core::types::TenantContext,
            _key: &str
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn exists(
            &self,
            _ctx: mk_core::types::TenantContext,
            _key: &str
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }
        async fn get_ancestors(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }
        async fn get_descendants(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }
        async fn get_unit_policies(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str
        ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
            Ok(vec![])
        }
        async fn create_unit(
            &self,
            _unit: &mk_core::types::OrganizationalUnit
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn add_unit_policy(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _unit_id: &str,
            _policy: &mk_core::types::Policy
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn assign_role(
            &self,
            _user_id: &mk_core::types::UserId,
            _tenant_id: &mk_core::types::TenantId,
            _unit_id: &str,
            _role: mk_core::types::Role
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn remove_role(
            &self,
            _user_id: &mk_core::types::UserId,
            _tenant_id: &mk_core::types::TenantId,
            _unit_id: &str,
            _role: mk_core::types::Role
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn store_drift_result(
            &self,
            _result: mk_core::types::DriftResult
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_latest_drift_result(
            &self,
            _ctx: mk_core::types::TenantContext,
            _project_id: &str
        ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
            Ok(None)
        }
        async fn list_all_units(
            &self
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }
        async fn record_job_status(
            &self,
            _job_name: &str,
            _tenant_id: &str,
            _status: &str,
            _message: Option<&str>,
            _started_at: i64,
            _finished_at: Option<i64>
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_governance_events(
            &self,
            _ctx: mk_core::types::TenantContext,
            _since_timestamp: i64,
            _limit: usize
        ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
            Ok(vec![])
        }
        async fn create_suppression(
            &self,
            _suppression: mk_core::types::DriftSuppression
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn list_suppressions(
            &self,
            _ctx: mk_core::types::TenantContext,
            _project_id: &str
        ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
            Ok(vec![])
        }
        async fn delete_suppression(
            &self,
            _ctx: mk_core::types::TenantContext,
            _suppression_id: &str
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_drift_config(
            &self,
            _ctx: mk_core::types::TenantContext,
            _project_id: &str
        ) -> Result<Option<mk_core::types::DriftConfig>, Self::Error> {
            Ok(None)
        }
        async fn save_drift_config(
            &self,
            _config: mk_core::types::DriftConfig
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn persist_event(
            &self,
            _event: mk_core::types::PersistentEvent
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_pending_events(
            &self,
            _ctx: mk_core::types::TenantContext,
            _limit: usize
        ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
            Ok(vec![])
        }
        async fn update_event_status(
            &self,
            _event_id: &str,
            _status: mk_core::types::EventStatus,
            _error: Option<String>
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_dead_letter_events(
            &self,
            _ctx: mk_core::types::TenantContext,
            _limit: usize
        ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
            Ok(vec![])
        }
        async fn check_idempotency(
            &self,
            _consumer_group: &str,
            _idempotency_key: &str
        ) -> Result<bool, Self::Error> {
            Ok(false)
        }
        async fn record_consumer_state(
            &self,
            _state: mk_core::types::ConsumerState
        ) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn get_event_metrics(
            &self,
            _ctx: mk_core::types::TenantContext,
            _period_start: i64,
            _period_end: i64
        ) -> Result<Vec<mk_core::types::EventDeliveryMetrics>, Self::Error> {
            Ok(vec![])
        }
        async fn record_event_metrics(
            &self,
            _metrics: mk_core::types::EventDeliveryMetrics
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
            _resource: &str
        ) -> anyhow::Result<bool> {
            Ok(true)
        }
        async fn get_user_roles(
            &self,
            _ctx: &mk_core::types::TenantContext
        ) -> anyhow::Result<Vec<mk_core::types::Role>> {
            Ok(vec![])
        }
        async fn assign_role(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _user_id: &mk_core::types::UserId,
            _role: mk_core::types::Role
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn remove_role(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _user_id: &mk_core::types::UserId,
            _role: mk_core::types::Role
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct _MockRepo;
    #[async_trait::async_trait]
    impl mk_core::traits::KnowledgeRepository for _MockRepo {
        type Error = knowledge::repository::RepositoryError;
        async fn store(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: mk_core::types::KnowledgeEntry,
            _: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".into())
        }
        async fn get(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: mk_core::types::KnowledgeLayer,
            _: &str
        ) -> std::result::Result<Option<mk_core::types::KnowledgeEntry>, Self::Error> {
            Ok(None)
        }
        async fn list(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: mk_core::types::KnowledgeLayer,
            _: &str
        ) -> std::result::Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
            Ok(vec![])
        }
        async fn delete(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: mk_core::types::KnowledgeLayer,
            _: &str,
            _: &str
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".into())
        }
        async fn get_head_commit(
            &self,
            _ctx: mk_core::types::TenantContext
        ) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: &str
        ) -> std::result::Result<Vec<(mk_core::types::KnowledgeLayer, String)>, Self::Error>
        {
            Ok(vec![])
        }
        async fn search(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: &str,
            _: Vec<mk_core::types::KnowledgeLayer>,
            _: usize
        ) -> std::result::Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
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
            _tenant_id: &mk_core::types::TenantId
        ) -> std::result::Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>>
        {
            Ok(sync::state::SyncState::default())
        }
        async fn save(
            &self,
            _tenant_id: &mk_core::types::TenantId,
            _: &sync::state::SyncState
        ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_langchain_adapter_name() {
        let server = Arc::new(setup_server().await);
        let adapter = LangChainAdapter::new(server);
        assert_eq!(adapter.name(), "langchain");
    }

    #[tokio::test]
    async fn test_langchain_handle_request_missing_name() {
        let server = Arc::new(setup_server().await);
        let adapter = LangChainAdapter::new(server);
        let request = json!({"arguments": {}});
        let result = adapter.handle_mcp_request(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Missing tool name");
    }

    #[tokio::test]
    async fn test_to_langchain_tools_empty() {
        let server = Arc::new(setup_server().await);
        let adapter = LangChainAdapter::new(server);
        let tools = adapter.to_langchain_tools();
        assert!(!tools.is_empty());
    }
}
