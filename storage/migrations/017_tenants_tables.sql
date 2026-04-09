-- Tenant management tables
-- These tables back the TenantStore, TenantRepositoryBindingStore, and
-- verified-domain resolution.  They were previously created only through
-- initialize_schema (the in-process dev/test path); this migration makes them
-- part of the canonical schema so that the Helm post-install migration job
-- creates them on fresh cluster installs.
--
-- All statements are idempotent (IF NOT EXISTS) so running this migration
-- against a cluster that already has the tables is a no-op.

-- ============================================================================
-- TENANTS
-- One row per logical tenant.  The slug is the human-readable identifier used
-- in API paths and config files.
-- ============================================================================
CREATE TABLE IF NOT EXISTS tenants (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    slug          TEXT        UNIQUE NOT NULL,
    name          TEXT        NOT NULL,
    status        TEXT        NOT NULL DEFAULT 'active',
    source_owner  TEXT        NOT NULL DEFAULT 'admin',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deactivated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tenants_slug   ON tenants(slug)   WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_tenants_status ON tenants(status);

-- ============================================================================
-- TENANT DOMAIN MAPPINGS
-- Maps a verified email domain to a tenant so that users whose email matches
-- the domain are automatically associated with that tenant.
-- The source column distinguishes admin-managed mappings from sync-created
-- ones; admin rows cannot be overwritten by sync.
-- ============================================================================
CREATE TABLE IF NOT EXISTS tenant_domain_mappings (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  UUID        NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    domain     TEXT        NOT NULL,
    verified   BOOLEAN     NOT NULL DEFAULT false,
    source     TEXT        NOT NULL DEFAULT 'admin',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_tenant_domain_mappings_domain    ON tenant_domain_mappings(lower(domain)) WHERE verified = true;
CREATE INDEX IF NOT EXISTS idx_tenant_domain_mappings_tenant_id ON tenant_domain_mappings(tenant_id);

-- ============================================================================
-- TENANT REPOSITORY BINDINGS
-- One canonical repository binding per tenant.  The binding describes where
-- the tenant's knowledge repository lives and how to access it.
-- The source_owner guard prevents IdP/sync runs from overwriting
-- admin-managed bindings.
-- ============================================================================
CREATE TABLE IF NOT EXISTS tenant_repository_bindings (
    id                        UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID        UNIQUE NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    kind                      TEXT        NOT NULL,
    local_path                TEXT,
    remote_url                TEXT,
    branch                    TEXT        NOT NULL DEFAULT 'main',
    branch_policy             TEXT        NOT NULL DEFAULT 'direct_commit',
    credential_kind           TEXT        NOT NULL DEFAULT 'none',
    credential_ref            TEXT,
    github_owner              TEXT,
    github_repo               TEXT,
    source_owner              TEXT        NOT NULL DEFAULT 'admin',
    git_provider_connection_id TEXT,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tenant_repository_bindings_tenant_id ON tenant_repository_bindings(tenant_id);

-- Add git_provider_connection_id to pre-existing deployments that have the
-- table but were created before this column existed.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'tenant_repository_bindings'
          AND column_name = 'git_provider_connection_id'
    ) THEN
        ALTER TABLE tenant_repository_bindings
            ADD COLUMN git_provider_connection_id TEXT;
    END IF;
END;
$$;
