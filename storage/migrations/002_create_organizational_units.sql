-- Base tenant table referenced by all multi-tenant tables
CREATE TABLE IF NOT EXISTS organizational_units (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    parent_id TEXT REFERENCES organizational_units(id),
    tenant_id TEXT NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW()) * 1000)::BIGINT,
    updated_at BIGINT NOT NULL DEFAULT (EXTRACT(EPOCH FROM NOW()) * 1000)::BIGINT
);

CREATE INDEX IF NOT EXISTS idx_org_units_type ON organizational_units(type);
CREATE INDEX IF NOT EXISTS idx_org_units_parent ON organizational_units(parent_id);
CREATE INDEX IF NOT EXISTS idx_org_units_tenant ON organizational_units(tenant_id);
