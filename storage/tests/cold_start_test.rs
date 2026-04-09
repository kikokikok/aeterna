use storage::graph_duckdb::{
    ColdStartConfig, DuckDbGraphConfig, DuckDbGraphStore, LazyLoadResult, PartitionAccessRecord,
    WarmPoolRecommendation,
};

fn create_store_with_cold_start(config: ColdStartConfig) -> DuckDbGraphStore {
    let mut graph_config = DuckDbGraphConfig::default();
    graph_config.cold_start = config;
    DuckDbGraphStore::new(graph_config).unwrap()
}

#[test]
fn test_cold_start_config_defaults() {
    let config = ColdStartConfig::default();

    assert!(config.lazy_loading_enabled);
    assert_eq!(config.budget_ms, 3000);
    assert!(config.access_tracking_enabled);
    assert_eq!(config.prewarm_partition_count, 5);
    assert!(!config.warm_pool_enabled);
    assert_eq!(config.warm_pool_min_instances, 1);
}

#[test]
fn test_partition_access_recording() {
    let store = create_store_with_cold_start(ColdStartConfig::default());

    store
        .record_partition_access("tenant-1", "partition-a", 50.0)
        .unwrap();
    store
        .record_partition_access("tenant-1", "partition-a", 30.0)
        .unwrap();
    store
        .record_partition_access("tenant-1", "partition-b", 100.0)
        .unwrap();

    let records = store.get_partition_access_records("tenant-1").unwrap();

    assert_eq!(records.len(), 2);

    let partition_a = records.iter().find(|r| r.partition_key == "partition-a");
    assert!(partition_a.is_some());
    let partition_a = partition_a.unwrap();
    assert_eq!(partition_a.access_count, 2);
    assert!((partition_a.avg_load_time_ms - 40.0).abs() < 0.1);
}

#[test]
fn test_partition_access_recording_disabled() {
    let mut config = ColdStartConfig::default();
    config.access_tracking_enabled = false;

    let store = create_store_with_cold_start(config);

    store
        .record_partition_access("tenant-1", "partition-a", 50.0)
        .unwrap();

    let records = store.get_partition_access_records("tenant-1").unwrap();
    assert!(records.is_empty());
}

#[test]
fn test_partition_access_tenant_isolation() {
    let store = create_store_with_cold_start(ColdStartConfig::default());

    store
        .record_partition_access("tenant-1", "partition-a", 50.0)
        .unwrap();
    store
        .record_partition_access("tenant-2", "partition-a", 100.0)
        .unwrap();

    let tenant1_records = store.get_partition_access_records("tenant-1").unwrap();
    let tenant2_records = store.get_partition_access_records("tenant-2").unwrap();

    assert_eq!(tenant1_records.len(), 1);
    assert_eq!(tenant2_records.len(), 1);
    assert!((tenant1_records[0].avg_load_time_ms - 50.0).abs() < 0.1);
    assert!((tenant2_records[0].avg_load_time_ms - 100.0).abs() < 0.1);
}

#[test]
fn test_get_prewarm_partitions() {
    let store = create_store_with_cold_start(ColdStartConfig::default());

    store
        .record_partition_access("tenant-1", "partition-a", 10.0)
        .unwrap();
    store
        .record_partition_access("tenant-1", "partition-b", 20.0)
        .unwrap();
    store
        .record_partition_access("tenant-1", "partition-c", 30.0)
        .unwrap();

    let prewarm = store.get_prewarm_partitions("tenant-1").unwrap();

    assert_eq!(prewarm.len(), 3);
    assert!(prewarm.contains(&"partition-a".to_string()));
    assert!(prewarm.contains(&"partition-b".to_string()));
    assert!(prewarm.contains(&"partition-c".to_string()));
}

#[test]
fn test_prewarm_partition_limit() {
    let mut config = ColdStartConfig::default();
    config.prewarm_partition_count = 2;

    let store = create_store_with_cold_start(config);

    for i in 0..5 {
        store
            .record_partition_access("tenant-1", &format!("partition-{}", i), 10.0)
            .unwrap();
    }

    let prewarm = store.get_prewarm_partitions("tenant-1").unwrap();
    assert_eq!(prewarm.len(), 2);
}

#[tokio::test]
async fn test_lazy_load_disabled() {
    let mut config = ColdStartConfig::default();
    config.lazy_loading_enabled = false;

    let store = create_store_with_cold_start(config);

    let result = store
        .lazy_load_partitions("tenant-1", &["partition-a".to_string()])
        .await
        .unwrap();

    assert_eq!(result.partitions_loaded, 0);
    assert_eq!(result.total_load_time_ms, 0);
    assert!(result.deferred_partitions.is_empty());
}

#[tokio::test]
async fn test_lazy_load_no_s3() {
    let store = create_store_with_cold_start(ColdStartConfig::default());

    let result = store
        .lazy_load_partitions(
            "tenant-1",
            &["partition-a".to_string(), "partition-b".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(result.partitions_loaded, 2);
    assert!(result.budget_remaining_ms > 0);
    assert!(result.deferred_partitions.is_empty());
}

#[test]
fn test_enforce_cold_start_budget_within() {
    let store = create_store_with_cold_start(ColdStartConfig::default());
    let start = std::time::Instant::now();

    let result = store.enforce_cold_start_budget(start);
    assert!(result.is_ok());
}

#[test]
fn test_get_cold_start_config() {
    let mut config = ColdStartConfig::default();
    config.budget_ms = 5000;

    let store = create_store_with_cold_start(config);

    assert_eq!(store.get_cold_start_config().budget_ms, 5000);
}

#[test]
fn test_warm_pool_recommendation_disabled() {
    let mut config = ColdStartConfig::default();
    config.warm_pool_enabled = false;

    let store = create_store_with_cold_start(config);
    let recommendation = store.get_warm_pool_recommendation();

    assert!(!recommendation.recommended);
    assert_eq!(recommendation.min_instances, 0);
}

#[test]
fn test_warm_pool_recommendation_enabled() {
    let mut config = ColdStartConfig::default();
    config.warm_pool_enabled = true;
    config.warm_pool_min_instances = 3;

    let store = create_store_with_cold_start(config);
    let recommendation = store.get_warm_pool_recommendation();

    assert!(recommendation.recommended);
    assert_eq!(recommendation.min_instances, 3);
}

#[test]
fn test_partition_access_validates_tenant_id() {
    let store = create_store_with_cold_start(ColdStartConfig::default());

    let result = store.record_partition_access("", "partition-a", 50.0);
    assert!(result.is_err());

    let result = store.record_partition_access("tenant'; DROP TABLE--", "partition-a", 50.0);
    assert!(result.is_err());
}

#[test]
fn test_lazy_load_result_struct() {
    let result = LazyLoadResult {
        partitions_loaded: 5,
        total_load_time_ms: 1500,
        budget_remaining_ms: 1500,
        deferred_partitions: vec!["p1".to_string(), "p2".to_string()],
    };

    assert_eq!(result.partitions_loaded, 5);
    assert_eq!(result.total_load_time_ms, 1500);
    assert_eq!(result.budget_remaining_ms, 1500);
    assert_eq!(result.deferred_partitions.len(), 2);
}

#[test]
fn test_partition_access_record_struct() {
    let record = PartitionAccessRecord {
        partition_key: "test-partition".to_string(),
        tenant_id: "tenant-1".to_string(),
        access_count: 10,
        last_access: chrono::Utc::now(),
        avg_load_time_ms: 45.5,
    };

    assert_eq!(record.partition_key, "test-partition");
    assert_eq!(record.tenant_id, "tenant-1");
    assert_eq!(record.access_count, 10);
    assert!((record.avg_load_time_ms - 45.5).abs() < 0.1);
}

#[test]
fn test_warm_pool_recommendation_struct() {
    let rec = WarmPoolRecommendation {
        recommended: true,
        min_instances: 5,
        reason: "High traffic".to_string(),
    };

    assert!(rec.recommended);
    assert_eq!(rec.min_instances, 5);
    assert_eq!(rec.reason, "High traffic");
}
