use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, StorageBackend};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeType, MemoryLayer};
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use tokio::sync::RwLock;

struct MockStorage {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
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
    storage: Arc<MockStorage>,
}

#[async_trait]
impl SyncStatePersister for SimplePersister {
    async fn load(&self) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        match self.storage.retrieve("sync_state").await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(SyncState::default()),
        }
    }

    async fn save(
        &self,
        state: &SyncState,
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

    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = Box::new(MockProvider::new());
    memory_manager
        .register_provider(MemoryLayer::Project, mock_provider)
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(SimplePersister {
        storage: storage.clone(),
    });

    let sync_manager = SyncManager::new(
        memory_manager.clone(),
        knowledge_repo.clone(),
        persister.clone(),
    )
    .await?;

    // AND initial knowledge
    let entry = KnowledgeEntry {
        path: "test.md".to_string(),
        content: "initial content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    knowledge_repo.store(entry.clone(), "first commit").await?;

    // WHEN performing full sync
    sync_manager.sync_all().await?;

    // THEN sync state is persisted
    assert!(storage.exists("sync_state").await?);
    let state = persister.load().await?;
    assert_eq!(state.stats.total_items_synced, 1);
    assert!(state.last_knowledge_commit.is_some());

    // WHEN updating knowledge
    let updated_entry = KnowledgeEntry {
        content: "updated content".to_string(),
        ..entry.clone()
    };
    knowledge_repo.store(updated_entry, "second commit").await?;

    // AND performing incremental sync
    sync_manager.sync_incremental().await?;

    // THEN sync state is updated
    let state = persister.load().await?;
    assert_eq!(state.stats.total_items_synced, 2);
    assert_eq!(state.stats.total_syncs, 2);

    // WHEN triggering sync cycle (manual)
    sync_manager.run_sync_cycle(0).await?;

    let _ = sync_manager.detect_conflicts().await?;

    // WHEN corrupting memory (missing pointer)
    let memory_id = format!("ptr_{}", entry.path);
    memory_manager
        .delete_from_layer(MemoryLayer::Project, &memory_id)
        .await?;

    // THEN conflict is detected
    let conflicts = sync_manager.detect_conflicts().await?;
    assert_eq!(conflicts.len(), 1);

    // WHEN resolving conflicts
    sync_manager.resolve_conflicts(conflicts).await?;

    // THEN conflict is resolved
    let conflicts = sync_manager.detect_conflicts().await?;
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
    let memory_manager = Arc::new(MemoryManager::new());
    let mock_provider = Box::new(MockProvider::new());
    memory_manager
        .register_provider(MemoryLayer::Project, mock_provider)
        .await;

    let storage = Arc::new(MockStorage::new());
    let persister = Arc::new(SimplePersister {
        storage: storage.clone(),
    });

    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_repo.clone(),
            persister.clone(),
        )
        .await?,
    );

    // AND initial knowledge
    let entry = KnowledgeEntry {
        path: "bg_test.md".to_string(),
        content: "initial content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    knowledge_repo.store(entry.clone(), "first commit").await?;

    // WHEN starting background sync with short interval
    let handle = sync_manager.clone().start_background_sync(1, 0).await;

    // THEN after some time, the item should be synced
    let mut synced = false;
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let state = persister.load().await?;
        if state.stats.total_items_synced > 0 {
            synced = true;
            break;
        }
    }

    handle.abort();
    assert!(synced, "Background sync should have picked up the change");

    Ok(())
}
