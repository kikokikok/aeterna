# Design — Decide RLS Enforcement Model

## 1. Problem

The codebase ships with a comprehensive RLS story on paper: 22 tables with `ENABLE ROW LEVEL SECURITY`, 5 with `FORCE`, an explicit `app.tenant_id` GUC, and 24 migrations' worth of policies. The runtime behavior does not match. `PostgresBackend::activate_tenant_context` is called in two places (both orphan no-ops in `sync.rs`), no migration creates a non-BYPASSRLS role, and the shipped `DATABASE_URL` points at the `postgres` superuser. The app-layer `WHERE tenant_id = $N` clauses are the de facto isolation. Any handler that forgets that clause leaks cross-tenant rows and the RLS policies do nothing.

There is no production environment and no live user base. The system is pre-launch. Fixing the enforcement model now costs a refactor with zero blast radius; inheriting BYPASSRLS into launch costs a rewrite under customer pressure.

## 2. Goal

Make row-level security the authoritative tenant isolation mechanism at runtime, not a paper artifact. Retain the app-layer `WHERE tenant_id = ?` clauses as required defense in depth on top. Build an end-to-end CI regression guard that catches any future code path that bypasses the model.

This is not a threat-model-driven change (no threat model is interesting pre-production). It is an architectural correctness change: the documented enforcement posture and the runtime enforcement posture must agree.

## 3. Option analysis

Three options are viable; Option A is chosen.

### 3.1 Option A — Activate RLS on every request-scoped connection (chosen)

**Shape.** Two PostgreSQL roles, two connection pools, two helpers. The application connects via `aeterna_app` (non-BYPASSRLS) for all per-tenant work and via `aeterna_admin` (BYPASSRLS) for the narrow admin surface. Every request that touches tenant-scoped tables acquires its connection through `with_tenant_context(&ctx, |tx| …)` which opens a transaction, issues `SET LOCAL app.tenant_id = $1`, runs the body, and commits. Cross-tenant admin work goes through `with_admin_context(&ctx, |tx| …)` on a separate small pool, with audit-logging wired into the helper itself.

**Why chosen.** Only option that makes RLS authoritative at runtime. The dual-role design solves the PlatformAdmin cross-tenant and scheduled-jobs problems without forcing BYPASSRLS onto the default role. The CI verification suite acts as the permanent regression guard. Pre-production timing makes the refactor essentially free.

**Cost.** 4–6 weeks of engineering split across 3 bundles. Transaction-per-request overhead on the default pool; negligible on the current transaction-mode pgbouncer setup, and not a concern pre-production regardless.

### 3.2 Option B — Drop RLS entirely (rejected)

**Shape.** `ALTER TABLE … DISABLE ROW LEVEL SECURITY` on all 22 tables, drop all policies, document app-layer-only.

**Why rejected.** Honest about the current reality but throws away the migrations that wrote the policies and eliminates the path to defense in depth. Same correctness properties as the current state; different label.

### 3.3 Option C — RLS as a CI-only gate (rejected)

**Shape.** Keep the default role on BYPASSRLS; add `aeterna_app` but use it only in an integration test suite that exercises every RLS table.

**Why rejected.** Covers the "regression caught pre-merge" case but does nothing at runtime. Accepts that a missing `WHERE` clause in a never-tested code path still leaks. Not worse than today but not better either; the CI value survives by shipping the suite inside Bundle A.2.

## 4. The dual-role, dual-pool design

The design rests on four concepts that must be introduced together or the parts don't hold up.

### 4.1 Two roles

`aeterna_app` has `NOBYPASSRLS` and is granted table-level DML only on tables that have `rowsecurity = true`. Any query it runs against an RLS-protected table is filtered by the policy. This is the role that runs 99% of traffic.

`aeterna_admin` has `BYPASSRLS` and is granted DML on all application tables. It exists to cover the small set of legitimate cross-tenant operations: PlatformAdmin list endpoints with `?tenant=*`, scheduled cross-tenant maintenance, and the migration runner. Nothing else.

If the application role had `BYPASSRLS` granted directly (the shortest-path alternative), every handler would silently bypass RLS regardless of intent, and the whole point of the change evaporates.

### 4.2 Two pools

`state.pool` opens connections as `aeterna_app`, has the normal pool size, and serves request traffic. `state.admin_pool` opens connections as `aeterna_admin`, has `max_connections = 4`, and serves admin/scheduled work. The small pool is a deliberate signal: admin operations are rare, so accidentally running hot-loop work through the admin pool fails loudly.

### 4.3 Two helpers

```rust
pub async fn with_tenant_context<F, T>(&self, ctx: &TenantContext, body: F) -> Result<T>
where F: FnOnce(&mut Transaction<'_, Postgres>) -> BoxFuture<Result<T>>
{
    let mut tx = self.pool.begin().await?;
    sqlx::query!("SELECT set_config('app.tenant_id', $1, true)", ctx.tenant_id.to_string())
        .execute(&mut *tx).await?;
    let out = body(&mut tx).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn with_admin_context<F, T>(&self, ctx: &TenantContext, body: F) -> Result<T>
where F: FnOnce(&mut Transaction<'_, Postgres>) -> BoxFuture<Result<T>>
{
    let mut tx = self.admin_pool.begin().await?;
    let out = body(&mut tx).await?;
    tx.commit().await?;
    self.audit_admin_access(ctx).await?;
    Ok(out)
}
```

The tenant helper issues `SET LOCAL`; the admin helper does not. Rustdoc on both declares that direct `state.pool` / `state.admin_pool` access is forbidden outside these helpers. A CI lint (warn-level in A.2, deny-level in A.3 wave 6) enforces it.

The admin helper's `audit_admin_access` call writes a `governance_audit_log` row with `actor_id`, `actor_type`, `admin_scope = true`, and `acting_as_tenant_id = NULL`. Because the audit call lives inside the helper, every cross-tenant admin access is traced without any per-call-site bookkeeping.

### 4.4 Context routing

Handlers choose the helper from the resolved request context:

- `ctx.scope = Tenant(id)` or `CrossTenantSingle(id)` → `with_tenant_context` (the single-tenant-foreign case of #44.d is still one tenant at a time from the DB's perspective).
- `ctx.scope = CrossTenantAll` and `ctx.actor.is_platform_admin()` → `with_admin_context`.
- Scheduled per-tenant jobs → scheduler enumerates tenants via `with_admin_context(&system_ctx, …)`, per-tenant work dispatches through `with_tenant_context(&TenantContext::from_scheduled_job(t, job_id), …)`.
- Scheduled cross-tenant jobs (audit compaction, global rate-limit sweeps) → `with_admin_context(&system_ctx, …)`.

`system_ctx` is a sentinel `TenantContext` with `actor_type = 'system'`, no human actor, no tenant. It exists so scheduled work has a first-class way to identify itself in audit attribution.

## 5. Implementation strategy

### 5.1 Why three bundles

A single PR that stands up roles, wires pools, refactors 100 call sites, and flips the connection string would be unreviewable and would couple every failure mode into one incident. The bundle structure decouples them:

- **A.1 (hazards)** lands in any order, no coupling to the rest.
- **A.2 (roles + pools + helpers + CI)** lands before any A.3 wave but does not change runtime behavior of existing code — the helpers exist but nothing calls them yet.
- **A.3 (call-site refactor)** is 6 waves. The first 5 are behavioral no-ops under BYPASSRLS (the `SET LOCAL` is ignored by a BYPASSRLS role). Wave 6 is the single commit that flips `DATABASE_URL` and turns RLS from "tested" to "enforced."

### 5.2 Pre-production simplification

The previous revision of this document contained a Bundle A.4 (`AETERNA_DB_ROLE` feature flag) and a Bundle A.5 (escape-hatch removal + lint graduation). Both were written under the assumption that a production deployment existed. With no prod and no users, both bundles collapse into A.3 wave 6: one commit flips `DATABASE_URL`, deletes the orphan calls, and graduates the lint. No feature flag, no canary, no soak period, no rollback runbook.

This simplification is explicit in the decision: if a production environment comes later, the feature-flag + canary posture returns as a separate operational change at that time.

### 5.3 Refactor mechanics

Each call site transforms from:

```rust
let mut conn = self.pool.acquire().await?;
let rows = sqlx::query_as::<_, Row>("SELECT … FROM users WHERE tenant_id = $1")
    .bind(tenant_id).fetch_all(&mut *conn).await?;
```

to:

```rust
let rows = self.with_tenant_context(&ctx, |tx| async move {
    sqlx::query_as::<_, Row>("SELECT … FROM users")   // WHERE kept as DiD
        .fetch_all(&mut **tx).await
        .map_err(Into::into)
}).await?;
```

The explicit `WHERE tenant_id = $N` stays as required defense in depth. Bundle A.3 wave 6 updates `AGENTS.md` to document that this is mandatory, not optional — the RLS policy is the floor, the `WHERE` clause is the ceiling, and any query where they disagree is a bug surfaced by `rls_enforcement_test.rs`.

Admin-scope sites (those that currently build a cross-tenant `SELECT` under PlatformAdmin) transform into `with_admin_context(&ctx, |tx| …)`. The `WHERE tenant_id` clause, if present, typically disappears on these because the whole point is cross-tenant read; if present for narrower filtering (e.g., `WHERE status = 'active'`), it stays.

### 5.4 Async workers

A.3 Wave 4 establishes the pattern for sync / webhook / backup / GDPR workers. The pattern is documented in `DEVELOPER_GUIDE.md` (A.2.4.1) and enforced by the CI suite (A.2.3.5, A.2.3.6). There is no "unscoped worker" escape hatch: a worker that touches tenant-scoped tables either (a) knows its tenant and uses `with_tenant_context` or (b) is genuinely cross-tenant and uses `with_admin_context` with `system_ctx`. No third option.

## 6. Risk register

**H1 — session-scope `set_config(..., false)` on pooled connections.** Today: silent under BYPASSRLS. Post-A.3-wave-6: live cross-tenant leak. Mitigation: closed in Bundle A.1 with a regression test.

**H2 — dual GUC namespace.** Today: silent. Post-A.3-wave-6: handler runs, RLS policy reads the wrong variable, gets NULL, returns zero rows → visible outage. Mitigation: closed in Bundle A.1 with a grep regression test.

**H3 — orphan `activate_tenant_context` in `sync.rs`.** Today: no-op. Post-A.3-wave-6: double-`SET LOCAL` inside `with_tenant_context`, harmless but noisy. Mitigation: deletion in A.3 wave 6.

**H4 — missed grant on a newly-added RLS table.** A contributor adds a migration with `ENABLE ROW LEVEL SECURITY` but forgets to extend the `aeterna_app` grant list. The table is empty under the RLS role. Mitigation: pre-flight in A.2.3.1 enumerates `pg_tables WHERE rowsecurity = true` and fails the CI run with a named-table error.

**H5 — admin pool misuse.** A handler reaches for `state.admin_pool` directly (skipping `with_admin_context`) and bypasses RLS without audit attribution. Mitigation: CI lint, warn-level in A.2 (A.2.3.7), deny-level in A.3 wave 6. Admin helper's `audit_admin_access` is inside the helper so it cannot be skipped when the helper IS used.

**H6 — scheduled job without tenant attribution.** A new scheduled job touches tenant-scoped tables without picking `with_tenant_context` or `with_admin_context`. Mitigation: the direct-pool-access lint catches this; the handler model forces an explicit choice.

## 7. Open questions

- **Migration runner role.** Migrations currently run as whatever role `DATABASE_URL` points at. Post-A.3-wave-6 that's `aeterna_app`, which cannot run migrations (no DDL privileges). Two options: (a) migration runner uses a third role `aeterna_migrate` with DDL grants; (b) migration runner uses `aeterna_admin`. Leaning (b) for simplicity; flag for review.
- **pgbouncer auth.** If pgbouncer is in the path, its `auth_file` or `auth_query` needs to resolve both `aeterna_app` and `aeterna_admin` passwords. Tracked as an infra follow-up within Bundle A.3 wave 6.
- **Audit volume from `with_admin_context`.** Every admin helper call writes an audit row. For frequent admin operations (e.g., PlatformAdmin browsing `/user?tenant=*` repeatedly) this may need deduplication or batched audit. Flagged for review once A.2 CI suite surfaces the actual call volume.
