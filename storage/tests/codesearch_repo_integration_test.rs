//! Integration tests for Code Search Repository Management
//!
//! Tests cover: data type construction, policy evaluation caching,
//! secret provider behavior, consistent hashing properties, and
//! full DB-backed workflows (marked `#[ignore]` when requiring Postgres).

use async_trait::async_trait;
use chrono::Utc;
use errors::CodeSearchError;
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use storage::RepoStorage;
use storage::policy_evaluator::{CachedPolicyEvaluator, PolicyContext, PolicyEvaluator};
use storage::repo_manager::{
    CreateIdentity, CreateRepository, RepoManager, RepoRequest, RepoRequestStatus, Repository,
    RepositoryStatus, RepositoryType, SyncStrategy,
};
use storage::secret_provider::{LocalSecretProvider, SecretProvider};
use testing::postgres;

struct MockPolicyEvaluator {
    allow_all: bool,
}

impl MockPolicyEvaluator {
    fn allowing() -> Self {
        Self { allow_all: true }
    }

    #[allow(unused)]
    fn denying() -> Self {
        Self { allow_all: false }
    }
}

#[async_trait]
impl PolicyEvaluator for MockPolicyEvaluator {
    async fn evaluate_request(
        &self,
        _context: &PolicyContext,
        _action: &str,
        _repo: &Repository,
    ) -> Result<bool, CodeSearchError> {
        Ok(self.allow_all)
    }

    async fn evaluate_approval(
        &self,
        _context: &PolicyContext,
        _request: &RepoRequest,
    ) -> Result<bool, CodeSearchError> {
        Ok(self.allow_all)
    }
}

// ---------------------------------------------------------------------------
// Counting policy evaluator (tracks call count for cache tests)
// ---------------------------------------------------------------------------

struct CountingPolicyEvaluator {
    call_count: std::sync::atomic::AtomicU32,
    allow: bool,
}

impl CountingPolicyEvaluator {
    fn new(allow: bool) -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU32::new(0),
            allow,
        }
    }

    fn calls(&self) -> u32 {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl PolicyEvaluator for CountingPolicyEvaluator {
    async fn evaluate_request(
        &self,
        _context: &PolicyContext,
        _action: &str,
        _repo: &Repository,
    ) -> Result<bool, CodeSearchError> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self.allow)
    }

    async fn evaluate_approval(
        &self,
        _context: &PolicyContext,
        _request: &RepoRequest,
    ) -> Result<bool, CodeSearchError> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self.allow)
    }
}

// ---------------------------------------------------------------------------
// Helper: build a dummy Repository for policy testing
// ---------------------------------------------------------------------------

fn make_test_repo(name: &str, repo_type: RepositoryType, status: RepositoryStatus) -> Repository {
    Repository {
        id: Uuid::new_v4(),
        tenant_id: "test-tenant".to_string(),
        identity_id: None,
        name: name.to_string(),
        r#type: repo_type,
        remote_url: Some("https://github.com/test/repo.git".to_string()),
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
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_test_request(repo_id: Uuid) -> RepoRequest {
    RepoRequest {
        id: Uuid::new_v4(),
        repository_id: repo_id,
        requester_id: "user-1".to_string(),
        status: RepoRequestStatus::Requested,
        policy_result: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn make_policy_context(principal: &str, roles: Vec<&str>) -> PolicyContext {
    PolicyContext {
        principal_id: principal.to_string(),
        principal_roles: roles.into_iter().map(|r| r.to_string()).collect(),
        tenant_id: "test-tenant".to_string(),
    }
}

// ===========================================================================
// 1. Repository Status enum & State Machine properties
// ===========================================================================

#[test]
fn test_repository_status_variants() {
    let states = vec![
        RepositoryStatus::Requested,
        RepositoryStatus::Pending,
        RepositoryStatus::Approved,
        RepositoryStatus::Cloning,
        RepositoryStatus::Indexing,
        RepositoryStatus::Ready,
        RepositoryStatus::Error,
    ];
    for (i, a) in states.iter().enumerate() {
        for (j, b) in states.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn test_repository_type_variants() {
    assert_ne!(RepositoryType::Local, RepositoryType::Remote);
    assert_ne!(RepositoryType::Local, RepositoryType::Hybrid);
    assert_ne!(RepositoryType::Remote, RepositoryType::Hybrid);
}

#[test]
fn test_sync_strategy_variants() {
    assert_ne!(SyncStrategy::Hook, SyncStrategy::Job);
    assert_ne!(SyncStrategy::Hook, SyncStrategy::Manual);
    assert_ne!(SyncStrategy::Job, SyncStrategy::Manual);
}

#[test]
fn test_repo_request_status_variants() {
    let variants = vec![
        RepoRequestStatus::Requested,
        RepoRequestStatus::Pending,
        RepoRequestStatus::Approved,
        RepoRequestStatus::Rejected,
    ];
    for (i, a) in variants.iter().enumerate() {
        for (j, b) in variants.iter().enumerate() {
            assert_eq!(i == j, a == b);
        }
    }
}

// ===========================================================================
// 2. CreateRepository defaults
// ===========================================================================

#[test]
fn test_create_repository_struct_defaults() {
    let cr = CreateRepository {
        tenant_id: "t1".to_string(),
        identity_id: None,
        name: "my-repo".to_string(),
        r#type: RepositoryType::Local,
        remote_url: None,
        local_path: Some("/tmp/repo".to_string()),
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };
    assert_eq!(cr.tenant_id, "t1");
    assert!(cr.remote_url.is_none());
    assert!(cr.current_branch.is_none());
}

// ===========================================================================
// 3. LocalSecretProvider
// ===========================================================================

#[tokio::test]
async fn test_local_secret_provider_get_existing() {
    let mut secrets = HashMap::new();
    secrets.insert("gh-token".to_string(), "ghp_abc123".to_string());
    let provider = LocalSecretProvider::new(secrets);

    let result = provider.get_secret("gh-token").await;
    assert_eq!(result.unwrap(), "ghp_abc123");
}

#[tokio::test]
async fn test_local_secret_provider_get_missing() {
    let provider = LocalSecretProvider::new(HashMap::new());
    let result = provider.get_secret("does-not-exist").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_local_secret_provider_is_available() {
    let provider = LocalSecretProvider::new(HashMap::new());
    assert!(provider.is_available().await);
}

// ===========================================================================
// 4. MockPolicyEvaluator (allow / deny)
// ===========================================================================

#[tokio::test]
async fn test_mock_policy_evaluator_allow() {
    let evaluator = MockPolicyEvaluator::allowing();
    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Requested);

    assert!(
        evaluator
            .evaluate_request(&ctx, "RequestRepository", &repo)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_mock_policy_evaluator_deny() {
    let evaluator = MockPolicyEvaluator::denying();
    let ctx = make_policy_context("alice", vec!["viewer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Requested);

    assert!(
        !evaluator
            .evaluate_request(&ctx, "RequestRepository", &repo)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_mock_policy_evaluator_approval_allow() {
    let evaluator = MockPolicyEvaluator::allowing();
    let ctx = make_policy_context("admin", vec!["lead"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Requested);
    let request = make_test_request(repo.id);

    assert!(evaluator.evaluate_approval(&ctx, &request).await.unwrap());
}

#[tokio::test]
async fn test_mock_policy_evaluator_approval_deny() {
    let evaluator = MockPolicyEvaluator::denying();
    let ctx = make_policy_context("dev", vec!["viewer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Requested);
    let request = make_test_request(repo.id);

    assert!(!evaluator.evaluate_approval(&ctx, &request).await.unwrap());
}

// ===========================================================================
// 5. CachedPolicyEvaluator – caching behavior
// ===========================================================================

#[tokio::test]
async fn test_cached_evaluator_caches_results() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_secs(60));

    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Ready);

    let result = cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert!(result);
    assert_eq!(inner.calls(), 1);

    let result2 = cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert!(result2);
    assert_eq!(inner.calls(), 1);
}

#[tokio::test]
async fn test_cached_evaluator_different_actions_not_shared() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_secs(60));

    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Ready);

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    cached
        .evaluate_request(&ctx, "IndexRepository", &repo)
        .await
        .unwrap();

    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn test_cached_evaluator_invalidate_clears_cache() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_secs(60));

    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Ready);

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert_eq!(inner.calls(), 1);

    cached.invalidate();

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn test_cached_evaluator_ttl_expiry() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_millis(50));

    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Ready);

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert_eq!(inner.calls(), 1);

    tokio::time::sleep(Duration::from_millis(100)).await;

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn test_cached_evaluator_evict_expired() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_millis(10));

    let ctx = make_policy_context("alice", vec!["developer"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Ready);

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    cached.evict_expired();

    cached
        .evaluate_request(&ctx, "RequestRepository", &repo)
        .await
        .unwrap();
    assert_eq!(inner.calls(), 2);
}

#[tokio::test]
async fn test_cached_evaluator_approval_caching() {
    let inner = Arc::new(CountingPolicyEvaluator::new(true));
    let cached = CachedPolicyEvaluator::new(inner.clone(), Duration::from_secs(60));

    let ctx = make_policy_context("admin", vec!["lead"]);
    let repo = make_test_repo("r1", RepositoryType::Remote, RepositoryStatus::Requested);
    let request = make_test_request(repo.id);

    cached.evaluate_approval(&ctx, &request).await.unwrap();
    cached.evaluate_approval(&ctx, &request).await.unwrap();

    assert_eq!(inner.calls(), 1);
}

// ===========================================================================
// 6. Consistent Hashing properties (DefaultHasher)
// ===========================================================================

fn hash_key(key: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_hash_determinism() {
    let key = "shard-1:42";
    let h1 = hash_key(key);
    let h2 = hash_key(key);
    assert_eq!(h1, h2, "Same key must always produce the same hash");
}

#[test]
fn test_hash_distribution_across_virtual_nodes() {
    let virtual_nodes = 150;
    let mut hashes = HashSet::new();
    for i in 0..virtual_nodes {
        let key = format!("shard-A:{}", i);
        hashes.insert(hash_key(&key));
    }
    assert_eq!(hashes.len(), virtual_nodes);
}

#[test]
fn test_consistent_hashing_assignment_stability() {
    let shards = vec!["shard-1", "shard-2", "shard-3"];
    let virtual_nodes = 150;

    let mut ring: Vec<(u64, &str)> = Vec::new();
    for shard in &shards {
        for i in 0..virtual_nodes {
            let key = format!("{}:{}", shard, i);
            ring.push((hash_key(&key), shard));
        }
    }
    ring.sort_by_key(|(h, _)| *h);

    let repo_id = Uuid::new_v4();
    let repo_hash = hash_key(&repo_id.to_string());

    let target1 = ring
        .iter()
        .find(|(h, _)| *h >= repo_hash)
        .unwrap_or(&ring[0])
        .1;
    let target2 = ring
        .iter()
        .find(|(h, _)| *h >= repo_hash)
        .unwrap_or(&ring[0])
        .1;

    assert_eq!(target1, target2, "Same repo should route to same shard");
}

#[test]
fn test_consistent_hashing_even_distribution() {
    let shards = vec!["shard-A", "shard-B", "shard-C"];
    let virtual_nodes = 150;

    let mut ring: Vec<(u64, &str)> = Vec::new();
    for shard in &shards {
        for i in 0..virtual_nodes {
            let key = format!("{}:{}", shard, i);
            ring.push((hash_key(&key), shard));
        }
    }
    ring.sort_by_key(|(h, _)| *h);

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for _ in 0..3000 {
        let repo_hash = hash_key(&Uuid::new_v4().to_string());
        let target = ring
            .iter()
            .find(|(h, _)| *h >= repo_hash)
            .unwrap_or(&ring[0])
            .1;
        *counts.entry(target).or_default() += 1;
    }

    // Each shard should get roughly 1000 repos (1/3 of 3000), ±40% tolerance
    for (shard, count) in &counts {
        assert!(
            *count > 400 && *count < 1600,
            "Shard {} got {} repos, expected ~1000 (±40%)",
            shard,
            count
        );
    }
}

#[test]
fn test_consistent_hashing_minimal_disruption_on_node_add() {
    let shards_before = vec!["shard-1", "shard-2", "shard-3"];
    let shards_after = vec!["shard-1", "shard-2", "shard-3", "shard-4"];
    let virtual_nodes = 150;

    fn build_ring<'a>(shards: &[&'a str], virtual_nodes: usize) -> Vec<(u64, &'a str)> {
        let mut ring = Vec::new();
        for shard in shards {
            for i in 0..virtual_nodes {
                ring.push((hash_key(&format!("{}:{}", shard, i)), *shard));
            }
        }
        ring.sort_by_key(|(h, _)| *h);
        ring
    }

    let ring_before = build_ring(&shards_before, virtual_nodes);
    let ring_after = build_ring(&shards_after, virtual_nodes);

    fn assign<'a>(ring: &[(u64, &'a str)], key: &str) -> &'a str {
        let h = hash_key(key);
        ring.iter().find(|(rh, _)| *rh >= h).unwrap_or(&ring[0]).1
    }

    let test_count = 1000;
    let mut changed = 0;
    for i in 0..test_count {
        let key = format!("repo-{}", i);
        let before = assign(&ring_before, &key);
        let after = assign(&ring_after, &key);
        if before != after {
            changed += 1;
        }
    }

    let disruption_ratio = changed as f64 / test_count as f64;
    assert!(
        disruption_ratio < 0.40,
        "Adding a shard disrupted {:.1}% of keys (expected <40%)",
        disruption_ratio * 100.0
    );
}

// ===========================================================================
// 7. Repository construction helper tests (make_test_repo)
// ===========================================================================

#[test]
fn test_make_test_repo_local() {
    let repo = make_test_repo("local-proj", RepositoryType::Local, RepositoryStatus::Ready);
    assert_eq!(repo.name, "local-proj");
    assert_eq!(repo.r#type, RepositoryType::Local);
    assert_eq!(repo.status, RepositoryStatus::Ready);
    assert_eq!(repo.current_branch, "main");
}

#[test]
fn test_make_test_repo_remote() {
    let repo = make_test_repo(
        "remote-proj",
        RepositoryType::Remote,
        RepositoryStatus::Requested,
    );
    assert_eq!(repo.r#type, RepositoryType::Remote);
    assert_eq!(repo.status, RepositoryStatus::Requested);
    assert!(repo.remote_url.is_some());
}

// ===========================================================================
// 8. CodeSearchError variant construction
// ===========================================================================

#[test]
fn test_codesearch_error_variants() {
    let err1 = CodeSearchError::RepoNotFound {
        name: "foo".to_string(),
    };
    assert!(format!("{}", err1).contains("foo"));

    let err2 = CodeSearchError::PolicyViolation {
        policy: "Cedar".to_string(),
        reason: "denied".to_string(),
    };
    assert!(format!("{}", err2).contains("Cedar"));
    assert!(format!("{}", err2).contains("denied"));

    let err3 = CodeSearchError::GitError {
        reason: "clone failed".to_string(),
    };
    assert!(format!("{}", err3).contains("clone failed"));

    let err4 = CodeSearchError::IndexingFailed {
        repo: "my-repo".to_string(),
        reason: "timeout".to_string(),
    };
    assert!(format!("{}", err4).contains("my-repo"));
    assert!(format!("{}", err4).contains("timeout"));

    let err5 = CodeSearchError::DatabaseError {
        reason: "connection lost".to_string(),
    };
    assert!(format!("{}", err5).contains("connection lost"));
}

// ===========================================================================
// 9. PolicyContext construction
// ===========================================================================

#[test]
fn test_policy_context_construction() {
    let ctx = PolicyContext {
        principal_id: "user-alice".to_string(),
        principal_roles: vec!["developer".to_string(), "lead".to_string()],
        tenant_id: "acme".to_string(),
    };
    assert_eq!(ctx.principal_id, "user-alice");
    assert_eq!(ctx.principal_roles.len(), 2);
    assert_eq!(ctx.tenant_id, "acme");
}

// ===========================================================================
// 10. CreateIdentity struct
// ===========================================================================

#[test]
fn test_create_identity_struct() {
    let ci = CreateIdentity {
        tenant_id: "tenant-1".to_string(),
        name: "github-bot".to_string(),
        provider: "github".to_string(),
        auth_type: "token".to_string(),
        secret_id: "gh-token-1".to_string(),
        secret_provider: "local".to_string(),
        scopes: Some(vec!["repo".to_string(), "read:org".to_string()]),
    };
    assert_eq!(ci.provider, "github");
    assert_eq!(ci.scopes.as_ref().unwrap().len(), 2);
}

#[test]
fn test_create_identity_no_scopes() {
    let ci = CreateIdentity {
        tenant_id: "tenant-1".to_string(),
        name: "basic-bot".to_string(),
        provider: "gitlab".to_string(),
        auth_type: "token".to_string(),
        secret_id: "gl-token".to_string(),
        secret_provider: "vault".to_string(),
        scopes: None,
    };
    assert!(ci.scopes.is_none());
}

// ===========================================================================
// 11. RepoManager construction (requires PgPool but tests builder pattern)
//     These tests use #[ignore] since they need a live Postgres.
// ===========================================================================

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_request_local_repository_auto_approves() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::allowing());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let repo_data = CreateRepository {
        tenant_id: "test-tenant".to_string(),
        identity_id: None,
        name: format!("test-local-{}", Uuid::new_v4()),
        r#type: RepositoryType::Local,
        remote_url: None,
        local_path: Some("/tmp/nonexistent-path".to_string()),
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };

    let result = manager
        .request_repository(
            "test-tenant",
            "user-1",
            vec!["developer".to_string()],
            repo_data,
        )
        .await;

    assert!(result.is_ok());
    let repo = result.unwrap();
    assert_eq!(repo.status, RepositoryStatus::Approved);
}

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_request_remote_repository_creates_request() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::allowing());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let repo_data = CreateRepository {
        tenant_id: "test-tenant".to_string(),
        identity_id: None,
        name: format!("test-remote-{}", Uuid::new_v4()),
        r#type: RepositoryType::Remote,
        remote_url: Some("https://github.com/test/repo.git".to_string()),
        local_path: None,
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };

    let result = manager
        .request_repository(
            "test-tenant",
            "user-1",
            vec!["developer".to_string()],
            repo_data,
        )
        .await;

    assert!(result.is_ok());
    let repo = result.unwrap();
    assert!(repo.status == RepositoryStatus::Requested || repo.status == RepositoryStatus::Pending);
}

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_request_repository_policy_denied() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::denying());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let repo_data = CreateRepository {
        tenant_id: "test-tenant".to_string(),
        identity_id: None,
        name: format!("test-denied-{}", Uuid::new_v4()),
        r#type: RepositoryType::Remote,
        remote_url: Some("https://github.com/test/repo.git".to_string()),
        local_path: None,
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };

    let result = manager
        .request_repository(
            "test-tenant",
            "user-1",
            vec!["viewer".to_string()],
            repo_data,
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CodeSearchError::PolicyViolation { policy, .. } => {
            assert!(policy.contains("Cedar"));
        }
        other => panic!("Expected PolicyViolation, got: {:?}", other),
    }
}

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_duplicate_repository_rejected() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::allowing());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let name = format!("test-dup-{}", Uuid::new_v4());
    let mk_data = || CreateRepository {
        tenant_id: "test-tenant".to_string(),
        identity_id: None,
        name: name.clone(),
        r#type: RepositoryType::Local,
        remote_url: None,
        local_path: Some("/tmp/dup-test".to_string()),
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };

    let r1 = manager
        .request_repository(
            "test-tenant",
            "user-1",
            vec!["developer".to_string()],
            mk_data(),
        )
        .await;
    assert!(r1.is_ok());

    let r2 = manager
        .request_repository(
            "test-tenant",
            "user-1",
            vec!["developer".to_string()],
            mk_data(),
        )
        .await;
    assert!(r2.is_err());
}

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_list_repositories() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::allowing());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let tenant = format!("test-list-{}", Uuid::new_v4());
    let repo_data = CreateRepository {
        tenant_id: tenant.clone(),
        identity_id: None,
        name: format!("list-repo-{}", Uuid::new_v4()),
        r#type: RepositoryType::Local,
        remote_url: None,
        local_path: Some("/tmp/list-test".to_string()),
        current_branch: None,
        tracked_branches: None,
        sync_strategy: None,
        sync_interval_mins: None,
        config: None,
    };

    manager
        .request_repository(&tenant, "user-1", vec!["developer".to_string()], repo_data)
        .await
        .unwrap();

    let repos = manager.list_repositories(&tenant).await.unwrap();
    assert!(!repos.is_empty());
}

#[tokio::test]
#[ignore = "requires live Postgres"]
async fn test_create_and_retrieve_identity() {
    let fixture = postgres().await.expect("Postgres pool required for this test");
    let pool = Pool::<Postgres>::connect(fixture.url()).await.expect("Failed to connect to Postgres");
    let storage = RepoStorage::new(pool.clone());
    let secret_provider: Arc<dyn SecretProvider> =
        Arc::new(LocalSecretProvider::new(HashMap::new()));
    let policy: Arc<dyn PolicyEvaluator> = Arc::new(MockPolicyEvaluator::allowing());

    let manager = RepoManager::new(
        storage,
        std::path::PathBuf::from("/tmp/aeterna-test"),
        secret_provider,
        policy,
    );

    let identity = manager
        .create_identity(CreateIdentity {
            tenant_id: "test-tenant".to_string(),
            name: format!("bot-{}", Uuid::new_v4()),
            provider: "github".to_string(),
            auth_type: "token".to_string(),
            secret_id: "gh-token".to_string(),
            secret_provider: "local".to_string(),
            scopes: Some(vec!["repo".to_string()]),
        })
        .await
        .unwrap();

    assert_eq!(identity.provider, "github");
    assert_eq!(identity.auth_type, "token");
}

// ===========================================================================
// 12. Serialization round-trip tests
// ===========================================================================

#[test]
fn test_repository_type_serialization() {
    let json = serde_json::to_string(&RepositoryType::Remote).unwrap();
    let deserialized: RepositoryType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, RepositoryType::Remote);
}

#[test]
fn test_sync_strategy_serialization() {
    let json = serde_json::to_string(&SyncStrategy::Hook).unwrap();
    let deserialized: SyncStrategy = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, SyncStrategy::Hook);
}

#[test]
fn test_repository_status_serialization() {
    for status in [
        RepositoryStatus::Requested,
        RepositoryStatus::Pending,
        RepositoryStatus::Approved,
        RepositoryStatus::Cloning,
        RepositoryStatus::Indexing,
        RepositoryStatus::Ready,
        RepositoryStatus::Error,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: RepositoryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}

#[test]
fn test_repo_request_status_serialization() {
    for status in [
        RepoRequestStatus::Requested,
        RepoRequestStatus::Pending,
        RepoRequestStatus::Approved,
        RepoRequestStatus::Rejected,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: RepoRequestStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }
}
