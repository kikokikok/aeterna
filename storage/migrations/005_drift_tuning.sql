-- Migration: Add drift suppression and configuration tables
-- MT-C3: Drift Detection Tuning

CREATE TABLE IF NOT EXISTS drift_suppressions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    policy_id TEXT NOT NULL,
    rule_pattern TEXT,
    reason TEXT NOT NULL,
    created_by TEXT NOT NULL,
    expires_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_drift_suppressions_project 
    ON drift_suppressions(tenant_id, project_id);

CREATE INDEX IF NOT EXISTS idx_drift_suppressions_policy 
    ON drift_suppressions(tenant_id, policy_id);

CREATE TABLE IF NOT EXISTS drift_configs (
    project_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    threshold REAL NOT NULL DEFAULT 0.2,
    low_confidence_threshold REAL NOT NULL DEFAULT 0.7,
    auto_suppress_info BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, project_id)
);

ALTER TABLE drift_results ADD COLUMN IF NOT EXISTS confidence REAL DEFAULT 1.0;
ALTER TABLE drift_results ADD COLUMN IF NOT EXISTS requires_manual_review BOOLEAN DEFAULT FALSE;
ALTER TABLE drift_results ADD COLUMN IF NOT EXISTS suppressed_violations JSONB DEFAULT '[]';
