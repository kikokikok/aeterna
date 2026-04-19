# Design: RLS Enforcement Model

## 1. Current State

### 1.1 What exists on paper

| Migration | Tables | Policy shape |
|-----------|--------|--------------|
| `004_enable_rls.sql` | `sync_state`, `memory_entries`, `knowledge_items` | `USING (tenant_id = current_setting('app.tenant_id', true)::uuid)` |
| `016_governance_rls.sql` | `governance_configs`, `approval_requests`, `governance_roles`, `approval_decisions`, `escalation_queue` | `FORCE ROW LEVEL SECURITY` + scope helper functions keyed on `current_setting('app.company_id', true)` |
| `024_normalize_rls_session_variables.sql` | `governance_events`, `event_delivery_metrics`, `event_consumer_state` | `USING (tenant_id = current_setting('app.tenant_id', true))` |
| various intermediate migrations | 14 other tables | mix of `app.tenant_id` / `app.current_tenant_id` (normalization pass deferred) |

Total: **22 tables, ENABLE; 5 of those additionally FORCE**.

### 1.2 What exists in code

`activate_tenant_context` call sites (repo-wide grep):

```
storage/src/postgres.rs:63        set_config('app.tenant_id', $1, false)  ← helper body
storage/src/gdpr.rs:407,490,580   set_config('app.tenant_id', $1, true)
cli/src/server/backup_api.rs:1597 set_config('app.tenant_id', $1, false)
cli/src/server/sync.rs:131,256    activate_tenant_context(...)            ← only 2 high-level call sites
```

Repository-layer SELECTs against the 22 RLS tables: **100+ call sites** across `cli/src/server/user_api.rs`, `project_api.rs`, `org_api.rs`, `memory_api.rs`, `knowledge_api.rs`, `governance_api.rs`, `audit_api.rs`. None of them pre-activate a tenant context. All of them carry an explicit `WHERE tenant_id = $1` clause (the real enforcement).

### 1.3 Prior-art hazards surfaced by this analysis

**Hazard H1 — session-scope `set_config`.** `storage/src/postgres.rs:63` and `cli/src/server/backup_api.rs:1597` both use `set_config('app.tenant_id', $1, false)`. The third argument `false` means *session-level* — the setting survives on the pooled connection after the query completes. The next request borrowing that connection inherits tenant A's `app.tenant_id` unless it explicitly resets. Under the current prod BYPASSRLS reality this has no blast radius; under Option A it becomes a cross-tenant leak. Either way the fix is the same: switch to `set_config(..., true)` (transaction-local) and wrap the query in an explicit BEGIN.

**Hazard H2 — dual GUC namespace.** Older migrations use `app.company_id`; newer use `app.tenant_id`; the normalization pass (`024`) only converted three tables. A request that sets one GUC leaves the other stale on the connection. Again invisible under BYPASSRLS, weaponized under Option A.

**Hazard H3 — orphan `activate_tenant_context` call sites in `sync.rs`.** The two call sites that DO activate context are themselves no-ops (they run as BYPASSRLS). They read as correct defense-in-depth code; they are actually dead code. This misleads contributors reviewing `sync.rs` as a reference for how to write RLS-aware code.

## 2. Options Considered

### Option A — Make RLS real

Prod enforcement via RLS. Shape:

1. New migration: `CREATE ROLE aeterna_app_rls LOGIN NOBYPASSRLS PASSWORD …`; grant `USAGE` on schema + per-table DML.
2. Prod `DATABASE_URL` switches to `aeterna_app_rls`. Deploy ordering: role exists before image flip.
3. Auth middleware acquires a pool connection, `BEGIN`, `SET LOCAL app.tenant_id = $1`, runs handler, `COMMIT` (or `ROLLBACK` on error). Connection lifetime == request lifetime.
4. The 100+ repository call sites DO NOT change — they continue to carry their `WHERE tenant_id = ?` clauses (belt-and-suspenders).
5. Fix H1/H2/H3 as preconditions (mandatory; otherwise prod breaks under non-BYPASSRLS).

**Pros.** Genuine defense in depth. A missed `WHERE tenant_id = ?` in app code is caught at the DB layer. Compliance narrative is real, not paper.

**Cons.**
- Transaction-per-request overhead. For read-mostly endpoints this roughly doubles round trips to the DB (BEGIN + query + COMMIT).
- Connection handle must be threaded through every handler (or hidden in an axum Extension). Either way, the 100+ repository sites need to accept a `&mut PgConnection` or a `&mut Transaction` instead of a `&PgPool`. That is the ~100-site refactor.
- `FORCE ROW LEVEL SECURITY` on governance tables means even the table owner (migration runner) is subject to policy — migrations need to `SET ROLE postgres` explicitly or we lose the ability to run DDL against those tables as any non-bypassing role.
- The session-variable leak (H1) MUST be fixed first or every pooled connection becomes a cross-tenant time bomb.

**Effort.** 4–6 weeks of focused work, high coordination cost across storage + server + CI.

### Option B — Drop RLS

Remove all 22 `ENABLE ROW LEVEL SECURITY` / 5 `FORCE ROW LEVEL SECURITY`. Drop all policies. Delete `activate_tenant_context`. Document in `AGENTS.md` that tenant isolation is app-layer `WHERE tenant_id = ?`, enforced by code review and `sqlx` query macros.

**Pros.** Honest. Cheap (one migration + code removal). No overhead. No session-variable leak to worry about.

**Cons.** Loses the paper defense-in-depth story that Kyriba security / compliance reviews may require. Zero DB-layer safety net against a missed `WHERE` clause. Future Kyriba auditors will ask why a multi-tenant product has no RLS; answer ("application code enforces") is correct but unsatisfying.

**Effort.** 1 week.

### Option C — RLS as test-time gate

Prod connection stays BYPASSRLS (no change to runtime). Integration tests run under a dedicated `aeterna_app_rls` role that is non-BYPASSRLS, and exercise the actual handler code under a real tenant context.

Concrete shape:

1. New migration: `CREATE ROLE aeterna_app_rls LOGIN NOBYPASSRLS PASSWORD 'test_only_insecure'`; grant minimal DML. Role is test-only; prod deploys do not use it.
2. `cli/tests/rls_enforcement_test.rs` — new integration test file:
   - Opens a second pool using `DATABASE_URL` with username replaced by `aeterna_app_rls`.
   - For each RLS-protected table, runs two queries: (a) with `SELECT set_config('app.tenant_id', <tid>, true)` in a BEGIN → expect N rows; (b) without setting context → expect 0 rows.
   - Additionally exercises the high-level list paths (`/user`, `/project`, `/org`, `/govern/audit`) against the RLS role to prove the handler + repository stack produces tenant-filtered results even with RLS actually enforced.
3. Fix H1/H2/H3 opportunistically. H1 is a hard requirement for the test to pass without flakes.

**Pros.**
- Prod overhead unchanged (0 ms, 0 connection-lifecycle churn).
- Real compile-time-ish guarantee: any future PR that introduces a query missing `WHERE tenant_id = ?` against an RLS-protected table fails CI. This is the main value RLS provides today; Option C captures that value at 5% of Option A's cost.
- Kyriba security review answer: “RLS policies are authored, enforced in CI against a non-BYPASSRLS role, and additionally enforced at the application layer in prod for latency reasons.” That is a defensible posture.
- Reversible. If threat model changes, we flip prod to `aeterna_app_rls` and we're in Option A with the session-variable bugs already fixed.

**Cons.**
- Not true defense in depth *in prod*. A prod-only regression (e.g. a hand-rolled query added via a migration-time admin script) could still leak across tenants.
- Requires test discipline: the RLS test role must be kept in sync with per-table grants as migrations add tables. We guard this with a schema-introspection test that fails if a new RLS-enabled table lacks an explicit grant to `aeterna_app_rls`.

**Effort.** 1–2 weeks.

## 3. Recommendation — Option C

Option A is technically the "right answer" for a product under a strict zero-trust threat model. Aeterna is not that product: it is an internal-tenant tool where the adversary is a buggy handler, not a malicious user of the public API. Against that threat model, Option A's runtime overhead and refactor cost buys us protection against a class of bug (missed `WHERE tenant_id = ?`) that Option C already catches in CI.

Option B throws away the authored policies, which we would regret the first time a Kyriba security review asks "what stops a SQL injection from reading another tenant?". With Option C the answer is "the app's DB role has access to all tenants but every query carries `WHERE tenant_id = ?` and the CI suite runs under a role where missing that clause returns zero rows."

Option C preserves the policies as a live artifact (they must compile, they must pass the test suite) and converts them from decorative to enforceable. It is reversible to Option A if the threat model changes.

## 4. Spec Impact

One MODIFIED and one ADDED requirement on `runtime-security-hardening`:

- **MODIFIED** "Backend-Specific Persistence Isolation" — clarifies that on Postgres, tenant isolation is enforced by application-layer `WHERE tenant_id = ?` clauses; RLS policies are authored for CI enforcement, not prod.
- **ADDED** "RLS Test-Time Enforcement" — normative requirement that a dedicated non-BYPASSRLS role exists, that the integration test suite exercises all RLS-protected tables under that role, and that schema introspection fails CI if a new RLS-enabled table lacks a grant to the test role.

Spec delta lives in `specs/runtime-security-hardening/spec.md`.

## 5. Rollout

1. Land this change (proposal + design + spec delta + migration adding `aeterna_app_rls` role + test suite + H1 fixes).
2. Archive it once the test suite is green and `AGENTS.md` / `DEVELOPER_GUIDE.md` are updated.
3. If / when the threat model changes: file a new change `flip-prod-to-rls-enforced` that builds on this one. The role exists, the session-variable hygiene is fixed — that change becomes a middleware-wiring PR instead of a six-week architectural lift.
