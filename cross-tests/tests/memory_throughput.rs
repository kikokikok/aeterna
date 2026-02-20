use std::time::Instant;

use memory::matryoshka::{Dimension, MatryoshkaEmbedder};
use storage::shard_manager::ShardManager;
use storage::tenant_router::{TenantRouter, TenantSize};

struct MemoryThroughputConfig {
    embedding_batch_size: usize,
    embedding_dimensions: usize,
    shard_tenant_count: usize,
    target_ops_per_sec: u64,
}

impl Default for MemoryThroughputConfig {
    fn default() -> Self {
        Self {
            embedding_batch_size: 1000,
            embedding_dimensions: 1536,
            shard_tenant_count: 50,
            target_ops_per_sec: 10_000,
        }
    }
}

fn make_embedding(dim: usize) -> Vec<f32> {
    (0..dim).map(|i| (i as f32 + 1.0) * 0.01).collect()
}

#[test]
#[ignore] // Run explicitly: cargo test -p cross-tests --test load -- --ignored
fn load_embedding_truncation_throughput() {
    let config = MemoryThroughputConfig::default();
    let embedder = MatryoshkaEmbedder::with_defaults();
    let full = make_embedding(config.embedding_dimensions);

    let start = Instant::now();
    for _ in 0..config.embedding_batch_size {
        let _ = embedder
            .embed(&full, Dimension::D256)
            .expect("embed should succeed");
    }
    let elapsed = start.elapsed();

    let ops_per_sec = config.embedding_batch_size as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Embedding truncation throughput: {:.0} ops/sec ({} ops in {:.2?})",
        ops_per_sec, config.embedding_batch_size, elapsed
    );

    assert!(
        ops_per_sec > config.target_ops_per_sec as f64,
        "Throughput {ops_per_sec:.0} ops/sec below target {} ops/sec",
        config.target_ops_per_sec
    );
}

#[test]
#[ignore] // Run explicitly: cargo test -p cross-tests --test load -- --ignored
fn load_shard_routing_throughput() {
    let config = MemoryThroughputConfig::default();
    let mut manager = ShardManager::new();
    let router = TenantRouter::new();

    let start = Instant::now();
    for i in 0..config.shard_tenant_count {
        let tenant_id = format!("load-tenant-{i}");
        let size = if i % 10 == 0 {
            TenantSize::Large
        } else {
            TenantSize::Small
        };

        router.assign_shard(&tenant_id, size);

        if size == TenantSize::Large {
            manager.create_dedicated_shard(&tenant_id).unwrap();
            manager
                .activate_shard(&format!("dedicated-{tenant_id}"))
                .unwrap();
        } else {
            let _ = manager.increment_tenant_count("shared-shard-1");
        }
    }
    let elapsed = start.elapsed();

    let ops_per_sec = config.shard_tenant_count as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Shard routing throughput: {:.0} ops/sec ({} tenants in {:.2?})",
        ops_per_sec, config.shard_tenant_count, elapsed
    );

    let stats = manager.get_statistics();
    assert!(stats.total_tenants > 0);
    assert!(
        stats.total_shards > 1,
        "should have created dedicated shards"
    );

    let assignments = router.list_assignments();
    assert_eq!(assignments.len(), config.shard_tenant_count);
}

#[test]
#[ignore] // Run explicitly: cargo test -p cross-tests --test load -- --ignored
fn load_anomaly_detection_throughput() {
    use observability::{AnomalyDetector, AnomalyDetectorConfig};

    let detector = AnomalyDetector::new(AnomalyDetectorConfig {
        window_size: 200,
        stddev_threshold: 2.0,
        min_data_points: 10,
    });

    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let value = 100.0 + (i as f64 % 20.0);
        detector.record_and_detect("load_metric", value);
    }
    let elapsed = start.elapsed();

    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "Anomaly detection throughput: {ops_per_sec:.0} ops/sec ({iterations} ops in {elapsed:.2?})"
    );

    assert!(
        ops_per_sec > 50_000.0,
        "Anomaly detection too slow: {ops_per_sec:.0} ops/sec"
    );
}
