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
    key: String
}

impl<S: StorageBackend> DatabasePersister<S> {
    pub fn new(storage: Arc<S>, key: String) -> Self {
        Self { storage, key }
    }
}

#[async_trait]
impl<S: StorageBackend> SyncStatePersister for DatabasePersister<S>
where
    S::Error: std::error::Error + Send + Sync + 'static
{
    async fn load(&self) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        match self.storage.retrieve(&self.key).await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(SyncState::default())
        }
    }

    async fn save(
        &self,
        state: &SyncState
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(state)?;
        self.storage.store(&self.key, &data).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
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

    #[tokio::test]
    async fn test_database_persister_new() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        assert_eq!(persister.key, "test_key");
    }

    #[tokio::test]
    async fn test_database_persister_load_default() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());

        let state = persister.load().await.unwrap();
        assert_eq!(state, SyncState::default());
    }

    #[tokio::test]
    async fn test_database_persister_save_and_load() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());

        let mut state = SyncState::default();
        state.stats.total_syncs = 5;
        state.stats.total_items_synced = 42;

        persister.save(&state).await.unwrap();

        let loaded_state = persister.load().await.unwrap();
        assert_eq!(loaded_state.stats.total_syncs, 5);
        assert_eq!(loaded_state.stats.total_items_synced, 42);
    }

    #[tokio::test]
    async fn test_database_persister_save_overwrites() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());

        let mut state1 = SyncState::default();
        state1.stats.total_syncs = 1;

        let mut state2 = SyncState::default();
        state2.stats.total_syncs = 2;

        persister.save(&state1).await.unwrap();
        persister.save(&state2).await.unwrap();

        let loaded_state = persister.load().await.unwrap();
        assert_eq!(loaded_state.stats.total_syncs, 2);
    }

    #[tokio::test]
    async fn test_database_persister_different_keys() {
        let storage = Arc::new(MockStorage::new());
        let persister1 = DatabasePersister::new(storage.clone(), "key1".to_string());
        let persister2 = DatabasePersister::new(storage, "key2".to_string());

        let mut state1 = SyncState::default();
        state1.stats.total_syncs = 100;

        let mut state2 = SyncState::default();
        state2.stats.total_syncs = 200;

        persister1.save(&state1).await.unwrap();
        persister2.save(&state2).await.unwrap();

        let loaded1 = persister1.load().await.unwrap();
        let loaded2 = persister2.load().await.unwrap();

        assert_eq!(loaded1.stats.total_syncs, 100);
        assert_eq!(loaded2.stats.total_syncs, 200);
    }

    #[tokio::test]
    async fn test_database_persister_storage_error() {
        struct ErrorStorage;

        #[async_trait]
        impl StorageBackend for ErrorStorage {
            type Error = std::io::Error;

            async fn store(&self, _key: &str, _value: &[u8]) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error"
                ))
            }

            async fn retrieve(&self, _key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error"
                ))
            }

            async fn delete(&self, _key: &str) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error"
                ))
            }

            async fn exists(&self, _key: &str) -> Result<bool, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error"
                ))
            }
        }

        let storage = Arc::new(ErrorStorage);
        let persister = DatabasePersister::new(storage, "test_key".to_string());

        let state = SyncState::default();
        let save_result = persister.save(&state).await;
        assert!(save_result.is_err());

        let load_result = persister.load().await;
        assert!(load_result.is_err());
    }
}
