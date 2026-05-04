use duckdb::Connection;
use parking_lot::Mutex;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use storage::graph_event_log::GraphEventLog;
use storage::graph_projector::{GraphProjector, ProjectorConfig};
use tokio::time::Duration;

async fn fixture_pool() -> Option<sqlx::PgPool> {
    let fx = testing::postgres().await?;
    Some(
        PgPoolOptions::new()
            .max_connections(10)
            .connect(fx.url())
            .await
            .expect("pool"),
    )
}

fn setup_duckdb() -> Arc<Mutex<Connection>> {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE memory_nodes (
            id VARCHAR PRIMARY KEY,
            label VARCHAR NOT NULL,
            properties VARCHAR DEFAULT '{}',
            tenant_id VARCHAR NOT NULL,
            seq BIGINT DEFAULT 0,
            created_at TIMESTAMP DEFAULT now(),
            updated_at TIMESTAMP DEFAULT now(),
            deleted_at TIMESTAMP
        );
        CREATE TABLE memory_edges (
            id VARCHAR PRIMARY KEY,
            source_id VARCHAR NOT NULL,
            target_id VARCHAR NOT NULL,
            relation VARCHAR NOT NULL,
            properties VARCHAR DEFAULT '{}',
            tenant_id VARCHAR NOT NULL,
            seq BIGINT DEFAULT 0,
            created_at TIMESTAMP DEFAULT now(),
            deleted_at TIMESTAMP
        );
        "#,
    )
    .unwrap();
    Arc::new(Mutex::new(conn))
}

fn count_nodes(conn: &Connection, tenant_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM memory_nodes WHERE tenant_id = ? AND deleted_at IS NULL",
        duckdb::params![tenant_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn count_edges(conn: &Connection, tenant_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM memory_edges WHERE tenant_id = ? AND deleted_at IS NULL",
        duckdb::params![tenant_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[tokio::test]
async fn test_projector_replays_events_into_duckdb() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let tenant = testing::unique_id("proj-replay");
    let event_log = Arc::new(GraphEventLog::new(pool));
    let writer = setup_duckdb();

    for i in 0..5 {
        event_log
            .append(
                &tenant,
                "add_node",
                serde_json::json!({
                    "id": format!("n-{i}"),
                    "label": "Replayed",
                    "properties": "{}",
                    "tenant_id": tenant,
                }),
            )
            .await
            .unwrap();
    }

    let config = ProjectorConfig {
        poll_interval: Duration::from_millis(20),
        batch_size: 100,
        lag_threshold: 100,
        checkpoint_interval: Duration::from_secs(1),
    };
    let projector = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        Arc::clone(&writer),
        config,
    ));

    projector.start_tenant(tenant.clone());

    // Wait for projector to catch up
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(count_nodes(&writer.lock(), &tenant), 5);
    assert_eq!(projector.last_applied_seq(&tenant).unwrap(), 5);
    assert!(projector.is_ready().await);

    projector.stop();
}

/// 8.6: Kill projector, append more events, restart — projector must
/// resume from checkpoint and converge.
#[tokio::test]
async fn test_projector_kill_restart_recovery() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let tenant = testing::unique_id("proj-recovery");
    let event_log = Arc::new(GraphEventLog::new(pool));
    let writer = setup_duckdb();

    // Phase 1: append 5 events, let projector process them
    for i in 0..5 {
        event_log
            .append(
                &tenant,
                "add_node",
                serde_json::json!({
                    "id": format!("n-{i}"), "label": "P1", "properties": "{}", "tenant_id": tenant,
                }),
            )
            .await
            .unwrap();
    }

    let config = ProjectorConfig {
        poll_interval: Duration::from_millis(20),
        batch_size: 100,
        lag_threshold: 100,
        checkpoint_interval: Duration::from_millis(50), // fast checkpoint for test
    };
    let projector = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        Arc::clone(&writer),
        config.clone(),
    ));
    projector.start_tenant(tenant.clone());
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(count_nodes(&writer.lock(), &tenant), 5);

    // Kill
    projector.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Phase 2: append 5 more events while projector is down
    for i in 5..10 {
        event_log
            .append(
                &tenant,
                "add_node",
                serde_json::json!({
                    "id": format!("n-{i}"), "label": "P2", "properties": "{}", "tenant_id": tenant,
                }),
            )
            .await
            .unwrap();
    }

    // Restart: new projector on same DuckDB (has checkpoint)
    let projector2 = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        Arc::clone(&writer),
        config,
    ));
    projector2.start_tenant(tenant.clone());
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(count_nodes(&writer.lock(), &tenant), 10);
    assert_eq!(projector2.last_applied_seq(&tenant).unwrap(), 10);
    assert!(projector2.is_ready().await);

    projector2.stop();
}

/// 12.2: When projector lag exceeds threshold, is_ready() returns false.
#[tokio::test]
async fn test_projector_lag_exceeds_threshold_not_ready() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let tenant = testing::unique_id("proj-lag");
    let event_log = Arc::new(GraphEventLog::new(pool));
    let writer = setup_duckdb();

    // Append 20 events
    for i in 0..20 {
        event_log
            .append(
                &tenant,
                "add_node",
                serde_json::json!({
                    "id": format!("n-{i}"), "label": "Lag", "properties": "{}", "tenant_id": tenant,
                }),
            )
            .await
            .unwrap();
    }

    let config = ProjectorConfig {
        poll_interval: Duration::from_secs(600), // will never poll during this test
        batch_size: 100,
        lag_threshold: 5, // low threshold
        checkpoint_interval: Duration::from_secs(600),
    };
    let projector = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        Arc::clone(&writer),
        config,
    ));
    projector.start_tenant(tenant.clone());

    // Projector hasn't polled yet — lag = 20, threshold = 5 → not ready
    assert!(
        !projector.is_ready().await,
        "Projector should NOT be ready when lag > threshold"
    );

    projector.stop();
}

/// Edges + soft deletes flow through the projector correctly.
#[tokio::test]
async fn test_projector_handles_edges_and_soft_deletes() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Docker / Postgres not available");
        return;
    };

    let tenant = testing::unique_id("proj-edge-del");
    let event_log = Arc::new(GraphEventLog::new(pool));
    let writer = setup_duckdb();

    event_log
        .append(
            &tenant,
            "add_node",
            serde_json::json!({"id": "a", "label": "A", "properties": "{}", "tenant_id": tenant}),
        )
        .await
        .unwrap();
    event_log
        .append(
            &tenant,
            "add_node",
            serde_json::json!({"id": "b", "label": "B", "properties": "{}", "tenant_id": tenant}),
        )
        .await
        .unwrap();
    event_log
        .append(&tenant, "add_edge", serde_json::json!({"id": "e1", "source_id": "a", "target_id": "b", "relation": "LINK", "properties": "{}", "tenant_id": tenant}))
        .await.unwrap();
    event_log
        .append(
            &tenant,
            "soft_delete_node",
            serde_json::json!({"node_id": "b", "tenant_id": tenant}),
        )
        .await
        .unwrap();

    let config = ProjectorConfig {
        poll_interval: Duration::from_millis(20),
        batch_size: 100,
        lag_threshold: 100,
        checkpoint_interval: Duration::from_secs(10),
    };
    let projector = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        Arc::clone(&writer),
        config,
    ));
    projector.start_tenant(tenant.clone());
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Node "a" alive, "b" soft-deleted
    assert_eq!(count_nodes(&writer.lock(), &tenant), 1);
    // Edge also soft-deleted (cascade from node "b")
    assert_eq!(count_edges(&writer.lock(), &tenant), 0);
    assert_eq!(projector.last_applied_seq(&tenant).unwrap(), 4);

    projector.stop();
}
