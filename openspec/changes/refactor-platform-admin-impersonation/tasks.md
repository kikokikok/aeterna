## 1. Schema migration

- [x] 1.1 Create `storage/migrations/023_platform_admin_impersonation.sql` adding `users.default_tenant_id UUID NULL REFERENCES tenants(id) ON DELETE SET NULL` with an index `idx_users_default_tenant_id`.
- [x] 1.2 In the same migration, add `acting_as_tenant_id UUID NULL REFERENCES tenants(id)` to `referential_audit_log` and `governance_audit_log` with matching indexes `idx_*_audit_log_acting_as_tenant`.
- [ ] 1.3 Verify migration applies cleanly on a copy of a production-shape database (no backfill required; all existing rows stay `NULL`). _(deferred to PR CI bootstrap run — no local Postgres available)_
- [x] 1.4 Add downgrade notes to `storage/migrations/README.md` (columns are drop-safe).

## 2. Core `RequestContext` resolver

- [ ] 2.1 Define `pub struct RequestContext { user: AuthenticatedUser, is_platform_admin: bool, tenant: Option<ResolvedTenant>, request_id: RequestId }` in `cli/src/server/context.rs` (new file).
- [ ] 2.2 Implement `pub async fn request_context(state: &AppState, headers: &HeaderMap) -> Result<RequestContext, Response>` that executes the resolution chain in order: `X-Tenant-ID` header → `users.default_tenant_id` lookup → single-tenant auto-select → `None` for PlatformAdmin or `Err(select_tenant)` otherwise.
- [ ] 2.3 Validate `X-Tenant-ID` resolves to an existing `tenants` row; return `404 tenant_not_found` for orphan IDs (include a scenario in tests that regression-guards the current silent-accept bug).
- [ ] 2.4 For non-admin users targeting a foreign tenant via `X-Tenant-ID`, return `403 forbidden_tenant` with a message that does NOT enumerate foreign tenant names.
- [ ] 2.5 Add `pub fn require_platform_admin(ctx: &RequestContext) -> Result<(), Response>` that checks `ctx.is_platform_admin` without any tenant dependency.
- [ ] 2.6 Add `pub fn require_target_tenant(ctx: &RequestContext) -> Result<&ResolvedTenant, Response>` that returns the resolved tenant or emits `400 select_tenant` with enriched payload.
- [ ] 2.7 Implement `select_tenant` error builder that attaches `{ availableTenants: [{slug, name, id}], hint }` to the JSON body; add `Accept-Error-Legacy` compat header support (emits legacy `tenant_required` when set).
- [ ] 2.8 Unit-test `request_context` across every branch of the resolution chain (10+ scenarios).

## 3. Retire legacy auth helpers

- [ ] 3.1 Mark `authenticated_tenant_context`, `tenant_scoped_context`, and the old `require_platform_admin` in `cli/src/server/mod.rs` as `#[deprecated]` with a pointer to `RequestContext`.
- [ ] 3.2 Migrate every call site in `cli/src/server/*_api.rs` to `RequestContext`, preserving behavior. Use `require_target_tenant` for tenant-scoped handlers, `require_platform_admin` for platform-scoped handlers.
- [ ] 3.3 Remove deprecated helpers after the migration is complete; confirm `cargo build --all-targets` passes with `#[deny(deprecated)]`.
- [ ] 3.4 Audit every 400 response in the API surface: any remaining `tenant_required` emission sites either move to `select_tenant` or are removed.

## 4. Default tenant preference endpoints

- [ ] 4.1 Add `GET /api/v1/user/me/default-tenant` returning `{ defaultTenantId: string | null, defaultTenantSlug: string | null }`.
- [ ] 4.2 Add `PUT /api/v1/user/me/default-tenant` with body `{ slug: string }`; validate the caller is a member of the target tenant (PlatformAdmin exempt); update `users.default_tenant_id`; return updated payload.
- [ ] 4.3 Add `DELETE /api/v1/user/me/default-tenant` clearing the preference (sets to `NULL`).
- [ ] 4.4 Extend `/api/v1/auth/session` response shape to include `defaultTenantId` and `defaultTenantSlug` fields (backward-compatible additive change).
- [ ] 4.5 Add storage methods: `UserStore::set_default_tenant(user_id, Option<TenantId>)`, `UserStore::get_default_tenant(user_id) -> Option<ResolvedTenant>`.
- [ ] 4.6 Ensure `ON DELETE SET NULL` FK behavior is covered by an integration test (create tenant, set as default, delete tenant, confirm default is cleared).

## 5. PlatformAdmin cross-tenant listing

- [ ] 5.1 `GET /api/v1/admin/users?tenant=*` returns users across all tenants (PlatformAdmin only); response items include `tenantId` + `tenantSlug` columns.
- [ ] 5.2 `GET /api/v1/admin/projects?tenant=*` same shape for projects.
- [ ] 5.3 `GET /api/v1/admin/orgs?tenant=*` same shape for organizational units.
- [ ] 5.4 `?tenant=*` requests by non-PlatformAdmin return `403 forbidden`.
- [ ] 5.5 Without `?tenant=*`, existing single-tenant listing semantics are preserved (requires `X-Tenant-ID` via `require_target_tenant`).
- [ ] 5.6 Add `tenant_filter` query param documentation to the OpenAPI/Redoc schema.

## 6. Audit log impersonation tracking

- [ ] 6.1 In the audit-log middleware/helpers, populate `acting_as_tenant_id` from `ctx.tenant.as_ref().map(|t| t.id)` for every recorded event.
- [ ] 6.2 Populate `actor_id` from `ctx.user.id` (unchanged) and add a derived column `is_impersonation = actor_tenant_id IS DISTINCT FROM acting_as_tenant_id` at query time.
- [ ] 6.3 Extend audit query endpoints (`GET /api/v1/admin/audit`) with a `?onlyImpersonation=true` filter.
- [ ] 6.4 Add structured log fields `actor_user_id`, `acting_as_tenant_id`, `is_platform_admin`, `request_id` to every request-scoped log line via the tracing middleware.

## 7. Tests

- [ ] 7.1 Integration test: fresh database → PlatformAdmin JWT → `POST /api/v1/admin/tenants/provision` with manifest, no `X-Tenant-ID` → 201 Created, tenant row exists.
- [ ] 7.2 Integration test: PlatformAdmin with `X-Tenant-ID: <foreign-tenant>` → `GET /api/v1/user` → 200 with that tenant's users.
- [ ] 7.3 Integration test: PlatformAdmin with no `X-Tenant-ID` → `GET /api/v1/admin/users?tenant=*` → 200 with cross-tenant list.
- [ ] 7.4 Integration test: regular user with two tenant memberships, no `X-Tenant-ID`, no default → `GET /api/v1/user` → 400 `select_tenant` with `availableTenants[]` populated.
- [ ] 7.5 Integration test: regular user, `PUT /user/me/default-tenant` with a slug they are NOT a member of → 403.
- [ ] 7.6 Integration test: regular user, `PUT /user/me/default-tenant` with a valid slug → 200; subsequent `GET /user` (no header) → 200.
- [ ] 7.7 Integration test: any user, `X-Tenant-ID: nonexistent-slug` → 404 `tenant_not_found`.
- [ ] 7.8 Integration test: regular user, `X-Tenant-ID: <tenant they are not a member of>` → 403 `forbidden_tenant`.
- [ ] 7.9 Integration test: audit log row for PlatformAdmin impersonation records both `actor_id` and `acting_as_tenant_id` correctly.
- [ ] 7.10 Compatibility test: request with `Accept-Error-Legacy: true` and missing tenant → 400 `tenant_required` (legacy code) instead of `select_tenant`.

## 8. Documentation

- [ ] 8.1 Update `docs/auth.md` (or create if missing) with the resolution chain diagram and PlatformAdmin impersonation model.
- [ ] 8.2 Document the `select_tenant` error shape in `docs/api-errors.md`.
- [ ] 8.3 Update `README.md` "First-time deployment" section: fresh-deploy workflow is now `aeterna admin provision-tenant -f manifest.yaml` by the bootstrapped PlatformAdmin, no tenant seeding required.
- [ ] 8.4 Add an example tenant manifest at `examples/tenant-manifest.yaml`.
- [ ] 8.5 Update the OpenAPI schema generation for the three endpoints in section 4 and the cross-tenant query params in section 5.

## 9. Rollout

- [ ] 9.1 Merge behind no feature flag — the change is additive and safe.
- [ ] 9.2 Release notes highlight: (a) PlatformAdmin UX unblocked on fresh deploys, (b) new `/user/me/default-tenant` endpoints, (c) `tenant_required` deprecation schedule (removed in 2 minor versions).
- [ ] 9.3 Coordinate with `add-cli-auth-tenant-switch` and `persist-ui-tenant-selection` so CLI and UI handle `select_tenant` before the server migration lands in production.
