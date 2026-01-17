use mk_core::types::{EventStatus, GovernanceEvent, PersistentEvent, TenantId, UnitType};
use redis::AsyncCommands;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;
use tools::redis_publisher::RedisPublisher;
use uuid::Uuid;

struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String,
}

static REDIS: OnceCell<Option<RedisFixture>> = OnceCell::const_new();
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn get_redis_fixture() -> Option<&'static RedisFixture> {
    REDIS
        .get_or_init(|| async {
            match Redis::default().start().await {
                Ok(container) => {
                    let port = container.get_host_port_ipv4(6379).await.ok()?;
                    let url = format!("redis://localhost:{}", port);
                    Some(RedisFixture { container, url })
                }
                Err(_) => None,
            }
        })
        .await
        .as_ref()
}

fn unique_tenant_id(prefix: &str) -> TenantId {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    TenantId::new(format!("{}-{}", prefix, id)).unwrap()
}

#[tokio::test]
async fn test_redis_publisher_start_run() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let publisher = RedisPublisher::new_with_tenant_isolation(
        fixture.url.clone(),
        "governance:events".to_string(),
    );
    let (tx, _handle) = publisher.start();

    let tenant_id = unique_tenant_id("tenant-abc");
    let event = GovernanceEvent::UnitCreated {
        unit_id: "u1".to_string(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        timestamp: 1000,
    };

    tx.send(event).unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let client = redis::Client::open(fixture.url.clone()).unwrap();
    let mut conn = client.get_connection_manager().await.unwrap();

    let stream_key = format!("governance:events:{}", tenant_id.as_str());

    let mut results: redis::streams::StreamReadReply =
        redis::streams::StreamReadReply { keys: vec![] };
    for _ in 0..15 {
        let r: redis::streams::StreamReadReply = conn.xread(&[&stream_key], &["0"]).await.unwrap();
        if !r.keys.is_empty() {
            results = r;
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    assert!(!results.keys.is_empty());
    assert!(!results.keys[0].ids.is_empty());
}

#[tokio::test]
async fn test_publish_to_dlq_and_read() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id("dlq-tenant");
    let gov_event = GovernanceEvent::UnitDeleted {
        unit_id: "unit-dlq".to_string(),
        tenant_id: tenant_id.clone(),
        timestamp: 1234567890,
    };

    let event = PersistentEvent {
        id: Uuid::new_v4().to_string(),
        event_id: Uuid::new_v4().to_string(),
        idempotency_key: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.clone(),
        event_type: "test.event".to_string(),
        payload: gov_event,
        status: EventStatus::DeadLettered,
        retry_count: 3,
        max_retries: 5,
        last_error: Some("test error message".to_string()),
        created_at: 1234567890,
        published_at: None,
        acknowledged_at: None,
        dead_lettered_at: Some(1234567891),
    };

    RedisPublisher::publish_to_dlq(&fixture.url, &event)
        .await
        .expect("Failed to publish to DLQ");

    let len = RedisPublisher::get_dlq_length(&fixture.url, &tenant_id)
        .await
        .expect("Failed to get DLQ length");
    assert_eq!(len, 1);

    let events = RedisPublisher::read_dlq_events(&fixture.url, &tenant_id, 10)
        .await
        .expect("Failed to read DLQ events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1.event_id, event.event_id);
    assert_eq!(events[0].1.retry_count, 3);
    assert_eq!(
        events[0].1.last_error,
        Some("test error message".to_string())
    );

    let message_id = &events[0].0;

    RedisPublisher::ack_dlq_event(&fixture.url, &tenant_id, message_id)
        .await
        .expect("Failed to ack DLQ event");

    let len_after = RedisPublisher::get_dlq_length(&fixture.url, &tenant_id)
        .await
        .expect("Failed to get DLQ length after ack");
    assert_eq!(len_after, 0);
}

#[tokio::test]
async fn test_factory_functions() {
    let redis_url = "redis://localhost:6379".to_string();
    let tenant_id = TenantId::new("t1".to_string()).unwrap();

    let _ = tools::redis_publisher::create_redis_publisher_with_tenant_isolation(redis_url.clone());
    let _ =
        tools::redis_publisher::create_redis_publisher_for_tenant(redis_url.clone(), &tenant_id);
    let _ = tools::redis_publisher::create_redis_publisher(redis_url, "custom".to_string());

    assert!(true);
}
