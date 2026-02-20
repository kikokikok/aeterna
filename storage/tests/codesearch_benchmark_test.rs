//! Performance benchmarks for Code Search Repository Management
//!
//! Uses `std::time::Instant` for timing. Thresholds are set for debug builds.

use async_trait::async_trait;
use errors::CodeSearchError;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use storage::policy_evaluator::{CachedPolicyEvaluator, PolicyContext, PolicyEvaluator};
use storage::repo_manager::{
    RepoRequest, RepoRequestStatus, Repository, RepositoryStatus, RepositoryType, SyncStrategy,
};
use storage::secret_provider::LocalSecretProvider;
use storage::secret_provider::SecretProvider;

const DEBUG_MULTIPLIER: f64 = 50.0;

struct BenchPolicyEvaluator;

#[async_trait]
impl PolicyEvaluator for BenchPolicyEvaluator {
    async fn evaluate_request(
        &self,
        _context: &PolicyContext,
        _action: &str,
        _repo: &Repository,
    ) -> Result<bool, CodeSearchError> {
        Ok(true)
    }

    async fn evaluate_approval(
        &self,
        _context: &PolicyContext,
        _request: &RepoRequest,
    ) -> Result<bool, CodeSearchError> {
        Ok(true)
    }
}

fn make_bench_repo(name: &str, status: RepositoryStatus) -> Repository {
    Repository {
        id: Uuid::new_v4(),
        tenant_id: "bench-tenant".to_string(),
        identity_id: None,
        name: name.to_string(),
        r#type: RepositoryType::Remote,
        remote_url: Some("https://github.com/bench/repo.git".to_string()),
        local_path: None,
        current_branch: "main".to_string(),
        tracked_branches: vec![],
        sync_strategy: SyncStrategy::Manual,
        sync_interval_mins: None,
        status,
        last_indexed_commit: None,
        last_indexed_at: None,
        last_used_at: None,
        owner_id: None,
        shard_id: None,
        cold_storage_uri: None,
        config: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn hash_key(key: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn bench_consistent_hashing_lookup() {
    let shard_count = 5;
    let virtual_nodes = 150;

    let mut ring: Vec<(u64, usize)> = Vec::with_capacity(shard_count * virtual_nodes);
    for shard in 0..shard_count {
        for vn in 0..virtual_nodes {
            ring.push((hash_key(&format!("shard-{}:{}", shard, vn)), shard));
        }
    }
    ring.sort_by_key(|(h, _)| *h);

    let iterations = 10_000;
    let repo_ids: Vec<String> = (0..iterations)
        .map(|_| Uuid::new_v4().to_string())
        .collect();

    let start = Instant::now();
    let mut assignments = Vec::with_capacity(iterations);
    for repo_id in &repo_ids {
        let h = hash_key(repo_id);
        let target = ring.iter().find(|(rh, _)| *rh >= h).unwrap_or(&ring[0]).1;
        assignments.push(target);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "consistent_hashing_lookup: {} iterations in {:?} (avg: {:.0}ns/op, {:.4}ms/op)",
        iterations, elapsed, avg_ns, avg_ms
    );

    assert!(
        avg_ms < 1.0 * DEBUG_MULTIPLIER,
        "Hash lookup avg {:.4}ms exceeded threshold",
        avg_ms
    );
    assert_eq!(assignments.len(), iterations);
}

#[test]
fn bench_consistent_hashing_ring_construction() {
    let shard_count = 10;
    let virtual_nodes = 150;
    let iterations = 100;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut ring: Vec<(u64, usize)> = Vec::with_capacity(shard_count * virtual_nodes);
        for shard in 0..shard_count {
            for vn in 0..virtual_nodes {
                ring.push((hash_key(&format!("shard-{}:{}", shard, vn)), shard));
            }
        }
        ring.sort_by_key(|(h, _)| *h);
        assert_eq!(ring.len(), shard_count * virtual_nodes);
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "ring_construction (10 shards, 150 vnodes): {} iterations in {:?} (avg: {:.3}ms/op)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 50.0 * DEBUG_MULTIPLIER,
        "Ring construction avg {:.3}ms exceeded threshold",
        avg_ms
    );
}

#[tokio::test]
async fn bench_policy_evaluation_throughput() {
    let evaluator = BenchPolicyEvaluator;
    let ctx = PolicyContext {
        principal_id: "bench-user".to_string(),
        principal_roles: vec!["developer".to_string()],
        tenant_id: "bench-tenant".to_string(),
    };
    let repo = make_bench_repo("bench-repo", RepositoryStatus::Ready);

    let iterations = 5_000;
    let start = Instant::now();
    for _ in 0..iterations {
        evaluator
            .evaluate_request(&ctx, "RequestRepository", &repo)
            .await
            .unwrap();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "policy_evaluation: {} iterations in {:?} (avg: {:.0}ns/op, {:.4}ms/op)",
        iterations, elapsed, avg_ns, avg_ms
    );

    assert!(
        avg_ms < 1.0 * DEBUG_MULTIPLIER,
        "Policy evaluation avg {:.4}ms exceeded threshold",
        avg_ms
    );
}

#[tokio::test]
async fn bench_cached_policy_evaluation() {
    let inner = Arc::new(BenchPolicyEvaluator);
    let cached = CachedPolicyEvaluator::new(inner, Duration::from_secs(300));

    let ctx = PolicyContext {
        principal_id: "bench-user".to_string(),
        principal_roles: vec!["developer".to_string()],
        tenant_id: "bench-tenant".to_string(),
    };
    let repo = make_bench_repo("bench-repo", RepositoryStatus::Ready);

    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        cached
            .evaluate_request(&ctx, "RequestRepository", &repo)
            .await
            .unwrap();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "cached_policy_evaluation: {} iterations in {:?} (avg: {:.0}ns/op, {:.4}ms/op)",
        iterations, elapsed, avg_ns, avg_ms
    );

    assert!(
        avg_ms < 0.5 * DEBUG_MULTIPLIER,
        "Cached policy avg {:.4}ms exceeded threshold",
        avg_ms
    );
}

#[tokio::test]
async fn bench_cached_policy_mixed_actions() {
    let inner = Arc::new(BenchPolicyEvaluator);
    let cached = CachedPolicyEvaluator::new(inner, Duration::from_secs(300));

    let actions = [
        "RequestRepository",
        "IndexRepository",
        "DeleteRepository",
        "CloneRepository",
        "SyncRepository",
    ];
    let repos: Vec<Repository> = (0..20)
        .map(|i| make_bench_repo(&format!("repo-{}", i), RepositoryStatus::Ready))
        .collect();
    let ctx = PolicyContext {
        principal_id: "bench-user".to_string(),
        principal_roles: vec!["developer".to_string(), "lead".to_string()],
        tenant_id: "bench-tenant".to_string(),
    };

    let iterations = 5_000;
    let start = Instant::now();
    for i in 0..iterations {
        let action = actions[i % actions.len()];
        let repo = &repos[i % repos.len()];
        cached.evaluate_request(&ctx, action, repo).await.unwrap();
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "cached_policy_mixed (5 actions × 20 repos): {} iterations in {:?} (avg: {:.4}ms/op)",
        iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 1.0 * DEBUG_MULTIPLIER,
        "Mixed cached policy avg {:.4}ms exceeded threshold",
        avg_ms
    );
}

#[tokio::test]
async fn bench_secret_provider_lookup() {
    let mut secrets = HashMap::new();
    for i in 0..100 {
        secrets.insert(format!("secret-{}", i), format!("value-{}", i));
    }
    let provider = LocalSecretProvider::new(secrets);

    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let key = format!("secret-{}", i % 100);
        let _ = provider.get_secret(&key).await;
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "secret_provider_lookup (100 keys): {} iterations in {:?} (avg: {:.0}ns/op, {:.4}ms/op)",
        iterations, elapsed, avg_ns, avg_ms
    );

    assert!(
        avg_ms < 0.5 * DEBUG_MULTIPLIER,
        "Secret lookup avg {:.4}ms exceeded threshold",
        avg_ms
    );
}

#[test]
fn bench_incremental_indexing_delta_simulation() {
    let total_files = 10_000;
    let changed_files = 150;

    let file_hashes: Vec<(String, u64)> = (0..total_files)
        .map(|i| {
            (
                format!("src/file_{}.rs", i),
                hash_key(&format!("content-v1-{}", i)),
            )
        })
        .collect();

    let mut updated_hashes = file_hashes.clone();
    for i in 0..changed_files {
        updated_hashes[i * (total_files / changed_files)].1 =
            hash_key(&format!("content-v2-{}", i));
    }

    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let mut changed = Vec::new();
        for (idx, (path, hash)) in updated_hashes.iter().enumerate() {
            if file_hashes[idx].1 != *hash {
                changed.push(path.clone());
            }
        }
        assert_eq!(changed.len(), changed_files);
    }
    let elapsed = start.elapsed();

    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "incremental_delta ({} files, {} changed): {} iterations in {:?} (avg: {:.3}ms/op)",
        total_files, changed_files, iterations, elapsed, avg_ms
    );

    assert!(
        avg_ms < 10.0 * DEBUG_MULTIPLIER,
        "Delta computation avg {:.3}ms exceeded threshold",
        avg_ms
    );
}

#[test]
fn bench_repository_status_transitions() {
    let transitions: Vec<(RepositoryStatus, RepositoryStatus)> = vec![
        (RepositoryStatus::Requested, RepositoryStatus::Pending),
        (RepositoryStatus::Pending, RepositoryStatus::Approved),
        (RepositoryStatus::Approved, RepositoryStatus::Cloning),
        (RepositoryStatus::Cloning, RepositoryStatus::Indexing),
        (RepositoryStatus::Indexing, RepositoryStatus::Ready),
    ];

    let iterations = 50_000;
    let start = Instant::now();
    for i in 0..iterations {
        let (from, to) = &transitions[i % transitions.len()];
        assert_ne!(from, to);
        let json_from = serde_json::to_string(from).unwrap();
        let _deserialized: RepositoryStatus = serde_json::from_str(&json_from).unwrap();
        let json_to = serde_json::to_string(to).unwrap();
        let _deserialized: RepositoryStatus = serde_json::from_str(&json_to).unwrap();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
    println!(
        "status_transitions (serialize+deserialize): {} iterations in {:?} (avg: {:.0}ns/op, {:.4}ms/op)",
        iterations, elapsed, avg_ns, avg_ms
    );

    assert!(
        avg_ms < 0.5 * DEBUG_MULTIPLIER,
        "Status transition avg {:.4}ms exceeded threshold",
        avg_ms
    );
}
