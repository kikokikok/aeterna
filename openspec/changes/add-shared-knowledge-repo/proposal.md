## Why

The `GitRepository` knowledge backend uses a local-only git repo (`/tmp/knowledge-repo`). With multiple Aeterna replicas, each pod has its own isolated filesystem — writes on pod A are invisible to pod B. This breaks CRUD consistency for any multi-replica deployment and makes the Knowledge API unreliable under load balancing.

A shared remote GitHub repository as the backing store solves this: all replicas clone, pull, and push to the same origin, achieving strong consistency across pods.

## What Changes

- Add remote git sync to `GitRepository` — clone from remote on init, pull before reads, push after writes
- Add SSH/token-based authentication for remote git operations (reuse existing deploy key pattern)
- Add config knobs: `AETERNA_KNOWLEDGE_REPO_URL`, `AETERNA_KNOWLEDGE_REPO_BRANCH`, `AETERNA_KNOWLEDGE_REPO_SSH_KEY`
- Add write-lock coordination (file lock or advisory) to prevent concurrent push conflicts across async tasks within a single replica
- Add retry + rebase-on-conflict logic for push failures (another replica pushed first)
- Helm chart: add knowledge repo URL/branch/SSH key configuration to deployment template
- **BREAKING**: Local-only mode (no remote URL) remains supported but is no longer the default for production deployments

## Capabilities

### New Capabilities
- (none — this is an enhancement to the existing knowledge-repository)

### Modified Capabilities
- `knowledge-repository`: Adding requirements for remote git synchronization, multi-replica consistency, and authentication for remote operations
- `deployment`: Adding Helm values for knowledge repo remote URL, branch, and SSH key

## Impact

- **Code**: `knowledge/src/repository.rs` (major changes to `GitRepository`), `config/src/config.rs` (new config fields), `cli/src/server/bootstrap.rs` (init with remote), `config/src/loader.rs` (env var loading)
- **Helm**: `charts/aeterna/templates/aeterna/deployment.yaml` (new env vars + secret mount), `charts/aeterna/values.yaml` (new knowledge repo config block)
- **Infrastructure**: Requires a GitHub repo with deploy key (already exists: `kyriba-eng/aeterna-knowledge` or similar), K8s secret for SSH key
- **Dependencies**: `git2` crate already in use — remote operations use its `Remote` API (fetch, push). No new crate dependencies expected.
- **Risk**: Push conflicts under high write concurrency — mitigated by pull-rebase-push retry loop
