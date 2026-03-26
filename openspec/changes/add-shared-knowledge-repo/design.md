## Context

The `GitRepository` knowledge backend (`knowledge/src/repository.rs`) uses a local-only git repo at a configurable path (default `./knowledge-repo`, overridden to `/tmp/knowledge-repo` in production). With Kubernetes HPA scaling Aeterna to 2+ replicas, each pod has an isolated filesystem. Writes on pod A are invisible to pod B, causing 404s on reads routed to the "wrong" pod. This breaks 5/79 Newman E2E assertions in CRUD lifecycle tests.

Beyond multi-replica consistency, knowledge governance requires controlled change management. Status promotions (Draft to Accepted), cross-layer writes (Team, Org, Company), and knowledge lifecycle transitions must be reviewable and auditable. A PR-based workflow maps naturally to Git and provides the governance gate the existing `GovernanceEngine` expects but does not yet enforce at the storage layer.

### Stakeholders
- Aeterna server runtime (all replicas)
- Knowledge governance engine (`knowledge/src/governance.rs`)
- Sync bridge (`sync/src/bridge.rs`) — CommitMismatch trigger
- MCP knowledge tools (`tools/src/knowledge.rs`) — proposal storage
- Helm chart / deployment configuration
- CI/CD pipeline (GitHub Actions image rebuild)
- Kyriba deployment values (private env)

## Goals / Non-Goals

**Goals:**
- All Aeterna replicas share a single source of truth for knowledge entries via a remote GitHub repository
- Two-track write model: fast direct-push for Project-layer drafts, PR-based governance for everything else
- PR lifecycle maps to GovernanceEvents: opened → RequestCreated, merged → RequestApproved, closed → RequestRejected
- Webhook endpoint receives GitHub PR merge notifications for immediate sync trigger
- Provider abstraction (`GitProvider` trait) supports GitHub now, GitLab later
- SSH key authentication for private remote repos (reuse existing deploy key pattern)
- Graceful fallback: local-only mode when no remote URL is configured

**Non-Goals:**
- Real-time push notifications between replicas (webhook + pull-on-read is sufficient)
- GitLab MR support in this change (trait is designed for it, implementation deferred)
- Branch-per-tenant strategies (single branch, tenant isolation via directory structure as today)
- Full code review UI in Aeterna (reviews happen in GitHub)
- Automated PR auto-merge (human or bot reviewer must explicitly merge)

## Decisions

### Decision 1: Two-Track Write Architecture

**Choice**: Knowledge writes are routed through one of two tracks based on layer and status:

**Fast Track** (direct push to main):
- Condition: `entry.layer == Project AND entry.status == Draft`
- Flow: write file → commit locally → push to origin/main
- Rationale: Project-layer drafts are low-risk, high-frequency. Requiring PRs for every scratch note would cripple developer productivity.

**Governance Track** (branch + PR):
- Condition: everything else — status transitions to Accepted, promotions to Team/Org/Company layers, any non-Draft write to non-Project layers
- Flow: create branch → write file → commit to branch → open PR → await merge → sync
- Rationale: Higher-layer writes and status promotions affect multiple teams/projects and require review.

**Routing logic** (pseudocode):
```rust
fn requires_governance(entry: &KnowledgeEntry, operation: WriteOperation) -> bool {
    match operation {
        WriteOperation::Create | WriteOperation::Update => {
            entry.layer != KnowledgeLayer::Project || entry.status != KnowledgeStatus::Draft
        }
        WriteOperation::StatusChange { to } => {
            to != KnowledgeStatus::Draft  // Any non-Draft status change needs review
        }
        WriteOperation::Promote { .. } => true,  // All promotions need review
        WriteOperation::Delete => true,           // All deletes need review
    }
}
```

**Alternatives considered:**
- **All writes through PRs**: Too slow for Project-layer drafts. Developers would bypass the system entirely.
- **All writes direct-push**: No governance. Defeats the purpose of the knowledge management hierarchy.
- **Configurable per-tenant**: Over-engineered for now. Can be added later by making the routing predicate configurable.

### Decision 2: PR Lifecycle → GovernanceEvent Mapping

**Choice**: Map GitHub PR state transitions to existing `GovernanceEvent` variants:

| GitHub Event | GovernanceEvent | Knowledge Effect |
|---|---|---|
| PR opened | `RequestCreated` | Entry visible in proposals list |
| PR merged | `RequestApproved` | Entry committed to main, status updated |
| PR closed (not merged) | `RequestRejected` | Branch cleaned up, no main changes |
| PR review requested | (no event) | GitHub handles reviewer assignment |
| PR review approved | (no event) | Informational only, merge is the gate |

The webhook handler parses `pull_request` events and emits the corresponding `GovernanceEvent` via the existing `EventPublisher`. This also fixes the current gap where `approve_proposal` and `reject_proposal` in `api.rs` do NOT emit events.

**Rationale**: The existing governance engine already defines these event types but nothing emits them at the storage layer. PR lifecycle is the natural source of these events.

### Decision 3: `GitProvider` Trait (GitHub First, Abstract Later)

**Choice**: Define a `GitProvider` async trait that encapsulates all Git hosting operations:

```rust
#[async_trait]
pub trait GitProvider: Send + Sync {
    async fn create_branch(&self, name: &str, from_sha: &str) -> Result<()>;
    async fn commit_to_branch(&self, branch: &str, path: &str, content: &[u8], message: &str) -> Result<String>;
    async fn create_pull_request(&self, title: &str, body: &str, head: &str, base: &str) -> Result<PullRequestInfo>;
    async fn merge_pull_request(&self, pr_number: u64, merge_method: MergeMethod) -> Result<String>;
    async fn list_open_prs(&self, head_prefix: Option<&str>) -> Result<Vec<PullRequestInfo>>;
    async fn parse_webhook(&self, headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent>;
    async fn get_default_branch_sha(&self) -> Result<String>;
}
```

Implement `GitHubProvider` using the `octocrab` crate. The trait enables GitLab MR support later without modifying the repository or governance code.

**Alternatives considered:**
- **GitHub-only, no trait**: Simpler initially but creates coupling that makes GitLab support a rewrite.
- **Generic git operations only (no hosting API)**: Would miss PR lifecycle, review assignment, webhook parsing — the core governance features.

### Decision 4: `octocrab` for GitHub API

**Choice**: Use the `octocrab` crate (MIT, well-maintained, 700+ stars) for all GitHub API interactions:

- **Branch creation**: `octocrab.repos(owner, repo).create_ref(&Reference::Branch(name), sha)`
- **File commit to branch**: `octocrab.repos(owner, repo).create_file(path, msg, content).branch(name).send()`
- **PR creation**: `octocrab.pulls(owner, repo).create(title, head, base).body(desc).send()`
- **PR merge**: `octocrab.pulls(owner, repo).merge(pr_num).sha(&head_sha).method(MergeMethod::Squash).send()`
- **Webhook parsing**: `WebhookEvent::try_from_header_and_body(header, body)` — validates HMAC signature

**Alternatives considered:**
- **Raw reqwest + GitHub REST API**: More code, no type safety, manual pagination. `octocrab` already handles all of this.
- **`hubcaps` crate**: Unmaintained since 2021.

### Decision 5: Webhook Endpoint for PR Merge Detection

**Choice**: Add `POST /api/v1/webhooks/github` endpoint to the Aeterna Axum router. This endpoint:

1. Validates the `X-Hub-Signature-256` HMAC using a shared webhook secret
2. Parses the `X-GitHub-Event` header and request body via `octocrab::models::webhook_events`
3. On `pull_request.closed` where `merged == true`:
   - Emits `GovernanceEvent::RequestApproved` with PR metadata
   - Triggers immediate `git pull` on the local knowledge repo clone
   - Notifies the sync bridge of a `CommitMismatch` (new HEAD != cached HEAD)
4. On `pull_request.closed` where `merged == false`:
   - Emits `GovernanceEvent::RequestRejected`
   - Cleans up the local branch reference
5. On `pull_request.opened`:
   - Emits `GovernanceEvent::RequestCreated`

The webhook secret is injected via `AETERNA_WEBHOOK_SECRET` env var (from K8s secret). The endpoint is exposed through the existing ingress — GitHub sends to `https://aeterna.ci-dev-04.dev.kyriba.io/api/v1/webhooks/github`.

**Alternatives considered:**
- **Polling**: Simpler but adds latency (30s-5min depending on interval). Webhook provides immediate notification.
- **GitHub Actions workflow_dispatch callback**: Would require a separate workflow and more infrastructure. Webhook is the standard GitHub pattern.

### Decision 6: Branch Naming Convention

**Choice**: Governance track branches follow the pattern `governance/<verb>-<slug>-<yyyymmdd>`:

- `governance/promote-api-auth-pattern-20260326`
- `governance/accept-security-baseline-20260326`
- `governance/create-team-coding-standards-20260326`

Verbs match the operation: `create`, `update`, `delete`, `promote`, `accept`, `deprecate`.

**Rationale**: The `governance/` prefix keeps branches organized and filterable. Date suffix prevents name collisions for repeated operations on the same entry. The pattern is predictable enough for idempotency checks (list PRs by head prefix).

### Decision 7: Idempotency for PR Creation

**Choice**: Before creating a PR, check if one already exists for the same governance operation:

1. List open PRs where `head` starts with the expected branch name (minus date suffix for broader matching)
2. If a matching PR exists and is open: return its info without creating a duplicate
3. If a matching PR exists but was merged/closed: create a new one (new date suffix)

**Rationale**: Network failures, pod restarts, or retries could cause duplicate PR creation attempts. Idempotency prevents cluttering the GitHub repo with duplicate governance PRs.

### Decision 8: SSH Key Auth + Clone-on-Init

**Choice** (retained from original design, still applies):

- SSH private key content injected via `AETERNA_KNOWLEDGE_REPO_SSH_KEY` env var (from K8s secret)
- Authentication uses `git2::Cred::ssh_key_from_memory()` — no key file written to disk
- On first start (empty `/tmp/knowledge-repo`): `git clone` from remote
- On restart with cached data: `git pull` to catch up

Three new env vars:
- `AETERNA_KNOWLEDGE_REPO_URL`: SSH URL (e.g., `git@github.com:kyriba-eng/aeterna-knowledge.git`)
- `AETERNA_KNOWLEDGE_REPO_BRANCH`: Branch name (default: `main`)
- `AETERNA_KNOWLEDGE_REPO_SSH_KEY`: SSH private key content

When `AETERNA_KNOWLEDGE_REPO_URL` is unset, the repository operates in local-only mode (current behavior, no remote operations, no governance track).

### Decision 9: Tokio RwLock + Retry for Concurrent Access

**Choice** (retained from original design, still applies):

- `tokio::sync::RwLock<()>` guards all git operations within a single replica
- Read operations acquire read lock (concurrent reads allowed)
- Write operations acquire write lock (exclusive)
- Fast-track push conflicts retry up to 3 times: pull → re-apply → push
- Governance-track writes go to branches so push conflicts are extremely unlikely (unique branch names)

### Decision 10: Replace InMemoryKnowledgeProposalStorage with PR-Backed Storage

**Choice**: The MCP tools (`KnowledgeProposeTool`, `KnowledgeProposalSubmitTool`) currently use `InMemoryKnowledgeProposalStorage` which loses all proposals on pod restart. Replace with a PR-backed storage:

- `propose()` → creates a governance branch + commits the entry file
- `submit()` → opens a PR from the governance branch to main
- `list_pending()` → lists open PRs with `governance/` prefix
- `get()` → reads the entry content from the PR branch

This wires the MCP tool interface directly to the Git provider, giving proposals persistence and the governance review workflow.

### Decision 11: Webhook Configuration in Helm Chart

**Choice**: Add to the Helm values:
- `aeterna.webhook.secret`: Webhook secret (K8s secret ref)
- `aeterna.webhook.enabled`: Boolean toggle (default: `false`)
- `aeterna.knowledgeRepo.githubToken`: Personal access token or GitHub App token for `octocrab` API calls (K8s secret ref)
- `aeterna.knowledgeRepo.owner`: GitHub org/user (e.g., `kyriba-eng`)
- `aeterna.knowledgeRepo.name`: Repo name (e.g., `aeterna-knowledge`)

The webhook path (`/api/v1/webhooks/github`) is served by the existing ingress — no additional ingress rule needed since it's under the same host/path prefix.

## Risks / Trade-offs

- **PR creation latency**: Governance-track writes take 2-5 seconds (branch + commit + PR creation via GitHub API) instead of milliseconds. Acceptable since these are infrequent, high-impact operations.

- **GitHub API rate limits**: 5000 requests/hour for authenticated users. With 2 replicas and moderate governance activity, well within limits. **Mitigation**: Track rate limit headers, log warnings at 80% usage.

- **Webhook delivery reliability**: GitHub retries webhook delivery for up to 3 days, but network issues could delay notifications. **Mitigation**: Background sync still polls at a configurable interval as a safety net.

- **Webhook secret rotation**: Changing the secret requires updating both GitHub webhook config and K8s secret. **Mitigation**: Standard secret rotation workflow.

- **PR merge conflicts**: If two governance operations modify the same file, the second PR may have merge conflicts. **Mitigation**: GitHub shows conflict status. Human reviewer resolves before merge. The entry's path-based isolation (each entry is a separate file) minimizes this risk.

- **Local clone divergence**: Between webhook events, the local clone may be stale. **Mitigation**: Pull-before-read ensures reads always see latest state. The webhook just triggers an eager pull to reduce latency.

## Migration Plan

1. **No schema migration needed**: `GitRepository` struct gains optional fields; `None`/empty = local-only mode (backward compatible).
2. **Deploy sequence**:
   a. Add `octocrab` dependency to `knowledge/Cargo.toml`
   b. Implement `GitProvider` trait + `GitHubProvider`
   c. Add two-track routing to `GitRepository::store()`
   d. Add webhook endpoint to Axum router
   e. Wire PR lifecycle to `GovernanceEvent` emission
   f. Replace `InMemoryKnowledgeProposalStorage` with PR-backed storage
   g. Update config loader with new env vars
   h. Update Helm chart with knowledge repo + webhook config
   i. Create GitHub repo `kyriba-eng/aeterna-knowledge` (empty, private)
   j. Generate deploy key (read-write) + webhook, add to repo
   k. Create K8s secrets for SSH key, GitHub token, webhook secret
   l. Build and push new Docker image
   m. Helm upgrade — pods clone the empty repo on init
3. **Rollback**: Remove `AETERNA_KNOWLEDGE_REPO_URL` from env → falls back to local-only mode immediately. No PRs will be created, no webhooks received. Existing knowledge in the remote repo is preserved for re-enablement.
