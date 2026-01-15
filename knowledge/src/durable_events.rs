use mk_core::traits::{EventPublisher, StorageBackend};
use mk_core::types::{EventStatus, GovernanceEvent, PersistentEvent, TenantContext};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct DurableEventPublisher<S, P>
where
    S: StorageBackend + Send + Sync,
    P: EventPublisher + Send + Sync,
{
    storage: Arc<S>,
    redis_publisher: Arc<P>,
    tenant_ctx: TenantContext,
}

impl<S, P> DurableEventPublisher<S, P>
where
    S: StorageBackend + Send + Sync,
    S::Error: std::error::Error + Send + Sync + 'static,
    P: EventPublisher + Send + Sync,
    P::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(storage: Arc<S>, redis_publisher: Arc<P>, tenant_ctx: TenantContext) -> Self {
        Self {
            storage,
            redis_publisher,
            tenant_ctx,
        }
    }

    pub async fn publish_durable(
        &self,
        event: GovernanceEvent,
    ) -> Result<String, DurablePublishError> {
        let persistent_event = PersistentEvent::new(event.clone());
        let event_id = persistent_event.event_id.clone();
        let idempotency_key = persistent_event.idempotency_key.clone();

        self.storage
            .persist_event(persistent_event)
            .await
            .map_err(|e| DurablePublishError::PersistenceError(e.to_string()))?;

        match self.redis_publisher.publish(event).await {
            Ok(()) => {
                self.storage
                    .update_event_status(&event_id, EventStatus::Published, None)
                    .await
                    .map_err(|e| DurablePublishError::StatusUpdateError(e.to_string()))?;

                info!(event_id = %event_id, "Event published successfully");
                Ok(idempotency_key)
            }
            Err(e) => {
                warn!(event_id = %event_id, error = %e, "Failed to publish to Redis, event persisted for retry");
                self.storage
                    .update_event_status(&event_id, EventStatus::Pending, Some(e.to_string()))
                    .await
                    .ok();

                Err(DurablePublishError::PublishError(e.to_string()))
            }
        }
    }

    pub async fn retry_pending_events(
        &self,
        limit: usize,
    ) -> Result<RetryResult, DurablePublishError> {
        let pending = self
            .storage
            .get_pending_events(self.tenant_ctx.clone(), limit)
            .await
            .map_err(|e| DurablePublishError::StorageError(e.to_string()))?;

        let mut result = RetryResult::default();
        result.total = pending.len();

        for event in pending {
            if !event.is_retriable() {
                continue;
            }

            match self.redis_publisher.publish(event.payload.clone()).await {
                Ok(()) => {
                    self.storage
                        .update_event_status(&event.event_id, EventStatus::Published, None)
                        .await
                        .ok();
                    result.succeeded += 1;
                }
                Err(e) => {
                    let mut updated_event = event.clone();
                    let can_retry = updated_event.mark_failed(e.to_string());

                    if can_retry {
                        self.storage
                            .update_event_status(
                                &event.event_id,
                                EventStatus::Pending,
                                Some(e.to_string()),
                            )
                            .await
                            .ok();
                        result.retried += 1;
                    } else {
                        self.storage
                            .update_event_status(
                                &event.event_id,
                                EventStatus::DeadLettered,
                                Some(e.to_string()),
                            )
                            .await
                            .ok();
                        result.dead_lettered += 1;
                        error!(
                            event_id = %event.event_id,
                            "Event moved to dead letter queue after max retries"
                        );
                    }
                }
            }
        }

        Ok(result)
    }

    pub async fn process_dead_letter_queue(
        &self,
        limit: usize,
        handler: impl Fn(&PersistentEvent) -> bool,
    ) -> Result<DlqResult, DurablePublishError> {
        let dead_letters = self
            .storage
            .get_dead_letter_events(self.tenant_ctx.clone(), limit)
            .await
            .map_err(|e| DurablePublishError::StorageError(e.to_string()))?;

        let mut result = DlqResult::default();
        result.total = dead_letters.len();

        for event in dead_letters {
            if handler(&event) {
                match self.redis_publisher.publish(event.payload.clone()).await {
                    Ok(()) => {
                        self.storage
                            .update_event_status(&event.event_id, EventStatus::Published, None)
                            .await
                            .ok();
                        result.reprocessed += 1;
                    }
                    Err(e) => {
                        warn!(
                            event_id = %event.event_id,
                            error = %e,
                            "DLQ reprocessing failed"
                        );
                        result.failed += 1;
                    }
                }
            } else {
                result.skipped += 1;
            }
        }

        Ok(result)
    }
}

#[derive(Debug, Default)]
pub struct RetryResult {
    pub total: usize,
    pub succeeded: usize,
    pub retried: usize,
    pub dead_lettered: usize,
}

#[derive(Debug, Default)]
pub struct DlqResult {
    pub total: usize,
    pub reprocessed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum DurablePublishError {
    #[error("Failed to persist event: {0}")]
    PersistenceError(String),

    #[error("Failed to publish event: {0}")]
    PublishError(String),

    #[error("Failed to update event status: {0}")]
    StatusUpdateError(String),

    #[error("Storage operation failed: {0}")]
    StorageError(String),
}

pub struct IdempotentConsumer<S>
where
    S: StorageBackend + Send + Sync,
{
    storage: Arc<S>,
    consumer_group: String,
}

impl<S> IdempotentConsumer<S>
where
    S: StorageBackend + Send + Sync,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(storage: Arc<S>, consumer_group: String) -> Self {
        Self {
            storage,
            consumer_group,
        }
    }

    pub async fn process_if_new<F, T, E>(
        &self,
        idempotency_key: &str,
        tenant_id: &mk_core::types::TenantId,
        handler: F,
    ) -> Result<Option<T>, IdempotentConsumerError>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::error::Error,
    {
        let already_processed = self
            .storage
            .check_idempotency(&self.consumer_group, idempotency_key)
            .await
            .map_err(|e| IdempotentConsumerError::StorageError(e.to_string()))?;

        if already_processed {
            return Ok(None);
        }

        let result = handler
            .await
            .map_err(|e| IdempotentConsumerError::ProcessingError(e.to_string()))?;

        let state = mk_core::types::ConsumerState::new(
            self.consumer_group.clone(),
            idempotency_key.to_string(),
            tenant_id.clone(),
        );

        self.storage
            .record_consumer_state(state)
            .await
            .map_err(|e| IdempotentConsumerError::StorageError(e.to_string()))?;

        Ok(Some(result))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdempotentConsumerError {
    #[error("Storage operation failed: {0}")]
    StorageError(String),

    #[error("Event processing failed: {0}")]
    ProcessingError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_result_default() {
        let result = RetryResult::default();
        assert_eq!(result.total, 0);
        assert_eq!(result.succeeded, 0);
        assert_eq!(result.retried, 0);
        assert_eq!(result.dead_lettered, 0);
    }

    #[test]
    fn test_dlq_result_default() {
        let result = DlqResult::default();
        assert_eq!(result.total, 0);
        assert_eq!(result.reprocessed, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn test_durable_publish_error_display() {
        let error = DurablePublishError::PersistenceError("DB down".to_string());
        assert!(error.to_string().contains("persist"));

        let error = DurablePublishError::PublishError("Redis timeout".to_string());
        assert!(error.to_string().contains("publish"));
    }

    #[test]
    fn test_idempotent_consumer_error_display() {
        let error = IdempotentConsumerError::StorageError("Connection lost".to_string());
        assert!(error.to_string().contains("Storage"));

        let error = IdempotentConsumerError::ProcessingError("Handler failed".to_string());
        assert!(error.to_string().contains("processing"));
    }
}
