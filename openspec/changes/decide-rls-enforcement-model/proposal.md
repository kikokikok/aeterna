# Decide RLS Enforcement Model

## Why

The database layer has 22 tables with `ENABLE ROW LEVEL SECURITY` and 5 with `FORCE ROW LEVEL SECURITY` — across `storage/migrations/004_enable_rls.sql`, `016_governance_rls.sql`, and `024_normalize_rls_session_variables.sql`. Every policy is keyed off `current_setting('app.tenant_id', true)` (or `app.company_id`, `app.current_tenant_id` in older policies). Yet `PostgresBackend::activate_tenant_context` is called in exactly **2 code paths**, both in `cli/src/server/sync.rs`.

The remaining 100+ query sites across `cli/src/server/*_api.rs` never set the session variable before running a query. There are only two ways that can be true while tests still pass:

1. **`current_setting('app.tenant_id', true)` returns NULL** on those queries → RLS policies evaluate `tenant_id = NULL` → every row excluded → queries return empty. This contradicts observed behavior — tests pass and features work.
2. **The app's DB role has `BYPASSRLS`** (or is the table owner and tables do not have `FORCE ROW LEVEL SECURITY`). RLS is decorative; the *actual* tenant isolation is the app-level `WHERE tenant_id = $1` clauses sprinkled throughout the repository layer.

Option 2 matches reality. No migration creates an explicit app role; the connection uses whatever Postgres user the operator's `DATABASE_URL` points at, which for a default compose setup is `postgres` (superuser, implicit BYPASSRLS). The governance tables that use `FORCE ROW LEVEL SECURITY` only work because `backup_api.rs` and `gdpr.rs` paths call `SELECT set_config('app.tenant_id', $1, false)` before their queries — and the `false` third argument makes it session-level, which leaks to the next request borrowing that pooled connection.

The result is a defense-in-depth story that reads well in security reviews but does not actually exist at runtime. Any missed `WHERE tenant_id = ?` in application code becomes a cross-tenant data leak. The one path that *does* call `activate_tenant_context` is itself a no-op because it runs under BYPASSRLS too.

This is an **architectural decision**, not a patch. Issue #58 explicitly asks for a recorded decision under `openspec/changes/` before any implementation work begins.

## What Changes

This change is **decision-first, implementation-conditional**. It records the chosen enforcement model in the `runtime-security-hardening` capability so downstream changes inherit a settled premise. The implementation path attached to this change is only the work needed to make the decision true; options not selected stay out of scope.

Three options are analyzed in full in `design.md`; the summary here is normative once this change is accepted:

- **Option A — Activate RLS on every request-scoped connection.** Middleware-owned connection handle, `SET LOCAL app.tenant_id = $1` inside a BEGIN per request, app role loses BYPASSRLS in prod. Genuine defense in depth; ~100-site repository refactor plus transaction-per-request overhead. Inherits the session-variable leak hazard already present in `backup_api.rs`.
- **Option B — Drop RLS entirely.** Remove all policies, `ALTER TABLE … DISABLE ROW LEVEL SECURITY`, document app-layer-only isolation. Honest and cheap; loses the paper defense-in-depth story that Kyriba security review may require.
- **Option C — RLS as a test-time gate, not a prod control.** Prod connections stay BYPASSRLS; integration tests explicitly `SET ROLE aeterna_app_rls` (non-BYPASSRLS) and `activate_tenant_context` before exercising handlers. Missing `WHERE tenant_id = ?` in app code fails tests, never prod. Low prod overhead, real compile-time-ish guarantee via CI.

**Decision:** Option C, with two guardrails (see `design.md` §3 for full rationale):

1. Create an explicit `aeterna_app_rls` role via migration, non-BYPASSRLS, granted `USAGE` + per-table `SELECT/INSERT/UPDATE/DELETE`.
2. A new integration test suite runs under that role end-to-end for the `user`, `project`, `org`, `memory`, `knowledge`, `governance` list/get paths, exercising both the positive path (tenant context set → rows returned) and the negative path (tenant context unset → 0 rows).

The prod `DATABASE_URL` user stays as-is (typically superuser / BYPASSRLS). We do NOT flip prod to non-BYPASSRLS: the per-request transaction overhead and repository-site refactor are not justified by the threat model improvement, and the session-variable leak bug surfaced above would need fixing first anyway.

Follow-on work to fix the prior-art hazards surfaced by this analysis is in `tasks.md`:

- Fix the `set_config(..., false)` → `set_config(..., true)` typo pattern in `backup_api.rs:1597`, `gdpr.rs:407/490/580`, and `storage/src/postgres.rs:63` so session state never leaks across pooled requests (whether or not we adopt Option A later).
- Audit the remaining repository sites for missing `WHERE tenant_id = ?` clauses on RLS-protected tables. This is the real enforcement surface.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `runtime-security-hardening`: adds a normative requirement (RLS is a test-time gate, not a prod enforcement control) and a supporting requirement (test-role setup, session-variable hygiene). No behavioral change to any existing runtime surface — only a statement of intent that makes the current implementation honest and protects future contributors from assuming RLS blocks cross-tenant reads in prod.

## Not In Scope

- Flipping prod connections to non-BYPASSRLS (Option A). Tracked separately for reconsideration if the threat model changes.
- Removing RLS policies / dropping the RLS GUC infrastructure (Option B). Retained for test-time enforcement.
- Refactoring the ~100 repository call sites to explicitly filter by `tenant_id` — they already do; this change documents that the app layer is the authoritative enforcement point, it does not move the line.
