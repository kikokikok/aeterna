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
        let repo = Arc::new(MockRepo);
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let sync_manager = Arc::new(
            SyncManager::new(
                memory_manager.clone(),
                repo.clone(),
                governance,
                None,
                Arc::new(MockPersister)
            )
            .await
            .unwrap()
        );

        McpServer::new(memory_manager, sync_manager, repo)
    }

    struct MockRepo;
    #[async_trait::async_trait]
    impl mk_core::traits::KnowledgeRepository for MockRepo {
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
