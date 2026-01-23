use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tools::server::JsonRpcRequest;
use tools::server::McpServer;

use context::{CedarContextResolver, ContextResolver};
use mk_core::types::TenantContext;

#[async_trait]
pub trait EcosystemAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value>;
}

pub struct OpenCodeAdapter {
    server: Arc<McpServer>,
    context_resolver: Arc<ContextResolver>,
    cedar_resolver: Arc<CedarContextResolver>,
}

impl OpenCodeAdapter {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self {
            server,
            context_resolver: Arc::new(ContextResolver::new()),
            cedar_resolver: Arc::new(CedarContextResolver::new()),
        }
    }

    pub fn with_resolver(server: Arc<McpServer>, resolver: Arc<ContextResolver>) -> Self {
        Self {
            server,
            context_resolver: resolver,
            cedar_resolver: Arc::new(CedarContextResolver::new()),
        }
    }

    pub async fn resolve_implicit_context(&self) -> anyhow::Result<TenantContext> {
        let local_ctx = self
            .context_resolver
            .resolve()
            .map_err(|e| anyhow::anyhow!(e))?;
        let enriched = self
            .cedar_resolver
            .enrich(local_ctx)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(enriched.to_tenant_context())
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

    pub fn get_governance_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("governance_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }

    pub fn get_cca_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| {
                t.name.starts_with("context_")
                    || t.name.starts_with("note_")
                    || t.name.starts_with("hindsight_")
                    || t.name.starts_with("meta_")
            })
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }

    pub fn get_sync_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("sync_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }

    pub fn get_graph_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("graph_"))
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
        let mut request = request;

        // Zero-Config: Inject implicit context if missing
        if let Some(params) = request.get_mut("params") {
            if let Some(args) = params.get_mut("arguments") {
                if args.get("tenant_id").is_none() && args.get("tenantContext").is_none() {
                    if let Ok(ctx) = self.resolve_implicit_context().await {
                        if let Ok(ctx_val) = serde_json::to_value(ctx) {
                            args.as_object_mut()
                                .unwrap()
                                .insert("tenantContext".to_string(), ctx_val);
                        }
                    }
                }
            }
        }

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
        async fn check_idempotency(&self, _group: &str, _key: &str) -> Result<bool, Self::Error> {
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
                timed_out: false,
                duration_ms: 0,
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
        ) -> Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>> {
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

    #[tokio::test]
    async fn test_opencode_adapter_handle_request_failure() {
        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(knowledge::repository::GitRepository::new_mock());
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            repo.clone(),
            governance.clone(),
        ));
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                knowledge_manager,
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
                None,
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            governance,
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
    }

    #[tokio::test]
    async fn test_opencode_adapter_implicit_context_injection() {
        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(knowledge::repository::GitRepository::new_mock());
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            repo.clone(),
            governance.clone(),
        ));
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                knowledge_manager,
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
                None,
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            governance,
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
                "arguments": {
                    "query": "test"
                }
            },
            "id": 1
        });

        let result = adapter.handle_mcp_request(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_opencode_adapter_git_mock_resolution() {
        use std::process::Command;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "alice@acme.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/acme-corp/payments-service.git",
            ])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let resolver = Arc::new(ContextResolver::from_dir(repo_path));

        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(knowledge::repository::GitRepository::new_mock());
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            repo.clone(),
            governance.clone(),
        ));
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                knowledge_manager,
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
                None,
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            governance,
            Arc::new(MockReasoner),
            Arc::new(MockAuthService),
            None,
            None,
        ));

        let adapter = OpenCodeAdapter::with_resolver(server, resolver);
        let ctx = adapter.resolve_implicit_context().await.unwrap();

        assert_eq!(ctx.user_id.as_str(), "alice@acme.com");
        let resolved = adapter.context_resolver.resolve().unwrap();
        assert_eq!(
            resolved.project_id.unwrap().value,
            "acme-corp/payments-service"
        );
    }

    #[test]
    fn test_ecosystem_adapter_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OpenCodeAdapter>();
    }

    #[tokio::test]
    async fn test_opencode_adapter_get_tools() {
        let memory_manager = Arc::new(memory::manager::MemoryManager::new());
        let repo = Arc::new(knowledge::repository::GitRepository::new_mock());
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
            repo.clone(),
            governance.clone(),
        ));
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                knowledge_manager,
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
                None,
            )
            .await
            .unwrap(),
        );

        let server = Arc::new(McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(MockStorage),
            governance,
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
