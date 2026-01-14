//! MinIO/S3 integration tests for DuckDB graph store persistence
//!
//! These tests use testcontainers to spin up a MinIO instance for S3-compatible
//! storage testing. They are marked #[ignore] by default and require Docker.

use mk_core::types::{TenantContext, TenantId, UserId};
use std::time::Duration;
use storage::graph::{GraphEdge, GraphNode, GraphStore};
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore, GraphError};
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

const MINIO_ACCESS_KEY: &str = "minioadmin";
const MINIO_SECRET_KEY: &str = "minioadmin";
const TEST_BUCKET: &str = "aeterna-test";

fn test_tenant_context() -> TenantContext {
    let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
    let user_id = UserId::new("test-user".to_string()).unwrap();
    TenantContext::new(tenant_id, user_id)
}

async fn setup_minio_bucket(endpoint: &str) {
    use aws_config::BehaviorVersion;

    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", MINIO_ACCESS_KEY);
        std::env::set_var("AWS_SECRET_ACCESS_KEY", MINIO_SECRET_KEY);
    }

    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url(endpoint)
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;

    let s3_client = aws_sdk_s3::Client::new(&config);

    match s3_client.create_bucket().bucket(TEST_BUCKET).send().await {
        Ok(_) => {}
        Err(e) => {
            let err_str = format!("{:?}", e);
            if !err_str.contains("BucketAlreadyOwnedByYou")
                && !err_str.contains("BucketAlreadyExists")
            {
                panic!("Failed to create bucket: {:?}", e);
            }
        }
    }
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_persist_and_load_s3_roundtrip() {
    let minio = GenericImage::new("minio/minio", "latest")
        .with_exposed_port(9000.into())
        .with_env_var("MINIO_ROOT_USER", MINIO_ACCESS_KEY)
        .with_env_var("MINIO_ROOT_PASSWORD", MINIO_SECRET_KEY)
        .with_cmd(vec!["server", "/data"])
        .start()
        .await
        .expect("Failed to start MinIO");

    let host = minio.get_host().await.unwrap();
    let port = minio.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    setup_minio_bucket(&endpoint).await;

    let config = DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some("test-graphs".to_string()),
        s3_endpoint: Some(endpoint.clone()),
        s3_region: Some("us-east-1".to_string()),
        ..Default::default()
    };

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");
    let ctx = test_tenant_context();
    let tenant_id = ctx.tenant_id.as_str().to_string();

    for i in 1..=3 {
        let node = GraphNode {
            id: format!("node-{}", i),
            label: format!("TestNode-{}", i),
            properties: serde_json::json!({"index": i, "data": "test"}),
            tenant_id: tenant_id.clone(),
        };
        store.add_node(ctx.clone(), node).await.unwrap();
    }

    let edge = GraphEdge {
        id: "edge-1".to_string(),
        source_id: "node-1".to_string(),
        target_id: "node-2".to_string(),
        relation: "CONNECTS".to_string(),
        properties: serde_json::json!({"weight": 1.5}),
        tenant_id: tenant_id.clone(),
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
#[ignore = "requires Docker"]
async fn test_s3_checksum_verification() {
    let minio = GenericImage::new("minio/minio", "latest")
        .with_exposed_port(9000.into())
        .with_env_var("MINIO_ROOT_USER", MINIO_ACCESS_KEY)
        .with_env_var("MINIO_ROOT_PASSWORD", MINIO_SECRET_KEY)
        .with_cmd(vec!["server", "/data"])
        .start()
        .await
        .expect("Failed to start MinIO");

    let host = minio.get_host().await.unwrap();
    let port = minio.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    setup_minio_bucket(&endpoint).await;

    let config = DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some("checksum-test".to_string()),
        s3_endpoint: Some(endpoint),
        s3_region: Some("us-east-1".to_string()),
        ..Default::default()
    };

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");
    let ctx = test_tenant_context();
    let tenant_id = ctx.tenant_id.as_str().to_string();

    let node = GraphNode {
        id: "node-1".to_string(),
        label: "ChecksumTest".to_string(),
        properties: serde_json::Value::Null,
        tenant_id: tenant_id.clone(),
    };
    store.add_node(ctx, node).await.unwrap();

    let snapshot_key = store.persist_to_s3(&tenant_id).await.unwrap();

    let store2 = DuckDbGraphStore::new(config).expect("Failed to create second store");
    let result = store2.load_from_s3(&tenant_id, &snapshot_key).await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_s3_not_configured_error() {
    let store =
        DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("Failed to create store");

    let result = store.persist_to_s3("test-tenant").await;
    assert!(matches!(result, Err(GraphError::S3(_))));

    let result = store.load_from_s3("test-tenant", "some/key").await;
    assert!(matches!(result, Err(GraphError::S3(_))));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_multi_tenant_s3_isolation() {
    let minio = GenericImage::new("minio/minio", "latest")
        .with_exposed_port(9000.into())
        .with_env_var("MINIO_ROOT_USER", MINIO_ACCESS_KEY)
        .with_env_var("MINIO_ROOT_PASSWORD", MINIO_SECRET_KEY)
        .with_cmd(vec!["server", "/data"])
        .start()
        .await
        .expect("Failed to start MinIO");

    let host = minio.get_host().await.unwrap();
    let port = minio.get_host_port_ipv4(9000).await.unwrap();
    let endpoint = format!("http://{}:{}", host, port);

    tokio::time::sleep(Duration::from_secs(2)).await;

    setup_minio_bucket(&endpoint).await;

    let config = DuckDbGraphConfig {
        path: ":memory:".to_string(),
        s3_bucket: Some(TEST_BUCKET.to_string()),
        s3_prefix: Some("multi-tenant".to_string()),
        s3_endpoint: Some(endpoint),
        s3_region: Some("us-east-1".to_string()),
        ..Default::default()
    };

    let store = DuckDbGraphStore::new(config.clone()).expect("Failed to create store");

    let ctx1 = TenantContext::new(
        TenantId::new("tenant-1".to_string()).unwrap(),
        UserId::new("user-1".to_string()).unwrap(),
    );
    let ctx2 = TenantContext::new(
        TenantId::new("tenant-2".to_string()).unwrap(),
        UserId::new("user-2".to_string()).unwrap(),
    );

    let node1 = GraphNode {
        id: "tenant1-node".to_string(),
        label: "Tenant1Data".to_string(),
        properties: serde_json::json!({"secret": "tenant1-only"}),
        tenant_id: "tenant-1".to_string(),
    };
    store.add_node(ctx1.clone(), node1).await.unwrap();

    let node2 = GraphNode {
        id: "tenant2-node".to_string(),
        label: "Tenant2Data".to_string(),
        properties: serde_json::json!({"secret": "tenant2-only"}),
        tenant_id: "tenant-2".to_string(),
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
