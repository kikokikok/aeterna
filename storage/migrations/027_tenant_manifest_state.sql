-- ============================================================================
-- 027_tenant_manifest_state.sql
--
-- Adds the two columns that let provision_tenant implement idempotent
-- short-circuit and strict-monotonic revision checks, per
-- openspec/changes/harden-tenant-provisioning/design.md D5 + D6 and
-- GitHub #99 (Phase B2 tracking).
--
-- Columns:
--
--   last_applied_manifest_hash TEXT NULL
--       Canonical SHA-256 fingerprint of the last successfully-applied
--       manifest for this tenant. Format: "sha256:" + 64 hex chars
--       (see cli/src/server/manifest_hash.rs::HASH_PREFIX). NULL means the
--       tenant has never been (re-)applied via the B2 idempotent path --
--       either it predates this migration or it was created via direct
--       SQL / CLI bootstrap. A NULL column always triggers a full apply,
--       never a short-circuit; this is safe for older rows.
--
--   manifest_generation BIGINT NOT NULL DEFAULT 0
--       Monotonic revision counter owned by the caller of the manifest
--       apply API. `provision_tenant` enforces strict increase
--       (manifest.metadata.generation > manifest_generation) on every
--       change and rejects conflicts with 409. When the caller omits
--       metadata.generation, the server auto-increments
--       (manifest_generation + 1) and writes that back.
--
--       We deliberately name the column `manifest_generation` rather than
--       `generation`, because `generation` is both a SQL-reserved-adjacent
--       term and ambiguous with other monotonic counters that will exist
--       elsewhere (cache generation, schema generation, etc.). The
--       manifest_ prefix makes the DB column self-documenting. On the
--       wire (TenantManifest.metadata.generation) the prefix is implicit
--       from context.
--
-- Index:
--
--   We do NOT add an index. Lookups are always keyed by slug or id, which
--   already have indices (see 017_tenants_tables.sql). Neither new column
--   is ever used as a WHERE predicate on its own.
--
-- Idempotency:
--
--   IF NOT EXISTS guards keep this migration safe to re-run.
-- ============================================================================

ALTER TABLE tenants
    ADD COLUMN IF NOT EXISTS last_applied_manifest_hash TEXT;

ALTER TABLE tenants
    ADD COLUMN IF NOT EXISTS manifest_generation BIGINT NOT NULL DEFAULT 0;

-- Defence in depth: enforce the sha256:... shape at the DB level so stale
-- writers (pre-B2 code paths, manual ops scripts) cannot insert a value
-- that looks valid but wouldn't decode. NULL remains legal (see column
-- docs above).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'tenants_manifest_hash_format'
    ) THEN
        ALTER TABLE tenants
            ADD CONSTRAINT tenants_manifest_hash_format
            CHECK (
                last_applied_manifest_hash IS NULL
                OR last_applied_manifest_hash ~ '^sha256:[0-9a-f]{64}$'
            );
    END IF;
END$$;

-- Defence in depth: the generation column is owner-chosen but must be
-- non-negative; `0` is reserved as the "never applied" sentinel so no
-- in-wire generation of 0 is ever valid (cli/src/server/tenant_api.rs
-- already rejects metadata.generation == 0 at the schema layer).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'tenants_manifest_generation_nonneg'
    ) THEN
        ALTER TABLE tenants
            ADD CONSTRAINT tenants_manifest_generation_nonneg
            CHECK (manifest_generation >= 0);
    END IF;
END$$;
