-- ---------------------------------------------------------------------
-- 032_tenant_delete_cascade.sql
--
-- Issue #RC7-9 (rc.7 provisioning fix-pack)
--
-- Audit & harden ON DELETE behaviour for every FK that points at
-- `tenants(id)`. By the end of this migration:
--
--   * tenant_domain_mappings.tenant_id     CASCADE  (already, 017)
--   * tenant_repository_bindings.tenant_id CASCADE  (already, 017)
--   * tenant_secrets.tenant_id             CASCADE  (already, 026)
--   * organizational_units.tenant_id       CASCADE  (already, 028)
--   * agents.tenant_id                     CASCADE  (this migration)
--   * impersonation audit tables           SET NULL (intentional, 023 -
--                                          deleting a tenant must NOT
--                                          erase historical audit
--                                          rows; 023 stays untouched).
--
-- The motivation is that any future `tenant_provisioner::delete()`
-- path needs a single transactional shape: DELETE FROM tenants
-- WHERE id = $1, relying on the database to propagate cleanup.
-- Without CASCADE on agents.tenant_id the delete fails with 23503
-- foreign_key_violation as soon as a tenant has any registered
-- agent.
--
-- ALTER CONSTRAINT cannot change ON DELETE in PG 15, so we use the
-- standard drop+recreate pattern. The constraint name follows the
-- default `<table>_<column>_fkey` convention from migration 029.
-- ---------------------------------------------------------------------

BEGIN;

-- Defensive: discover the actual constraint name in case a previous
-- run named it differently. Works for both `agents_tenant_id_fkey`
-- and any non-default name.
DO $$
DECLARE
    cname TEXT;
BEGIN
    SELECT con.conname
      INTO cname
      FROM pg_constraint con
      JOIN pg_class       rel ON rel.oid = con.conrelid
      JOIN pg_attribute   att ON att.attrelid = rel.oid
                              AND att.attnum  = ANY (con.conkey)
     WHERE rel.relname = 'agents'
       AND att.attname = 'tenant_id'
       AND con.contype = 'f';

    IF cname IS NULL THEN
        RAISE NOTICE 'agents.tenant_id has no FK constraint; skipping';
        RETURN;
    END IF;

    EXECUTE format('ALTER TABLE agents DROP CONSTRAINT %I', cname);
END
$$;

-- Recreate with ON DELETE CASCADE. An agent that has lost its
-- owning tenant has no meaningful runtime identity (the scope
-- evaluator returns empty and every invocation 403s); cascading
-- avoids ghost rows in listing endpoints.
ALTER TABLE agents
    ADD CONSTRAINT agents_tenant_id_fkey
    FOREIGN KEY (tenant_id)
    REFERENCES tenants(id)
    ON DELETE CASCADE;

COMMIT;

-- Regression guard: docs/operations/tenant-provisioning.md → Hard
-- reset section. The cleanup-tenant.sh script relies on this
-- cascade to drop dependent agents in the same statement.
