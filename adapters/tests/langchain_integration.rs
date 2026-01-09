use adapters::ecosystem::EcosystemAdapter;
use adapters::langchain::LangChainAdapter;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryLayer};
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
        _layer: KnowledgeLayer,
        _path: &str
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        Ok(None)
    }

    async fn store(&self, _entry: KnowledgeEntry, _message: &str) -> Result<String, Self::Error> {
        Ok("hash".to_string())
    }

    async fn list(
        &self,
        _layer: KnowledgeLayer,
        _prefix: &str
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }

    async fn delete(
        &self,
        _layer: KnowledgeLayer,
        _path: &str,
        _message: &str
    ) -> Result<String, Self::Error> {
        Ok("hash".to_string())
    }

    async fn get_head_commit(&self) -> Result<Option<String>, Self::Error> {
        Ok(Some("head".to_string()))
    }

    async fn get_affected_items(
        &self,
        _since_commit: &str
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        Ok(vec![])
    }
}

struct MockPersister;

#[async_trait::async_trait]
impl sync::state_persister::SyncStatePersister for MockPersister {
    async fn load(&self) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }
    async fn save(
        &self,
        _state: &SyncState
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn setup_server() -> Arc<McpServer> {
    let memory_manager = Arc::new(MemoryManager::new());
    memory_manager
        .register_provider(MemoryLayer::User, Box::new(MockProvider::new()))
        .await;

    let repo = Arc::new(MockRepo);
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            repo.clone(),
            Arc::new(MockPersister)
        )
        .await
        .unwrap()
    );

    Arc::new(McpServer::new(memory_manager, sync_manager, repo))
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
        "name": "memory_add",
        "arguments": {
            "content": "test content",
            "layer": "user",
            "identifiers": {
                "user_id": "test_user_123"
            }
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
