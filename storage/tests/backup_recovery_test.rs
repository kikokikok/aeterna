use storage::graph_duckdb::{
    BackupConfig, BackupResult, DuckDbGraphConfig, DuckDbGraphStore, GraphError, RecoveryResult,
    SnapshotMetadata
};

#[test]
fn test_backup_config_defaults() {
    let config = BackupConfig::default();
    assert_eq!(config.snapshot_interval_secs, 3600);
    assert_eq!(config.retention_count, 24);
    assert_eq!(config.retention_max_age_secs, 86400 * 7);
    assert!(!config.auto_backup_enabled);
    assert_eq!(config.backup_prefix, "backups");
}

#[test]
fn test_backup_config_custom() {
    let config = BackupConfig {
        snapshot_interval_secs: 1800,
        retention_count: 48,
        retention_max_age_secs: 86400 * 14,
        auto_backup_enabled: true,
        backup_prefix: "custom-backups".to_string()
    };
    assert_eq!(config.snapshot_interval_secs, 1800);
    assert_eq!(config.retention_count, 48);
    assert!(config.auto_backup_enabled);
}

#[test]
fn test_snapshot_metadata_serialization() {
    let metadata = SnapshotMetadata {
        snapshot_id: "snap-123".to_string(),
        tenant_id: "tenant-1".to_string(),
        s3_key: "backups/tenant-1/20240101_120000/snapshot_snap-123.parquet".to_string(),
        created_at: chrono::Utc::now(),
        size_bytes: 1024,
        checksum: "abc123".to_string(),
        node_count: 100,
        edge_count: 50,
        schema_version: 1
    };

    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: SnapshotMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.snapshot_id, metadata.snapshot_id);
    assert_eq!(deserialized.tenant_id, metadata.tenant_id);
    assert_eq!(deserialized.size_bytes, metadata.size_bytes);
    assert_eq!(deserialized.node_count, metadata.node_count);
}

#[test]
fn test_backup_result_fields() {
    let result = BackupResult {
        snapshot_id: "snap-456".to_string(),
        s3_key: "backups/tenant/snapshot.parquet".to_string(),
        size_bytes: 2048,
        duration_ms: 150,
        checksum: "def456".to_string()
    };

    assert_eq!(result.snapshot_id, "snap-456");
    assert_eq!(result.size_bytes, 2048);
    assert_eq!(result.duration_ms, 150);
}

#[test]
fn test_recovery_result_fields() {
    let result = RecoveryResult {
        snapshot_id: "snap-789".to_string(),
        nodes_restored: 100,
        edges_restored: 50,
        duration_ms: 200
    };

    assert_eq!(result.snapshot_id, "snap-789");
    assert_eq!(result.nodes_restored, 100);
    assert_eq!(result.edges_restored, 50);
}

#[tokio::test]
async fn test_create_backup_requires_s3_config() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let backup_config = BackupConfig::default();

    let result = store.create_backup("tenant-1", &backup_config).await;

    assert!(result.is_err(), "Expected S3 error");
    let err = result.unwrap_err();

    // Handle both boxed errors and direct GraphError
    let graph_err: &GraphError =
        if let Some(e) = (&err as &dyn std::any::Any).downcast_ref::<GraphError>() {
            e
        } else {
            panic!("Expected GraphError, got error of type: {:?}", err);
        };

    match graph_err {
        GraphError::S3(msg) => {
            assert!(msg.contains("S3 bucket not configured"));
        }
        _ => panic!("Expected S3 error, got {:?}", graph_err)
    }
}

#[tokio::test]
async fn test_list_snapshots_requires_s3_config() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let backup_config = BackupConfig::default();

    let result = store.list_snapshots("tenant-1", &backup_config).await;

    assert!(result.is_err(), "Expected S3 error");
    let err = result.unwrap_err();

    // Convert to Any to check type
    let any_err = &err as &dyn std::any::Any;

    if let Some(graph_err) = any_err.downcast_ref::<GraphError>() {
        // err is GraphError
        match graph_err {
            GraphError::S3(msg) => {
                assert!(msg.contains("S3 bucket not configured"));
            }
            _ => panic!("Expected S3 error, got {:?}", graph_err)
        }
    } else {
        // err might be Box<dyn Error>, try to downcast
        panic!("Expected GraphError, got error of type: {:?}", err);
    }
}

#[tokio::test]
async fn test_restore_from_snapshot_requires_s3_config() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let backup_config = BackupConfig::default();

    let result = store
        .restore_from_snapshot("tenant-1", "snap-123", &backup_config)
        .await;

    assert!(matches!(result, Err(GraphError::S3(_))));
}

#[tokio::test]
async fn test_apply_retention_policy_requires_s3_config() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();
    let backup_config = BackupConfig::default();

    let result = store
        .apply_retention_policy("tenant-1", &backup_config)
        .await;

    assert!(matches!(result, Err(GraphError::S3(_))));
}

#[tokio::test]
async fn test_create_backup_validates_tenant_id() {
    let config = DuckDbGraphConfig {
        s3_bucket: Some("test-bucket".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();
    let backup_config = BackupConfig::default();

    let result = store.create_backup("", &backup_config).await;
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));

    let result = store
        .create_backup("tenant'; DROP TABLE", &backup_config)
        .await;
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));
}

#[tokio::test]
async fn test_list_snapshots_validates_tenant_id() {
    let config = DuckDbGraphConfig {
        s3_bucket: Some("test-bucket".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();
    let backup_config = BackupConfig::default();

    let result = store.list_snapshots("", &backup_config).await;
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));
}

#[tokio::test]
async fn test_restore_validates_tenant_id() {
    let config = DuckDbGraphConfig {
        s3_bucket: Some("test-bucket".to_string()),
        ..Default::default()
    };
    let store = DuckDbGraphStore::new(config).unwrap();
    let backup_config = BackupConfig::default();

    let result = store
        .restore_from_snapshot("tenant--injection", "snap-1", &backup_config)
        .await;
    assert!(matches!(result, Err(GraphError::InvalidTenantIdFormat(_))));
}

#[test]
fn test_backup_config_retention_calculations() {
    let config = BackupConfig {
        snapshot_interval_secs: 3600,
        retention_count: 24,
        retention_max_age_secs: 86400,
        auto_backup_enabled: true,
        backup_prefix: "backups".to_string()
    };

    let expected_daily_snapshots = 86400 / config.snapshot_interval_secs;
    assert_eq!(expected_daily_snapshots, 24);
    assert!(config.retention_count >= expected_daily_snapshots as usize);
}
