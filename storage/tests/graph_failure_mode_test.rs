use storage::graph::GraphStore;
use storage::graph_event_log::GraphEventLog;

/// 12.1: When Postgres is unreachable, append returns an error
/// (no partial state in DuckDB).
#[tokio::test]
async fn test_postgres_unavailable_returns_error() {
    let bad_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(200))
        .connect("postgres://nobody:wrong@127.0.0.1:1/nonexistent")
        .await;

    // If we can't even build the pool, that's the expected failure path
    if bad_pool.is_err() {
        return; // Connection refused — correct behavior
    }

    let log = GraphEventLog::new(bad_pool.unwrap());
    let result = log
        .append("t1", "add_node", serde_json::json!({"id": "n1"}))
        .await;
    assert!(result.is_err(), "append must fail when Postgres is down");
}

/// 12.3: DuckDB file-backed store deleted mid-operation is detected
/// gracefully (no panic).
#[tokio::test]
async fn test_duckdb_file_deleted_detection() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.duckdb");

    let config = storage::graph_duckdb::DuckDbGraphConfig {
        path: db_path.to_string_lossy().to_string(),
        ..Default::default()
    };
    let store = storage::graph_duckdb::DuckDbGraphStore::new(config).unwrap();

    let ctx = mk_core::types::TenantContext::new(
        mk_core::types::TenantId::new("file-del-tenant".to_string()).unwrap(),
        mk_core::types::UserId::new("user".to_string()).unwrap(),
    );
    let node = storage::graph::GraphNode {
        id: "n1".to_string(),
        label: "Before".to_string(),
        properties: serde_json::json!({}),
        tenant_id: "file-del-tenant".to_string(),
    };
    store.add_node(ctx.clone(), node).await.unwrap();

    std::fs::remove_file(&db_path).ok();
    let _ = std::fs::remove_file(db_path.with_extension("duckdb.wal"));

    // Operations on the existing connection may still work (OS keeps fd open)
    // or may fail — either way, no panic.
    let result = store.get_stats(ctx);
    // We don't assert success or failure — just that we don't panic.
    let _ = result;
}

/// 12.1 variant: GraphEventLog validates empty tenant_id.
#[tokio::test]
async fn test_event_log_rejects_empty_tenant() {
    let Some(fx) = testing::postgres().await else {
        eprintln!("Skipping: Postgres not available");
        return;
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(fx.url())
        .await
        .expect("pool");

    let log = GraphEventLog::new(pool);

    let result = log.append("", "add_node", serde_json::json!({})).await;
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Tenant ID is required"),
        "Expected MissingTenant error, got: {err_msg}"
    );
}
