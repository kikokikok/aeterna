use mk_core::types::{TenantContext, TenantId, UserId};
use storage::graph::{GraphEdge, GraphNode, GraphStore};
use storage::graph_duckdb::{ColdStartConfig, DuckDbGraphConfig, DuckDbGraphStore, GraphError};
use testing::minio;

const TEST_BUCKET: &str = "aeterna-test";

fn test_tenant_context() -> TenantContext {
    let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
    let user_id = UserId::new("test-user".to_string()).unwrap();
    TenantContext::new(tenant_id, user_id)
}

fn make_config(endpoint: &str, prefix: &str) -> DuckDbGraphConfig {
    DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some(prefix.to_string()),
        s3_endpoint: Some(endpoint.to_string()),
        s3_region: Some("us-east-1".to_string()),
        s3_force_path_style: true,
        ..Default::default()
    }
}

fn make_config_with_cold_start(
    endpoint: &str,
    prefix: &str,
    cold_start: ColdStartConfig
) -> DuckDbGraphConfig {
    DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some(prefix.to_string()),
        s3_endpoint: Some(endpoint.to_string()),
        s3_region: Some("us-east-1".to_string()),
        s3_force_path_style: true,
        cold_start,
        ..Default::default()
    }
}

#[tokio::test]
async fn test_persist_and_load_s3_roundtrip() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let config = make_config(minio_fixture.endpoint(), "test-graphs");

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");
    let ctx = test_tenant_context();
    let tenant_id = ctx.tenant_id.as_str().to_string();

    for i in 1..=3 {
        let node = GraphNode {
            id: format!("node-{}", i),
            label: format!("TestNode-{}", i),
            properties: serde_json::json!({"index": i, "data": "test"}),
            tenant_id: tenant_id.clone()
        };
        store.add_node(ctx.clone(), node).await.unwrap();
    }

    let edge = GraphEdge {
        id: "edge-1".to_string(),
        source_id: "node-1".to_string(),
        target_id: "node-2".to_string(),
        relation: "CONNECTS".to_string(),
        properties: serde_json::json!({"weight": 1.5}),
        tenant_id: tenant_id.clone()
    };
    store.add_edge(ctx.clone(), edge).await.unwrap();

    let snapshot_key = store.persist_to_s3(&tenant_id).await.unwrap();
    assert!(snapshot_key.contains("test-graphs"));
    assert!(snapshot_key.contains(&tenant_id));
    assert!(snapshot_key.ends_with(".parquet"));

    let store2 = DuckDbGraphStore::new(config).expect("Failed to create second store");

    store2
        .load_from_s3(&tenant_id, &snapshot_key)
        .await
        .unwrap();

    let stats = store2.get_stats(ctx.clone()).unwrap();
    assert_eq!(stats.node_count, 3);
    assert_eq!(stats.edge_count, 1);

    let neighbors = store2.get_neighbors(ctx, "node-1").await.unwrap();
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].1.id, "node-2");
}

#[tokio::test]
async fn test_s3_checksum_verification() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let config = make_config(minio_fixture.endpoint(), "checksum-test");

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");
    let ctx = test_tenant_context();
    let tenant_id = ctx.tenant_id.as_str().to_string();

    let node = GraphNode {
        id: "node-1".to_string(),
        label: "ChecksumTest".to_string(),
        properties: serde_json::Value::Null,
        tenant_id: tenant_id.clone()
    };
    store.add_node(ctx, node).await.unwrap();

    let snapshot_key = store.persist_to_s3(&tenant_id).await.unwrap();

    let store2 = DuckDbGraphStore::new(config).expect("Failed to create second store");
    let result = store2.load_from_s3(&tenant_id, &snapshot_key).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_s3_not_configured_error() {
    let store =
        DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("Failed to create store");

    let result = store.persist_to_s3("test-tenant").await;
    assert!(matches!(result, Err(GraphError::S3(_))));

    let result = store.load_from_s3("test-tenant", "some/key").await;
    assert!(matches!(result, Err(GraphError::S3(_))));
}

#[tokio::test]
async fn test_multi_tenant_s3_isolation() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let config = make_config(minio_fixture.endpoint(), "multi-tenant");

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");

    let ctx1 = TenantContext::new(
        TenantId::new("tenant-1".to_string()).unwrap(),
        UserId::new("user-1".to_string()).unwrap()
    );
    let ctx2 = TenantContext::new(
        TenantId::new("tenant-2".to_string()).unwrap(),
        UserId::new("user-2".to_string()).unwrap()
    );

    let node1 = GraphNode {
        id: "tenant1-node".to_string(),
        label: "Tenant1Data".to_string(),
        properties: serde_json::json!({"secret": "tenant1-only"}),
        tenant_id: "tenant-1".to_string()
    };
    store.add_node(ctx1.clone(), node1).await.unwrap();

    let node2 = GraphNode {
        id: "tenant2-node".to_string(),
        label: "Tenant2Data".to_string(),
        properties: serde_json::json!({"secret": "tenant2-only"}),
        tenant_id: "tenant-2".to_string()
    };
    store.add_node(ctx2.clone(), node2).await.unwrap();

    let snapshot1 = store.persist_to_s3("tenant-1").await.unwrap();
    let snapshot2 = store.persist_to_s3("tenant-2").await.unwrap();

    assert!(snapshot1.contains("tenant-1"));
    assert!(snapshot2.contains("tenant-2"));
    assert_ne!(snapshot1, snapshot2);

    let store2 = DuckDbGraphStore::new(config).expect("Failed to create new store");
    store2.load_from_s3("tenant-1", &snapshot1).await.unwrap();

    let stats1 = store2.get_stats(ctx1).unwrap();
    assert_eq!(stats1.node_count, 1);

    let stats2 = store2.get_stats(ctx2).unwrap();
    assert_eq!(stats2.node_count, 0);
}

#[tokio::test]
async fn test_s3_partition_fetch_error_trigger() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let cold_start = ColdStartConfig {
        lazy_loading_enabled: true,
        budget_ms: 5000,
        access_tracking_enabled: true,
        prewarm_partition_count: 5,
        warm_pool_enabled: false,
        warm_pool_min_instances: 0
    };
    let config =
        make_config_with_cold_start(minio_fixture.endpoint(), "partition-error-test", cold_start);

    let store = DuckDbGraphStore::new(config).expect("Failed to create store");

    let partition_keys = vec!["partition-1".to_string(), "partition-2".to_string()];
    let result = store
        .lazy_load_partitions("TRIGGER_S3_PARTITION_ERROR", &partition_keys)
        .await;

    assert!(
        result.is_ok(),
        "lazy_load_partitions should not fail entirely"
    );
    let load_result = result.unwrap();

    assert_eq!(
        load_result.partitions_loaded, 0,
        "No partitions should be successfully loaded"
    );
    assert_eq!(
        load_result.deferred_partitions.len(),
        2,
        "Both partitions should be deferred"
    );
    assert!(
        load_result
            .deferred_partitions
            .contains(&"partition-1".to_string())
    );
    assert!(
        load_result
            .deferred_partitions
            .contains(&"partition-2".to_string())
    );
}

#[tokio::test]
async fn test_s3_partition_not_found_graceful_handling() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let cold_start = ColdStartConfig {
        lazy_loading_enabled: true,
        budget_ms: 5000,
        access_tracking_enabled: true,
        prewarm_partition_count: 5,
        warm_pool_enabled: false,
        warm_pool_min_instances: 0
    };
    let config =
        make_config_with_cold_start(minio_fixture.endpoint(), "not-found-test", cold_start);

    let store = DuckDbGraphStore::new(config).expect("Failed to create store");

    let partition_keys = vec![
        "nonexistent-partition-1".to_string(),
        "nonexistent-partition-2".to_string(),
    ];
    let result = store
        .lazy_load_partitions("valid-tenant-id", &partition_keys)
        .await;

    assert!(
        result.is_ok(),
        "lazy_load_partitions should handle missing partitions gracefully: {:?}",
        result.err()
    );
    let load_result = result.unwrap();

    assert_eq!(
        load_result.partitions_loaded, 2,
        "Missing partitions should be counted as loaded (NoSuchKey returns Ok)"
    );
    assert!(
        load_result.deferred_partitions.is_empty(),
        "No partitions should be deferred for missing keys"
    );
}

#[tokio::test]
async fn test_s3_partition_budget_exhaustion_defers_remaining() {
    let Some(minio_fixture) = minio().await else {
        eprintln!("Skipping MinIO test: Docker not available");
        return;
    };
    let cold_start = ColdStartConfig {
        lazy_loading_enabled: true,
        budget_ms: 1,
        access_tracking_enabled: true,
        prewarm_partition_count: 5,
        warm_pool_enabled: false,
        warm_pool_min_instances: 0
    };
    let config = make_config_with_cold_start(minio_fixture.endpoint(), "budget-test", cold_start);

    let store = DuckDbGraphStore::new(config).expect("Failed to create store");

    let partition_keys: Vec<String> = (1..=10).map(|i| format!("partition-{}", i)).collect();
    let result = store
        .lazy_load_partitions("budget-test-tenant", &partition_keys)
        .await;

    assert!(result.is_ok(), "lazy_load_partitions should succeed");
    let load_result = result.unwrap();

    assert!(
        load_result.deferred_partitions.len() > 0 || load_result.partitions_loaded > 0,
        "Either some partitions loaded or some were deferred"
    );

    assert_eq!(
        load_result.partitions_loaded + load_result.deferred_partitions.len(),
        10,
        "Sum of loaded and deferred should equal requested partitions"
    );

    assert!(
        load_result.budget_remaining_ms < load_result.total_load_time_ms
            || load_result.budget_remaining_ms == 0,
        "Budget should be consumed or exceeded"
    );
}

#[tokio::test]
async fn test_s3_lazy_loading_disabled_skips_all() {
    let cold_start = ColdStartConfig {
        lazy_loading_enabled: false,
        budget_ms: 5000,
        access_tracking_enabled: false,
        prewarm_partition_count: 0,
        warm_pool_enabled: false,
        warm_pool_min_instances: 0
    };
    let config = DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some("disabled-test".to_string()),
        s3_endpoint: Some("http://localhost:9000".to_string()),
        s3_region: Some("us-east-1".to_string()),
        s3_force_path_style: true,
        cold_start,
        ..Default::default()
    };

    let store = DuckDbGraphStore::new(config).expect("Failed to create store");

    let partition_keys = vec!["partition-1".to_string(), "partition-2".to_string()];
    let result = store
        .lazy_load_partitions("any-tenant", &partition_keys)
        .await;

    assert!(
        result.is_ok(),
        "Should succeed when lazy loading is disabled"
    );
    let load_result = result.unwrap();

    assert_eq!(load_result.partitions_loaded, 0);
    assert!(load_result.deferred_partitions.is_empty());
}
