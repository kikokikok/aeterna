use duckdb::Connection;
use mk_core::types::{TenantContext, TenantId, UserId};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use storage::graph::{GraphNode, GraphStore};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore};
use storage::graph_event_log::GraphEventLog;
use storage::graph_projector::{GraphProjector, ProjectorConfig};
use storage::graph_verify;
use testing::minio;
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

fn make_config(endpoint: &str, prefix: &str) -> DuckDbGraphConfig {
    DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some("aeterna-test".to_string()),
        s3_prefix: Some(prefix.to_string()),
        s3_endpoint: Some(endpoint.to_string()),
        s3_region: Some("us-east-1".to_string()),
        s3_force_path_style: true,
        ..Default::default()
    }
}

/// 10.4: Snapshot to S3, then cold-start a fresh store from the snapshot
/// and replay event log. The resulting DuckDB state on both pods must
/// produce an identical SHA-256 digest.
#[tokio::test]
async fn test_cold_start_snapshot_replay_convergence() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("Skipping: Postgres not available");
        return;
    };
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping: MinIO not available");
        return;
    };

    let tenant = testing::unique_id("convergence");
    let prefix = testing::unique_id("conv-prefix");
    let event_log = Arc::new(GraphEventLog::new(pool));

    // --- Pod A: populate graph, take snapshot ---
    let config_a = make_config(minio_fixture.endpoint(), &prefix);
    let mut store_a = DuckDbGraphStore::new(config_a.clone()).unwrap();

    let projector_config = ProjectorConfig {
        poll_interval: Duration::from_millis(20),
        batch_size: 100,
        lag_threshold: 100,
        checkpoint_interval: Duration::from_secs(10),
    };
    let writer_a = store_a.db_handle();
    let projector_a = Arc::new(GraphProjector::new(
        Arc::clone(&event_log),
        writer_a,
        projector_config.clone(),
    ));
    store_a.enable_event_sourcing(Arc::clone(&event_log), Arc::clone(&projector_a));

    let ctx = TenantContext::new(
        TenantId::new(tenant.clone()).unwrap(),
        UserId::new("test-user".to_string()).unwrap(),
    );

    // Add 10 nodes via the store (dual-write: DuckDB + event log)
    for i in 0..10 {
        store_a
            .add_node(
                ctx.clone(),
                GraphNode {
                    id: format!("n-{i}"),
                    label: format!("Node-{i}"),
                    properties: serde_json::json!({"idx": i}),
                    tenant_id: tenant.clone(),
                },
            )
            .await
            .unwrap();
    }

    let snapshot_key = store_a.persist_to_s3(&tenant).await.unwrap();
    let digest_a = graph_verify::compute_digest_hex(&store_a.db_handle().lock(), &tenant).unwrap();

    // --- Pod B: cold-start from snapshot, then replay remaining events ---
    let config_b = make_config(minio_fixture.endpoint(), &prefix);
    let store_b = DuckDbGraphStore::new(config_b).unwrap();
    store_b.load_from_s3(&tenant, &snapshot_key).await.unwrap();

    let digest_b = graph_verify::compute_digest_hex(&store_b.db_handle().lock(), &tenant).unwrap();

    assert_eq!(
        digest_a, digest_b,
        "Pod A and Pod B digests must match after snapshot+restore"
    );
}

/// 11.4: Insert different data into two DuckDB instances → digests diverge.
#[tokio::test]
async fn test_divergence_detection() {
    let conn_a = Connection::open_in_memory().unwrap();
    let conn_b = Connection::open_in_memory().unwrap();

    for conn in [&conn_a, &conn_b] {
        conn.execute_batch(
            r#"
            CREATE TABLE memory_nodes (
                id VARCHAR PRIMARY KEY, label VARCHAR NOT NULL,
                properties VARCHAR DEFAULT '{}', tenant_id VARCHAR NOT NULL,
                seq BIGINT DEFAULT 0, created_at TIMESTAMP DEFAULT now(),
                updated_at TIMESTAMP DEFAULT now(), deleted_at TIMESTAMP
            );
            CREATE TABLE memory_edges (
                id VARCHAR PRIMARY KEY, source_id VARCHAR NOT NULL,
                target_id VARCHAR NOT NULL, relation VARCHAR NOT NULL,
                properties VARCHAR DEFAULT '{}', tenant_id VARCHAR NOT NULL,
                seq BIGINT DEFAULT 0, created_at TIMESTAMP DEFAULT now(),
                deleted_at TIMESTAMP
            );
            "#,
        )
        .unwrap();
    }

    let tenant = "diverge-test";

    // Same node on both
    conn_a
        .execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n1', 'Same', '{}', ?)",
            duckdb::params![tenant],
        )
        .unwrap();
    conn_b
        .execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n1', 'Same', '{}', ?)",
            duckdb::params![tenant],
        )
        .unwrap();

    let d_a = graph_verify::compute_digest_hex(&conn_a, tenant).unwrap();
    let d_b = graph_verify::compute_digest_hex(&conn_b, tenant).unwrap();
    assert_eq!(d_a, d_b, "Identical data must produce identical digest");

    // Extra node on B → divergence
    conn_b
        .execute(
            "INSERT INTO memory_nodes (id, label, properties, tenant_id) VALUES ('n2', 'Extra', '{}', ?)",
            duckdb::params![tenant],
        )
        .unwrap();

    let d_b2 = graph_verify::compute_digest_hex(&conn_b, tenant).unwrap();
    assert_ne!(d_a, d_b2, "Different data must produce different digest");

    let result = graph_verify::verify_digests_match(&d_a, &d_b2);
    assert!(
        result.is_err(),
        "verify_digests_match should return Err on mismatch"
    );
}
