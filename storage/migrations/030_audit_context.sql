-- Migration 030: Audit log request-context extensions (B2 §11.1)
--
-- Adds five nullable columns to `governance_audit_log` so provision-path
-- mutations can record WHO they came from (via), WHICH client version,
-- WHICH manifest they were applied against (manifest_hash + generation),
-- and WHETHER the call was a dry-run.
--
-- All columns are NULLABLE because:
--   * Existing rows predate this migration and have no natural default.
--   * Non-provision audit actions (login, token revoke, governance config
--     changes) legitimately have no manifest_hash/generation/dry_run.
--   * `via` is filled by the §11.2 middleware; calls from pre-middleware
--     code paths (e.g. system-initiated background jobs) will stay NULL
--     rather than be silently mislabelled as "api".
--
-- Column semantics (see also openspec/changes/harden-tenant-provisioning):
--   via             : normalized client kind -- 'cli' | 'ui' | 'api' (see §11.3)
--   client_version  : free-form version string the client self-reports
--                     (e.g. 'aeterna-cli/0.8.0-rc.3', 'admin-ui/2026.4.23').
--                     Stored verbatim, not validated -- this field is for
--                     forensics, not authorization.
--   manifest_hash   : the `tenant_manifest_state.hash` value at the moment
--                     the action was recorded. Plain TEXT (no FK) because
--                     audit rows must survive tenant deletion.
--   generation      : the manifest generation counter at record time.
--                     BIGINT to match tenant_manifest_state.generation.
--   dry_run         : TRUE if this audit row corresponds to a dry-run /
--                     validation call that did not mutate state. Lets
--                     `/govern/audit?dry_run=false` filter out noise in
--                     compliance exports.

ALTER TABLE governance_audit_log
    ADD COLUMN IF NOT EXISTS via            TEXT,
    ADD COLUMN IF NOT EXISTS client_version TEXT,
    ADD COLUMN IF NOT EXISTS manifest_hash  TEXT,
    ADD COLUMN IF NOT EXISTS generation     BIGINT,
    ADD COLUMN IF NOT EXISTS dry_run        BOOLEAN;

-- `via` is the primary new filter dimension -- expect queries like
-- "show me all CLI-originated provision applies in the last 24h".
-- A partial index on non-NULL values keeps pre-migration rows out
-- of the index entirely.
CREATE INDEX IF NOT EXISTS idx_governance_audit_via
    ON governance_audit_log(via)
    WHERE via IS NOT NULL;

-- Manifest-hash lookups support the "what was applied when" forensics
-- use case (did this deploy correspond to manifest v3 or v4?).
CREATE INDEX IF NOT EXISTS idx_governance_audit_manifest_hash
    ON governance_audit_log(manifest_hash)
    WHERE manifest_hash IS NOT NULL;

-- Data-integrity guards. Kept as CHECKs rather than enums to avoid a
-- schema-level coupling between storage and the normalization table in
-- Rust; §11.3 is authoritative on the allowed set. This constraint is
-- the last line of defence against a caller bypassing the middleware
-- and inserting garbage. NULL is explicitly allowed (see preamble).
ALTER TABLE governance_audit_log
    DROP CONSTRAINT IF EXISTS governance_audit_log_via_check;
ALTER TABLE governance_audit_log
    ADD CONSTRAINT governance_audit_log_via_check
    CHECK (via IS NULL OR via IN ('cli', 'ui', 'api'));

COMMENT ON COLUMN governance_audit_log.via IS
    'Normalized client kind at request time: cli | ui | api. Populated from X-Aeterna-Client-Kind header by auth middleware (migration 030, B2 §11.2). Unknown/unset values normalize to api; original header value is preserved in the request-scoped RequestContext but NOT persisted here.';
COMMENT ON COLUMN governance_audit_log.client_version IS
    'Self-reported client version string (e.g. aeterna-cli/0.8.0-rc.3). Forensic-only; not validated, not used for authorization.';
COMMENT ON COLUMN governance_audit_log.manifest_hash IS
    'tenant_manifest_state.hash at record time (no FK -- audit rows must outlive tenant deletion).';
COMMENT ON COLUMN governance_audit_log.generation IS
    'tenant_manifest_state.generation at record time.';
COMMENT ON COLUMN governance_audit_log.dry_run IS
    'TRUE for validation / dry-run calls that did not mutate state.';
