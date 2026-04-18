-- Migration 023: Platform-admin impersonation foundation
--
-- Background:
--   Implements the schema side of OpenSpec change
--   `refactor-platform-admin-impersonation` (issue #44).
--
-- Two independent schema additions:
--
--   1. users.default_tenant_id
--      A portable server-side "default tenant" preference consumed by the
--      new tenant resolution chain (see RequestContext in cli/src/server/).
--      NULL means "no preference set" (the historic default and the only
--      valid state at migration time -- no backfill required).
--      ON DELETE SET NULL so deleting a tenant silently clears the
--      preference for affected users rather than cascading.
--
--   2. {referential,governance}_audit_log.acting_as_tenant_id
--      Records the tenant the actor was impersonating at the time of the
--      logged action. For non-impersonated actions this equals the actor's
--      own tenant membership (or NULL for PlatformAdmin acting on the
--      platform). Required to distinguish "PlatformAdmin operated inside
--      tenant X" from "user inside tenant X did the thing".
--      References tenants(id) with ON DELETE SET NULL so audit rows
--      survive tenant deletion (history must be preserved even if the
--      target tenant is gone).
--
-- All statements are idempotent (IF NOT EXISTS) so running this migration
-- against a cluster that already has the columns is a no-op.
--
-- Downgrade: both columns are safely droppable (NULL default, no
-- dependent code until migration 024+ lands). See
-- storage/migrations/README.md for the revert procedure.

-- ============================================================================
-- users.default_tenant_id
-- ============================================================================
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS default_tenant_id UUID
    REFERENCES tenants(id) ON DELETE SET NULL;

COMMENT ON COLUMN users.default_tenant_id IS
    'Portable per-user default tenant preference. Consumed by the tenant '
    'resolution chain when X-Tenant-ID is absent. NULL = no preference. '
    'Cleared automatically if the referenced tenant is deleted '
    '(ON DELETE SET NULL).';

CREATE INDEX IF NOT EXISTS idx_users_default_tenant_id
    ON users(default_tenant_id)
    WHERE default_tenant_id IS NOT NULL AND deleted_at IS NULL;

-- ============================================================================
-- referential_audit_log.acting_as_tenant_id
-- ============================================================================
ALTER TABLE referential_audit_log
    ADD COLUMN IF NOT EXISTS acting_as_tenant_id UUID
    REFERENCES tenants(id) ON DELETE SET NULL;

COMMENT ON COLUMN referential_audit_log.acting_as_tenant_id IS
    'The tenant scope the actor was operating in when this event was '
    'recorded. NULL means platform-scoped (PlatformAdmin without a target '
    'tenant) or historic rows predating migration 023. When this differs '
    'from the actor''s own tenant membership, the event represents '
    'PlatformAdmin impersonation.';

CREATE INDEX IF NOT EXISTS idx_referential_audit_log_acting_as_tenant
    ON referential_audit_log(acting_as_tenant_id)
    WHERE acting_as_tenant_id IS NOT NULL;

-- ============================================================================
-- governance_audit_log.acting_as_tenant_id
-- ============================================================================
ALTER TABLE governance_audit_log
    ADD COLUMN IF NOT EXISTS acting_as_tenant_id UUID
    REFERENCES tenants(id) ON DELETE SET NULL;

COMMENT ON COLUMN governance_audit_log.acting_as_tenant_id IS
    'The tenant scope the actor was operating in when this governance '
    'event was recorded. See referential_audit_log.acting_as_tenant_id '
    'for the full semantics.';

CREATE INDEX IF NOT EXISTS idx_governance_audit_log_acting_as_tenant
    ON governance_audit_log(acting_as_tenant_id)
    WHERE acting_as_tenant_id IS NOT NULL;
