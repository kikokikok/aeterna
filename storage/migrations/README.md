# Schema Migrations

SQL files in this directory are applied in numeric order by the Aeterna migration runner (`cli/src/server/bootstrap.rs` for local/dev, and the Helm post-install job for cluster installs).

## Conventions

- **Filename**: `NNN_short_description.sql` where `NNN` is a zero-padded three-digit sequence number. Next free slot is the maximum existing number + 1.
- **Idempotency**: every statement must be safe to re-run. Use `IF NOT EXISTS` on `CREATE`, `ADD COLUMN IF NOT EXISTS` on `ALTER TABLE`, guarded `DO $$ ... $$` blocks for conditional changes, etc.
- **No transactions**: do not wrap the file in `BEGIN; ... COMMIT;`. The runner already applies each file inside a transaction and records the checksum in `_aeterna_migrations`.
- **Header comment**: top of the file must describe what the migration does and why. Future readers depend on this -- the commit message is harder to find.
- **No data backfill in the SQL**: if a migration needs to rewrite existing rows, keep the column addition here and put the backfill in a separate one-shot script under `storage/backfills/`. This keeps migrations fast and deterministic.

## Downgrade / revert

Aeterna does **not** auto-generate down-migrations. If a migration needs to be reverted:

1. Write a new forward migration (`NNN+1`) that undoes the change.
2. Never edit an applied migration file -- the checksum in `_aeterna_migrations` will mismatch and bootstrap will refuse to start.

### Per-migration downgrade notes

- **024_normalize_rls_session_variables.sql** -- rewrites three RLS policies from `app.current_tenant_id` to `app.tenant_id`. To revert, re-run the original `CREATE POLICY` blocks from `006_event_streaming.sql:120-138` after `DROP POLICY IF EXISTS`. No data change; policies are metadata only. See issue #59 for context.
- **023_platform_admin_impersonation.sql** -- safe to drop. All three added columns default to `NULL` and are not read by any code until migration 024+. To revert:
  ```sql
  ALTER TABLE users                  DROP COLUMN IF EXISTS default_tenant_id;
  ALTER TABLE referential_audit_log  DROP COLUMN IF EXISTS acting_as_tenant_id;
  ALTER TABLE governance_audit_log   DROP COLUMN IF EXISTS acting_as_tenant_id;
  DROP INDEX IF EXISTS idx_users_default_tenant_id;
  DROP INDEX IF EXISTS idx_referential_audit_log_acting_as_tenant;
  DROP INDEX IF EXISTS idx_governance_audit_log_acting_as_tenant;
  ```
- **022_drop_dead_vector_columns.sql** -- columns were already unused. Reverting is not useful; re-run on clusters with leftover `VECTOR` columns is a no-op via `IF EXISTS`.

## Checksum mismatches

If bootstrap reports `checksum mismatch on migration NNN`, it means the applied SQL on disk differs from what was recorded when the migration first ran. Fix options, in order of preference:

1. **Revert the local edit** to the migration file so it matches the historic bytes, and add a new migration for the new change.
2. In dev databases: wipe the DB (`docker compose down -v`) and re-bootstrap from scratch.
3. In prod: manually update `_aeterna_migrations.checksum` after carefully reviewing the diff. Only do this when you understand exactly what changed and why.

## Testing

Before shipping a migration:

```bash
# Apply against a clean DB
docker compose up -d postgres
cargo run --bin aeterna -- server bootstrap --skip-serve

# Apply against a DB that already has it (idempotency check)
cargo run --bin aeterna -- server bootstrap --skip-serve

# Verify schema
docker compose exec postgres psql -U aeterna -d aeterna -c '\\d+ <table>'
```
