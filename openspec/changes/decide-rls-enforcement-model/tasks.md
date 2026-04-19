# Tasks

## 1. Decision ratification

- [ ] 1.1 Review `proposal.md` and `design.md` with Christian / Kyriba security reviewer
- [ ] 1.2 Record the accept/decline decision on issue #58 as a top-level comment referencing this change
- [ ] 1.3 Lock the threat model (see `design.md` §2): "compromised or buggy handler on a live production request MUST NOT be able to read another tenant's rows." Subsequent bundles inherit this.

## 2. Bundle A.1 — Hazard fixes (P0 prerequisite, independently shippable)

**Goal:** close Hazards H1 and H2 before any role flip. Safe to merge immediately; no behavior change under BYPASSRLS, essential invariant once BYPASSRLS is revoked. Target: 1 PR, ≤ 1 week.

- [ ] 2.1 **H1 — session-scope leak in `storage/src/postgres.rs:63`.** Change `set_config('app.tenant_id', $1, false)` → `set_config('app.tenant_id', $1, true)` and make `activate_tenant_context` require an open transaction. Caller contract documented via rustdoc; a `debug_assert!` at entry checks the connection is inside a transaction (via `SELECT current_setting('transaction_isolation')` side-effect semantics or equivalent).
- [ ] 2.2 **H1 — `cli/src/server/backup_api.rs:1597`.** Same fix; the handler already opens a transaction for the backup export, so this is a one-character change plus moving the `set_config` call inside the existing `BEGIN` scope.
- [ ] 2.3 **H1 — `cli/src/server/gdpr.rs:407,490,580`.** Audit each call: `true` is already passed (good), but verify each is inside an explicit transaction and add a module-level `// INVARIANT:` comment pinning the expectation for future edits.
- [ ] 2.4 **H2 — dual-GUC normalization.** Grep every `set_config` and `current_setting` call in the codebase for the four variants (`app.tenant_id`, `app.company_id`, `app.current_tenant_id`, `app.current_company_id`). Normalize every code-side write to `app.tenant_id`. Every code-side read to `app.tenant_id`. Migration 024 already normalized the policies; this closes the read/write side.
- [ ] 2.5 **H1 regression test.** `storage/tests/session_variable_hygiene.rs` — acquire a connection from the pool, set `app.tenant_id` via `activate_tenant_context`, return it to the pool, acquire a fresh connection, assert `current_setting('app.tenant_id', true)` returns empty. Fails if anyone reverts to session-scope `set_config`.
- [ ] 2.6 **H2 regression test.** `storage/tests/guc_namespace.rs` — grep assertion (compile-time via `include_str!`) that no source file outside of `storage/migrations/` contains `app.company_id` or `app.current_tenant_id`. Fails if a new handler re-introduces the legacy namespace.

## 3. Bundle A.2 — Dedicated non-BYPASSRLS role + migration + CI verification (independently shippable)

**Goal:** stand up the `aeterna_app` role, make it grant-complete, and wire a CI suite that runs every RLS-enabled table through it end-to-end. Prod `DATABASE_URL` is NOT changed. Target: 1 PR, ≤ 2 weeks.

- [ ] 3.1 `storage/migrations/025_add_app_role.sql` — `CREATE ROLE aeterna_app LOGIN NOBYPASSRLS PASSWORD 'managed_via_secret'` (idempotent via `DO $$ … pg_roles WHERE rolname = 'aeterna_app' … $$`). Password is an `${APP_DB_PASSWORD}` placeholder consumed by the migration runner.
- [ ] 3.2 `GRANT USAGE ON SCHEMA public TO aeterna_app`.
- [ ] 3.3 `GRANT SELECT, INSERT, UPDATE, DELETE` on every table currently having `rowsecurity = true` (22 tables) to `aeterna_app`. Enumerated explicitly — no `GRANT … ON ALL TABLES` catch-all, so adding a new RLS-protected table without updating the grant fails the A.2.5 pre-flight.
- [ ] 3.4 `GRANT USAGE, SELECT` on every sequence the granted tables depend on.
- [ ] 3.5 Migration header documents: "Non-BYPASSRLS application role. Bundle A.4 flips `DATABASE_URL` to this role. See `openspec/changes/decide-rls-enforcement-model/proposal.md`."
- [ ] 3.6 **CI suite — `cli/tests/rls_enforcement_test.rs`.** For every table with `rowsecurity = true`:
  - [ ] 3.6.1 Pre-flight: enumerate `pg_tables WHERE rowsecurity = true`; assert each has been granted `SELECT` to `aeterna_app`; fail with a named-table error if a grant is missing.
  - [ ] 3.6.2 Positive path: open a connection as `aeterna_app`, `BEGIN`, `set_config('app.tenant_id', tenant_a, true)`, `SELECT *` — assert only tenant_a's rows.
  - [ ] 3.6.3 Negative path: same connection without `set_config` — assert zero rows returned.
  - [ ] 3.6.4 Cross-tenant path: seed rows for tenants A and B, set context to A, attempt `SELECT` for B's rows by WHERE id — assert zero rows.
- [ ] 3.7 **CI suite — handler-level `cli/tests/rls_handler_smoke.rs`.** Run `/user`, `/project`, `/org`, `/govern/audit` list endpoints against a test server whose pool connects as `aeterna_app`. Assert each list returns only the authenticated tenant's rows; assert a request with no resolved tenant context returns 400 `select_tenant` or an empty list (never foreign rows).
- [ ] 3.8 Document in `DEVELOPER_GUIDE.md` how to run the A.2 suites locally (`docker compose -f docker-compose.rls.yml up` pattern).

## 4. Bundle A.3 — Repository call-site refactor (multi-PR, wave-by-wave)

**Goal:** thread per-request transaction-scoped tenant context through every RLS-protected query. Each wave is a standalone PR, each wave is independently revertable, each wave runs against the A.2 CI suite so regressions surface immediately. Target: 4–6 PRs, ≤ 3 weeks.

- [ ] 4.1 **Helper — `storage/src/postgres.rs::with_tenant_context`.**
  - Signature: `async fn with_tenant_context<F, T>(&self, ctx: &TenantContext, body: F) -> Result<T> where F: FnOnce(&mut PgConnection) -> BoxFuture<Result<T>>`.
  - Implementation: `self.pool.begin()` → `SET LOCAL app.tenant_id = $1` → body(&mut tx) → `tx.commit()`.
  - Rustdoc states every RLS-protected query MUST acquire its connection this way; direct `pool.acquire()` is forbidden on RLS paths and will be enforced by lint in Bundle A.5.
- [ ] 4.2 **Wave 1 — `user_api.rs` + `team_api.rs` + `org_api.rs` + `project_api.rs` list/get/create/update/delete paths.** Refactor each call site. CI suite (A.2) proves per-request isolation post-refactor. PR size target: ~25 call sites.
- [ ] 4.3 **Wave 2 — `govern_api.rs` + `govern_policy.rs` + audit write paths.** ~15 call sites.
- [ ] 4.4 **Wave 3 — `memory_api.rs` + `knowledge_api.rs` + ingest paths.** ~20 call sites.
- [ ] 4.5 **Wave 4 — `sync.rs` + `webhook.rs` + async worker paths.** Async workers need their own tenant-context acquisition pattern (they don't have a request); establish and document the pattern. ~15 call sites.
- [ ] 4.6 **Wave 5 — `backup_api.rs` + `gdpr.rs` + admin paths.** These already did manual `set_config` (fixed in A.1); convert to the helper. ~10 call sites.
- [ ] 4.7 **Wave 6 — sweep.** Audit tool: `storage/tools/find_unwrapped_queries.sh` that greps for `self.pool.acquire()` / `self.pool.begin()` outside of `with_tenant_context`. Any remaining sites either (a) are RLS-protected and the sweep wave fixes them, or (b) are documented as intentionally context-free (e.g., health checks) with an `#[allow(direct_pool_access)]` attribute and a justification comment.
- [ ] 4.8 Add CI `deny_lint` (warn-level) that fails on direct `self.pool.acquire()` / `.begin()` outside the allowlist. Graduates to deny-level in Bundle A.5.

## 5. Bundle A.4 — Prod flip (feature-flagged, canary-first)

**Goal:** change the prod connection role from the legacy BYPASSRLS user to `aeterna_app`. Rollback is a single env-var change. Target: 1 PR to land the flag, then operational rollout.

- [ ] 5.1 Add `AETERNA_DB_ROLE` env var (`bypassrls|rls`, default `bypassrls`) consumed in `cli/src/server/app_state.rs` when building the pool.
- [ ] 5.2 `bypassrls` keeps the existing `DATABASE_URL` behavior.
- [ ] 5.3 `rls` composes a new connection string pointing at `aeterna_app` with the password from `APP_DB_PASSWORD`.
- [ ] 5.4 Startup log prints the effective role (`info!("db_role={role}")`) — searchable post-incident.
- [ ] 5.5 Flag-land PR includes zero env changes in deployed config: prod keeps `bypassrls` at merge time.
- [ ] 5.6 **Canary rollout** (ops-tracked, not a PR): flip `AETERNA_DB_ROLE=rls` in `dev` first, then `staging`, then per-region prod canaries. Minimum 2-week soak on staging before any prod region.
- [ ] 5.7 **Rollback runbook** in `docs/ops/rls_rollback.md`: flip env var → rolling pool restart → verify log line — under 5 minutes per region.
- [ ] 5.8 Post-flip observability: Grafana panel for `pg_stat_activity.state = 'active' transaction_duration_ms` distribution (detect BEGIN-per-request overhead regression).

## 6. Bundle A.5 — Remove escape hatch + ratchet (independently shippable)

**Goal:** remove the `bypassrls` code path once all environments have soaked on `rls` for ≥ 4 weeks. Graduate the lint to deny. Close Hazard H3. Target: 1 PR.

- [ ] 6.1 Delete the `bypassrls` branch in the pool builder; `AETERNA_DB_ROLE` env var becomes a no-op (or is removed after a deprecation cycle).
- [ ] 6.2 Delete the orphan `activate_tenant_context` calls in `cli/src/server/sync.rs:131,256` (Hazard H3) — they become redundant once every call site uses `with_tenant_context`.
- [ ] 6.3 Graduate the A.3.8 `deny_lint` from warn to deny.
- [ ] 6.4 Update `AGENTS.md` to describe the final two-layer model: RLS is authoritative, app-layer `WHERE tenant_id = ?` is defense in depth. Both remain required.
- [ ] 6.5 Update `DEVELOPER_GUIDE.md` with the `with_tenant_context` pattern as the canonical query-acquisition idiom.
- [ ] 6.6 Close issue #58 referencing the merged A.5 PR.
