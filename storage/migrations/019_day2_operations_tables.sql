-- Migration 019: Day-2 operations tables (remediation requests + dead-letter queue)
-- These tables back the lifecycle manager's remediation approval system and
-- the dead-letter queue for permanently failed sync/promotion items.

-- Remediation requests: human-in-the-loop approval for destructive lifecycle actions.
CREATE TABLE IF NOT EXISTS remediation_requests (
    id              TEXT PRIMARY KEY,
    request_type    TEXT NOT NULL,
    risk_tier       TEXT NOT NULL,           -- auto_execute, notify_and_execute, require_approval
    entity_type     TEXT NOT NULL,
    entity_ids      JSONB NOT NULL DEFAULT '[]',
    tenant_id       TEXT,
    description     TEXT NOT NULL,
    proposed_action TEXT NOT NULL,
    detected_by     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending, approved, rejected, executed, expired, failed
    created_at      BIGINT NOT NULL,
    reviewed_by     TEXT,
    reviewed_at     BIGINT,
    resolution_notes TEXT,
    executed_at     BIGINT
);

CREATE INDEX IF NOT EXISTS idx_remediation_requests_status
    ON remediation_requests (status);
CREATE INDEX IF NOT EXISTS idx_remediation_requests_tenant
    ON remediation_requests (tenant_id) WHERE tenant_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_remediation_requests_created
    ON remediation_requests (created_at);

-- Dead-letter queue: permanently failed sync/promotion items quarantined for manual review.
CREATE TABLE IF NOT EXISTS dead_letter_items (
    id              TEXT PRIMARY KEY,
    item_type       TEXT NOT NULL,           -- sync_entry, promotion, federation
    item_id         TEXT NOT NULL,
    tenant_id       TEXT NOT NULL,
    error           TEXT NOT NULL,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    max_retries     INTEGER NOT NULL DEFAULT 5,
    first_failed_at BIGINT NOT NULL,
    last_failed_at  BIGINT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'active',  -- active, retrying, discarded
    metadata        JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_dead_letter_items_status
    ON dead_letter_items (status);
CREATE INDEX IF NOT EXISTS idx_dead_letter_items_tenant
    ON dead_letter_items (tenant_id);
CREATE INDEX IF NOT EXISTS idx_dead_letter_items_type
    ON dead_letter_items (item_type);

-- Storage quota tracking (optional, for per-tenant quota enforcement).
-- Usage is computed from COUNT queries, but this table caches the quota limits.
CREATE TABLE IF NOT EXISTS tenant_storage_quotas (
    tenant_id       TEXT PRIMARY KEY,
    memory_max      BIGINT,                  -- NULL = unlimited
    knowledge_max   BIGINT,
    vector_max      BIGINT,
    total_max       BIGINT,
    updated_at      BIGINT NOT NULL
);
