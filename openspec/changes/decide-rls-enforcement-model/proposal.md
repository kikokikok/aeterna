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

This change is **decision-first, implementation-staged**. It records the chosen enforcement model in the `runtime-security-hardening` capability so downstream changes inherit a settled premise. The implementation path is a 5-bundle staged rollout; each bundle ships a standalone PR and is independently revertable. Bundle boundaries are structured so that any one bundle landing in prod without the rest cannot introduce a regression.

Three options are analyzed in full in `design.md`; the summary here is normative once this change is accepted:

- **Option A — Activate RLS on every request-scoped connection.** App role loses `BYPASSRLS` in prod, every request runs inside an explicit `BEGIN` with `SET LOCAL app.tenant_id`, and the ~100 repository query sites are refactored to acquire their connection through a `with_tenant_context(ctx, |conn| …)` helper. Real, database-enforced defense in depth. 4–6 weeks, transaction-per-request overhead.
- **Option B — Drop RLS entirely.** Remove all policies, `ALTER TABLE … DISABLE ROW LEVEL SECURITY`, document app-layer-only isolation. Honest about the current reality; loses the defense-in-depth story entirely.
- **Option C — RLS as a test-time gate, not a prod control.** Prod connections stay BYPASSRLS; integration tests run under a dedicated non-BYPASSRLS role and catch missing `WHERE tenant_id = ?` clauses in CI. Captures ~95% of Option A's value at ~5% of the cost. No prod overhead.

**Decision: Option A.** An initial recommendation of Option C was overridden by the reviewing architect (see the #72 commit trail for the pivot). The threat model this change accepts as authoritative is "a compromised or buggy handler on a live production request MUST NOT be able to read another tenant's rows", not merely "a missing `WHERE` clause MUST be caught in CI." Under that threat model Option C is insufficient — it makes the missing clause a `cargo test` failure but leaves the prod database defenseless once the clause is missing at request time — and only Option A provides the property. The cost is justified.

The implementation lands in five stacked bundles (see `tasks.md` for the itemized work and `design.md` §4 for rationale):

- **Bundle A.1 — Hazard fixes (P0 prerequisite).** Close Hazards H1 (session-scope `set_config(..., false)` on a pooled connection) and H2 (dual GUC namespace `app.company_id` vs `app.tenant_id` only partially normalized by migration 024). Safe to ship immediately; MUST ship before A.4 because the role flip weaponizes both hazards into live cross-tenant leaks.
- **Bundle A.2 — Dedicated `aeterna_app` non-BYPASSRLS role + migration + grants.** Infrastructure only. Prod `DATABASE_URL` continues to point at the legacy BYPASSRLS role until A.4. An integration test suite runs every RLS-enabled table under the new role end-to-end on every CI run — this is the "Option C safety net" preserved as a permanent regression guard even though A is the chosen enforcement path.
- **Bundle A.3 — Repository call-site refactor.** Introduce `with_tenant_context(&ctx, |conn| async { … })` helper in `storage/src/postgres.rs`. Refactor the ~100 query sites in 4–6 domain-themed waves (user → org → project → team → governance → audit → sync → backup/gdpr), each wave a standalone PR. Post-refactor, `grep` for direct `self.pool.acquire()` on RLS-protected paths fails CI via a new `deny_lint`.
- **Bundle A.4 — Prod flip.** Ship a feature flag (`AETERNA_DB_ROLE=bypassrls|rls`) that controls which connection string the pool opens on. Default remains `bypassrls` at flag-land time. Canary environments flip to `rls` first; prod flips after a minimum 2-week soak. Rollback is a single env-var change plus pool restart.
- **Bundle A.5 — Remove BYPASSRLS escape hatch + ratchets.** Delete the `bypassrls` code path, remove the `activate_tenant_context` dead calls in `sync.rs` (Hazard H3), graduate the `deny_lint` for direct pool access from warn to deny, update `AGENTS.md`/`DEVELOPER_GUIDE.md` to reflect the final model.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `runtime-security-hardening`: rewrites the PostgreSQL isolation requirement to make row-level security the authoritative runtime control (previously: application layer was authoritative, RLS was a paper artifact); adds supporting requirements for per-request transaction-scoped tenant context, non-BYPASSRLS application role in production, session-variable hygiene, and CI verification under the non-BYPASSRLS role. These requirements become enforceable progressively as each bundle lands; the final-state wording in the spec delta represents the post-A.5 world and is the target the bundles drive toward.

## Not In Scope

- Replacing the `app.tenant_id` / `app.company_id` GUC namespace with a single unified variable. Migration 024 partially normalized this and Hazard H2 (Bundle A.1 of `tasks.md`) completes the job for the paths the app actually traverses; a full GUC consolidation migration is deferred as a separate change.
- Changing the tenant resolution middleware. This change is about enforcement at the DB layer; the request-scoped tenant context is assumed to already be correct when it reaches the connection pool.
- Removing app-layer `WHERE tenant_id = ?` clauses. They remain as defense in depth on top of the new primary enforcement; `AGENTS.md` is updated to describe both layers as required.
- Option C's standalone landing. The integration test suite described in Option C ships inside Bundle A.2 as a permanent regression guard, but the decision on the enforcement model is Option A.
