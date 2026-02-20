use serde::Deserialize;
use std::path::Path;

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

#[test]
fn ha_patroni_config_exists_and_has_synchronous_mode() {
    let path = project_root().join("infrastructure/ha/patroni/patroni.yml");
    assert!(
        path.exists(),
        "Patroni config must exist at {}",
        path.display()
    );

    let content = std::fs::read_to_string(&path).expect("failed to read patroni.yml");
    let doc: serde_yaml::Value = serde_yaml::from_str(&content).expect("invalid YAML");

    let sync_mode = doc
        .get("bootstrap")
        .and_then(|b| b.get("dcs"))
        .and_then(|d| d.get("synchronous_mode"))
        .and_then(serde_yaml::Value::as_bool);

    assert_eq!(
        sync_mode,
        Some(true),
        "synchronous_mode must be true for HA"
    );
}

#[test]
fn ha_qdrant_cluster_has_three_replicas_and_pdb() {
    let path = project_root().join("infrastructure/ha/qdrant/qdrant-cluster.yaml");
    assert!(path.exists(), "Qdrant cluster manifest must exist");

    let content = std::fs::read_to_string(&path).expect("failed to read qdrant-cluster.yaml");

    let mut found_replicas_3 = false;
    let mut found_pdb_min_2 = false;

    for doc in serde_yaml::Deserializer::from_str(&content) {
        let value = serde_yaml::Value::deserialize(doc).expect("invalid YAML doc");

        if let Some(kind) = value.get("kind").and_then(serde_yaml::Value::as_str) {
            if kind == "StatefulSet" {
                let replicas = value
                    .get("spec")
                    .and_then(|s| s.get("replicas"))
                    .and_then(serde_yaml::Value::as_u64);
                if replicas == Some(3) {
                    found_replicas_3 = true;
                }
            }
            if kind == "PodDisruptionBudget" {
                let min_available = value
                    .get("spec")
                    .and_then(|s| s.get("minAvailable"))
                    .and_then(serde_yaml::Value::as_u64);
                if min_available == Some(2) {
                    found_pdb_min_2 = true;
                }
            }
        }
    }

    assert!(found_replicas_3, "Qdrant StatefulSet must have 3 replicas");
    assert!(found_pdb_min_2, "Qdrant PDB must have minAvailable: 2");
}

#[test]
fn ha_redis_sentinel_has_three_replicas() {
    let path = project_root().join("infrastructure/ha/redis/redis-sentinel.yaml");
    assert!(path.exists(), "Redis sentinel manifest must exist");

    let content = std::fs::read_to_string(&path).expect("failed to read redis-sentinel.yaml");

    let mut sentinel_replicas_3 = false;
    let mut redis_replicas_3 = false;

    for doc in serde_yaml::Deserializer::from_str(&content) {
        let value = serde_yaml::Value::deserialize(doc).expect("invalid YAML doc");

        let kind = value
            .get("kind")
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or("");
        let name = value
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or("");

        if let Some(replicas) = value
            .get("spec")
            .and_then(|s| s.get("replicas"))
            .and_then(serde_yaml::Value::as_u64)
        {
            if replicas == 3 {
                if kind == "Deployment" && name.contains("sentinel") {
                    sentinel_replicas_3 = true;
                }
                if kind == "StatefulSet" && name.contains("redis") {
                    redis_replicas_3 = true;
                }
            }
        }
    }

    assert!(redis_replicas_3, "Redis StatefulSet must have 3 replicas");
    assert!(
        sentinel_replicas_3,
        "Sentinel Deployment must have 3 replicas"
    );
}

#[test]
fn ha_backup_restore_test_script_has_rto_limit() {
    let path = project_root().join("infrastructure/ha/backups/restore-test.sh");
    assert!(path.exists(), "Restore test script must exist");

    let content = std::fs::read_to_string(&path).expect("failed to read restore-test.sh");
    assert!(
        content.contains("RTO_LIMIT_SECONDS"),
        "restore-test.sh must define RTO_LIMIT_SECONDS"
    );
    assert!(
        content.contains("rto_met"),
        "restore-test.sh must report rto_met in its output"
    );
}

#[test]
fn ha_backup_scripts_exist_for_all_datastores() {
    let root = project_root().join("infrastructure/ha/backups");
    let required = ["backup-postgres.sh", "backup-qdrant.sh", "backup-redis.sh"];

    for script in &required {
        let path = root.join(script);
        assert!(path.exists(), "Backup script must exist: {}", script);

        let content = std::fs::read_to_string(&path).expect("failed to read backup script");
        assert!(!content.is_empty(), "{} must not be empty", script);
    }
}
