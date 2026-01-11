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

    async fn store(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str,
        value: &[u8]
    ) -> Result<(), Self::Error> {
        let mut conn = self.connection_manager.clone();
        conn.set(key, value)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string()
            })
    }

    async fn retrieve(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let mut conn = self.connection_manager.clone();
        conn.get(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string()
        })
    }

    async fn delete(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<(), Self::Error> {
        self.delete_key(key).await
    }

    async fn exists(
        &self,
        _ctx: mk_core::types::TenantContext,
        key: &str
    ) -> Result<bool, Self::Error> {
        self.exists_key(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use errors::StorageError;

    // Test error type conversions and patterns
    #[test]
    fn test_storage_error_display() {
        let conn_error = StorageError::ConnectionError {
            backend: "Redis".to_string(),
            reason: "Connection refused".to_string()
        };

        let query_error = StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: "Command failed".to_string()
        };

        assert_eq!(
            conn_error.to_string(),
            "Connection to Redis failed: Connection refused"
        );

        assert_eq!(
            query_error.to_string(),
            "Query on Redis failed: Command failed"
        );
    }

    // Test RedisStorage struct creation (without actual connection)
    #[tokio::test]
    async fn test_redis_storage_error_handling() {
        // This test verifies that invalid connection strings produce appropriate errors
        // Note: We can't easily mock the redis client, but we can verify error types

        // Test with obviously invalid URL
        let result = RedisStorage::new("not-a-valid-url").await;
        assert!(result.is_err());

        if let Err(StorageError::ConnectionError { backend, .. }) = result {
            assert_eq!(backend, "Redis");
        } else {
            panic!("Expected ConnectionError for invalid URL");
        }
    }

    // Test StorageBackend trait implementation consistency
    #[test]
    fn test_storage_backend_trait_bounds() {
        use mk_core::traits::StorageBackend;

        // This is a compile-time test to ensure RedisStorage implements StorageBackend
        fn assert_storage_backend<T: StorageBackend>() {}

        // If this compiles, RedisStorage implements StorageBackend
        assert_storage_backend::<RedisStorage>();
    }

    // Test error message formatting for different scenarios
    #[test]
    fn test_error_messages_include_backend_name() {
        let errors = vec![
            StorageError::ConnectionError {
                backend: "Redis".to_string(),
                reason: "test".to_string()
            },
            StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: "test".to_string()
            },
            StorageError::SerializationError {
                error_type: "JSON".to_string(),
                reason: "test".to_string()
            },
            StorageError::NotFound {
                backend: "Redis".to_string(),
                id: "key123".to_string()
            },
            StorageError::TransactionError {
                backend: "Redis".to_string(),
                reason: "test".to_string()
            },
        ];

        for error in errors {
            let msg = error.to_string();
            assert!(
                msg.contains("Redis") || msg.contains("JSON"),
                "Error message should contain backend or error type: {}",
                msg
            );
        }
    }

    // Test that RedisStorage methods have correct signatures
    #[test]
    fn test_method_signatures() {
        // This is a compile-time check

        // Verify RedisStorage has the expected method signature
        // The existence of the method is verified by compilation
        // We can't easily test async method signatures in a unit test
        let _ = RedisStorage::new;
    }
}
