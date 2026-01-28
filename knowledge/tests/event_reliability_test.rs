use knowledge::durable_events::{
    DlqResult, DurablePublishError, IdempotentConsumerError, RetryResult
};
use mk_core::types::{
    ConsumerState, EventDeliveryMetrics, EventStatus, GovernanceEvent, PersistentEvent, TenantId,
    UnitType
};

fn create_test_tenant() -> TenantId {
    TenantId::new("test-tenant".to_string()).unwrap()
}

fn create_test_event(tenant_id: TenantId) -> GovernanceEvent {
    GovernanceEvent::UnitCreated {
        unit_id: "unit-1".to_string(),
        unit_type: UnitType::Project,
        tenant_id,
        parent_id: None,
        timestamp: chrono::Utc::now().timestamp()
    }
}

#[test]
fn test_persistent_event_creation() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id.clone());
    let persistent = PersistentEvent::new(event);

    assert_eq!(persistent.tenant_id, tenant_id);
    assert_eq!(persistent.status, EventStatus::Pending);
    assert_eq!(persistent.retry_count, 0);
    assert_eq!(persistent.max_retries, 3);
    assert!(!persistent.idempotency_key.is_empty());
    assert!(persistent.published_at.is_none());
    assert!(persistent.acknowledged_at.is_none());
    assert!(persistent.dead_lettered_at.is_none());
}

#[test]
fn test_persistent_event_idempotency_key_uniqueness() {
    let tenant_id = create_test_tenant();
    let event1 = create_test_event(tenant_id.clone());
    let event2 = create_test_event(tenant_id.clone());

    let persistent1 = PersistentEvent::new(event1);
    let persistent2 = PersistentEvent::new(event2);

    assert_ne!(persistent1.idempotency_key, persistent2.idempotency_key);
    assert_ne!(persistent1.event_id, persistent2.event_id);
}

#[test]
fn test_persistent_event_mark_published() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let mut persistent = PersistentEvent::new(event);

    assert_eq!(persistent.status, EventStatus::Pending);
    assert!(persistent.published_at.is_none());

    persistent.mark_published();

    assert_eq!(persistent.status, EventStatus::Published);
    assert!(persistent.published_at.is_some());
}

#[test]
fn test_persistent_event_mark_acknowledged() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let mut persistent = PersistentEvent::new(event);
    persistent.mark_published();

    persistent.mark_acknowledged();

    assert_eq!(persistent.status, EventStatus::Acknowledged);
    assert!(persistent.acknowledged_at.is_some());
}

#[test]
fn test_persistent_event_mark_failed_allows_retry() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let mut persistent = PersistentEvent::new(event);

    let can_retry = persistent.mark_failed("Connection timeout".to_string());

    assert!(can_retry);
    assert_eq!(persistent.retry_count, 1);
    assert_eq!(persistent.status, EventStatus::Pending);
    assert_eq!(
        persistent.last_error,
        Some("Connection timeout".to_string())
    );
}

#[test]
fn test_persistent_event_mark_failed_exhausts_retries() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let mut persistent = PersistentEvent::new(event);

    persistent.mark_failed("Error 1".to_string());
    persistent.mark_failed("Error 2".to_string());
    let can_retry = persistent.mark_failed("Error 3".to_string());

    assert!(!can_retry);
    assert_eq!(persistent.retry_count, 3);
    assert_eq!(persistent.status, EventStatus::DeadLettered);
    assert!(persistent.dead_lettered_at.is_some());
}

#[test]
fn test_persistent_event_is_retriable() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let mut persistent = PersistentEvent::new(event);

    assert!(persistent.is_retriable());

    persistent.mark_failed("Error 1".to_string());
    assert!(persistent.is_retriable());

    persistent.mark_failed("Error 2".to_string());
    assert!(persistent.is_retriable());

    persistent.mark_failed("Error 3".to_string());
    assert!(!persistent.is_retriable());
}

#[test]
fn test_event_status_display() {
    assert_eq!(format!("{}", EventStatus::Pending), "pending");
    assert_eq!(format!("{}", EventStatus::Published), "published");
    assert_eq!(format!("{}", EventStatus::Acknowledged), "acknowledged");
    assert_eq!(format!("{}", EventStatus::DeadLettered), "dead_lettered");
}

#[test]
fn test_event_delivery_metrics_creation() {
    let tenant_id = create_test_tenant();
    let now = chrono::Utc::now().timestamp();
    let one_hour_ago = now - 3600;

    let metrics = EventDeliveryMetrics::new(
        tenant_id.clone(),
        "unit_created".to_string(),
        one_hour_ago,
        now
    );

    assert_eq!(metrics.tenant_id, tenant_id);
    assert_eq!(metrics.event_type, "unit_created");
    assert_eq!(metrics.period_start, one_hour_ago);
    assert_eq!(metrics.period_end, now);
    assert_eq!(metrics.total_events, 0);
    assert_eq!(metrics.delivered_events, 0);
    assert_eq!(metrics.retried_events, 0);
    assert_eq!(metrics.dead_lettered_events, 0);
    assert!(metrics.avg_delivery_time_ms.is_none());
}

#[test]
fn test_event_delivery_metrics_success_rate() {
    let tenant_id = create_test_tenant();
    let now = chrono::Utc::now().timestamp();

    let mut metrics =
        EventDeliveryMetrics::new(tenant_id, "unit_created".to_string(), now - 3600, now);

    assert_eq!(metrics.delivery_success_rate(), 1.0);

    metrics.total_events = 100;
    metrics.delivered_events = 95;

    assert!((metrics.delivery_success_rate() - 0.95).abs() < 0.001);
}

#[test]
fn test_consumer_state_creation() {
    let tenant_id = create_test_tenant();
    let state = ConsumerState::new(
        "drift-processor".to_string(),
        "abc123def456".to_string(),
        tenant_id.clone()
    );

    assert_eq!(state.consumer_group, "drift-processor");
    assert_eq!(state.idempotency_key, "abc123def456");
    assert_eq!(state.tenant_id, tenant_id);
    assert!(state.processed_at > 0);
}

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
    let err = DurablePublishError::PersistenceError("DB connection failed".to_string());
    assert!(err.to_string().contains("persist"));
    assert!(err.to_string().contains("DB connection failed"));

    let err = DurablePublishError::PublishError("Redis timeout".to_string());
    assert!(err.to_string().contains("publish"));

    let err = DurablePublishError::StatusUpdateError("Update failed".to_string());
    assert!(err.to_string().contains("status"));

    let err = DurablePublishError::StorageError("Storage unavailable".to_string());
    assert!(err.to_string().contains("Storage"));
}

#[test]
fn test_idempotent_consumer_error_display() {
    let err = IdempotentConsumerError::StorageError("Connection lost".to_string());
    assert!(err.to_string().contains("Storage"));

    let err = IdempotentConsumerError::ProcessingError("Handler panicked".to_string());
    assert!(err.to_string().contains("processing"));
}

#[test]
fn test_event_type_extraction() {
    let tenant_id = create_test_tenant();

    let events = vec![
        (
            GovernanceEvent::UnitCreated {
                unit_id: "u1".to_string(),
                unit_type: UnitType::Project,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                timestamp: 0
            },
            "unit_created"
        ),
        (
            GovernanceEvent::UnitUpdated {
                unit_id: "u1".to_string(),
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            "unit_updated"
        ),
        (
            GovernanceEvent::UnitDeleted {
                unit_id: "u1".to_string(),
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            "unit_deleted"
        ),
        (
            GovernanceEvent::DriftDetected {
                project_id: "p1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.5,
                timestamp: 0
            },
            "drift_detected"
        ),
    ];

    for (event, expected_type) in events {
        let persistent = PersistentEvent::new(event);
        assert_eq!(persistent.event_type, expected_type);
    }
}

#[test]
fn test_persistent_event_serialization() {
    let tenant_id = create_test_tenant();
    let event = create_test_event(tenant_id);
    let persistent = PersistentEvent::new(event);

    let json = serde_json::to_string(&persistent).unwrap();
    let deserialized: PersistentEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.event_id, persistent.event_id);
    assert_eq!(deserialized.idempotency_key, persistent.idempotency_key);
    assert_eq!(deserialized.tenant_id, persistent.tenant_id);
    assert_eq!(deserialized.status, persistent.status);
}

#[test]
fn test_event_status_serialization() {
    let statuses = vec![
        EventStatus::Pending,
        EventStatus::Published,
        EventStatus::Acknowledged,
        EventStatus::DeadLettered,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: EventStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_consumer_state_serialization() {
    let tenant_id = create_test_tenant();
    let state = ConsumerState::new("group1".to_string(), "key123".to_string(), tenant_id);

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: ConsumerState = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.consumer_group, state.consumer_group);
    assert_eq!(deserialized.idempotency_key, state.idempotency_key);
    assert_eq!(deserialized.tenant_id, state.tenant_id);
}
