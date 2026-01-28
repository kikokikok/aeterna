use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use mk_core::traits::{KnowledgeRepository, MemoryProviderAdapter};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, TenantId};
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
    async fn load(
        &self,
        _tenant_id: &TenantId
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }
    async fn save(
        &self,
        _tenant_id: &TenantId,
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
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync
    > = Arc::new(MockProvider::new());
    memory_manager
        .register_provider(mk_core::types::MemoryLayer::Project, provider)
        .await;

    let persister = Arc::new(MockPersister);

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager,
            Arc::new(knowledge::manager::KnowledgeManager::new(
                knowledge_repo.clone(),
                Arc::new(knowledge::governance::GovernanceEngine::new())
            )),
            config::config::DeploymentConfig::default(),
            None,
            persister,
            None
        )
        .await?
    );

    let sync_now_tool = SyncNowTool::new(sync_manager.clone());
    let sync_status_tool = SyncStatusTool::new(sync_manager.clone());

    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);

    // WHEN initial sync status is requested
    let status_resp = sync_status_tool
        .call(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            }
        }))
        .await?;

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
        summaries: HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    knowledge_repo.store(ctx, entry, "commit").await?;

    let sync_resp = sync_now_tool
        .call(json!({
            "force": false,
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            }
        }))
        .await?;
    assert!(sync_resp["success"].as_bool().unwrap());

    // THEN status should reflect the sync
    let status_resp = sync_status_tool
        .call(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            }
        }))
        .await?;
    assert_eq!(status_resp["stats"]["totalSyncs"], 1);
    assert_eq!(status_resp["stats"]["totalItemsSynced"], 1);

    Ok(())
}
