# Tasks — Cross-Tenant Admin Listing (#44.d)

## 1. Core scope resolver

- [ ] 1.1 Add `ListTenantScope` enum to `cli/src/server/context.rs` with `Single(ResolvedTenant)` and `All` variants. Derive `Debug`.
- [ ] 1.2 Implement `RequestContext::list_scope(&self, state, headers, query_tenant: Option<&str>) -> Result<ListTenantScope, Response>` with the resolution rules documented in the proposal.
- [ ] 1.3 Implement a new error builder `forbidden_scope_response(required_role)` emitting `403 { error: "forbidden_scope", required_role: "PlatformAdmin", message: "..." }`.
- [ ] 1.4 Accept both `?tenant=*` and `?tenant=all` in the parser; emit a deprecation warning log for `all` to steer toward `*`.
- [ ] 1.5 Unit tests (8, all in `context.rs::tests`):
  - [ ] `list_scope_omitted_resolves_to_single_via_context`
  - [ ] `list_scope_explicit_slug_resolves_to_single`
  - [ ] `list_scope_uuid_resolves_to_single`
  - [ ] `list_scope_star_as_admin_returns_all`
  - [ ] `list_scope_star_as_non_admin_returns_forbidden_scope`
  - [ ] `list_scope_alias_all_accepted_with_warning`
  - [ ] `list_scope_single_cross_tenant_requires_membership_or_admin`
  - [ ] `list_scope_nonexistent_tenant_returns_404`

## 2. Handler migration (one commit per handler)

- [x] 2.1 `GET /admin/tenants` — replace local `require_platform_admin` with `RequestContext` + `list_scope`; default to `All` for backward compat (this endpoint was always cross-tenant).
- [~] 2.2 `GET /user` — accept `?tenant=<slug|uuid|*>`; return `scope`+`tenant`+`items[]` envelope **only when `scope=all`**, otherwise keep existing body (backward compat); decorate each item with `tenantId`+`tenantSlug` in `scope=all` mode. **Partial:** `?tenant=*` and `?tenant=all` (deprecated) implemented; `?tenant=<slug>` returns `501 scope_not_implemented` pending PR #65. Per-tenant role aggregation in All mode returns `[]` with TODO → PR #66.
- [~] 2.3 `GET /project` — same treatment. **Partial:** `?tenant=*`/`all` implemented; `?tenant=<slug>` returns `501 scope_not_implemented` pending PR #65 cluster.
- [~] 2.4 `GET /org` — same treatment. **Partial:** `?tenant=*`/`all` implemented; `?tenant=<slug>` returns `501 scope_not_implemented` pending PR #65 cluster.
- [~] 2.5 `GET /govern/audit` — **Partial:** gates + envelope wrapper + `?actor`/`?since` filter composition implemented. Per-item `tenantId`/`tenantSlug` decoration is **explicitly deferred**: `governance_audit_log` has no row-level `tenant_id` (only the nullable `acting_as_tenant_id` from migration 023, which isn't exposed in `AuditRow`). Full tenant decoration requires a follow-up PR that (a) surfaces `acting_as_tenant_id` in `AuditRow` + the SQL, and (b) arguably adds a proper `tenant_id` column via migration + backfill. `?tenant=<slug>` returns `501 scope_not_implemented`. Excluded from the §4.1 `tenantId+tenantSlug` contract test for this reason.
- [ ] 2.6 Add `tenant_filter` param + new envelope to OpenAPI/Redoc schema for each of the 5.

## 3. Cross-tenant repository layer

- [ ] 3.1 Each of the 5 stores (`user_store`, `project_store`, `org_store`, `tenant_store`, `audit_store`) gains a `list_all(pagination) -> Result<Vec<Row>, StorageError>` method returning rows with `tenant_id` joined in.
- [ ] 3.2 Stable sort key `(tenant_id, id)` for deterministic pagination across the union.
- [ ] 3.3 Ensure `SELECT ... FROM table` does NOT implicitly filter by tenant (these are the *only* queries on the entire codebase allowed to read across tenant boundaries — add a `#[doc(hidden)]` marker and a comment flagging the audit requirement).

## 4. Contract tests

- [~] 4.1 Contract tests landed in `cli/tests/server_runtime_test.rs` (not a dedicated file — sharing the fixture would have duplicated ~300 lines; can be migrated once a `tests/common/mod.rs` exists). `assert_cross_tenant_envelope_contract` helper covers contract for `/project` and `/org` across 2 seeded tenants; `/user` is best-effort (asserts contract when 200, skips on 503 fixture variant). `/govern/audit` will extend this when §2.5 lands.
- [ ] 4.2 Add RLS regression guard `storage/tests/rls_boundary_test.rs`: asserts each of `users`, `projects`, `orgs`, `tenants`, `referential_audit_log`, `governance_audit_log` has `relrowsecurity = false` in `pg_class`. If a future migration RLS-enables one of these, this test fails and forces a redesign of the cross-tenant reader.

## 5. Integration tests

- [ ] 5.1 PlatformAdmin + `GET /admin/users?tenant=*` → 200, items span ≥2 tenants, every item has `tenantId`/`tenantSlug`.
- [ ] 5.2 Regular user + `GET /admin/users?tenant=*` → 403 `forbidden_scope`.
- [ ] 5.3 PlatformAdmin + `GET /admin/users` (no `?tenant`) → behaves as today (single tenant from `X-Tenant-ID`/default).
- [ ] 5.4 PlatformAdmin + `GET /admin/users?tenant=foreign-slug` → 200, items are from `foreign-slug` only.
- [ ] 5.5 PlatformAdmin + `GET /admin/users?tenant=nonexistent` → 404 `tenant_not_found`.
- [ ] 5.6 `GET /admin/users?tenant=all` → 200 + deprecation log line (caught with `tracing-test`).
- [ ] 5.7 Pagination with `?tenant=*` yields consistent ordering across pages (snapshot test with seeded data).
- [ ] 5.8 `POST /admin/users` with `?tenant=*` → 400 `scope_not_allowed_for_write` (writes explicitly forbidden in `All` scope).

## 6. CLI updates (separate PR, tracked here for cross-reference)

- [ ] 6.1 Stop emitting `x-target-tenant-id` in `AeternaClient::get/post/put/delete`. Remove the `target_tenant` field from the client builder.
- [ ] 6.2 Add `--all-tenants` flag to `aeterna admin users list`, `projects list`, `orgs list`, `audit list` (maps to `?tenant=*`).
- [ ] 6.3 Add `--tenant <slug>` flag to the same commands (maps to `?tenant=<slug>`). Mutually exclusive with `--all-tenants`.
- [ ] 6.4 Decorate CLI table output with a `TENANT` column when in `--all-tenants` mode.

## 7. Documentation

- [ ] 7.1 Update `docs/api/admin.md` (or create if absent) with the `?tenant=` convention table from the proposal.
- [ ] 7.2 Add a "Cross-tenant operations" section to `USER_GUIDE.md` / `DEVELOPER_GUIDE.md` describing when to use `--all-tenants`.
- [ ] 7.3 Update the 7-stage framework proposal README for `refactor-platform-admin-impersonation` to mark section 5 delegated to this change.

## 8. Cleanup (feeds #44.e)

- [ ] 8.1 Add `#[deprecated(note = "Use ?tenant=<slug> or ?tenant=* instead")]` to the header-reading code path in `authenticated_tenant_context`.
- [ ] 8.2 Structured log every `X-Target-Tenant-Id` read with `warn!(target: "compat", header = "x-target-tenant-id", ...)` so observability can tally remaining clients.
- [ ] 8.3 Ship one minor release with deprecation warnings active; #44.e removes the code path entirely.
