-- Provisioning idempotency: tighten constraints so re-applying a tenant
-- manifest is a true no-op when desired state already matches.
--
-- Two issues fixed by this migration:
--
--   1. organizational_units allows duplicate (tenant_id, parent_id, name)
--      tuples. Re-running `tenant apply` after a partial failure inserted
--      duplicate root units (#RC7-2). We add a composite unique index
--      (tenant_id, COALESCE(parent_id,''), name) so create_unit can target
--      it via ON CONFLICT.
--
--   2. tenant_domain_mappings has a non-unique partial index on
--      lower(domain) WHERE verified = true. Two tenants can both verify
--      the same domain, breaking domain → tenant resolution (#RC7-15).
--      We replace the index with a UNIQUE partial index.
--
-- Both statements are guarded so the migration is idempotent.
--
-- WARNING: if existing rows violate either constraint the migration will
-- fail. See docs/operations/tenant-provisioning.md for the cleanup query.

-- 1) Composite unique index on organizational_units --------------------------
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = current_schema()
          AND indexname = 'idx_organizational_units_tenant_parent_name_uniq'
    ) THEN
        CREATE UNIQUE INDEX idx_organizational_units_tenant_parent_name_uniq
            ON organizational_units (tenant_id, COALESCE(parent_id, ''), name);
    END IF;
END;
$$;

-- 2) UNIQUE partial index on tenant_domain_mappings(lower(domain)) WHERE verified -
DROP INDEX IF EXISTS idx_tenant_domain_mappings_domain;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes
        WHERE schemaname = current_schema()
          AND indexname = 'idx_tenant_domain_mappings_domain_uniq'
    ) THEN
        CREATE UNIQUE INDEX idx_tenant_domain_mappings_domain_uniq
            ON tenant_domain_mappings (lower(domain))
            WHERE verified = true;
    END IF;
END;
$$;
