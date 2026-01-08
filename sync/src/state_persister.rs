use crate::state::SyncState;
use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use std::sync::Arc;

#[async_trait]
pub trait SyncStatePersister: Send + Sync {
    async fn load(&self) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>>;
    async fn save(&self, state: &SyncState)
    -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct DatabasePersister<S: StorageBackend> {
    storage: Arc<S>,
    key: String,
}

impl<S: StorageBackend> DatabasePersister<S> {
    pub fn new(storage: Arc<S>, key: String) -> Self {
        Self { storage, key }
    }
}

#[async_trait]
impl<S: StorageBackend> SyncStatePersister for DatabasePersister<S>
where
    S::Error: std::error::Error + Send + Sync + 'static,
{
    async fn load(&self) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        match self.storage.retrieve(&self.key).await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(SyncState::default()),
        }
    }

    async fn save(
        &self,
        state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(state)?;
        self.storage.store(&self.key, &data).await?;
        Ok(())
    }
}
