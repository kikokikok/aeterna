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

    pub async fn get(&mut self, key: &str) -> Result<Option<String>, StorageError> {
        self.connection_manager
            .get(key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }

    pub async fn set(
        &mut self,
        key: &str,
        value: &str,
        ttl_seconds: Option<usize>
    ) -> Result<(), StorageError> {
        if let Some(ttl) = ttl_seconds {
            self.connection_manager
                .set_ex(key, value, ttl as u64)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string()
                })
        } else {
            self.connection_manager
                .set(key, value)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string()
                })
        }
    }

    pub async fn delete(&mut self, key: &str) -> Result<(), StorageError> {
        self.connection_manager
            .del(key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }

    pub async fn exists(&mut self, key: &str) -> Result<bool, StorageError> {
        self.connection_manager
            .exists(key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }
}
