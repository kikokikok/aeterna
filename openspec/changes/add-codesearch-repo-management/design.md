# Design: Enhanced Code Search Repository Management

## Architecture Overview

The enhanced Code Search repository management system extends Phase 1 by adding a governance layer, a lifecycle manager, and an advanced sync engine.

### Components

1.  **Repository Manager**: Manages the `codesearch_repositories` table and coordinate git operations.
2.  **Request Flow Engine**: Implements the 7-state approval machine.
3.  **Policy Evaluator (Cedar)**: Evaluates access and indexing requests against organizational policies.
4.  **Policy Distributor (OPAL)**: Synchronizes Cedar policies from a central git repository to local agents.
5.  **Incremental Indexer**: Uses git diff/GraphQL to identify changed files and update vectors.
6.  **Lifecycle Manager**: Tracks usage metrics and performs automatic cleanup.
7.  **Sync Scheduler (Job Strategy)**: Background worker for periodic delta checks.
8.  **Webhook Handler (Hook Strategy)**: Endpoint for receiving PR merge events from GitHub/GitLab.
9.  **Identity Provider**: Manages Git credentials and identity mapping.
10. **Secret Provider Abstraction**: Interface for AWS Secrets Manager, Vault, etc.

## Secret and Identity Management

### Component: `SecretProvider`
An abstraction layer to fetch sensitive tokens from external cloud providers or HashiCorp Vault.

```rust
#[async_trait]
pub trait SecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError>;
}
```

- **GitHub App Support**: Automatic token rotation using App installation IDs and private keys stored in KMS.
- **PAT Support**: Personal Access Tokens retrieved from Secret Manager.

### Component: `IdentityManager`
Maps a `codesearch_repository` to a specific `codesearch_identity`. Before indexing, the system verifies:
1.  Existence of the identity.
2.  Retrieval of the secret from the configured `SecretProvider`.
3.  Verification of "READ" permissions on the target repository using the retrieved token.

The repository lifecycle follows these states:
1.  **REQUESTED**: User or agent has requested a new repo.
2.  **PENDING**: Policy evaluation in progress or manual approval required.
3.  **APPROVED**: Request authorized. Ready for initial setup.
4.  **CLONING**: Git clone in progress (for remote/hybrid).
5.  **INDEXING**: Initial full index in progress.
6.  **READY**: Index is active and receiving incremental updates.
7.  **ERROR**: Any failure state (clone failed, index failed, policy denied).

## Reindexing Strategies

| Strategy | Trigger | Use Case |
|----------|---------|----------|
| `hook` | GitHub/GitLab Webhook (onMerge) | Immediate updates for active repositories. High precision. |
| `job` | Periodic background task (e.g., every 15m) | Repositories without webhook access. Batch updates. |
| `manual` | CLI or API call (`aeterna codesearch update`) | On-demand indexing for feature branches or testing. |

## Data Model

### Table: `codesearch_repositories`
- `id` (UUID): Primary Key
- `name` (String): Unique name
- `type` (Enum): `local`, `remote`, `hybrid`
- `url` (String): Remote repository URL
- `current_branch` (String): Active branch
- `sync_strategy` (Enum): `hook`, `job`, `manual`
- `sync_interval` (Interval): Frequency for `job` strategy
- `status` (Enum): State machine status
- `last_indexed_commit` (String): SHA of last index
- `owner_id` (String): Detected GitHub owner/CODEOWNER

### Table: `codesearch_requests`
- `id` (UUID)
- `repository_id` (UUID)
- `requester_id` (String)
- `status` (Enum): `requested`, `pending`, `approved`, `rejected`
- `policy_result` (JSONB): Details of Cedar evaluation
- `created_at` (Timestamp)

### Table: `codesearch_usage_metrics`
- `repository_id` (UUID)
- `branch` (String)
- `search_count` (Integer)
- `trace_count` (Integer)
- `last_active_at` (Timestamp)
- `period_start` (Timestamp)

## Policy Examples (Cedar)

```cedar
// Allow developers to index any repository in their department
permit(
    principal in Role::"developer",
    action == Action::"RequestIndexing",
    resource in Department::"Backend"
);

// Require manual approval for repositories > 1GB
forbid(
    principal,
    action == Action::"AutoApprove",
    resource
)
when { resource.size_bytes > 1073741824 };
```

## Workflows

### 1. Request Flow (A2A/CLI/MCP)
- Request arrives via tool or command.
- Evaluator checks Cedar policies.
- If policy allows → state moves to `APPROVED`.
- If policy requires manual check → state moves to `PENDING`.

### 2. Incremental Indexing (Hook Strategy)
- Webhook receives `push` or `pull_request.merged`.
- Extracted delta files via GitHub/GitLab API.
- Incremental indexer updates vectors only for modified files.
- Metadata table updated with new commit SHA.

### 3. Periodic Sync (Job Strategy)
- Scheduler picks a repository.
- Performs `git remote update`.
- Checks if `HEAD` > `last_indexed_commit`.
- Triggers incremental indexing if delta found.
