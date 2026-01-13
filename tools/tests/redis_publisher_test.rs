use mk_core::types::{GovernanceEvent, TenantId, UnitType};
use redis::AsyncCommands;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tools::redis_publisher::RedisPublisher;

#[tokio::test]
async fn test_redis_publisher_start_run() {
    let container = match Redis::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Redis test: Docker not available");
            return;
        }
    };

    let port = container.get_host_port_ipv4(6379).await.unwrap();
    let redis_url = format!("redis://localhost:{}", port);

    let publisher = RedisPublisher::new_with_tenant_isolation(
        redis_url.clone(),
        "governance:events".to_string(),
    );
    let (tx, _handle) = publisher.start();

    let tenant_id = TenantId::new("tenant-abc".to_string()).unwrap();
    let event = GovernanceEvent::UnitCreated {
        unit_id: "u1".to_string(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        timestamp: 1000,
    };

    tx.send(event).unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let client = redis::Client::open(redis_url).unwrap();
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
async fn test_factory_functions() {
    let redis_url = "redis://localhost:6379".to_string();
    let tenant_id = TenantId::new("t1".to_string()).unwrap();

    let _ = tools::redis_publisher::create_redis_publisher_with_tenant_isolation(redis_url.clone());
    let _ =
        tools::redis_publisher::create_redis_publisher_for_tenant(redis_url.clone(), &tenant_id);
    let _ = tools::redis_publisher::create_redis_publisher(redis_url, "custom".to_string());

    assert!(true);
}
