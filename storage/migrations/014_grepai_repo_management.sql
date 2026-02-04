-- 1. Identity Store Table
CREATE TABLE IF NOT EXISTS codesearch_identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id TEXT NOT NULL,
    name VARCHAR(255) NOT NULL,
    provider VARCHAR(50) NOT NULL, -- 'github', 'gitlab', 'bitbucket'
    auth_type VARCHAR(50) NOT NULL, -- 'pat', 'oauth', 'app_token', 'ssh_key'
    secret_id TEXT NOT NULL, -- Reference in Secret Manager
    secret_provider VARCHAR(50) NOT NULL, -- 'aws-secrets', 'vault', 'gcp-secrets'
    scopes TEXT[] DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);

-- 2. Repository Metadata Table
CREATE TABLE IF NOT EXISTS codesearch_repositories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id TEXT NOT NULL,
    identity_id UUID REFERENCES codesearch_identities(id) ON DELETE SET NULL,
    name VARCHAR(255) NOT NULL,
    type VARCHAR(20) NOT NULL CHECK (type IN ('local', 'remote', 'hybrid')),
    remote_url TEXT,
    local_path TEXT,
    current_branch VARCHAR(255) DEFAULT 'main',
    tracked_branches TEXT[] DEFAULT '{}',
    sync_strategy VARCHAR(20) NOT NULL DEFAULT 'manual' CHECK (sync_strategy IN ('hook', 'job', 'manual')),
    sync_interval_mins INTEGER DEFAULT 15,
    status VARCHAR(20) NOT NULL DEFAULT 'requested' CHECK (status IN ('requested', 'pending', 'approved', 'cloning', 'indexing', 'ready', 'error')),
    last_indexed_commit VARCHAR(40),
    last_indexed_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    owner_id TEXT,
    shard_id TEXT, -- Assigned indexer shard/pod
    cold_storage_uri TEXT, -- S3/GCS URI for backup bundle
    config JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(tenant_id, name)
);

-- 2. Index Metadata Table
CREATE TABLE IF NOT EXISTS codesearch_index_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES codesearch_repositories(id) ON DELETE CASCADE,
    commit_sha VARCHAR(40) NOT NULL,
    parent_commit_sha VARCHAR(40),
    files_indexed INTEGER NOT NULL,
    files_removed INTEGER DEFAULT 0,
    files_renamed INTEGER DEFAULT 0,
    indexing_duration_ms INTEGER,
    embedding_api_calls INTEGER,
    indexed_at TIMESTAMPTZ DEFAULT NOW()
);

-- 3. Approval Workflow Requests
CREATE TABLE IF NOT EXISTS codesearch_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES codesearch_repositories(id) ON DELETE CASCADE,
    requester_id TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'requested' CHECK (status IN ('requested', 'pending', 'approved', 'rejected')),
    policy_result JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 4. Usage Metrics for Cleanup
CREATE TABLE IF NOT EXISTS codesearch_usage_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES codesearch_repositories(id) ON DELETE CASCADE,
    branch VARCHAR(255) NOT NULL,
    search_count INTEGER DEFAULT 0,
    trace_count INTEGER DEFAULT 0,
    last_active_at TIMESTAMPTZ DEFAULT NOW(),
    period_start TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(repository_id, branch)
);

-- 6. Cleanup Audit Logs
CREATE TABLE IF NOT EXISTS codesearch_cleanup_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL,
    repository_name VARCHAR(255) NOT NULL,
    branch TEXT,
    reason TEXT NOT NULL,
    action_taken VARCHAR(50) NOT NULL,
    performed_by TEXT,
    executed_at TIMESTAMPTZ DEFAULT NOW()
);

-- 7. Indexer Shards for Distributed Routing
CREATE TABLE IF NOT EXISTS codesearch_indexer_shards (
    shard_id TEXT PRIMARY KEY,
    pod_name TEXT NOT NULL,
    pod_ip TEXT NOT NULL,
    capacity INTEGER NOT NULL DEFAULT 100,
    current_load INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'draining', 'offline', 'maintenance')),
    last_heartbeat TIMESTAMPTZ NOT NULL,
    registered_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index for fast shard lookups
CREATE INDEX IF NOT EXISTS idx_shards_status ON codesearch_indexer_shards(status);
CREATE INDEX IF NOT EXISTS idx_shards_heartbeat ON codesearch_indexer_shards(last_heartbeat);

-- Enable RLS for multi-tenancy
ALTER TABLE codesearch_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE codesearch_repositories ENABLE ROW LEVEL SECURITY;
ALTER TABLE codesearch_index_metadata ENABLE ROW LEVEL SECURITY;
ALTER TABLE codesearch_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE codesearch_usage_metrics ENABLE ROW LEVEL SECURITY;

-- 6. RLS Policies
CREATE POLICY codesearch_identities_tenant_isolation ON codesearch_identities
    FOR ALL USING (tenant_id = current_setting('app.tenant_id', true)::text);

CREATE POLICY codesearch_repositories_tenant_isolation ON codesearch_repositories
    FOR ALL USING (tenant_id = current_setting('app.tenant_id', true)::text);

CREATE POLICY codesearch_index_metadata_tenant_isolation ON codesearch_index_metadata
    FOR ALL USING (repository_id IN (SELECT id FROM codesearch_repositories));

CREATE POLICY codesearch_requests_tenant_isolation ON codesearch_requests
    FOR ALL USING (repository_id IN (SELECT id FROM codesearch_repositories));

CREATE POLICY codesearch_usage_metrics_tenant_isolation ON codesearch_usage_metrics
    FOR ALL USING (repository_id IN (SELECT id FROM codesearch_repositories));

-- ============================================================================
-- OPAL VIEWS FOR CODE SEARCH
-- ============================================================================

CREATE OR REPLACE VIEW v_code_search_repositories AS
SELECT
    id,
    tenant_id,
    name,
    status,
    sync_strategy,
    current_branch
FROM codesearch_repositories;

CREATE OR REPLACE VIEW v_code_search_requests AS
SELECT
    id,
    repository_id,
    requester_id,
    status,
    tenant_id
FROM codesearch_requests;

CREATE OR REPLACE VIEW v_code_search_identities AS
SELECT
    id,
    tenant_id,
    name,
    provider
FROM codesearch_identities;

COMMENT ON VIEW v_code_search_repositories IS 'OPAL view: Code Search repositories for Cedar entities.';
COMMENT ON VIEW v_code_search_requests IS 'OPAL view: Code Search requests for Cedar entities.';
COMMENT ON VIEW v_code_search_identities IS 'OPAL view: Code Search identities for Cedar entities.';
