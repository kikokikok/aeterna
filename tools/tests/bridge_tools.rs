use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeType};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use tools::bridge::{SyncNowTool, SyncStatusTool};
use tools::tools::Tool;

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
async fn test_sync_tools() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a SyncManager and tools
    let repo_dir = tempfile::tempdir()?;
    let knowledge_repo = Arc::new(GitRepository::new(repo_dir.path())?);
    let memory_manager = Arc::new(MemoryManager::new());

    use memory::providers::MockProvider;
    memory_manager
        .register_provider(
            mk_core::types::MemoryLayer::Project,
            Box::new(MockProvider::new())
        )
        .await;

    let persister = Arc::new(MockPersister);

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager,
            knowledge_repo.clone(),
            Arc::new(knowledge::governance::GovernanceEngine::new()),
            None,
            persister
        )
        .await?
    );

    let sync_now_tool = SyncNowTool::new(sync_manager.clone());
    let sync_status_tool = SyncStatusTool::new(sync_manager.clone());

    // WHEN initial sync status is requested
    let status_resp = sync_status_tool.call(json!({})).await?;

    // THEN it should be healthy and have zero stats
    assert!(status_resp["success"].as_bool().unwrap());
    assert!(status_resp["healthy"].as_bool().unwrap());
    assert_eq!(status_resp["stats"]["totalSyncs"], 0);

    // WHEN adding knowledge and triggering sync_now
    let entry = KnowledgeEntry {
        path: "test.md".to_string(),
        content: "test content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    knowledge_repo.store(entry, "commit").await?;

    let sync_resp = sync_now_tool.call(json!({"force": false})).await?;
    assert!(sync_resp["success"].as_bool().unwrap());

    // THEN status should reflect the sync
    let status_resp = sync_status_tool.call(json!({})).await?;
    assert_eq!(status_resp["stats"]["totalSyncs"], 1);
    assert_eq!(status_resp["stats"]["totalItemsSynced"], 1);

    Ok(())
}
