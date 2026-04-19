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
- [x] 2.5 `GET /govern/audit` — **Complete (Bundle D).** Full `?tenant=` grammar supported: gates + envelope + `?actor`/`?since` filter composition + per-item `tenantId`/`tenantSlug`/`actingAsTenantId` decoration + `?tenant=<slug>` filter via `governance_audit_log.acting_as_tenant_id`. Resolution: surfaced `acting_as_tenant_id` in `AuditRow` + `GovernanceAuditEntry` (serde-skipped when `None` for wire compat), added the column to `AuditFilters`, threaded `acting_as_tenant_id = target_tenant_id.unwrap_or(own_tenant_id)` through all 8 tenant-scoped `log_audit` call sites (+ 4 tools-layer sites with `None`). No migration needed — migration 023 had already added the column; the work was pipeline-side only. Backward compat: pre-Bundle-D rows have `acting_as_tenant_id = NULL`, remain visible under `scope=all` (with null tenant decoration), filtered out under `scope=tenant` (the only correct behavior given the write-time gap). Covered by `list_audit_cross_tenant_scope_gates_and_filter_compose` in `cli/tests/server_runtime_test.rs`.
- [~] 2.6 Scope-adjusted: the repo has no OpenAPI/Redoc generation in place (no `utoipa` or similar). Landed the **source-of-truth doc** (`docs/api/admin.md`) that a future OpenAPI generator should read from. All 5 endpoints, the `?tenant=` grammar, the envelope contract, error codes, and the audit exception are documented there. Generator work deferred.

## 3. Cross-tenant repository layer

- [ ] 3.1 Each of the 5 stores (`user_store`, `project_store`, `org_store`, `tenant_store`, `audit_store`) gains a `list_all(pagination) -> Result<Vec<Row>, StorageError>` method returning rows with `tenant_id` joined in.
- [ ] 3.2 Stable sort key `(tenant_id, id)` for deterministic pagination across the union.
- [ ] 3.3 Ensure `SELECT ... FROM table` does NOT implicitly filter by tenant (these are the *only* queries on the entire codebase allowed to read across tenant boundaries — add a `#[doc(hidden)]` marker and a comment flagging the audit requirement).

## 4. Contract tests

- [x] 4.1 Contract tests in `cli/tests/server_runtime_test.rs` (not a dedicated file — sharing the fixture would have duplicated ~300 lines; can be migrated once a `tests/common/mod.rs` exists). `assert_cross_tenant_envelope_contract` helper covers contract for `/project` and `/org` across 2 seeded tenants; `/user` is best-effort (asserts contract when 200, skips on 503 fixture variant). `/govern/audit` has its own dedicated test (`list_audit_cross_tenant_scope_gates_and_filter_compose`) rather than using the generic helper, because pre-Bundle-D rows have `acting_as_tenant_id = NULL` and surface without tenant decoration — this is the only acceptable wire-compat behavior and the dedicated test validates the dispatch, gates, filter composition, and both envelope variants (scope=all and scope=tenant) explicitly.
- [x] 4.2 `storage/tests/rls_boundary_test.rs` landed. Checks both `relrowsecurity` AND `relforcerowsecurity` in `pg_class` for every table in the cross-tenant readable set (`tenants`, `users`, `organizational_units`, `governance_audit_log`, `referential_audit_log`). Note: this project uses `organizational_units` rather than separate `projects`/`orgs` tables — the original task listed logical entities; the guard tracks the physical tables the readers actually touch. Error message documents 3 resolution paths so a future dev hitting this failure has immediate direction.

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

- [x] 7.1 `docs/api/admin.md` created — canonical reference for the `?tenant=` grammar, envelope contract, error codes, audit exception, ordering invariants, and backward compatibility story.
- [~] 7.2 `docs/DEVELOPER_GUIDE.md` gained a "Cross-Tenant Operations (#44.d)" section. Links to the canonical doc, summarizes who-can-call-what, calls out the RLS safety net, and includes a common-debugging checklist. `USER_GUIDE.md` does not exist in this repo (only `DEVELOPER_GUIDE.md`); the user-facing content will move there when/if it's created, or be folded into the CLI flags PR (§6) which is where end-user-visible tenancy switching lands.
- [x] 7.3 `refactor-platform-admin-impersonation/tasks.md` §5 now marked as delegated to this change, with each sub-task linked to the landing PR. Ensures readers of the parent change see #44.d as the single source of truth instead of stale/duplicate tracking.

## 8. Cleanup (feeds #44.e)

- [ ] 8.1 Add `#[deprecated(note = "Use ?tenant=<slug> or ?tenant=* instead")]` to the header-reading code path in `authenticated_tenant_context`.
- [ ] 8.2 Structured log every `X-Target-Tenant-Id` read with `warn!(target: "compat", header = "x-target-tenant-id", ...)` so observability can tally remaining clients.
- [ ] 8.3 Ship one minor release with deprecation warnings active; #44.e removes the code path entirely.
