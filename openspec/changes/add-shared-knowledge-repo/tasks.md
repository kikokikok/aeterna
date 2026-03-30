## 1. GitProvider Trait & GitHub Implementation

- [x] 1.1 Add `octocrab` and `hmac`/`sha2` dependencies to `knowledge/Cargo.toml`
- [x] 1.2 Create `knowledge/src/git_provider.rs` with `GitProvider` async trait (create_branch, commit_to_branch, create_pull_request, merge_pull_request, list_open_prs, parse_webhook, get_default_branch_sha)
- [x] 1.3 Define supporting types: `PullRequestInfo`, `WebhookEvent`, `MergeMethod`, `GovernanceBranch` in `git_provider.rs`
- [x] 1.4 Implement `GitHubProvider` struct using `octocrab` crate — branch creation via `repos().create_ref()`
- [x] 1.5 Implement `GitHubProvider::commit_to_branch` — file create/update via `repos().create_file().branch().send()`
- [x] 1.6 Implement `GitHubProvider::create_pull_request` — PR creation via `pulls().create().body().send()`
- [x] 1.7 Implement `GitHubProvider::merge_pull_request` — squash merge via `pulls().merge().sha().method().send()`
- [x] 1.8 Implement `GitHubProvider::list_open_prs` — list PRs filtering by head branch prefix
- [x] 1.9 Implement `GitHubProvider::parse_webhook` — validate HMAC signature, parse `X-GitHub-Event` header and body
- [x] 1.10 Implement `GitHubProvider::get_default_branch_sha` — get HEAD SHA of the default branch
- [x] 1.11 Implement branch naming helper: `governance/{verb}-{slug}-{yyyymmdd}` with uniqueness suffix
- [x] 1.12 Implement idempotency check: query open PRs before creating to prevent duplicates
- [x] 1.13 Write unit tests for `GitHubProvider` (mock octocrab responses) — branch naming, idempotency, HMAC validation

## 2. Remote Git Synchronization in GitRepository

- [x] 2.1 Add remote-related fields to `GitRepository` struct: `remote_url`, `branch`, `ssh_key`, `git_provider`, `rw_lock`
- [x] 2.2 Update `GitRepository::new()` — clone from remote if local path empty, pull if existing clone, error if wrong remote
- [x] 2.3 Implement SSH callback using `git2::Cred::ssh_key_from_memory()` for remote auth
- [x] 2.4 Implement `pull_from_remote()` — fetch + fast-forward merge, reset on divergence
- [x] 2.5 Implement `push_to_remote()` — push to origin/main with retry loop (3 attempts, pull-rebase-push)
- [x] 2.6 Add `tokio::sync::RwLock<()>` for per-replica read/write coordination
- [x] 2.7 Wrap `get()`, `list()`, `search()` with read lock + pull-before-read when remote is configured
- [x] 2.8 Write integration tests for remote sync using testcontainers (gitea or bare git server)

## 3. Two-Track Write Routing

- [x] 3.1 Implement `requires_governance()` function — routing predicate based on layer + status + operation type
- [x] 3.2 Modify `GitRepository::store()` — fast track: commit + push for Project/Draft; governance track: branch + commit + PR
- [x] 3.3 Implement governance-track write flow: create branch → commit to branch → open PR → return PR info
- [x] 3.4 Modify `GitRepository::delete()` — route through governance track when remote configured
- [x] 3.5 Add `update_status()` method to `KnowledgeRepository` trait in `mk_core/src/traits.rs`
- [x] 3.6 Implement `update_status()` in `GitRepository` — route non-Draft status changes through governance track
- [x] 3.7 Add `promote()` method to `KnowledgeRepository` trait for cross-layer promotion
- [x] 3.8 Implement `promote()` in `GitRepository` — always governance track
- [x] 3.9 Write unit tests for routing logic: verify fast-track vs governance-track selection for all combinations of layer/status/operation

## 4. GovernanceEvent Emission from PR Lifecycle

- [x] 4.1 Fix `approve_proposal()` in `knowledge/src/api.rs` — emit `GovernanceEvent::RequestApproved` (currently missing)
- [x] 4.2 Fix `reject_proposal()` in `knowledge/src/api.rs` — emit `GovernanceEvent::RequestRejected` (currently missing)
- [x] 4.3 Add PR metadata fields to `GovernanceEvent` variants (pr_number, merge_sha, entry_id) or create new variants
- [x] 4.4 Wire governance-track PR creation → `GovernanceEvent::RequestCreated` emission
- [x] 4.5 Wire webhook PR merge → `GovernanceEvent::RequestApproved` emission
- [x] 4.6 Wire webhook PR close → `GovernanceEvent::RequestRejected` emission

## 5. Webhook Endpoint

- [x] 5.1 Create `cli/src/server/webhooks.rs` — Axum handler for `POST /api/v1/webhooks/github`
- [x] 5.2 Implement HMAC-SHA256 signature validation from `X-Hub-Signature-256` header
- [x] 5.3 Implement event dispatch: pull_request.opened → RequestCreated, closed+merged → pull + RequestApproved + sync trigger, closed+!merged → RequestRejected
- [x] 5.4 Notify sync bridge of CommitMismatch on PR merge (trigger immediate knowledge→memory sync)
- [x] 5.5 Register webhook route in `cli/src/server/router.rs`
- [x] 5.6 Conditionally enable webhook endpoint based on `AETERNA_WEBHOOK_SECRET` presence (return 404 if not configured)
- [x] 5.7 Write integration tests for webhook handler — valid/invalid signatures, different event types

## 6. PR-Backed Proposal Storage

- [x] 6.1 Create `knowledge/src/pr_proposal_storage.rs` — implement proposal storage backed by GitProvider
- [x] 6.2 Implement `propose()` — create governance branch + commit entry file
- [x] 6.3 Implement `submit()` — open PR from governance branch to main
- [x] 6.4 Implement `list_pending()` — query open PRs with governance/ prefix
- [x] 6.5 Implement `get()` — read entry content from PR branch
- [x] 6.6 Replace `InMemoryKnowledgeProposalStorage` usage in `tools/src/knowledge.rs` with PR-backed storage
- [x] 6.7 Write tests for PR proposal storage (mock GitProvider)

## 7. Configuration & Bootstrap

- [x] 7.1 Add knowledge repo config fields to `config/src/config.rs` — remote URL, branch, SSH key, GitHub owner/name/token, webhook secret
- [x] 7.2 Add env var loading to `config/src/loader.rs` — `AETERNA_KNOWLEDGE_REPO_URL`, `AETERNA_KNOWLEDGE_REPO_BRANCH`, `AETERNA_KNOWLEDGE_REPO_SSH_KEY`, `AETERNA_GITHUB_OWNER`, `AETERNA_GITHUB_REPO`, `AETERNA_GITHUB_TOKEN`, `AETERNA_WEBHOOK_SECRET`
- [x] 7.3 Update `cli/src/server/bootstrap.rs` — initialize `GitRepository` with remote config, create `GitHubProvider` when token is configured
- [x] 7.4 Update `AppState` in `cli/src/server/mod.rs` — include GitProvider for webhook handler access

## 8. Helm Chart Updates

- [x] 8.1 Add `knowledgeRepo` and `webhook` config blocks to `charts/aeterna/values.yaml`
- [x] 8.2 Update `charts/aeterna/templates/aeterna/deployment.yaml` — add env vars for knowledge repo URL, branch, SSH key (secretKeyRef), GitHub owner/name/token (secretKeyRef), webhook secret (secretKeyRef)
- [x] 8.3 Verify existing ingress routes cover `/api/v1/webhooks/github` (should work since path prefix `/` already matches)

## 9. Infrastructure Setup

- [x] 9.1 Create GitHub repo for shared knowledge (private, empty)
- [x] 9.2 Generate SSH deploy key (read-write) and add to the repo
- [x] 9.3 Create K8s secret `aeterna-knowledge-repo-key` with SSH private key
- [x] 9.4 Create GitHub App on org, install on aeterna-knowledge repo
- [x] 9.5 Create K8s secret `aeterna-github-app-pem` with App private key PEM
- [x] 9.6 Generate webhook secret, create K8s secret `aeterna-webhook-secret`
- [x] 9.7 Configure GitHub webhook on knowledge repo → `https://<your-aeterna-host>/api/v1/webhooks/github`
- [x] 9.8 Update deployment values with knowledge repo config (in your private deployment repo)

## 10. Build, Deploy & Validate

- [x] 10.1 Run `cargo check -p aeterna` — verify compilation
- [x] 10.2 Run `cargo test -p knowledge` — verify unit tests pass
- [x] 10.3 Commit and push code to `kikokikok/aeterna` on `feat/server-runtime-deploy`
- [x] 10.4 Trigger GitHub Actions build → new Docker image
- [x] 10.5 Helm upgrade with new image tag and knowledge repo config
- [x] 10.6 Patch Dragonfly service selector (recurring workaround)
- [x] 10.7 Verify all pods Running and healthy
- [x] 10.8 Re-run Newman E2E tests — target 79/79 passing (multi-replica consistency fixed)
- [ ] 10.9 Test governance-track: create a Team-layer knowledge entry → verify PR created in GitHub
- [ ] 10.10 Test webhook: merge the PR in GitHub → verify entry appears on main, GovernanceEvent emitted
