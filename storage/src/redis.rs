use async_trait::async_trait;
use errors::StorageError;
use redis::AsyncCommands;

pub struct RedisStorage {
    #[allow(dead_code)]
    client: redis::Client,
    connection_manager: redis::aio::ConnectionManager
}

impl RedisStorage {
    pub async fn new(connection_string: &str) -> Result<Self, StorageError> {
        let client =
            redis::Client::open(connection_string).map_err(|e| StorageError::ConnectionError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })?;

        let connection_manager =
            client
                .get_connection_manager()
                .await
                .map_err(|e| StorageError::ConnectionError {
                    backend: "Redis".to_string(),
                    reason: e.to_string()
                })?;

        Ok(Self {
            client,
            connection_manager
        })
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.get(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string()
        })
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<usize>
    ) -> Result<(), StorageError> {
        let mut conn = self.connection_manager.clone();
        if let Some(ttl) = ttl_seconds {
            conn.set_ex(key, value, ttl as u64)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string()
                })
        } else {
            conn.set(key, value)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string()
                })
        }
    }

    pub async fn delete_key(&self, key: &str) -> Result<(), StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.del(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string()
        })
    }

    pub async fn exists_key(&self, key: &str) -> Result<bool, StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.exists(key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }
}

#[async_trait]
impl mk_core::traits::StorageBackend for RedisStorage {
    type Error = StorageError;

    async fn store(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        let mut conn = self.connection_manager.clone();
        conn.set(key, value)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }

    async fn retrieve(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        let mut conn = self.connection_manager.clone();
        conn.get(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string()
        })
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        self.delete_key(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, Self::Error> {
        self.exists_key(key).await
    }
}
