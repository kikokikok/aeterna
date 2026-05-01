-- 033_tenant_legal_entity.sql
--
-- Adds an optional `legal_entity_name` text column to the tenants table.
--
-- This is the metadata seed for the `add-legal-entity-tenant-grouping`
-- proposal: it lets sales/ops record which corporate entity a tenant
-- belongs to (e.g. "Acme Holding") today, on a column with no
-- foreign-key constraints, no auth implications, and no impact on RLS.
--
-- The proposal-level work — a first-class `legal_entities` table, a
-- `LegalEntityAdmin` principal that can read across the tenants of one
-- legal entity, and admin-ui navigation by legal entity — is explicitly
-- *out of scope* for this migration. When that lands it will use this
-- column as the migration source: each distinct non-NULL value becomes
-- a `legal_entities` row, and the column is rewritten to a FK.
--
-- Why a string rather than a structured FK now:
--   - we don't yet have legal_entities to point at;
--   - sales/ops needs to record this *today*, in rc.9, not after the
--     epic ships;
--   - leaving the column nullable means existing tenants are unaffected
--     and there is no data migration debt;
--   - any subsequent migration that introduces the FK can populate from
--     this column losslessly.
--
-- All statements are idempotent (IF NOT EXISTS) so re-running this
-- migration against a cluster that already has the column is a no-op,
-- consistent with the rest of storage/migrations/.

ALTER TABLE tenants
    ADD COLUMN IF NOT EXISTS legal_entity_name TEXT;

COMMENT ON COLUMN tenants.legal_entity_name IS
    'Optional human-readable name of the corporate legal entity that owns this tenant. '
    'Pure metadata in v1.5.x; will be promoted to a FK against the legal_entities table '
    'when the add-legal-entity-tenant-grouping proposal ships.';

-- Lookup index for the future "all tenants of legal entity X" query.
-- Partial index: most tenants will have NULL here, no need to bloat the
-- B-tree with them.
CREATE INDEX IF NOT EXISTS idx_tenants_legal_entity_name
    ON tenants(legal_entity_name)
    WHERE legal_entity_name IS NOT NULL;
