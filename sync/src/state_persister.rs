use crate::state::SyncState;
use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use std::path::PathBuf;
use std::sync::Arc;

#[async_trait]
pub trait SyncStatePersister: Send + Sync {
    async fn load(
        &self,
        tenant_id: &mk_core::types::TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>>;
    async fn save(
        &self,
        tenant_id: &mk_core::types::TenantId,
        state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct FilePersister {
    base_path: PathBuf,
}

impl FilePersister {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn get_path(&self, tenant_id: &mk_core::types::TenantId) -> PathBuf {
        self.base_path
            .join(format!("sync_state_{}.json", tenant_id.as_str()))
    }
}

#[async_trait]
impl SyncStatePersister for FilePersister {
    async fn load(
        &self,
        tenant_id: &mk_core::types::TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        let path = self.get_path(tenant_id);
        match tokio::fs::read(&path).await {
            Ok(data) => Ok(serde_json::from_slice(&data)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SyncState::default()),
            Err(e) => Err(e.into()),
        }
    }

    async fn save(
        &self,
        tenant_id: &mk_core::types::TenantId,
        state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = self.get_path(tenant_id);
        let data = serde_json::to_vec_pretty(state)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, data).await?;
        Ok(())
    }
}

pub struct DatabasePersister<S: StorageBackend> {
    storage: Arc<S>,
    key_prefix: String,
}

impl<S: StorageBackend> DatabasePersister<S> {
    pub fn new(storage: Arc<S>, key_prefix: String) -> Self {
        Self {
            storage,
            key_prefix,
        }
    }

    fn get_key(&self, tenant_id: &mk_core::types::TenantId) -> String {
        format!("{}:{}", self.key_prefix, tenant_id.as_str())
    }
}

#[async_trait]
impl<S: StorageBackend> SyncStatePersister for DatabasePersister<S>
where
    S::Error: std::error::Error + Send + Sync + 'static,
{
    async fn load(
        &self,
        tenant_id: &mk_core::types::TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        let key = self.get_key(tenant_id);
        let ctx = mk_core::types::TenantContext::new(
            tenant_id.clone(),
            mk_core::types::UserId::default(),
        );
        match self.storage.retrieve(ctx, &key).await? {
            Some(data) => Ok(serde_json::from_slice(&data)?),
            None => Ok(SyncState::default()),
        }
    }

    async fn save(
        &self,
        tenant_id: &mk_core::types::TenantId,
        state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = self.get_key(tenant_id);
        let data = serde_json::to_vec(state)?;
        let ctx = mk_core::types::TenantContext::new(
            tenant_id.clone(),
            mk_core::types::UserId::default(),
        );
        self.storage.store(ctx, &key, &data).await?;
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

        async fn store(
            &self,
            _ctx: mk_core::types::TenantContext,
            key: &str,
            value: &[u8],
        ) -> Result<(), Self::Error> {
            self.data
                .write()
                .await
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }

        async fn retrieve(
            &self,
            _ctx: mk_core::types::TenantContext,
            key: &str,
        ) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(self.data.read().await.get(key).cloned())
        }

        async fn delete(
            &self,
            _ctx: mk_core::types::TenantContext,
            key: &str,
        ) -> Result<(), Self::Error> {
            self.data.write().await.remove(key);
            Ok(())
        }

        async fn exists(
            &self,
            _ctx: mk_core::types::TenantContext,
            key: &str,
        ) -> Result<bool, Self::Error> {
            Ok(self.data.read().await.contains_key(key))
        }

        async fn get_ancestors(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str,
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }

        async fn get_descendants(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str,
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }

        async fn get_unit_policies(
            &self,
            _ctx: mk_core::types::TenantContext,
            _unit_id: &str,
        ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
            Ok(vec![])
        }

        async fn create_unit(
            &self,
            _unit: &mk_core::types::OrganizationalUnit,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn add_unit_policy(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _unit_id: &str,
            _policy: &mk_core::types::Policy,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn assign_role(
            &self,
            _user_id: &mk_core::types::UserId,
            _tenant_id: &mk_core::types::TenantId,
            _unit_id: &str,
            _role: mk_core::types::Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_role(
            &self,
            _user_id: &mk_core::types::UserId,
            _tenant_id: &mk_core::types::TenantId,
            _unit_id: &str,
            _role: mk_core::types::Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn store_drift_result(
            &self,
            _result: mk_core::types::DriftResult,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn get_latest_drift_result(
            &self,
            _ctx: mk_core::types::TenantContext,
            _project_id: &str,
        ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
            Ok(None)
        }

        async fn list_all_units(
            &self,
        ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
            Ok(vec![])
        }

        async fn record_job_status(
            &self,
            _job_name: &str,
            _tenant_id: &str,
            _status: &str,
            _message: Option<&str>,
            _started_at: i64,
            _finished_at: Option<i64>,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn get_governance_events(
            &self,
            _ctx: mk_core::types::TenantContext,
            _since_timestamp: i64,
            _limit: usize,
        ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_file_persister_save_and_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let persister = FilePersister::new(base_path);
        let tenant_id = mk_core::types::TenantId::default();

        let mut state = SyncState::default();
        state.stats.total_syncs = 10;

        persister.save(&tenant_id, &state).await.unwrap();

        let loaded_state = persister.load(&tenant_id).await.unwrap();
        assert_eq!(loaded_state.stats.total_syncs, 10);
    }

    #[tokio::test]
    async fn test_file_persister_load_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let persister = FilePersister::new(base_path);
        let tenant_id = mk_core::types::TenantId::default();

        let state = persister.load(&tenant_id).await.unwrap();
        assert_eq!(state, SyncState::default());
    }

    #[tokio::test]
    async fn test_database_persister_new() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        assert_eq!(persister.key_prefix, "test_key");
    }

    #[tokio::test]
    async fn test_database_persister_load_default() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        let tenant_id = mk_core::types::TenantId::default();

        let state = persister.load(&tenant_id).await.unwrap();
        assert_eq!(state, SyncState::default());
    }

    #[tokio::test]
    async fn test_database_persister_save_and_load() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        let tenant_id = mk_core::types::TenantId::default();

        let mut state = SyncState::default();
        state.stats.total_syncs = 5;
        state.stats.total_items_synced = 42;

        persister.save(&tenant_id, &state).await.unwrap();

        let loaded_state = persister.load(&tenant_id).await.unwrap();
        assert_eq!(loaded_state.stats.total_syncs, 5);
        assert_eq!(loaded_state.stats.total_items_synced, 42);
    }

    #[tokio::test]
    async fn test_database_persister_save_overwrites() {
        let storage = Arc::new(MockStorage::new());
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        let tenant_id = mk_core::types::TenantId::default();

        let mut state1 = SyncState::default();
        state1.stats.total_syncs = 1;

        let mut state2 = SyncState::default();
        state2.stats.total_syncs = 2;

        persister.save(&tenant_id, &state1).await.unwrap();
        persister.save(&tenant_id, &state2).await.unwrap();

        let loaded_state = persister.load(&tenant_id).await.unwrap();
        assert_eq!(loaded_state.stats.total_syncs, 2);
    }

    #[tokio::test]
    async fn test_database_persister_different_keys() {
        let storage = Arc::new(MockStorage::new());
        let persister1 = DatabasePersister::new(storage.clone(), "key1".to_string());
        let persister2 = DatabasePersister::new(storage, "key2".to_string());
        let tenant_id = mk_core::types::TenantId::default();

        let mut state1 = SyncState::default();
        state1.stats.total_syncs = 100;

        let mut state2 = SyncState::default();
        state2.stats.total_syncs = 200;

        persister1.save(&tenant_id, &state1).await.unwrap();
        persister2.save(&tenant_id, &state2).await.unwrap();

        let loaded1 = persister1.load(&tenant_id).await.unwrap();
        let loaded2 = persister2.load(&tenant_id).await.unwrap();

        assert_eq!(loaded1.stats.total_syncs, 100);
        assert_eq!(loaded2.stats.total_syncs, 200);
    }

    #[tokio::test]
    async fn test_database_persister_storage_error() {
        struct ErrorStorage;

        #[async_trait]
        impl StorageBackend for ErrorStorage {
            type Error = std::io::Error;

            async fn store(
                &self,
                _ctx: mk_core::types::TenantContext,
                _key: &str,
                _value: &[u8],
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn retrieve(
                &self,
                _ctx: mk_core::types::TenantContext,
                _key: &str,
            ) -> Result<Option<Vec<u8>>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn delete(
                &self,
                _ctx: mk_core::types::TenantContext,
                _key: &str,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn exists(
                &self,
                _ctx: mk_core::types::TenantContext,
                _key: &str,
            ) -> Result<bool, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn get_ancestors(
                &self,
                _ctx: mk_core::types::TenantContext,
                _unit_id: &str,
            ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn get_descendants(
                &self,
                _ctx: mk_core::types::TenantContext,
                _unit_id: &str,
            ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn get_unit_policies(
                &self,
                _ctx: mk_core::types::TenantContext,
                _unit_id: &str,
            ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn create_unit(
                &self,
                _unit: &mk_core::types::OrganizationalUnit,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn add_unit_policy(
                &self,
                _ctx: &mk_core::types::TenantContext,
                _unit_id: &str,
                _policy: &mk_core::types::Policy,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn assign_role(
                &self,
                _user_id: &mk_core::types::UserId,
                _tenant_id: &mk_core::types::TenantId,
                _unit_id: &str,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn remove_role(
                &self,
                _user_id: &mk_core::types::UserId,
                _tenant_id: &mk_core::types::TenantId,
                _unit_id: &str,
                _role: mk_core::types::Role,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn store_drift_result(
                &self,
                _result: mk_core::types::DriftResult,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn get_latest_drift_result(
                &self,
                _ctx: mk_core::types::TenantContext,
                _project_id: &str,
            ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn list_all_units(
                &self,
            ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn record_job_status(
                &self,
                _job_name: &str,
                _tenant_id: &str,
                _status: &str,
                _message: Option<&str>,
                _started_at: i64,
                _finished_at: Option<i64>,
            ) -> Result<(), Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }

            async fn get_governance_events(
                &self,
                _ctx: mk_core::types::TenantContext,
                _since_timestamp: i64,
                _limit: usize,
            ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "storage error",
                ))
            }
        }

        let storage = Arc::new(ErrorStorage);
        let persister = DatabasePersister::new(storage, "test_key".to_string());
        let tenant_id = mk_core::types::TenantId::default();

        let state = SyncState::default();
        let save_result = persister.save(&tenant_id, &state).await;
        assert!(save_result.is_err());

        let load_result = persister.load(&tenant_id).await;
        assert!(load_result.is_err());
    }
}
