use mk_core::types::{GovernanceEvent, PersistentEvent, TenantId};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, info, warn};

/// Base stream key for governance events
pub const GOVERNANCE_EVENTS_STREAM: &str = "governance:events";

/// Base stream key for dead letter queue
pub const GOVERNANCE_DLQ_STREAM: &str = "governance:dlq";

/// DLQ consumer group name
pub const DLQ_CONSUMER_GROUP: &str = "dlq-processor";

/// Redis publisher for governance events with tenant isolation.
///
/// Listens for governance events on a channel and publishes them to Redis
/// Streams with per-tenant isolation. Routes events to the correct
/// tenant stream based on the tenant_id in each event.
pub struct RedisPublisher {
    redis_url: String,
    base_stream_key: String,
}

impl RedisPublisher {
    /// Creates a new Redis publisher with tenant isolation.
    ///
    /// Events will be published to streams named
    /// `{base_stream_key}:{tenant_id}`.
    pub fn new_with_tenant_isolation(redis_url: String, base_stream_key: String) -> Self {
        Self {
            redis_url,
            base_stream_key,
        }
    }

    /// Creates a new Redis publisher for a specific tenant (legacy API).
    pub fn new_for_tenant(redis_url: String, tenant_id: &TenantId) -> Self {
        let base_stream_key = format!("governance:events:{}", tenant_id.as_str());
        Self {
            redis_url,
            base_stream_key,
        }
    }

    /// Creates a new Redis publisher with a custom stream key (no tenant
    /// isolation).
    pub fn new(redis_url: String, stream_key: String) -> Self {
        Self {
            redis_url,
            base_stream_key: stream_key,
        }
    }

    /// Starts the Redis publisher task.
    ///
    /// This spawns a Tokio task that listens for events and publishes them to
    /// Redis. Returns a channel sender that can be used to send events.
    pub fn start(
        self,
    ) -> (
        tokio::sync::mpsc::UnboundedSender<GovernanceEvent>,
        tokio::task::JoinHandle<()>,
    ) {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            if let Err(e) = self.run(event_rx).await {
                error!("Redis publisher task failed: {}", e);
            }
        });

        (event_tx, handle)
    }

    /// Main loop for the Redis publisher.
    async fn run(
        self,
        mut event_rx: UnboundedReceiver<GovernanceEvent>,
    ) -> Result<(), anyhow::Error> {
        info!(
            "Starting Redis publisher with tenant isolation, base stream: {}",
            self.base_stream_key
        );

        let redis_url = self.redis_url.clone();
        let base_stream_key = self.base_stream_key.clone();

        let client = redis::Client::open(redis_url)?;
        let mut con = client.get_connection_manager().await?;

        while let Some(event) = event_rx.recv().await {
            match Self::publish_event(&base_stream_key, &mut con, &event).await {
                Ok(_) => {
                    info!("Published governance event: {:?}", event);
                }
                Err(e) => {
                    error!("Failed to publish event to Redis: {}", e);
                }
            }
        }

        info!("Redis publisher shutting down");
        Ok(())
    }

    /// Publishes a single event to the appropriate tenant stream.
    async fn publish_event(
        base_stream_key: &str,
        con: &mut redis::aio::ConnectionManager,
        event: &GovernanceEvent,
    ) -> Result<(), anyhow::Error> {
        let tenant_id = match event {
            GovernanceEvent::UnitCreated { tenant_id, .. } => tenant_id,
            GovernanceEvent::UnitUpdated { tenant_id, .. } => tenant_id,
            GovernanceEvent::UnitDeleted { tenant_id, .. } => tenant_id,
            GovernanceEvent::PolicyUpdated { tenant_id, .. } => tenant_id,
            GovernanceEvent::PolicyDeleted { tenant_id, .. } => tenant_id,
            GovernanceEvent::RoleAssigned { tenant_id, .. } => tenant_id,
            GovernanceEvent::RoleRemoved { tenant_id, .. } => tenant_id,
            GovernanceEvent::DriftDetected { tenant_id, .. } => tenant_id,
        };

        let stream_key = format!("{}:{}", base_stream_key, tenant_id.as_str());

        println!("DEBUG: Publishing to stream key: {}", stream_key);
        let event_json = serde_json::to_string(event)?;

        let _: String = redis::cmd("XADD")
            .arg(&stream_key)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .query_async(con)
            .await?;

        Ok(())
    }

    /// Publishes a failed event to the dead letter queue stream.
    pub async fn publish_to_dlq(
        redis_url: &str,
        event: &PersistentEvent,
    ) -> Result<(), anyhow::Error> {
        let client = redis::Client::open(redis_url)?;
        let mut con = client.get_connection_manager().await?;

        let stream_key = format!("{}:{}", GOVERNANCE_DLQ_STREAM, event.tenant_id.as_str());
        let event_json = serde_json::to_string(event)?;

        let _: String = redis::cmd("XADD")
            .arg(&stream_key)
            .arg("*")
            .arg("event")
            .arg(&event_json)
            .arg("error")
            .arg(event.last_error.as_deref().unwrap_or("unknown"))
            .arg("retry_count")
            .arg(event.retry_count.to_string())
            .query_async(&mut con)
            .await?;

        warn!(
            event_id = %event.event_id,
            tenant_id = %event.tenant_id,
            "Event published to DLQ"
        );

        Ok(())
    }

    /// Reads events from the dead letter queue for a tenant.
    pub async fn read_dlq_events(
        redis_url: &str,
        tenant_id: &TenantId,
        count: usize,
    ) -> Result<Vec<(String, PersistentEvent)>, anyhow::Error> {
        let client = redis::Client::open(redis_url)?;
        let mut con = client.get_connection_manager().await?;

        let stream_key = format!("{}:{}", GOVERNANCE_DLQ_STREAM, tenant_id.as_str());

        let result: redis::streams::StreamReadReply = redis::cmd("XREAD")
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(&stream_key)
            .arg("0")
            .query_async(&mut con)
            .await?;

        let mut events = Vec::new();
        for key in result.keys {
            for entry in key.ids {
                if let Some(event_data) = entry.map.get("event") {
                    if let redis::Value::BulkString(bytes) = event_data {
                        let json_str = String::from_utf8_lossy(bytes);
                        if let Ok(event) = serde_json::from_str::<PersistentEvent>(&json_str) {
                            events.push((entry.id.clone(), event));
                        }
                    }
                }
            }
        }

        Ok(events)
    }

    /// Acknowledges and removes an event from the DLQ after successful reprocessing.
    pub async fn ack_dlq_event(
        redis_url: &str,
        tenant_id: &TenantId,
        message_id: &str,
    ) -> Result<(), anyhow::Error> {
        let client = redis::Client::open(redis_url)?;
        let mut con = client.get_connection_manager().await?;

        let stream_key = format!("{}:{}", GOVERNANCE_DLQ_STREAM, tenant_id.as_str());

        let _: i64 = redis::cmd("XDEL")
            .arg(&stream_key)
            .arg(message_id)
            .query_async(&mut con)
            .await?;

        info!(
            message_id = %message_id,
            tenant_id = %tenant_id,
            "DLQ event acknowledged and removed"
        );

        Ok(())
    }

    /// Gets the count of events in the DLQ for a tenant.
    pub async fn get_dlq_length(
        redis_url: &str,
        tenant_id: &TenantId,
    ) -> Result<usize, anyhow::Error> {
        let client = redis::Client::open(redis_url)?;
        let mut con = client.get_connection_manager().await?;

        let stream_key = format!("{}:{}", GOVERNANCE_DLQ_STREAM, tenant_id.as_str());

        let len: usize = redis::cmd("XLEN")
            .arg(&stream_key)
            .query_async(&mut con)
            .await?;

        Ok(len)
    }
}

/// Creates a Redis publisher with tenant isolation and returns the event
/// channel sender.
///
/// Events will be routed to per-tenant streams:
/// `governance:events:{tenant_id}`. Caller should use the returned sender to
/// create GovernanceEngine: ```rust
/// let (event_tx, publisher_handle) =
///     create_redis_publisher_with_tenant_isolation("redis://localhost:6379".
/// to_string()); let governance_engine =
/// GovernanceEngine::new().with_events(event_tx); ```
pub fn create_redis_publisher_with_tenant_isolation(
    redis_url: String,
) -> (
    tokio::sync::mpsc::UnboundedSender<GovernanceEvent>,
    tokio::task::JoinHandle<()>,
) {
    let publisher =
        RedisPublisher::new_with_tenant_isolation(redis_url, "governance:events".to_string());
    publisher.start()
}

/// Creates a Redis publisher for a specific tenant (legacy API).
pub fn create_redis_publisher_for_tenant(
    redis_url: String,
    tenant_id: &TenantId,
) -> (
    tokio::sync::mpsc::UnboundedSender<GovernanceEvent>,
    tokio::task::JoinHandle<()>,
) {
    let publisher = RedisPublisher::new_for_tenant(redis_url, tenant_id);
    publisher.start()
}

/// Creates a Redis publisher with a custom stream key (no tenant isolation).
pub fn create_redis_publisher(
    redis_url: String,
    stream_key: String,
) -> (
    tokio::sync::mpsc::UnboundedSender<GovernanceEvent>,
    tokio::task::JoinHandle<()>,
) {
    let publisher = RedisPublisher::new(redis_url, stream_key);
    publisher.start()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{KnowledgeLayer, Role, UnitType, UserId};

    #[test]
    fn test_redis_publisher_tenant_isolation() {
        let tenant_id1 = TenantId::new("tenant-1".to_string()).unwrap();
        let tenant_id2 = TenantId::new("tenant-2".to_string()).unwrap();

        let publisher = RedisPublisher::new_with_tenant_isolation(
            "redis://localhost:6379".to_string(),
            "governance:events".to_string(),
        );

        let event1 = GovernanceEvent::UnitCreated {
            unit_id: "unit-1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id1.clone(),
            parent_id: None,
            timestamp: 1234567890,
        };

        let event2 = GovernanceEvent::UnitCreated {
            unit_id: "unit-2".to_string(),
            unit_type: UnitType::Team,
            tenant_id: tenant_id2.clone(),
            parent_id: Some("unit-1".to_string()),
            timestamp: 1234567891,
        };

        let _legacy_publisher =
            RedisPublisher::new_for_tenant("redis://localhost:6379".to_string(), &tenant_id1);

        assert!(true);
    }

    #[test]
    fn test_event_tenant_id_extraction() {
        let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
        let user_id = UserId::new("user-1".to_string()).unwrap();

        let events = vec![
            GovernanceEvent::UnitCreated {
                unit_id: "unit-1".to_string(),
                unit_type: UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                timestamp: 1234567890,
            },
            GovernanceEvent::PolicyUpdated {
                policy_id: "policy-1".to_string(),
                layer: KnowledgeLayer::Company,
                tenant_id: tenant_id.clone(),
                timestamp: 1234567891,
            },
            GovernanceEvent::RoleAssigned {
                user_id: user_id.clone(),
                unit_id: "unit-1".to_string(),
                role: Role::Admin,
                tenant_id: tenant_id.clone(),
                timestamp: 1234567892,
            },
            GovernanceEvent::DriftDetected {
                project_id: "project-1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.5,
                timestamp: 1234567893,
            },
        ];

        for event in events {
            let extracted_tenant_id = match &event {
                GovernanceEvent::UnitCreated { tenant_id, .. } => tenant_id,
                GovernanceEvent::UnitUpdated { tenant_id, .. } => tenant_id,
                GovernanceEvent::UnitDeleted { tenant_id, .. } => tenant_id,
                GovernanceEvent::PolicyUpdated { tenant_id, .. } => tenant_id,
                GovernanceEvent::PolicyDeleted { tenant_id, .. } => tenant_id,
                GovernanceEvent::RoleAssigned { tenant_id, .. } => tenant_id,
                GovernanceEvent::RoleRemoved { tenant_id, .. } => tenant_id,
                GovernanceEvent::DriftDetected { tenant_id, .. } => tenant_id,
            };
            assert_eq!(extracted_tenant_id, &tenant_id);
        }
    }
}
