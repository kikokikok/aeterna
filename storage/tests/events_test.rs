use mk_core::traits::EventPublisher;
use mk_core::types::{GovernanceEvent, TenantId};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use storage::events::{MultiPublisher, RedisPublisher};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;

struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String
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
                Err(_) => None
            }
        })
        .await
        .as_ref()
}

fn unique_stream_name(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}-{}", prefix, id)
}

#[tokio::test]
async fn test_redis_publisher_publish_subscribe() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let stream_name = unique_stream_name("test-stream");
    let publisher = RedisPublisher::new(&fixture.url, &stream_name).unwrap();
    let mut rx = publisher.subscribe(&[]).await.unwrap();

    let event = GovernanceEvent::UnitCreated {
        unit_id: "unit-1".to_string(),
        unit_type: mk_core::types::UnitType::Project,
        tenant_id: TenantId::new("tenant-1".to_string()).unwrap(),
        parent_id: None,
        timestamp: chrono::Utc::now().timestamp()
    };

    publisher.publish(event.clone()).await.unwrap();

    let received = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;

    assert!(received.is_ok(), "Timed out waiting for event");
    let received_event = received.unwrap().expect("Channel closed");

    match (event, received_event) {
        (
            GovernanceEvent::UnitCreated { unit_id: id1, .. },
            GovernanceEvent::UnitCreated { unit_id: id2, .. }
        ) => {
            assert_eq!(id1, id2);
        }
        _ => panic!("Event type mismatch or incorrect data")
    }
}

#[tokio::test]
async fn test_multi_publisher() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let stream1 = unique_stream_name("stream");
    let stream2 = unique_stream_name("stream");

    let pub1 = Box::new(RedisPublisher::new(&fixture.url, &stream1).unwrap());
    let pub2 = Box::new(RedisPublisher::new(&fixture.url, &stream2).unwrap());

    let multi = MultiPublisher::new(vec![pub1, pub2]);

    let event = GovernanceEvent::UnitUpdated {
        unit_id: "unit-1".to_string(),
        tenant_id: TenantId::new("tenant-1".to_string()).unwrap(),
        timestamp: chrono::Utc::now().timestamp()
    };

    let result = multi.publish(event).await;
    assert!(result.is_ok());
}

#[test]
fn test_event_error_display() {
    use storage::events::EventError;
    let err = EventError::Internal("test error".to_string());
    assert_eq!(format!("{}", err), "Internal error: test error");
}
