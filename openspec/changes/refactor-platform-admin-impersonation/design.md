## Context

Aeterna's request authentication has organically grown around a "tenant-first" assumption: every authenticated request is expected to be scoped to one tenant, and the `X-Tenant-ID` header is how the scope is declared. This assumption is enforced by `authenticated_tenant_context` (at `cli/src/server/mod.rs:235`) which rejects any PlatformAdmin request without `X-Tenant-ID`. It worked while every user had a home tenant, but breaks in three real-world cases:

1. **Fresh-deploy first-tenant provisioning.** The bootstrap seeds a PlatformAdmin user and role, but not a tenant row. The PlatformAdmin's first action must be creating a tenant — and that endpoint demands a tenant header. Workarounds (passing an orphan `X-Tenant-ID` string) happen to work because the header is not validated against the `tenants` table.
2. **Cross-tenant operator tasks.** PlatformAdmins should be able to list users, projects, or configs across all tenants without picking one. Today this requires N round trips through tenant selection.
3. **Per-user tenant preference portability.** `aeterna tenant use <slug>` writes to a local file. The UI TenantSelector lives in browser session. Third-party plugins receive no signal. A user who switches client loses their context.

## Goals

- PlatformAdmin can call any endpoint on any existing tenant (with `X-Tenant-ID`) or platform-wide (without).
- `X-Tenant-ID` is validated against the canonical `tenants` table; orphan IDs never accidentally authorize a request.
- Non-admin users targeting a foreign tenant are rejected with a clear `403`; users with ambiguous membership receive a `400 select_tenant` that carries everything the client needs to render a picker.
- A user's preferred tenant is persistable server-side so every client picks it up uniformly.
- Audit trail distinguishes actor identity from the tenant being impersonated.

## Non-goals

- CLI-side picker UX — lives in `add-cli-auth-tenant-switch`.
- UI-side selector persistence — lives in `persist-ui-tenant-selection`.
- Splitting admin-UI token kind from plugin-access — lives in `refactor-admin-ui-token-kind`.
- Bootstrap auto-seeding a default tenant — explicitly rejected in favor of PlatformAdmin deliberately provisioning the first tenant via `aeterna admin provision-tenant`.

## Decisions

### Decision 1: Single unified `RequestContext` replaces three overlapping helpers

Today there are three nearly identical helpers (`authenticated_tenant_context`, `tenant_scoped_context`, `require_platform_admin`) that all route through the same tenant-header check and differ only in role assertions. Consolidating them into one `RequestContext` resolver makes the resolution chain inspectable in a single place and ensures consistency: every handler sees the same logic.

Alternatives considered:
- **Keep three helpers, fix each individually.** Rejected: three places to keep in sync, and the asymmetry already caused today's bug where PlatformAdmin routes incorrectly chain through the tenant-header check.
- **Middleware that populates `RequestContext` into Axum request extensions.** Considered for ergonomics. Deferred to a follow-up — the explicit function call signature is easier to reason about during the initial migration, and Axum extensions can be layered later without another API break.

### Decision 2: Tenant resolution priority chain, deterministic

```
  1. X-Tenant-ID header        (session-scoped override, UI picker, CLI flag/env propagation)
  2. users.default_tenant_id   (portable user preference)
  3. Single-tenant auto-select (if user has exactly one membership)
  4. None + PlatformAdmin      → platform-scoped request, success
     None + regular user       → 400 select_tenant
```

CLI-level priority (`--tenant` flag, `AETERNA_TENANT` env, `.aeterna/context.toml`) is translated into the `X-Tenant-ID` header by the client before the request reaches the server. The server sees one input: the header. This keeps the server contract simple and lets every client (CLI, UI, plugin) layer preference logic on top.

### Decision 3: Validate `X-Tenant-ID` against `tenants` table, always

The current silent-accept of orphan IDs is a latent bug. Validation costs one cached lookup; the tenant store is already in-memory for most deployments. The 404 response matches REST conventions for a resource reference that does not exist.

### Decision 4: `select_tenant` error carries tenant list

Returning the list of accessible tenants in the error payload lets the CLI launch its picker immediately and lets the UI render the selector without a second API call. The payload never enumerates tenants outside the caller's membership (PlatformAdmin sees all, regular users see their own only), preserving isolation.

### Decision 5: Legacy `tenant_required` remains for two minor versions behind an opt-in header

All first-party clients (CLI, UI) migrate to `select_tenant` in the dependent proposals. Third-party plugin consumers that matched on `tenant_required` in error handlers get a grace period via `Accept-Error-Legacy: true` header: when set, the server emits the old code for backward compatibility. This is opt-in (never default) so correct clients move forward immediately.

### Decision 6: Audit log column additions, no table split

Adding `acting_as_tenant_id` to `referential_audit_log` and `governance_audit_log` is preferable to a new `impersonation_log` table. Existing dashboards and queries continue to work; operators query the one column to distinguish impersonated vs. direct actions. A derived `is_impersonation` expression (`acting_as_tenant_id IS DISTINCT FROM user.primary_tenant_id`) handles the PlatformAdmin case where the actor has no primary tenant.

### Decision 7: `?tenant=*` over a separate endpoint family for cross-tenant listing

Reuses the existing handler bodies (single filter change) and keeps URL shape consistent. PlatformAdmin-only guard at the handler entry point; non-admins receive `403`. Alternatives considered: (a) separate `/api/v1/platform/users` family — rejected as URL-sprawl; (b) infer from absent `X-Tenant-ID` — rejected because it collides with platform-scoped endpoints.

## Risks

- **Migration surface**: ~30 handler modules touch the old helpers. Mitigated by keeping the old helpers as `#[deprecated]` shims during the transition; Rust's warn-on-deprecated surfaces every remaining call site.
- **Silent authorization regression**: if a handler is accidentally migrated to `require_platform_admin` when it should call `require_target_tenant`, a PlatformAdmin might execute a tenant-scoped op without tenant context. Mitigated by task 3.4 (audit every 400 response) and integration tests that assert specific handlers still require a tenant.
- **Audit log join noise**: adding `acting_as_tenant_id` with FK means every PlatformAdmin impersonation references real tenants. If a tenant is later deleted, audit rows reference a dead tenant. Acceptable: `ON DELETE SET NULL` clears the reference but preserves the actor identity.
- **Breaking legacy clients**: any external plugin that `assert error.code == "tenant_required"` will break when receiving `select_tenant`. Mitigated by the two-version deprecation window and the `Accept-Error-Legacy` opt-in.
