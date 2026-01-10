use adapters::ecosystem::{EcosystemAdapter, OpenCodeAdapter};
use adapters::langchain::LangChainAdapter;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryLayer};
use serde_json::json;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use tools::server::{JsonRpcRequest, McpServer};

struct MockKnowledgeRepo;

#[async_trait]
impl KnowledgeRepository for MockKnowledgeRepo {
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

    async fn search(
        &self,
        _query: &str,
        _layers: Vec<KnowledgeLayer>,
        _limit: usize
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

struct MockPersister;

#[async_trait]
impl SyncStatePersister for MockPersister {
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

#[tokio::test]
async fn test_full_integration_mcp_to_adapters() -> anyhow::Result<()> {
    let memory_manager = Arc::new(MemoryManager::new());
    memory_manager
        .register_provider(MemoryLayer::User, Box::new(MockProvider::new()))
        .await;

    let knowledge_repo = Arc::new(MockKnowledgeRepo);
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            None,
            Arc::new(MockPersister)
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
    );

    let server = Arc::new(McpServer::new(
        memory_manager,
        sync_manager,
        knowledge_repo.clone()
    ));

    let opencode = OpenCodeAdapter::new(server.clone());
    let memory_tools = opencode.get_memory_tools();
    assert!(!memory_tools.is_empty());

    let langchain = LangChainAdapter::new(server.clone());
    let lc_tools = langchain.to_langchain_tools();
    assert_eq!(lc_tools.len(), 10);

    let response = langchain
        .handle_mcp_request(json!({
            "name": "memory_add",
            "arguments": {
                "content": "Integrated test",
                "layer": "user"
            }
        }))
        .await?;

    if let Some(error) = response["error"].as_object() {
        panic!("Tool call failed: {:?}", error);
    }
    assert!(!response["result"].is_null());

    Ok(())
}

#[tokio::test]
async fn test_server_timeout() -> anyhow::Result<()> {
    let memory_manager = Arc::new(MemoryManager::new());
    let knowledge_repo = Arc::new(MockKnowledgeRepo);
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            None,
            Arc::new(MockPersister)
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
    );

    let server = McpServer::new(memory_manager, sync_manager, knowledge_repo)
        .with_timeout(std::time::Duration::from_millis(1));

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/list".to_string(),
        params: None
    };

    let response = server.handle_request(request).await;

    if let Some(error) = response.error {
        assert_eq!(error.code, -32001);
    }

    Ok(())
}
