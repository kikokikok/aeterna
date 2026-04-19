# Design — Decide RLS Enforcement Model

## 1. Problem

The codebase ships with a comprehensive RLS story on paper: 22 tables with `ENABLE ROW LEVEL SECURITY`, 5 with `FORCE`, and an explicit GUC `app.tenant_id` that every policy consults. The runtime behavior does not match the paper. `PostgresBackend::activate_tenant_context` is called in two places (both in `cli/src/server/sync.rs`, both currently effective no-ops); no migration creates a non-BYPASSRLS role; prod `DATABASE_URL` points at whatever role the operator provisioned, which in the shipped compose config is `postgres` (superuser, implicit BYPASSRLS).

The app-layer `WHERE tenant_id = $N` clauses scattered across the repository layer are the de facto isolation. If any handler forgets that clause on an RLS-protected table, the query returns cross-tenant rows in prod even though RLS appears enabled.

Three viable resolutions exist. Each is a complete answer; they are not cumulative.

## 2. Threat model (the deciding input)

The threat model this change locks in, after architect override on the initial C recommendation:

> **T1.** A buggy handler on a live production request — a missing `WHERE tenant_id = ?` clause, a typo in a join, a repository method that was written for admin-scope and re-used for user-scope — MUST NOT be able to read rows belonging to another tenant, even if it successfully reaches `self.pool.acquire()` and issues the query.
>
> **T2.** A compromised handler (SQL injection, deserialization RCE, dependency supply-chain) on a live production request MUST be constrained by the database's own isolation mechanism, not only by the handler's own SQL string.

`T1` is within reach of Option C if we accept "regression caught in CI before merge" as sufficient. `T2` is not — a compromised handler in prod never ran against the CI role, and Option C's test-time enforcement is silent on that path. Option A is the only option that satisfies both.

## 3. Option analysis

### 3.1 Option A — Activate RLS on every request-scoped connection (chosen)

**Shape.** App connects as a non-BYPASSRLS role (`aeterna_app`). Every request borrows a connection via `with_tenant_context(&ctx, |conn| …)`, which opens an explicit transaction, issues `SET LOCAL app.tenant_id = $1`, runs the body, and commits or rolls back. RLS policies evaluate against the live GUC on every statement; any query without the `WHERE` clause still returns only the current tenant's rows because the policy enforces the predicate.

**Satisfies threat model.** Both `T1` and `T2`. The database itself is the final gate.

**Cost.**
- **4–6 weeks of engineering.** Helper implementation is 1–2 days; the 100-site repository refactor is 3–4 weeks spread across 4–6 waves; the canary rollout adds 2+ weeks of soak time that runs in parallel with cleanup work.
- **Transaction-per-request overhead.** Every read path now incurs `BEGIN` … `COMMIT` round-trips. On pgbouncer with transaction-mode pooling the overhead is negligible (pgbouncer was already transaction-scoping connections); on session-mode pooling the overhead is larger. Current deployments use transaction-mode pgbouncer per `docker-compose.yml`, so this is a near-zero regression in practice.
- **Hazard resolution required first.** H1 (session-scope `set_config(..., false)`) and H2 (dual GUC namespace) must be closed before the role flip, because revoking BYPASSRLS weaponizes both into live cross-tenant leaks. This is why Bundle A.1 is a hard prerequisite for Bundle A.4.

**Rollback.** Per-bundle: A.1–A.3 are warn-level or CI-only, trivially revertable. A.4 is a single env-var change (`AETERNA_DB_ROLE=bypassrls`) plus a rolling pool restart, under 5 minutes per region. A.5 is the only non-revertable bundle and only lands after A.4 has soaked in prod for ≥ 4 weeks.

### 3.2 Option B — Drop RLS entirely (rejected)

**Shape.** `ALTER TABLE … DISABLE ROW LEVEL SECURITY` on all 22 tables, drop all policies, update docs to say "app-layer only."

**Satisfies threat model.** Neither `T1` nor `T2`.

**Cost.** ~1 week. Simplifies the mental model at the cost of losing every downstream defense. Also throws away migration 024 (the partial GUC normalization) as dead code.

**Why rejected.** Unacceptable under the locked threat model. Also loses the ability to ever revisit defense-in-depth without reintroducing the same 22 policies from scratch.

### 3.3 Option C — RLS as a test-time gate (rejected)

**Shape.** Keep prod on BYPASSRLS; add a dedicated non-BYPASSRLS role used only by an integration test suite; every RLS-enabled table is exercised under that role on every CI run.

**Satisfies threat model.** `T1` partially (catches regressions pre-merge), not `T2`.

**Cost.** 1–2 weeks. The cheapest option that preserves some defense-in-depth value.

**Why rejected.** The gap between "caught in CI" and "caught in prod" is exactly the window that matters for `T1`’s live-request variant, and all of `T2`. The architect's override on the initial C recommendation was explicit: the threat model includes prod-time compromise, not only pre-merge regression. The CI suite from Option C is preserved as a permanent regression guard (shipped inside Bundle A.2), so nothing is lost by choosing A.

## 4. Implementation strategy

### 4.1 Why staged, not big-bang

A single PR that (a) ships the new role + grants, (b) refactors 100 query sites, (c) flips prod away from BYPASSRLS is unreviewable and unrevertable. It also couples the three hardest failure modes — grant gaps, refactor bugs, and transaction-per-request overhead — into one incident if anything goes wrong. The bundle structure exists to decouple them.

### 4.2 Bundle ordering and independence

Bundle A.1 and A.2 are **independent** — either can land first; neither changes runtime behavior under the current BYPASSRLS role. A.3 depends on A.2 (it needs the CI suite to prove the refactor). A.4 depends on A.1, A.2, A.3 all being complete — the role flip only works if hazards are closed, the role exists, and every query goes through the helper. A.5 depends on A.4 having soaked.

The net property: **any subset of {A.1, A.2, A.3} landing in prod without A.4 introduces zero risk.** A.3 in particular is net-beneficial pre-flip (closes the `activate_tenant_context` gap and makes the transaction scope explicit) and net-neutral under BYPASSRLS (the `SET LOCAL` is harmless when the role bypasses the policy).

### 4.3 The `with_tenant_context` helper

The helper pattern is load-bearing. Each call site transforms roughly from:

```
let mut conn = self.pool.acquire().await?;
let rows = sqlx::query_as::<_, Row>("SELECT … FROM users WHERE tenant_id = $1")
    .bind(tenant_id).fetch_all(&mut *conn).await?;
```

to:

```
let rows = self.with_tenant_context(&ctx, |tx| async move {
    sqlx::query_as::<_, Row>("SELECT … FROM users")   // WHERE clause now optional per RLS, kept for DiD
        .fetch_all(&mut **tx).await
        .map_err(Into::into)
}).await?;
```

The explicit `WHERE tenant_id = $N` stays as defense-in-depth — Bundle A.5's `AGENTS.md` update documents that this is required, not optional. The RLS policy is the floor; the `WHERE` clause is the ceiling; any query where the two disagree is a bug and `rls_enforcement_test.rs` surfaces it.

### 4.4 Async workers

Waves 4 and 5 of A.3 handle `sync.rs` / `webhook.rs` / backup / GDPR paths that don't have a request-scoped tenant. The pattern: each async task acquires its context from its own job record (`sync_jobs.tenant_id`, `backup_jobs.tenant_id`, …) and passes a synthesized `TenantContext::from_async_job(job_id, tenant_id)` into `with_tenant_context`. There is no "unscoped" escape hatch — a job without a tenant either doesn't need RLS-protected tables or is a bug.

## 5. Risk register

**H1 — session-scope `set_config(..., false)` on pooled connections.** Current behavior: `backup_api.rs:1597` and legacy call sites use session scope; the setting persists after `.release()` and leaks to whichever request next borrows that connection. Under BYPASSRLS this is silent. Under non-BYPASSRLS (post-A.4) this becomes a live cross-tenant leak. **Mitigation:** Bundle A.1 closes every instance and lands before A.4. Regression test in A.1.

**H2 — dual GUC namespace.** Migration 024 normalized the policies to read `app.tenant_id`, but several app-side call sites still write `app.company_id` or `app.current_tenant_id`. Under BYPASSRLS this is silent. Under non-BYPASSRLS the policy reads the canonical var and finds NULL → zero rows returned → user-visible outage. **Mitigation:** Bundle A.1 grep-normalizes every app-side read/write; grep regression test in A.1.

**H3 — orphan `activate_tenant_context` calls in `sync.rs`.** Currently no-ops under BYPASSRLS; post-A.3 they double-`SET LOCAL` inside `with_tenant_context`, which is harmless but noisy. **Mitigation:** Bundle A.5 deletes them.

**H4 — transaction-per-request overhead under session-mode pooling.** Not applicable to current deployments (transaction-mode pgbouncer), but documented so a future infra change doesn't regress. **Mitigation:** A.4.8 Grafana panel for transaction duration; alert threshold established during canary.

**H5 — missed grant on a newly-added RLS table.** A contributor adds a migration with `ENABLE ROW LEVEL SECURITY` but forgets to grant `aeterna_app` access → the table is empty under the RLS role. **Mitigation:** A.2.6.1 pre-flight query enumerates `pg_tables WHERE rowsecurity = true` and fails the test run with a named-table error if any grant is missing.

## 6. Open questions

- **pgbouncer auth plumbing.** Can pgbouncer's `auth_file` or `auth_query` resolve the `aeterna_app` password from the same secret store as the app, without duplicating credentials? Tracked for Bundle A.4; may require a small infra PR.
- **Password rotation.** Initial plan is a secret-manager-backed password referenced as `${APP_DB_PASSWORD}`. Rotation cadence and procedure tracked as an ops ticket after A.4 lands.
- **Read replicas.** Do read replicas inherit the RLS policies and the grants? Yes (replica replays the full catalog), but the `aeterna_app` role must exist on the replica's primary before failover. Covered by standard replica provisioning; flagged here so it's not forgotten during the A.4 rollout.
