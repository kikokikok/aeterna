# Cross-Tenant Admin Listing (#44.d)

**Status**: Proposed
**Depends on**: #44.a (schema), #44.b (`RequestContext` resolver), #44.c (handler shim) — all merged on `master` as of PR #52 and #55.
**Blocks**: #44.e (`X-Target-Tenant-Id` header removal).

## Why

`#44.c` (PR #55) migrated every legacy handler to the new `RequestContext` resolver transparently. That was the 80% win. But it preserved the `TenantContext` API, which **cannot express two things** PlatformAdmins genuinely need:

1. **"No target tenant"** — e.g. `GET /admin/tenants` lists every tenant and must not require the caller to pick one first.
2. **"Every tenant"** — e.g. audit search across the whole instance, user listing during incident response, cross-tenant billing roll-ups.

Today the codebase fakes #1 by silently accepting any `X-Tenant-ID` (now fixed by #44.b's existence check, which *breaks* the silent path — another reason this PR is necessary), and has no way to do #2 at all. The `X-Target-Tenant-Id` legacy header is a third shape of confusion used by some CLI paths.

We need a **single explicit vocabulary** for tenant scope on admin endpoints so handlers stop guessing.

## What Changes

### 1. Introduce `ListTenantScope` (server-side enum)

```rust
// cli/src/server/context.rs — additive

/// Declared tenant scope for a list/search handler. Derived from the
/// request's `?tenant=` query parameter by `RequestContext::list_scope()`.
pub enum ListTenantScope {
    /// Exactly one tenant (caller's resolved tenant, or path/query-specified).
    Single(ResolvedTenant),
    /// All tenants on the instance. PlatformAdmin only — non-admins get 403.
    All,
}

impl RequestContext {
    /// Parse `?tenant=<slug|uuid|*>` into `ListTenantScope`.
    /// - `?tenant=*` → `All` (requires PlatformAdmin)
    /// - `?tenant=<ref>` → `Single` (membership-checked unless admin)
    /// - omitted → `Single(self.require_target_tenant(headers)?)`
    pub async fn list_scope(
        &self,
        state: &AppState,
        headers: &HeaderMap,
        query_tenant: Option<&str>,
    ) -> Result<ListTenantScope, Response>;
}
```

### 2. New query-parameter convention: `?tenant=<value>`

| Value       | Meaning                              | Auth                       |
|-------------|--------------------------------------|----------------------------|
| (omitted)   | Resolved tenant (header/default/auto) | Standard membership        |
| `<slug>`    | That specific tenant                  | Membership or PlatformAdmin |
| `<uuid>`    | That specific tenant (canonical ID)   | Same                       |
| `*`         | **All tenants**                       | **PlatformAdmin required** |

Response envelope for `*` mode includes `tenantId` + `tenantSlug` on every item.

### 3. Migrate these handlers to `RequestContext` **directly** (not the shim)

| Handler                              | File                    | Today                              | After                             |
|--------------------------------------|-------------------------|------------------------------------|-----------------------------------|
| `GET /admin/tenants`                 | `tenant_api.rs:924`     | `require_platform_admin` (local)   | `RequestContext` + `list_scope`   |
| `GET /admin/users`                   | `user_api.rs`           | `tenant_scoped_context`            | `RequestContext::list_scope`      |
| `GET /admin/projects`                | `project_api.rs`        | `tenant_scoped_context`            | `RequestContext::list_scope`      |
| `GET /admin/orgs`                    | `org_api.rs`            | `tenant_scoped_context`            | `RequestContext::list_scope`      |
| `GET /admin/audit`                   | `govern_api.rs`         | `tenant_scoped_context`            | `RequestContext::list_scope`      |

These are the only 5. Every other list endpoint stays on the #44.c shim.

### 4. Cross-tenant response shape (consistent across all 5)

```json
{
  "success": true,
  "scope": "all" | "single",
  "tenant": { "id": "...", "slug": "..." } | null,    // null iff scope="all"
  "items": [
    { "id": "...", "tenantId": "...", "tenantSlug": "...", ... }
  ],
  "pagination": { ... }
}
```

**Rule**: in `scope="all"` mode, every item **must** carry `tenantId` + `tenantSlug` (non-negotiable — tested by a trait-level contract test).

### 5. Retire `X-Target-Tenant-Id` header (feeds #44.e)

Currently used by 6 CLI code paths and 20 handler-side reads. After #44.d:

- CLI `AeternaClient` stops emitting `x-target-tenant-id` on new calls. The preferred pattern is `X-Tenant-ID` for tenant-targeted ops + `?tenant=<slug|*>` for cross-tenant listing.
- Legacy handlers continue to read the header for one release (compat window); remove in #44.e with the deprecation warning structured log already landed in #44.b.

## Non-goals (explicitly out of scope)

- **Audit impersonation tracking** (section 6 of the original proposal). Separate PR — the schema columns already exist, just need to populate them.
- **Tenant wildcards with filters** (e.g. `?tenant=eu-*`). No demand signal yet. Revisit if/when multi-region becomes reality.
- **Writes across tenants**. Bulk `PATCH`/`DELETE` with `?tenant=*` is explicitly forbidden — use per-tenant batch endpoints.

## Alternatives considered

### A. Separate `/admin/cross-tenant/users` etc. endpoints

**Rejected** — doubles the API surface, duplicates pagination/filter logic, and clients must pick a URL statically. Query-param scope preserves a single URL shape with runtime scope selection.

### B. Keep `X-Target-Tenant-Id` as the admin vocabulary

**Rejected** — headers are invisible in URLs, request logs, HAR files. Debugging cross-tenant issues in production means reading full headers on every trace. A query param is self-documenting and cache-friendly (though these endpoints are not cached today).

### C. Implicit "no header = list all" for PlatformAdmin

**Rejected** — silent mode-switching. Today's code already suffers from this (an admin forgets a header and accidentally gets a cross-tenant view when they meant to target a specific tenant). Explicit `?tenant=*` is opt-in.

### D. GraphQL-style scope object

**Rejected** — we don't ship GraphQL, and inventing a JSON-body scope for GET requests is hostile to CLI/curl workflows.

## Open questions

1. **Q**: Should `?tenant=*` be spelled `?tenant=all` instead?
   **Proposal**: keep `*` — matches Unix/SQL convention, no confusion with a tenant actually named `all`. Document both if we want.

2. **Q**: Do we need a `?tenants=t1,t2,t3` multi-scope (not `*`, not single)?
   **Proposal**: no. YAGNI. Every real-world motivator is either single-tenant or all-tenants. Revisit on demand.

3. **Q**: Does `?tenant=*` paginate across tenants, or per-tenant with a continuation token?
   **Proposal**: flat pagination across the union (sort by `(tenantId, id)` for stable ordering). Per-tenant continuation is a premature optimisation.

4. **Q**: What error code for a non-admin calling `?tenant=*`?
   **Proposal**: `403 forbidden_scope` (new code) with `required_role: "PlatformAdmin"` in the body. Distinct from `forbidden_tenant` so clients can differentiate.

## Implementation plan (sketch — full tasks.md follows in the PR)

1. Add `ListTenantScope` + `RequestContext::list_scope` (~80 LOC in `context.rs`) + 8 unit tests.
2. Migrate each of the 5 handlers one commit at a time. Each commit self-contained, each adds an integration test.
3. Stop emitting `x-target-tenant-id` in `AeternaClient` (kept as read-only server-side for compat window).
4. Add contract test: "every item in a `scope=all` response has `tenantId` + `tenantSlug`".
5. Update OpenAPI/Redoc schema for the 5 endpoints.

**Estimated size**: ~600 LOC added, ~100 LOC removed. Single PR is feasible; two PRs acceptable if reviewers prefer (split: context resolver + per-handler migration).

## Impact

- **API surface**: additive `?tenant=*` on 5 endpoints. No breaking changes. New error code `forbidden_scope`.
- **Wire compat**: old clients continuing to call these 5 endpoints without `?tenant` get identical behaviour to today.
- **Schema**: none. Uses #44.a columns.
- **CLI**: new `--all-tenants` flag on `aeterna admin users list` (etc.), implemented in a CLI-only follow-up once server side ships.
- **Admin UI**: tenant picker gains an "All tenants" option on list views — UI-only follow-up.

## Rollout

Single-step rollout behind no feature flag. The feature is opt-in per-request (client must pass `?tenant=*`), so landing the code on `master` has zero behavioural impact until a client uses it. No migration, no data backfill, no kill-switch needed.
