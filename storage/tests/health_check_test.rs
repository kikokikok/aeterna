use storage::graph_duckdb::{
    ComponentHealth, DuckDbGraphConfig, DuckDbGraphStore, HealthCheckResult, ReadinessResult,
};

#[test]
fn test_health_check_healthy() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.health_check();

    assert!(result.healthy, "Store should be healthy");
    assert!(result.duckdb.is_healthy, "DuckDB should be healthy");
    assert!(
        result.duckdb_latency_ms < 1000,
        "DuckDB latency should be reasonable"
    );
    assert!(result.schema_version >= 0, "Schema version should be set");
}

#[test]
fn test_health_check_s3_not_configured() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.health_check();

    assert!(
        result.s3.is_healthy,
        "S3 should be healthy when not configured"
    );
    assert!(
        result.s3.message.contains("not configured") || result.s3.message.contains("optional"),
        "Message should indicate S3 is optional"
    );
}

#[test]
fn test_health_check_with_s3_configured() {
    let config = DuckDbGraphConfig {
        s3_bucket: Some("test-bucket".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();

    let result = store.health_check();

    assert!(result.s3.is_healthy, "S3 config check should pass");
    assert!(result.s3.message.contains("test-bucket"));
}

#[test]
fn test_readiness_check_ready() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.readiness_check();

    assert!(result.ready, "Store should be ready");
    assert!(result.duckdb_ready, "DuckDB should be ready");
    assert!(result.schema_ready, "Schema should be ready");
    assert!(result.latency_ms < 1000, "Readiness check should be fast");
}

#[test]
fn test_health_check_latency_measurements() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.health_check();

    assert!(
        result.total_latency_ms >= result.duckdb_latency_ms,
        "Total latency should include DuckDB latency"
    );
}

#[test]
fn test_component_health_serialization() {
    let health = ComponentHealth {
        is_healthy: true,
        message: "All systems operational".to_string(),
    };

    let json = serde_json::to_string(&health).unwrap();
    let deserialized: ComponentHealth = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.is_healthy, health.is_healthy);
    assert_eq!(deserialized.message, health.message);
}

#[test]
fn test_health_check_result_serialization() {
    let result = HealthCheckResult {
        healthy: true,
        duckdb: ComponentHealth {
            is_healthy: true,
            message: "OK".to_string(),
        },
        s3: ComponentHealth {
            is_healthy: true,
            message: "OK".to_string(),
        },
        schema_version: 1,
        total_latency_ms: 10,
        duckdb_latency_ms: 5,
        s3_latency_ms: 3,
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: HealthCheckResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.healthy, result.healthy);
    assert_eq!(deserialized.schema_version, result.schema_version);
}

#[test]
fn test_readiness_result_serialization() {
    let result = ReadinessResult {
        ready: true,
        duckdb_ready: true,
        schema_ready: true,
        latency_ms: 5,
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: ReadinessResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.ready, result.ready);
    assert_eq!(deserialized.duckdb_ready, result.duckdb_ready);
}

#[tokio::test]
async fn test_s3_connectivity_not_configured() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.check_s3_connectivity().await;

    assert!(result.is_healthy);
    assert!(result.message.contains("not configured"));
}

#[test]
fn test_multiple_health_checks_consistent() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result1 = store.health_check();
    let result2 = store.health_check();

    assert_eq!(result1.healthy, result2.healthy);
    assert_eq!(result1.schema_version, result2.schema_version);
}

#[test]
fn test_readiness_after_initialization() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let result = store.readiness_check();

    assert!(result.ready, "Freshly initialized store should be ready");
    assert!(result.schema_ready, "Schema should be initialized");
}
