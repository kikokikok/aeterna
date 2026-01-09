use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, StorageBackend};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeType, MemoryLayer};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state_persister::DatabasePersister;
use tokio::sync::RwLock;
use tools::server::{JsonRpcRequest, McpServer};

struct MockStorage {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new()))
        }
    }
}

#[async_trait]
impl StorageBackend for MockStorage {
    type Error = std::io::Error;

    async fn store(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        self.data
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        self.data.write().await.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, Self::Error> {
        Ok(self.data.read().await.contains_key(key))
    }
}

#[tokio::test]
async fn test_knowledge_lifecycle_integration() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().join("repo");
    let repo = Arc::new(GitRepository::new(&repo_path)?);

    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = MockProvider::new();
    memory_manager
        .register_provider(MemoryLayer::Project, Box::new(mock_provider))
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(DatabasePersister::new(storage, "sync_key".to_string()));

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            None,
            persister
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?
    );

    let server = McpServer::new(memory_manager, sync_manager, repo.clone());

    // GIVEN a knowledge entry is stored in the repository
    let entry = KnowledgeEntry {
        path: "specs/auth.md".to_string(),
        content: "Auth spec content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    repo.store(entry, "add auth spec").await?;

    // WHEN we query knowledge via MCP tool
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "knowledge_query",
            "arguments": {
                "query": "Auth",
                "layers": ["project"]
            }
        }))
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
            "name": "knowledge_get",
            "arguments": {
                "path": "specs/auth.md",
                "layer": "project"
            }
        }))
    };

    let response = server.handle_request(request).await;

    // THEN the entry content should match
    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["entry"]["content"], "Auth spec content");

    Ok(())
}
