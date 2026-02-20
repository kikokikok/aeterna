# Code Search Repository Management — Developer Guide

## Overview

The Code Search Repository Management system provides governed lifecycle management for code repositories within Aeterna's multi-tenant architecture. It supports local, remote, and hybrid repositories with incremental indexing, policy-gated access, and distributed shard routing.

**Key modules:** `storage::repo_manager`, `storage::shard_router`, `storage::policy_evaluator`, `storage::secret_provider`

## Architecture

```
┌──────────────┐     ┌───────────────────┐     ┌──────────────────┐
│  MCP / CLI   │────▶│   RepoManager     │────▶│   RepoStorage    │
│  (request)   │     │  (business logic) │     │  (PostgreSQL)    │
└──────────────┘     └──┬───────┬────┬───┘     └──────────────────┘
                        │       │    │
              ┌─────────┘  ┌────┘    └────────┐
              ▼            ▼                   ▼
     ┌────────────┐  ┌───────────┐    ┌──────────────┐
     │  Policy    │  │  Secret   │    │ ShardRouter  │
     │ Evaluator  │  │ Provider  │    │ (consistent  │
     │ (Cedar)    │  │ (Vault)   │    │  hashing)    │
     └────────────┘  └───────────┘    └──────────────┘
```

## Repository State Machine

```
Requested ──▶ Pending ──▶ Approved ──▶ Cloning ──▶ Indexing ──▶ Ready
    │             │            │           │            │
    └─────────────┴────────────┴───────────┴────────────┘
                         ▼ (on failure)
                       Error
```

| Transition | Trigger |
|---|---|
| Requested → Approved | Local repo auto-approval |
| Requested → Pending | Remote repo awaiting policy evaluation |
| Pending → Approved | Policy evaluator grants access |
| Pending → Rejected | Policy evaluator denies access |
| Approved → Cloning | Git clone initiated |
| Cloning → Indexing | Clone complete, indexing starts |
| Indexing → Ready | Index built successfully |
| Any → Error | Operation failure (retryable) |

## Repository Types

| Type | Description | Auto-approve | Sync Strategy |
|---|---|---|---|
| `Local` | Developer's filesystem | Yes (if path exists → Ready) | `watch` (fsnotify) |
| `Remote` | Git URL, cloned on demand | No (policy-gated) | `poll` or `hook` |
| `Hybrid` | Local clone of remote | Configurable | `job` (periodic pull) |

## Core API

### RepoManager

```rust
use storage::repo_manager::{RepoManager, CreateRepository, RepositoryType};

let manager = RepoManager::new(storage, base_path, secret_provider, policy_evaluator);

// Request a repository (policy-gated)
let repo = manager.request_repository(
    "tenant-id", "user-id", vec!["developer".into()],
    CreateRepository {
        tenant_id: "tenant-id".into(),
        name: "my-repo".into(),
        r#type: RepositoryType::Remote,
        remote_url: Some("https://github.com/org/repo.git".into()),
        ..Default::default()
    }
).await?;

// List repositories for tenant
let repos = manager.list_repositories("tenant-id").await?;

// Identity management for git credentials
let identity = manager.create_identity(CreateIdentity { ... }).await?;
```

### PolicyEvaluator

```rust
use storage::policy_evaluator::{PolicyEvaluator, CachedPolicyEvaluator, PolicyContext};

let ctx = PolicyContext {
    principal_id: "user-alice".into(),
    principal_roles: vec!["developer".into()],
    tenant_id: "acme".into(),
};

// Direct evaluation
let allowed = evaluator.evaluate_request(&ctx, "RequestRepository", &repo).await?;

// Cached evaluation (TTL-based)
let cached = CachedPolicyEvaluator::new(inner_evaluator, Duration::from_secs(300));
cached.invalidate();       // Clear all cached results
cached.evict_expired();    // Remove only expired entries
```

### ShardRouter

```rust
use storage::shard_router::ShardRouter;

let router = ShardRouter::new("local-shard-id", 150); // 150 virtual nodes
router.register_shard("shard-1", "http://shard-1:8080");
router.heartbeat("shard-1");

let target = router.get_shard_for_repo(&repo_id)?;
let is_mine = router.is_local(&repo_id)?;

// Graceful shutdown
router.drain_shard("shard-1");
router.rebalance_from_shard("shard-1");
```

### SecretProvider

```rust
use storage::secret_provider::{SecretProvider, LocalSecretProvider};

let provider = LocalSecretProvider::new(HashMap::from([
    ("gh-token".into(), "ghp_abc123".into()),
]));

let secret = provider.get_secret("gh-token").await?;
let available = provider.is_available().await; // true
```

## Incremental Indexing

Delta-based indexing compares commit hashes to determine changed files:

1. **Git diff**: Compare `last_indexed_commit` to `HEAD` for precise file deltas
2. **File-system watch**: fsnotify events for local repos (sub-second latency)
3. **Webhook trigger**: GitHub/GitLab push events for remote repos

Only changed files are re-indexed, reducing a full 10,000-file reindex (15-60 min) to seconds for typical commits.

## Multi-Tenant Isolation

Every repository is scoped to a `tenant_id`. The policy evaluator enforces Cedar-based RBAC:

- **Developers** can request repositories
- **Tech Leads** can approve requests
- **Admins** have full control
- **Agents** inherit permissions from the delegating user

## Distributed Indexing

The `ShardRouter` uses consistent hashing with virtual nodes to distribute repositories across indexer pods:

- Default: 150 virtual nodes per shard for even distribution
- Heartbeat-based health monitoring
- Graceful drain and rebalance on scale events
- `ColdStorageManager` for S3-based backup/restore via git bundles

## Error Types

All operations return `errors::CodeSearchError`:

| Variant | When |
|---|---|
| `RepoNotFound { name }` | Repository lookup fails |
| `PolicyViolation { policy, reason }` | Cedar policy denies action |
| `GitError { reason }` | Clone/fetch/diff failure |
| `IndexingFailed { repo, reason }` | Index build error |
| `DatabaseError { reason }` | PostgreSQL operation failure |

## Testing

```bash
# Run unit tests (no external dependencies)
cargo test -p storage codesearch

# Run integration tests (requires Postgres)
cargo test -p storage codesearch -- --ignored

# Run benchmarks
cargo test -p storage codesearch_benchmark
```

Test coverage includes: state machine properties, policy caching behavior, consistent hashing distribution and stability, serialization round-trips, and full DB-backed workflows.
