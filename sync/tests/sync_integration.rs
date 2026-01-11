use async_trait::async_trait;
use knowledge::governance::GovernanceEngine;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, StorageBackend};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, MemoryLayer, TenantContext,
    TenantId
};
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use tokio::sync::RwLock;

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

pub struct SimplePersister {
    storage: Arc<MockStorage>
}

#[async_trait]
impl SyncStatePersister for SimplePersister {
    async fn load(
        &self,
        _tenant_id: &TenantId
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        match self.storage.retrieve("sync_state").await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(SyncState::default())
        }
    }

    async fn save(
        &self,
        _tenant_id: &TenantId,
        state: &SyncState
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(state)?;
        self.storage.store("sync_state", &data).await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_sync_persistence_and_delta() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a SyncManager with mock storage and repositories
    let repo_dir = tempfile::tempdir()?;
    let knowledge_repo = Arc::new(GitRepository::new(repo_dir.path())?);
    let governance_engine = Arc::new(GovernanceEngine::new());

    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = MockProvider::new();
    memory_manager
        .register_provider(MemoryLayer::Project, Box::new(mock_provider))
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(SimplePersister {
        storage: storage.clone()
    });

    let sync_manager = SyncManager::new(
        memory_manager.clone(),
        knowledge_repo.clone(),
        governance_engine.clone(),
        None,
        persister.clone()
    )
    .await?;

    // AND initial knowledge
    let entry = KnowledgeEntry {
        path: "test.md".to_string(),
        content: "initial content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        status: KnowledgeStatus::Accepted,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    knowledge_repo
        .store(TenantContext::default(), entry.clone(), "first commit")
        .await?;

    // WHEN performing full sync
    sync_manager.sync_all(TenantContext::default()).await?;

    // THEN sync state is persisted
    assert!(storage.exists("sync_state").await?);
    let tenant_id = TenantId::default();
    let state = persister.load(&tenant_id).await?;
    assert_eq!(state.stats.total_items_synced, 1);
    assert!(state.last_knowledge_commit.is_some());

    // WHEN updating knowledge
    let updated_entry = KnowledgeEntry {
        content: "updated content".to_string(),
        ..entry.clone()
    };
    knowledge_repo
        .store(TenantContext::default(), updated_entry, "second commit")
        .await?;

    // AND performing incremental sync
    sync_manager
        .sync_incremental(TenantContext::default())
        .await?;

    // THEN sync state is updated
    let state = persister.load(&tenant_id).await?;
    assert_eq!(state.stats.total_items_synced, 2);
    assert_eq!(state.stats.total_syncs, 2);

    // WHEN triggering sync cycle (manual)
    sync_manager
        .run_sync_cycle(TenantContext::default(), 0)
        .await?;

    let _ = sync_manager
        .detect_conflicts(TenantContext::default())
        .await?;

    // WHEN corrupting memory (missing pointer)
    let memory_id = format!("ptr_{}", entry.path);
    memory_manager
        .delete_from_layer(TenantContext::default(), MemoryLayer::Project, &memory_id)
        .await?;

    // THEN conflict is detected
    let conflicts = sync_manager
        .detect_conflicts(TenantContext::default())
        .await?;
    assert_eq!(conflicts.len(), 1);

    // WHEN resolving conflicts
    sync_manager
        .resolve_conflicts(TenantContext::default(), conflicts)
        .await?;

    // THEN conflict is resolved
    let conflicts = sync_manager
        .detect_conflicts(TenantContext::default())
        .await?;
    if !conflicts.is_empty() {
        println!("Final conflicts: {:?}", conflicts);
    }
    assert!(conflicts.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_background_sync_trigger() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a SyncManager with mock storage
    let repo_dir = tempfile::tempdir()?;
    let knowledge_repo = Arc::new(GitRepository::new(repo_dir.path())?);
    let governance_engine = Arc::new(GovernanceEngine::new());
    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = MockProvider::new();
    memory_manager
        .register_provider(MemoryLayer::Project, Box::new(mock_provider))
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(SimplePersister {
        storage: storage.clone()
    });

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_repo.clone(),
            governance_engine.clone(),
            None,
            persister.clone()
        )
        .await?
    );

    // AND initial knowledge
    let entry = KnowledgeEntry {
        path: "bg_test.md".to_string(),
        content: "initial content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        status: KnowledgeStatus::Accepted,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    knowledge_repo
        .store(TenantContext::default(), entry.clone(), "first commit")
        .await?;

    // WHEN starting background sync with short interval
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let handle = sync_manager
        .clone()
        .start_background_sync(TenantContext::default(), 1, 0, rx)
        .await;

    // THEN after some time, the item should be synced
    let mut synced = false;
    let tenant_id = TenantId::default();
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let state = persister.load(&tenant_id).await?;
        if state.stats.total_items_synced > 0 {
            synced = true;
            break;
        }
    }

    handle.abort();
    assert!(synced, "Background sync should have picked up the change");

    Ok(())
}

#[tokio::test]
async fn test_governance_blocking_sync() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a SyncManager with a blocking policy
    let repo_dir = tempfile::tempdir()?;
    let knowledge_repo = Arc::new(GitRepository::new(repo_dir.path())?);
    let mut governance_engine = GovernanceEngine::new();

    governance_engine.add_policy(mk_core::types::Policy {
        id: "p1".to_string(),
        name: "No Secrets".to_string(),
        description: None,
        layer: KnowledgeLayer::Company,
        rules: vec![mk_core::types::PolicyRule {
            id: "r1".to_string(),
            target: mk_core::types::ConstraintTarget::Code,
            operator: mk_core::types::ConstraintOperator::MustNotMatch,
            value: serde_json::json!("SECRET"),
            severity: mk_core::types::ConstraintSeverity::Block,
            message: "No secrets allowed".to_string()
        }],
        metadata: HashMap::new()
    });

    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = MockProvider::new();
    memory_manager
        .register_provider(MemoryLayer::Project, Box::new(mock_provider))
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(SimplePersister {
        storage: storage.clone()
    });

    let sync_manager = SyncManager::new(
        memory_manager.clone(),
        knowledge_repo.clone(),
        Arc::new(governance_engine),
        None,
        persister.clone()
    )
    .await?;

    // AND knowledge containing a secret
    let entry = KnowledgeEntry {
        path: "secret.md".to_string(),
        content: "My SECRET is 12345".to_string(),
        layer: KnowledgeLayer::Company,
        kind: KnowledgeType::Spec,
        status: KnowledgeStatus::Draft,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp()
    };
    knowledge_repo
        .store(
            TenantContext::default(),
            entry.clone(),
            "commit with secret"
        )
        .await?;

    // WHEN syncing
    let mut state = SyncState::default();
    let result = sync_manager
        .sync_entry(TenantContext::default(), &entry, &mut state)
        .await;

    // THEN sync fails for that item
    assert!(result.is_err());
    assert!(
        state
            .failed_items
            .iter()
            .any(|f| f.error.contains("Governance violation (BLOCK)"))
    );

    Ok(())
}
