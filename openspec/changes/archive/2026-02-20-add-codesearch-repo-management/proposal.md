# Change: Code Search Repository Management (Enhanced)

## Why

The current Code Search integration (Phase 1) is limited to static, local-only indexing. To support enterprise-scale repository management and governance, we need:
1. **Remote Repository Support**: Track and index team-level repositories (GitHub, GitLab).
2. **Governance & Approval**: Policy-based approval workflow for indexing new repositories.
3. **Lifecycle Management**: Automatic cleanup of inactive or unmaintained repository indexes.
4. **Optimized Indexing**: Delta-based incremental indexing for fast updates.
5. **Intelligent Triggers**: Multiple strategies for triggering re-indexing (Hook, Job, Manual).

## What Changes

### Major Features

#### 1. Enhanced Approval Workflow
- **7-State Machine**: `requested` → `pending` → `approved` → `cloning` → `indexing` → `ready` → `error`.
- **Auto-Approval**: Local repositories are automatically approved.
- **Policy-Based Approval**: Remote repositories require approval based on Cedar policies pushed via OPAL.

#### 2. Policy-Based Governance
- **Cedar Integration**: Use Cedar for fine-grained policy evaluation.
- **OPAL Integration**: Real-time policy distribution to all agents.
- **GitHub Owner Detection**: Auto-detect repository owners via `CODEOWNERS` or API.

#### 3. Cleanup & Lifecycle Management
- **Commit-based cleanup**: Remove indexes for repositories with no commits for X days.
- **Usage-based cleanup**: Remove indexes for repositories with no search/trace queries for Y days.
- **Manual Cleanup**: CLI commands to force cleanup of specific repositories/branches.

#### 4. Advanced Indexing Strategies
- **Incremental Indexing**: Delta file extraction via GraphQL (99%+ faster than full re-index).
- **Trigger Strategies**:
    - `hook`: Trigger on merge events (webhooks).
    - `job`: Periodic background job to check for deltas and re-index.
    - `manual`: User-initiated re-indexing via CLI/API.

#### 5. Multi-Interface Requests
- **CLI**: `aeterna codesearch repo request`
- **MCP**: New `codesearch_repo_request` tool.
- **A2A**: Support for Agent-to-Agent repository indexing requests.

#### 6. Identity & Secret Management (NEW)
- **Pluggable Secret Providers**: Integration with AWS Secrets Manager, GCP Secret Manager, Azure Key Vault, and HashiCorp Vault.
- **Identity Store**: Centralized management of Git identities (PATs, SSH keys, App Tokens) mapped to repositories.
- **Permission Verification**: Real-time permission checking against Git providers before indexing.

### Database Schema (NEW/MODIFIED)

| Table | Description |
|-------|-------------|
| `codesearch_repositories` | Repository metadata (linked to `identity_id`) |
| `codesearch_identities` | Git identity metadata and secret references |
| `codesearch_index_metadata` | History of indexing operations and commit tracking |
| `codesearch_requests` | Approval workflow state and requester info |
| `codesearch_usage_metrics` | Search/Trace usage tracking for cleanup decisions |
| `codesearch_cleanup_log` | Audit log of automatic and manual cleanup actions |

### CLI Interface (16 commands)

- **Repo**: `request`, `list`, `requests`, `approve`, `reject`, `add`, `update`, `checkout`, `remove`
- **Cleanup**: `list-candidates`, `auto`, `repo`, `branch`, `log`
- **Stats**: `repo`, `branch`, `top-repos`, `inactive`, `trends`
- **Policy**: `test`, `list`, `sync`
- **Index**: `index --incremental/--full/--force`

## Impact

- **Storage**: Increased storage for index metadata and usage tracking.
- **Performance**: 99%+ faster PR indexing (5-10 sec vs 30-60 min).
- **Governance**: Enterprise-grade control over what code is indexed and searchable.

## Success Metrics

- 100% requirements coverage (14/14).
- PR indexing completion in < 15 seconds for 95th percentile.
- Zero unauthorized repository indexing.
- Automatic cleanup of 90%+ of inactive repositories.
