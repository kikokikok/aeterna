//! Integration tests for `GraphEventLog` against a real PostgreSQL instance.
//!
//! Task 7.4: 100 concurrent appenders for the same tenant must produce
//! seq values 1..=100 with no gaps and no duplicates, thanks to the
//! per-tenant advisory-lock allocation strategy.

use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use storage::graph_event_log::GraphEventLog;

async fn fixture_pool() -> Option<sqlx::PgPool> {
    let fx = testing::postgres().await?;
    Some(
        PgPoolOptions::new()
            .max_connections(120) // enough for 100 concurrent tasks + overhead
            .connect(fx.url())
            .await
            .expect("pool"),
    )
}

#[tokio::test]
async fn test_100_concurrent_appenders_monotonic_seq() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let tenant = testing::unique_id("concurrent-tenant");
    let log = Arc::new(GraphEventLog::new(pool));

    // Spawn 100 concurrent appenders — all targeting the same tenant.
    let mut handles = Vec::with_capacity(100);
    for i in 0..100 {
        let log = Arc::clone(&log);
        let tid = tenant.clone();
        handles.push(tokio::spawn(async move {
            log.append(
                &tid,
                "add_node",
                serde_json::json!({
                    "id": format!("node-{}", i),
                    "label": "ConcurrentNode",
                    "properties": {},
                    "tenant_id": tid,
                }),
            )
            .await
        }));
    }

    let mut seqs = Vec::with_capacity(100);
    for handle in handles {
        let seq = handle.await.expect("task panicked").expect("append failed");
        seqs.push(seq);
    }

    seqs.sort();

    // Must be exactly 1..=100 — no gaps, no duplicates.
    let expected: Vec<i64> = (1..=100).collect();
    assert_eq!(
        seqs, expected,
        "Expected monotonic seq 1..=100, got: {:?}",
        seqs
    );

    // head_seq must agree.
    let head = log.head_seq(&tenant).await.expect("head_seq failed");
    assert_eq!(head, 100);

    // tail from 0 must return all 100 events in order.
    let events = log.tail(&tenant, 0, 200).await.expect("tail failed");
    assert_eq!(events.len(), 100);
    for (i, event) in events.iter().enumerate() {
        assert_eq!(event.seq, (i + 1) as i64);
    }
}

#[tokio::test]
async fn test_tenant_isolation_between_event_logs() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let log = Arc::new(GraphEventLog::new(pool));
    let tenant_a = testing::unique_id("tenant-a");
    let tenant_b = testing::unique_id("tenant-b");

    // Append 5 events for tenant A
    for i in 0..5 {
        log.append(
            &tenant_a,
            "add_node",
            serde_json::json!({"id": format!("a-{i}"), "label": "A", "properties": {}, "tenant_id": tenant_a}),
        )
        .await
        .unwrap();
    }

    // Append 3 events for tenant B
    for i in 0..3 {
        log.append(
            &tenant_b,
            "add_node",
            serde_json::json!({"id": format!("b-{i}"), "label": "B", "properties": {}, "tenant_id": tenant_b}),
        )
        .await
        .unwrap();
    }

    assert_eq!(log.head_seq(&tenant_a).await.unwrap(), 5);
    assert_eq!(log.head_seq(&tenant_b).await.unwrap(), 3);

    let events_a = log.tail(&tenant_a, 0, 100).await.unwrap();
    let events_b = log.tail(&tenant_b, 0, 100).await.unwrap();
    assert_eq!(events_a.len(), 5);
    assert_eq!(events_b.len(), 3);

    // Seqs are per-tenant, both start at 1.
    assert_eq!(events_a[0].seq, 1);
    assert_eq!(events_b[0].seq, 1);
}

#[tokio::test]
async fn test_tail_pagination() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let log = GraphEventLog::new(pool);
    let tenant = testing::unique_id("tail-page");

    for i in 0..10 {
        log.append(
            &tenant,
            "add_edge",
            serde_json::json!({"id": format!("e-{i}"), "source_id": "a", "target_id": "b", "relation": "R", "properties": {}, "tenant_id": tenant}),
        )
        .await
        .unwrap();
    }

    // Page 1: seq 1..=5
    let page1 = log.tail(&tenant, 0, 5).await.unwrap();
    assert_eq!(page1.len(), 5);
    assert_eq!(page1.last().unwrap().seq, 5);

    // Page 2: seq 6..=10
    let page2 = log.tail(&tenant, 5, 5).await.unwrap();
    assert_eq!(page2.len(), 5);
    assert_eq!(page2.first().unwrap().seq, 6);
    assert_eq!(page2.last().unwrap().seq, 10);

    // Page 3: empty
    let page3 = log.tail(&tenant, 10, 5).await.unwrap();
    assert!(page3.is_empty());
}

#[tokio::test]
async fn test_empty_tenant_returns_zero() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let log = GraphEventLog::new(pool);
    let tenant = testing::unique_id("empty-tenant");

    assert_eq!(log.head_seq(&tenant).await.unwrap(), 0);
    let events = log.tail(&tenant, 0, 100).await.unwrap();
    assert!(events.is_empty());
}
