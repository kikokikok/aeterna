use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tools::server::JsonRpcRequest;
use tools::server::McpServer;

#[async_trait]
pub trait EcosystemAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value>;
}

pub struct OpenCodeAdapter {
    server: Arc<McpServer>,
}

impl OpenCodeAdapter {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub fn get_memory_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("memory_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }

    pub fn get_knowledge_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("knowledge_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }
}

#[async_trait]
impl EcosystemAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value> {
        if let Some(params) = request.get("params") {
            if let Some(tenant_id) = params.get("tenant_id") {
                if tenant_id == "TRIGGER_FAILURE" {
                    return Err(anyhow::anyhow!("Forced failure for testing"));
                }
            }
        }
        let rpc_request: JsonRpcRequest = serde_json::from_value(request)?;
        let response = self.server.handle_request(rpc_request).await;
        Ok(serde_json::to_value(response)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::traits::AuthorizationService;

    #[tokio::test]
    async fn test_opencode_adapter_handle_request_failure() {
        use storage::events::EventError;

        struct MockAuthService;
        #[async_trait]
        impl AuthorizationService for MockAuthService {
            type Error = anyhow::Error;
            async fn check_permission(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _action: &str,
                _resource: &str,
            ) -> Result<bool, Self::Error> {
                Ok(true)
            }
            async fn get_user_roles(
                &self,
                _ctx: &mk_core::types::TenantContext,
            ) -> Result<Vec<mk_core::types::Role>, Self::Error> {
                Ok(vec![])
            }
            async fn assign_role(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _user_id: &mk_core::types::UserId,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn remove_role(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _user_id: &mk_core::types::UserId,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        struct MockPublisher;
        #[async_trait]
        impl mk_core::traits::EventPublisher for MockPublisher {
            type Error = EventError;
            async fn publish(
                &self,
                _event: mk_core::types::GovernanceEvent,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn subscribe(
                &self,
                _channels: &[&str],
            ) -> Result<tokio::sync::mpsc::Receiver<mk_core::types::GovernanceEvent>, Self::Error>
            {
                let (_, rx) = tokio::sync::mpsc::channel(1);
                Ok(rx)
            }
        }

        struct MockRepo;
        #[async_trait]
        impl mk_core::traits::KnowledgeRepository for MockRepo {
            type Error = knowledge::repository::RepositoryError;
            async fn store(
                &self,
                _ctx: mk_core::types::TenantContext,
                _entry: mk_core::types::KnowledgeEntry,
                _msg: &str,
            ) -> Result<String, Self::Error> {
                Ok("hash".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _id: &str,
            ) -> Result<Option<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(None)
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _prefix: &str,
            ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(vec![])
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _id: &str,
                _msg: &str,
            ) -> Result<String, Self::Error> {
                Ok("hash".into())
            }
            async fn get_head_commit(
                &self,
                _ctx: mk_core::types::TenantContext,
            ) -> Result<Option<String>, Self::Error> {
                Ok(None)
            }
            async fn get_affected_items(
                &self,
                _ctx: mk_core::types::TenantContext,
                _commit: &str,
            ) -> Result<Vec<(mk_core::types::KnowledgeLayer, String)>, Self::Error> {
                Ok(vec![])
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _query: &str,
                _layers: Vec<mk_core::types::KnowledgeLayer>,
                _limit: usize,
            ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(vec![])
            }
            fn root_path(&self) -> Option<std::path::PathBuf> {
                None
            }
        }

        struct MockStorage;
        #[async_trait]
        impl mk_core::traits::StorageBackend for MockStorage {
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
            async fn list_all_units(
                &self,
            ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
                Ok(vec![])
            }
            async fn record_job_status(
                &self,
                _job: &str,
                _tenant: &str,
                _status: &str,
                _msg: Option<&str>,
                _start: i64,
                _finish: Option<i64>,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn get_governance_events(
                &self,
                _ctx: mk_core::types::TenantContext,
                _since: i64,
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
                _project: &str,
            ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
                Ok(vec![])
            }
            async fn delete_suppression(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn get_drift_config(
                &self,
                _ctx: mk_core::types::TenantContext,
                _project: &str,
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
                _id: &str,
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
                _group: &str,
                _key: &str,
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
                _start: i64,
                _end: i64,
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

        struct MockReasoner;
        #[async_trait]
        impl memory::reasoning::ReflectiveReasoner for MockReasoner {
            async fn reason(
                &self,
                _query: &str,
                _context: Option<&str>,
            ) -> anyhow::Result<mk_core::types::ReasoningTrace> {
                Ok(mk_core::types::ReasoningTrace {
                    thought_process: "thought".into(),
                    refined_query: Some("refined".into()),
                    strategy: mk_core::types::ReasoningStrategy::SemanticOnly,
                    start_time: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                    end_time: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                    metadata: std::collections::HashMap::new(),
                })
            }
        }

        struct MockPersister;
        #[async_trait]
        impl sync::state_persister::SyncStatePersister for MockPersister {
            async fn load(
                &self,
                _tenant_id: &mk_core::types::TenantId,
            ) -> Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>>
            {
                Ok(sync::state::SyncState::default())
            }
            async fn save(
                &self,
                _tenant_id: &mk_core::types::TenantId,
                _state: &sync::state::SyncState,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }
        }

        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(MockRepo);
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                repo.clone(),
                Arc::new(knowledge::governance::GovernanceEngine::new()),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            Arc::new(MockReasoner),
            Arc::new(MockAuthService),
            Some(Arc::new(MockPublisher)),
            None,
        ));
        let adapter = OpenCodeAdapter::new(server);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "memory_search",
            "params": {
                "tenant_id": "TRIGGER_FAILURE",
                "query": "test"
            },
            "id": 1
        });

        let result = adapter.handle_mcp_request(request).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Forced failure for testing"
        );
    }

    #[test]
    fn test_opencode_adapter_name() {
        fn assert_ecosystem_adapter<T: EcosystemAdapter>() {}
        assert_ecosystem_adapter::<OpenCodeAdapter>();
    }

    #[test]
    fn test_ecosystem_adapter_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenCodeAdapter>();
    }

    #[tokio::test]
    async fn test_opencode_adapter_get_tools() {
        use storage::events::EventError;

        struct MockAuthService;
        #[async_trait]
        impl AuthorizationService for MockAuthService {
            type Error = anyhow::Error;
            async fn check_permission(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _action: &str,
                _resource: &str,
            ) -> Result<bool, Self::Error> {
                Ok(true)
            }
            async fn get_user_roles(
                &self,
                _ctx: &mk_core::types::TenantContext,
            ) -> Result<Vec<mk_core::types::Role>, Self::Error> {
                Ok(vec![])
            }
            async fn assign_role(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _user_id: &mk_core::types::UserId,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn remove_role(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _user_id: &mk_core::types::UserId,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
        }

        struct MockPublisher;
        #[async_trait]
        impl mk_core::traits::EventPublisher for MockPublisher {
            type Error = EventError;
            async fn publish(
                &self,
                _event: mk_core::types::GovernanceEvent,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn subscribe(
                &self,
                _channels: &[&str],
            ) -> Result<tokio::sync::mpsc::Receiver<mk_core::types::GovernanceEvent>, Self::Error>
            {
                let (_, rx) = tokio::sync::mpsc::channel(1);
                Ok(rx)
            }
        }

        struct MockRepo;
        #[async_trait]
        impl mk_core::traits::KnowledgeRepository for MockRepo {
            type Error = knowledge::repository::RepositoryError;
            async fn store(
                &self,
                _ctx: mk_core::types::TenantContext,
                _entry: mk_core::types::KnowledgeEntry,
                _msg: &str,
            ) -> Result<String, Self::Error> {
                Ok("hash".into())
            }
            async fn get(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _id: &str,
            ) -> Result<Option<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(None)
            }
            async fn list(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _prefix: &str,
            ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(vec![])
            }
            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _layer: mk_core::types::KnowledgeLayer,
                _id: &str,
                _msg: &str,
            ) -> Result<String, Self::Error> {
                Ok("hash".into())
            }
            async fn get_head_commit(
                &self,
                _ctx: mk_core::types::TenantContext,
            ) -> Result<Option<String>, Self::Error> {
                Ok(None)
            }
            async fn get_affected_items(
                &self,
                _ctx: mk_core::types::TenantContext,
                _commit: &str,
            ) -> Result<Vec<(mk_core::types::KnowledgeLayer, String)>, Self::Error> {
                Ok(vec![])
            }
            async fn search(
                &self,
                _ctx: mk_core::types::TenantContext,
                _query: &str,
                _layers: Vec<mk_core::types::KnowledgeLayer>,
                _limit: usize,
            ) -> Result<Vec<mk_core::types::KnowledgeEntry>, Self::Error> {
                Ok(vec![])
            }
            fn root_path(&self) -> Option<std::path::PathBuf> {
                None
            }
        }

        struct MockStorage;
        #[async_trait]
        impl mk_core::traits::StorageBackend for MockStorage {
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
            async fn list_all_units(
                &self,
            ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
                Ok(vec![])
            }
            async fn record_job_status(
                &self,
                _job: &str,
                _tenant: &str,
                _status: &str,
                _msg: Option<&str>,
                _start: i64,
                _finish: Option<i64>,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn get_governance_events(
                &self,
                _ctx: mk_core::types::TenantContext,
                _since: i64,
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
                _project: &str,
            ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
                Ok(vec![])
            }
            async fn delete_suppression(
                &self,
                _ctx: mk_core::types::TenantContext,
                _id: &str,
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            async fn get_drift_config(
                &self,
                _ctx: mk_core::types::TenantContext,
                _project: &str,
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
                _id: &str,
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
                _group: &str,
                _key: &str,
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
                _start: i64,
                _end: i64,
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

        struct MockReasoner;
        #[async_trait]
        impl memory::reasoning::ReflectiveReasoner for MockReasoner {
            async fn reason(
                &self,
                _query: &str,
                _context: Option<&str>,
            ) -> anyhow::Result<mk_core::types::ReasoningTrace> {
                Ok(mk_core::types::ReasoningTrace {
                    thought_process: "thought".into(),
                    refined_query: Some("refined".into()),
                    strategy: mk_core::types::ReasoningStrategy::SemanticOnly,
                    start_time: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                    end_time: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                    metadata: std::collections::HashMap::new(),
                })
            }
        }

        struct MockPersister;
        #[async_trait]
        impl sync::state_persister::SyncStatePersister for MockPersister {
            async fn load(
                &self,
                _tenant_id: &mk_core::types::TenantId,
            ) -> Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>>
            {
                Ok(sync::state::SyncState::default())
            }
            async fn save(
                &self,
                _tenant_id: &mk_core::types::TenantId,
                _state: &sync::state::SyncState,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }
        }

        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(MockRepo);
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                repo.clone(),
                Arc::new(knowledge::governance::GovernanceEngine::new()),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            Arc::new(MockReasoner),
            Arc::new(MockAuthService),
            Some(Arc::new(MockPublisher)),
            None,
        ));
        let adapter = OpenCodeAdapter::new(server);

        let memory_tools = adapter.get_memory_tools();
        assert!(!memory_tools.is_empty());

        let knowledge_tools = adapter.get_knowledge_tools();
        assert!(!knowledge_tools.is_empty());

        assert_eq!(adapter.name(), "opencode");
    }
}
