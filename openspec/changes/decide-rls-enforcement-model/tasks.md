# Tasks

## 1. Decision ratification

- [ ] 1.1 Review `proposal.md` and `design.md` with Christian / Kyriba security reviewer
- [ ] 1.2 Record the accept/decline decision on issue #58 as a top-level comment referencing this change
- [ ] 1.3 If declined, document the alternative chosen (Option A or B) and archive this change without merging

## 2. Migration: dedicated non-BYPASSRLS role

- [ ] 2.1 Add `storage/migrations/025_add_rls_test_role.sql` that `CREATE ROLE aeterna_app_rls LOGIN NOBYPASSRLS PASSWORD 'test_only_insecure_override_in_ci'` (idempotent `DO $$ … IF NOT EXISTS …`)
- [ ] 2.2 Grant `USAGE` on `public` schema to `aeterna_app_rls`
- [ ] 2.3 Grant `SELECT, INSERT, UPDATE, DELETE` on every table currently having `ENABLE ROW LEVEL SECURITY` (22 tables) to `aeterna_app_rls`
- [ ] 2.4 Grant `USAGE, SELECT` on every sequence the granted tables depend on
- [ ] 2.5 Document in the migration file header: "Test-only role. Prod deploys continue to use a BYPASSRLS role (typically `postgres`). See `openspec/specs/runtime-security-hardening/spec.md` ‘RLS Test-Time Enforcement’."

## 3. Session-variable hygiene (Hazard H1)

- [ ] 3.1 `storage/src/postgres.rs:63` — change `set_config('app.tenant_id', $1, false)` to `set_config('app.tenant_id', $1, true)` and wrap the call site in an explicit `BEGIN`/`COMMIT` (the `activate_tenant_context` helper becomes transaction-aware)
- [ ] 3.2 `cli/src/server/backup_api.rs:1597` — same fix; the surrounding handler already opens a transaction for the backup export, so this is a one-character change plus moving the `set_config` inside the existing `BEGIN`
- [ ] 3.3 `storage/src/gdpr.rs:407,490,580` — already uses `true`; verify each call is inside an explicit transaction and document the invariant in a module-level `// INVARIANT:` comment
- [ ] 3.4 `cli/src/server/sync.rs:131,256` — document that `activate_tenant_context` is currently a no-op under the prod BYPASSRLS role; under the test RLS role it is load-bearing. No code change required.

## 4. Integration test suite

- [ ] 4.1 Add `cli/tests/rls_enforcement_test.rs` with a shared `rls_pool()` helper that clones `DATABASE_URL`, replaces the username/password with `aeterna_app_rls` / the role password, and returns a fresh `PgPool`
- [ ] 4.2 Pre-flight test `rls_role_has_required_grants` — enumerates `pg_tables WHERE rowsecurity = true`, asserts `aeterna_app_rls` has `SELECT` on each (fails with the table name if a grant is missing)
- [ ] 4.3 Per-table isolation test `rls_isolates_{table}_by_tenant` (parameterized via a `#[rstest]` or macro over the 22 tables) — positive (context set → tenant A rows) + negative (context unset → 0 rows)
- [ ] 4.4 Handler-level test `rls_list_endpoints_return_only_active_tenant` — spin up the test server with its pool pointed at `aeterna_app_rls`, hit `/user`, `/project`, `/org`, `/govern/audit`, assert only the authenticated tenant's rows
- [ ] 4.5 Session-variable-leak test `tenant_context_does_not_leak_across_pool_checkouts` — the scenario from spec §"Tenant Session Variable Hygiene" / "Test suite catches session-scope misuse"

## 5. Documentation

- [ ] 5.1 `AGENTS.md` — add a section "Multi-tenancy enforcement model" pointing to `runtime-security-hardening` and stating: prod = app-layer `WHERE tenant_id = ?`, CI = RLS under `aeterna_app_rls`
- [ ] 5.2 `DEVELOPER_GUIDE.md` — add a subsection describing how to write a new repository query against an RLS-protected table: always `WHERE tenant_id = $N`, never rely on RLS in prod, expect the CI suite to fail if you miss the clause
- [ ] 5.3 `storage/migrations/README.md` — append a row to the migration index for 025 and a note about the role in the "Conventions" section

## 6. CI wiring

- [ ] 6.1 Verify the CI compose file / action runs `migrations/025_…` as part of the test DB setup (it should — migrations run in order — but document the expectation)
- [ ] 6.2 Add a CI job step that sets `AETERNA_RLS_TEST_PASSWORD` from a GitHub secret (matches the password used by the migration in dev/test)

## 7. Follow-up (out of scope for this change, filed as new issues on merge)

- [ ] 7.1 File issue: "GUC namespace normalization: migrate remaining `app.company_id` / `app.current_tenant_id` policies to `app.tenant_id`" (continuation of migration 024)
- [ ] 7.2 File issue: "Repository-layer audit: confirm every SELECT against an RLS-protected table carries a `WHERE tenant_id = ?` clause" — mechanical sqlx-macro-level audit
- [ ] 7.3 File issue: "`sync.rs` `activate_tenant_context` call sites: either remove as dead code under prod BYPASSRLS, or document them as load-bearing-under-CI"
