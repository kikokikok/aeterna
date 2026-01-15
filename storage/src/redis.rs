use async_trait::async_trait;
use errors::StorageError;
use mk_core::traits::EventPublisher;
use mk_core::types::GovernanceEvent;
use redis::AsyncCommands;
use std::sync::Arc;

pub struct RedisStorage {
    client: Arc<redis::Client>,
    connection_manager: redis::aio::ConnectionManager,
}

impl RedisStorage {
    pub async fn new(connection_string: &str) -> Result<Self, StorageError> {
        let client =
            redis::Client::open(connection_string).map_err(|e| StorageError::ConnectionError {
                backend: "Redis".to_string(),
                reason: e.to_string(),
            })?;

        let connection_manager =
            client
                .get_connection_manager()
                .await
                .map_err(|e| StorageError::ConnectionError {
                    backend: "Redis".to_string(),
                    reason: e.to_string(),
                })?;

        Ok(Self {
            client: Arc::new(client),
            connection_manager,
        })
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.get(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string(),
        })
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<usize>,
    ) -> Result<(), StorageError> {
        let mut conn = self.connection_manager.clone();
        if let Some(ttl) = ttl_seconds {
            conn.set_ex(key, value, ttl as u64)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string(),
                })
        } else {
            conn.set(key, value)
                .await
                .map_err(|e| StorageError::QueryError {
                    backend: "Redis".to_string(),
                    reason: e.to_string(),
                })
        }
    }

    pub async fn delete_key(&self, key: &str) -> Result<(), StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.del(key).await.map_err(|e| StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: e.to_string(),
        })
    }

    pub async fn exists_key(&self, key: &str) -> Result<bool, StorageError> {
        let mut conn = self.connection_manager.clone();
        conn.exists(key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string(),
            })
    }
    pub fn scoped_key(&self, ctx: &mk_core::types::TenantContext, key: &str) -> String {
        format!("{}:{}", ctx.tenant_id.as_str(), key)
    }
}

#[async_trait]
impl EventPublisher for RedisStorage {
    type Error = StorageError;

    async fn publish(&self, event: GovernanceEvent) -> Result<(), Self::Error> {
        let mut conn = self.connection_manager.clone();
        let event_json =
            serde_json::to_string(&event).map_err(|e| StorageError::SerializationError {
                error_type: "JSON".to_string(),
                reason: e.to_string(),
            })?;

        let stream_key = format!("governance:events:{}", event.tenant_id());
        let _: String = conn
            .xadd(stream_key, "*", &[("event", event_json)])
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string(),
            })?;

        Ok(())
    }

    async fn subscribe(
        &self,
        channels: &[&str],
    ) -> Result<tokio::sync::mpsc::Receiver<GovernanceEvent>, Self::Error> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let client = self.client.clone();
        let stream_keys: Vec<String> = channels.iter().map(|s| s.to_string()).collect();

        tokio::spawn(async move {
            if let Ok(mut conn) = client.get_connection_manager().await {
                let mut last_ids: Vec<String> = vec!["$".to_string(); stream_keys.len()];

                loop {
                    let opts = redis::streams::StreamReadOptions::default()
                        .block(0)
                        .count(10);

                    let result: Result<redis::streams::StreamReadReply, redis::RedisError> =
                        conn.xread_options(&stream_keys, &last_ids, &opts).await;

                    match result {
                        Ok(reply) => {
                            for (i, stream) in reply.keys.into_iter().enumerate() {
                                for record in stream.ids {
                                    if let Some(event_json) = record.map.get("event") {
                                        if let Ok(event_str) =
                                            redis::from_redis_value::<String>(event_json.clone())
                                        {
                                            if let Ok(event) =
                                                serde_json::from_str::<GovernanceEvent>(&event_str)
                                            {
                                                if tx.send(event).await.is_err() {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                    last_ids[i] = record.id;
                                }
                            }
                        }
                        Err(_) => {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[async_trait]
impl mk_core::traits::StorageBackend for RedisStorage {
    type Error = StorageError;

    async fn store(
        &self,
        ctx: mk_core::types::TenantContext,
        key: &str,
        value: &[u8],
    ) -> Result<(), Self::Error> {
        let mut conn = self.connection_manager.clone();
        let scoped_key = self.scoped_key(&ctx, key);
        conn.set(scoped_key, value)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string(),
            })
    }

    async fn retrieve(
        &self,
        ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let mut conn = self.connection_manager.clone();
        let scoped_key = self.scoped_key(&ctx, key);
        conn.get(scoped_key)
            .await
            .map_err(|e| StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: e.to_string(),
            })
    }

    async fn delete(
        &self,
        ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<(), Self::Error> {
        let scoped_key = self.scoped_key(&ctx, key);
        self.delete_key(&scoped_key).await
    }

    async fn exists(
        &self,
        ctx: mk_core::types::TenantContext,
        key: &str,
    ) -> Result<bool, Self::Error> {
        let scoped_key = self.scoped_key(&ctx, key);
        self.exists_key(&scoped_key).await
    }

    async fn get_ancestors(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
    }

    async fn get_unit_policies(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
        Ok(Vec::new())
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

    async fn get_descendants(
        &self,
        _ctx: mk_core::types::TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
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

    async fn list_all_units(&self) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        Ok(Vec::new())
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
        Ok(Vec::new())
    }

    async fn create_suppression(
        &self,
        _suppression: mk_core::types::DriftSuppression,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn list_suppressions(
        &self,
        _ctx: mk_core::types::TenantContext,
        _project_id: &str,
    ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
        Ok(Vec::new())
    }

    async fn delete_suppression(
        &self,
        _ctx: mk_core::types::TenantContext,
        _suppression_id: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_drift_config(
        &self,
        _ctx: mk_core::types::TenantContext,
        _project_id: &str,
    ) -> Result<Option<mk_core::types::DriftConfig>, Self::Error> {
        Ok(None)
    }

    async fn save_drift_config(
        &self,
        _config: mk_core::types::DriftConfig,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn persist_event(
        &self,
        _event: mk_core::types::PersistentEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_pending_events(
        &self,
        _ctx: mk_core::types::TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(Vec::new())
    }

    async fn update_event_status(
        &self,
        _event_id: &str,
        _status: mk_core::types::EventStatus,
        _error: Option<String>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_dead_letter_events(
        &self,
        _ctx: mk_core::types::TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(Vec::new())
    }

    async fn check_idempotency(
        &self,
        _consumer_group: &str,
        _idempotency_key: &str,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn record_consumer_state(
        &self,
        _state: mk_core::types::ConsumerState,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_event_metrics(
        &self,
        _ctx: mk_core::types::TenantContext,
        _period_start: i64,
        _period_end: i64,
    ) -> Result<Vec<mk_core::types::EventDeliveryMetrics>, Self::Error> {
        Ok(Vec::new())
    }

    async fn record_event_metrics(
        &self,
        _metrics: mk_core::types::EventDeliveryMetrics,
    ) -> Result<(), Self::Error> {
        Ok(())
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
            reason: "Connection refused".to_string(),
        };

        let query_error = StorageError::QueryError {
            backend: "Redis".to_string(),
            reason: "Command failed".to_string(),
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
                reason: "test".to_string(),
            },
            StorageError::QueryError {
                backend: "Redis".to_string(),
                reason: "test".to_string(),
            },
            StorageError::SerializationError {
                error_type: "JSON".to_string(),
                reason: "test".to_string(),
            },
            StorageError::NotFound {
                backend: "Redis".to_string(),
                id: "key123".to_string(),
            },
            StorageError::TransactionError {
                backend: "Redis".to_string(),
                reason: "test".to_string(),
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
