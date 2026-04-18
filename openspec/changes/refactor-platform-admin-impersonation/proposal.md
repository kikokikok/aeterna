## Why

On a freshly deployed Aeterna instance with zero tenants, a PlatformAdmin cannot create the first tenant through the intended API or CLI path. Every PlatformAdmin request is forced through `authenticated_tenant_context`, which requires an `X-Tenant-ID` header — a chicken-and-egg that blocks `aeterna admin provision-tenant`, `POST /api/v1/admin/tenants`, and even `GET /api/v1/admin/tenants`. The only way through today is passing an `X-Tenant-ID` header whose value does not exist in the `tenants` table (orphan IDs are silently accepted), which is both a UX failure and a latent authorization bug.

Separately, regular users with multiple tenant memberships receive a bare `400 tenant_required` with no tenant list payload, giving clients no way to render a picker without an extra round trip. And there is no server-side per-user preferred tenant, so context is lost when a user switches clients (CLI ↔ UI ↔ third-party plugin).

This change reshapes request authentication around a single `RequestContext` resolver that makes `X-Tenant-ID` optional for PlatformAdmins, enables full cross-tenant impersonation, validates tenant IDs against the canonical table, and persists a per-user preferred tenant server-side.

## What Changes

- **BREAKING** (internal API surface, no external breaking changes): replace `authenticated_tenant_context` / `require_platform_admin` / `tenant_scoped_context` with a unified `RequestContext` resolver (`cli/src/server/mod.rs`). All handlers migrate to the new context.
- Introduce an explicit tenant resolution chain with deterministic priority: `--tenant` flag → `AETERNA_TENANT` env → `X-Tenant-ID` header → local `.aeterna/context.toml` → `users.default_tenant_id` (server preference) → single-tenant auto-select → `400 select_tenant` with enriched payload.
- PlatformAdmin impersonation becomes first-class: any PlatformAdmin may target any existing tenant via `X-Tenant-ID`; absent the header, the request is platform-scoped (and succeeds for platform-wide endpoints).
- Validate `X-Tenant-ID` resolves to an existing tenant row; orphan IDs return `404 tenant_not_found` instead of silently resolving.
- Add `users.default_tenant_id UUID NULL REFERENCES tenants(id) ON DELETE SET NULL` with new endpoints `GET|PUT|DELETE /api/v1/user/me/default-tenant` and surface `defaultTenantId` in `/api/v1/auth/session`.
- Replace bare `400 tenant_required` with `400 select_tenant` carrying `availableTenants[]` and a human-readable hint so CLI/UI can drive an interactive picker without extra calls.
- Extend existing audit tables (`referential_audit_log`, `governance_audit_log`) with `acting_as_tenant_id UUID NULL REFERENCES tenants(id)` so PlatformAdmin impersonation is traceable.
- Add cross-tenant list mode for PlatformAdmin on selected read endpoints (`?tenant=*` query parameter on `GET /api/v1/admin/users`, `/admin/projects`, `/admin/orgs`) so operators can audit the instance without round-tripping through tenant selection.
- CLI/UI behavior changes live in separate proposals (`add-cli-auth-tenant-switch`, `persist-ui-tenant-selection`) which depend on this one.

## Capabilities

### New Capabilities

None. This change refactors existing capabilities without introducing new ones.

### Modified Capabilities

- `user-auth`: request authentication model changes — `RequestContext` replaces `authenticated_tenant_context`; `X-Tenant-ID` is now optional for PlatformAdmin and validated against the tenants table for all roles; adds `users.default_tenant_id` and `/user/me/default-tenant` endpoints; `/auth/session` payload gains `defaultTenantId`; `400 select_tenant` replaces `400 tenant_required` with an enriched payload.
- `granular-authorization`: PlatformAdmin is granted full cross-tenant impersonation (may target any existing tenant via `X-Tenant-ID`, may call platform-scoped endpoints with no tenant header); non-admin users attempting to target a tenant outside their role assignments receive `403 forbidden_tenant`.
- `tenant-admin-control-plane`: `POST /admin/tenants`, `POST /admin/tenants/provision`, `GET /admin/tenants`, and all platform-wide admin endpoints no longer require `X-Tenant-ID`; `GET /admin/users`, `/admin/projects`, `/admin/orgs` accept `?tenant=*` for cross-tenant listing by PlatformAdmin; fresh-deploy provisioning (`aeterna admin provision-tenant -f manifest.yaml`) succeeds without any pre-existing tenant.
- `multi-tenant-governance`: audit log tables gain `acting_as_tenant_id` column; every request records both actor identity and the tenant being impersonated (when applicable); audit query APIs expose impersonation filter.

## Impact

- **Affected code**: `cli/src/server/mod.rs` (new `RequestContext`, retire legacy helpers), `cli/src/server/plugin_auth.rs` (session payload extension), `cli/src/server/tenant_api.rs` (remove tenant-header requirement on platform routes, cross-tenant listing), `cli/src/server/user_api.rs` (new default-tenant endpoints), all `*_api.rs` handler modules (migrate call sites from old helpers to `RequestContext` — mechanical), `cli/src/storage/user_store.rs` and related stores (add `default_tenant_id` accessors).
- **Affected APIs**: new `GET|PUT|DELETE /api/v1/user/me/default-tenant`; `/api/v1/auth/session` response gains `defaultTenantId`; error code rename `tenant_required` → `select_tenant` with new payload shape; `GET /api/v1/admin/users`, `/admin/projects`, `/admin/orgs` accept `?tenant=*`; `X-Tenant-ID` header becomes optional on all platform-admin endpoints.
- **Affected schema**: new migration adds `users.default_tenant_id`, `referential_audit_log.acting_as_tenant_id`, `governance_audit_log.acting_as_tenant_id` (all NULL-able, no backfill needed).
- **Affected clients**: CLI and admin UI must migrate from `tenant_required` to `select_tenant` handling — covered by dependent proposals (`add-cli-auth-tenant-switch`, `persist-ui-tenant-selection`). Third-party plugin consumers observe no breaking changes provided they continue to pass `X-Tenant-ID` on tenant-scoped endpoints; PlatformAdmin consumers see new flexibility rather than a break.
- **Rollout**: migration is additive and backward-compatible; feature gate not required. Old error code `tenant_required` is kept as a compatibility alias for two minor versions, emitted only when a client sends `Accept-Error-Legacy: true` — see design.md.
