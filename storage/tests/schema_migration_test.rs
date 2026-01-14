use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore, Migration, MigrationRecord};

#[test]
fn test_schema_version_tracked() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let version = store.get_current_schema_version().unwrap();

    assert!(
        version >= 1,
        "Schema version should be at least 1 after initialization"
    );
}

#[test]
fn test_migration_history_recorded() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let history = store.get_migration_history().unwrap();

    assert!(!history.is_empty(), "Migration history should not be empty");
    assert_eq!(history[0].version, 1, "First migration should be version 1");
}

#[test]
fn test_migration_history_ordered() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let history = store.get_migration_history().unwrap();

    for i in 1..history.len() {
        assert!(
            history[i].version > history[i - 1].version,
            "Migrations should be in ascending order"
        );
    }
}

#[test]
fn test_migration_idempotent() {
    let config = DuckDbGraphConfig::default();

    let store1 = DuckDbGraphStore::new(config.clone()).unwrap();
    let version1 = store1.get_current_schema_version().unwrap();
    drop(store1);

    let store2 = DuckDbGraphStore::new(config).unwrap();
    let version2 = store2.get_current_schema_version().unwrap();

    assert_eq!(
        version1, version2,
        "Re-running migrations should not change version"
    );
}

#[test]
fn test_migration_record_has_timestamp() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let history = store.get_migration_history().unwrap();

    for record in history {
        assert!(
            !record.applied_at.is_empty(),
            "Migration record should have applied_at timestamp"
        );
    }
}

#[test]
fn test_migration_record_has_description() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let history = store.get_migration_history().unwrap();

    for record in history {
        assert!(
            !record.description.is_empty(),
            "Migration record should have description"
        );
    }
}

#[test]
fn test_migration_struct_fields() {
    let migration = Migration {
        version: 2,
        description: "Add new column".to_string(),
        up_sql: vec!["ALTER TABLE test ADD COLUMN foo VARCHAR"],
        down_sql: vec!["ALTER TABLE test DROP COLUMN foo"],
    };

    assert_eq!(migration.version, 2);
    assert!(!migration.description.is_empty());
    assert_eq!(migration.up_sql.len(), 1);
    assert_eq!(migration.down_sql.len(), 1);
}

#[test]
fn test_migration_record_serialization() {
    let record = MigrationRecord {
        version: 1,
        applied_at: "2024-01-01T00:00:00".to_string(),
        description: "Initial migration".to_string(),
    };

    let json = serde_json::to_string(&record).unwrap();
    let deserialized: MigrationRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.version, record.version);
    assert_eq!(deserialized.applied_at, record.applied_at);
    assert_eq!(deserialized.description, record.description);
}

#[test]
fn test_health_check_reflects_schema_version() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let health = store.health_check();
    let schema_version = store.get_current_schema_version().unwrap();

    assert_eq!(
        health.schema_version, schema_version,
        "Health check should report current schema version"
    );
}

#[test]
fn test_readiness_requires_schema() {
    let store = DuckDbGraphStore::new(DuckDbGraphConfig::default()).unwrap();

    let readiness = store.readiness_check();

    assert!(
        readiness.schema_ready,
        "Schema should be ready after initialization"
    );
}
