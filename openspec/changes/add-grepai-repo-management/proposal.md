# Change: GrepAI Repository Management (Local, Remote, Hybrid, Incremental)

## Why

The current GrepAI integration (Phase 1) only supports **static, local-only indexing** with these limitations:

1. **No remote repository support** - Cannot clone and track team repos (GitHub, GitLab)
2. **No incremental indexing** - Full re-index required (30-60 min for 10k files)
3. **No branch management** - Stuck on one branch, can't switch or track multiple
4. **No automatic updates** - Manual re-index required after every code change
5. **No multi-repository support** - Single project only, no org-wide code intelligence

This means:
- **Developers**: Wait 30+ minutes after `git pull` for re-indexing
- **Teams**: Can't search across multiple repos in organization
- **AI Agents**: Outdated code context, poor feature branch understanding
- **Cost**: 10,000 unnecessary embedding API calls per re-index ($100-200/month wasted)

**User feedback**: _"So wait - Explain how grep AI will work on local repo clones and potentially allow for remote repos where master or branches are available? It needs some kind of hybrid and contextual increments of codebase no?"_

## What Changes

### Core Features

#### 1. Three Repository Types

**Local Repositories** (Active Development):
- Real-time file system watching (inotify/fsnotify)
- Immediate index updates on file save (500ms debounce)
- Works with uncommitted changes
- No git required

**Remote Repositories** (Team Repos):
- Automatic cloning on first use
- Periodic polling for updates (5-30 min configurable)
- Multi-branch support with wildcard patterns (`main`, `develop`, `feature/*`)
- Git credentials management (SSH keys, tokens, OAuth)
- Optional webhook integration for instant updates

**Hybrid Repositories** (Best of Both):
- Local clone of remote repo
- Watch local changes + sync with remote
- Automatic git pull on interval
- Branch switching capability
- Smart conflict detection

#### 2. Incremental Indexing (10-100x Speedup)

**Git-based change detection**:
```bash
git diff --name-status <last_commit>..HEAD
# Only index changed files (10 files, not 10,000)
```

**Performance impact**:
- Full re-index: 30-60 minutes for 10,000 files
- Incremental: 5-10 seconds for 10 changed files
- **99%+ cost savings** (10 API calls instead of 10,000)

#### 3. Multi-Branch Support

- Track multiple branches simultaneously
- Branch switching with delta calculation
- Smart index reuse (copy from main + apply delta)
- Feature branch development
- Code review (PR branch comparison)
- Multi-version support (v1.x, v2.x, v3.x)

#### 4. Automatic Update Strategies

1. **File System Watch**: Real-time for local/hybrid (500ms debounce)
2. **Git Hooks**: Post-commit, post-merge triggers
3. **Webhooks**: GitHub/GitLab push events (instant)
4. **Polling**: Configurable intervals (5-30 min)

### Database Schema (PostgreSQL)

```sql
CREATE TABLE grepai_repositories (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    name VARCHAR(255) NOT NULL,
    type VARCHAR(10) NOT NULL CHECK (type IN ('local', 'remote', 'hybrid')),
    remote_url TEXT,
    local_path TEXT NOT NULL,
    current_branch VARCHAR(255),
    tracked_branches TEXT[], -- ['main', 'develop', 'feature/*']
    status VARCHAR(20) NOT NULL, -- 'cloning', 'indexing', 'ready', 'error'
    last_indexed_commit VARCHAR(40),
    last_indexed_at TIMESTAMPTZ,
    config JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);

CREATE TABLE grepai_index_metadata (
    id UUID PRIMARY KEY,
    repository_id UUID NOT NULL REFERENCES grepai_repositories(id) ON DELETE CASCADE,
    commit_sha VARCHAR(40) NOT NULL,
    parent_commit_sha VARCHAR(40),
    files_indexed INTEGER NOT NULL,
    files_removed INTEGER NOT NULL,
    files_renamed INTEGER NOT NULL,
    indexing_duration_ms INTEGER NOT NULL,
    embedding_api_calls INTEGER NOT NULL,
    indexed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Enable RLS for multi-tenancy
ALTER TABLE grepai_repositories ENABLE ROW LEVEL SECURITY;
ALTER TABLE grepai_index_metadata ENABLE ROW LEVEL SECURITY;

CREATE POLICY repo_tenant_isolation ON grepai_repositories
    USING (tenant_id = current_setting('app.tenant_id', true)::UUID);
```

### CLI Commands

```bash
# Repository management
aeterna grepai repo add <name> \
    --url https://github.com/org/repo.git \
    --type remote \
    --branches main,develop \
    --poll-interval 5m

aeterna grepai repo list [--json]

aeterna grepai repo update <name>  # Pull + incremental index

aeterna grepai repo checkout <name> <branch>  # Switch branch

aeterna grepai repo remove <name>  # Delete repo and index

# Indexing
aeterna grepai index --incremental  # Smart re-index (default)

aeterna grepai index --full --repo <name>  # Force full re-index

aeterna grepai index --watch <path>  # Start file watcher
```

### Helm Chart Additions

```yaml
grepai:
  enabled: true
  repositories:
    # Development (local)
    - name: my-project
      type: local
      path: /workspace/my-project
      indexing:
        mode: watch  # Real-time
    
    # Team repos (remote)
    - name: backend
      type: remote
      url: https://github.com/company/backend.git
      branches: [main, develop]
      indexing:
        mode: poll
        interval: 5m
      credentials: git-secret
    
    # Hybrid
    - name: my-feature
      type: hybrid
      url: https://github.com/company/backend.git
      local_path: /workspace/backend
      branch: feature/my-feature
      sync:
        auto_pull: true
        watch_local: true
```

## Impact

### Affected Specs
- **NEW**: `grepai-repo-management` capability
- **MODIFIED**: `grepai-integration` capability (add repo management)

### Affected Code
- `storage/src/repo_manager.rs` (NEW) - Repository lifecycle management
- `tools/src/grepai/incremental.rs` (NEW) - Incremental indexing engine
- `cli/src/commands/grepai/repo.rs` (NEW) - Repository CLI commands
- `charts/aeterna/values.yaml` (MODIFIED) - Repository configuration
- `charts/aeterna/templates/aeterna/deployment.yaml` (MODIFIED) - Init job for cloning

### Migration Path

**Before** (Manual GrepAI):
```bash
cd /path/to/project
git pull
grepai init --force  # 30-60 minutes full re-index :(
```

**After** (Aeterna + GrepAI):
```bash
# Automatic: 5-10 seconds incremental update :)
# Or manual:
aeterna grepai repo update my-project
```

## Benefits

| Capability | Before | After | Improvement |
|------------|--------|-------|-------------|
| Index speed | 30-60 min (full) | 5-10 sec (incremental) | **99%+ faster** |
| API cost | $100-200/month | $1-2/month | **99%+ savings** |
| Update method | Manual re-index | Automatic | **Zero-touch** |
| Multi-repo search | Single project | Org-wide | **Team-scale** |
| Branch support | One branch | Multiple branches | **Feature dev** |
| Remote repos | Not supported | Full support | **Team collab** |

## Non-Goals

- Forking GrepAI or rewriting in Rust (it's Go, keep separate)
- Supporting non-git version control (SVN, Mercurial)
- Real-time collaboration features (concurrent editing)
- Code execution or compilation

## Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Git clone failures | High | Medium | Retry logic, credential validation, error reporting |
| Large repo performance | High | Low | Shallow clones, sparse checkouts, incremental indexing |
| Webhook security | High | Medium | Signature verification, rate limiting, API keys |
| Concurrent git operations | Medium | Medium | File locking, operation queuing |
| Storage growth | Medium | High | Branch pruning, old commit cleanup, compression |
| Network failures | Low | High | Offline mode, cached results, graceful degradation |

## Success Metrics

- **Performance**: 95%+ of index updates complete in <30 seconds
- **Cost**: 90%+ reduction in embedding API costs
- **Adoption**: 80%+ of users enable remote repo tracking within 30 days
- **Reliability**: 99%+ success rate for git clone/pull operations
- **User satisfaction**: NPS score improvement by +20 points

## Dependencies

- Git 2.30+ (git binary in PATH or container)
- PostgreSQL 14+ (for repository metadata)
- Existing GrepAI sidecar (Phase 1)
- GitHub/GitLab webhooks (optional, for instant updates)
