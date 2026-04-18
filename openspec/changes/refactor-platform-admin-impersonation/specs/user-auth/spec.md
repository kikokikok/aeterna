## MODIFIED Requirements

### Requirement: Request Authentication Context
The server SHALL resolve every authenticated request to a unified `RequestContext` that identifies the user, the user's PlatformAdmin status, and an optional resolved target tenant. The `X-Tenant-ID` header is a request-level override; when absent, the server SHALL apply a deterministic resolution chain to select the target tenant (or leave it unset for platform-scoped requests).

#### Scenario: X-Tenant-ID resolves to an existing tenant
- **WHEN** an authenticated request includes `X-Tenant-ID: <slug>`
- **AND** the slug corresponds to an existing row in the `tenants` table
- **THEN** the server SHALL populate `RequestContext.tenant` with the resolved tenant (id, slug, name)
- **AND** the server SHALL proceed to authorization checks using that tenant

#### Scenario: X-Tenant-ID does not resolve
- **WHEN** an authenticated request includes `X-Tenant-ID: <slug>`
- **AND** no row in the `tenants` table matches that slug
- **THEN** the server SHALL return `404 Not Found` with error code `tenant_not_found`
- **AND** the response body SHALL NOT enumerate any other tenants

#### Scenario: No X-Tenant-ID with server-side default
- **WHEN** an authenticated request does not include `X-Tenant-ID`
- **AND** the authenticated user has `users.default_tenant_id` set to a tenant the user is a member of
- **THEN** the server SHALL populate `RequestContext.tenant` with that default tenant

#### Scenario: No X-Tenant-ID, no default, single membership
- **WHEN** an authenticated request does not include `X-Tenant-ID`
- **AND** the user has no `default_tenant_id`
- **AND** the user is a member of exactly one tenant
- **THEN** the server SHALL populate `RequestContext.tenant` with that single tenant

#### Scenario: No X-Tenant-ID, PlatformAdmin
- **WHEN** an authenticated request does not include `X-Tenant-ID`
- **AND** the user has the PlatformAdmin role
- **THEN** the server SHALL leave `RequestContext.tenant` as `None`
- **AND** the request SHALL proceed; handlers decide whether a tenant is required via `require_target_tenant`

#### Scenario: No X-Tenant-ID, no default, multiple memberships, non-admin
- **WHEN** an authenticated request does not include `X-Tenant-ID`
- **AND** the user is not a PlatformAdmin
- **AND** the user has no `default_tenant_id`
- **AND** the user is a member of two or more tenants
- **THEN** the server SHALL return `400 Bad Request` with error code `select_tenant`
- **AND** the response body SHALL include `availableTenants: [{id, slug, name}, ...]` listing ONLY the user's tenant memberships
- **AND** the response body SHALL include a `hint` string indicating how to select a tenant (CLI and UI guidance)

### Requirement: User Default Tenant Preference
The server SHALL persist a per-user preferred tenant (`users.default_tenant_id`) and expose endpoints to read, set, and clear it. The default tenant is used when the user's active request does not specify `X-Tenant-ID`.

#### Scenario: Reading the default tenant
- **WHEN** an authenticated user sends `GET /api/v1/user/me/default-tenant`
- **THEN** the server SHALL return `{ defaultTenantId, defaultTenantSlug }` (both may be `null` if unset)

#### Scenario: Setting the default tenant as a member
- **WHEN** an authenticated user sends `PUT /api/v1/user/me/default-tenant` with body `{ slug: "<existing-tenant-slug>" }`
- **AND** the user is a member of that tenant, OR the user is a PlatformAdmin
- **THEN** the server SHALL update `users.default_tenant_id` to the tenant's id
- **AND** the server SHALL return `200 OK` with the updated `{ defaultTenantId, defaultTenantSlug }`

#### Scenario: Setting the default tenant without membership
- **WHEN** a non-admin user sends `PUT /api/v1/user/me/default-tenant` with a tenant slug they have no role assignments in
- **THEN** the server SHALL return `403 Forbidden` with error code `forbidden_tenant`
- **AND** the server SHALL NOT modify `users.default_tenant_id`

#### Scenario: Clearing the default tenant
- **WHEN** an authenticated user sends `DELETE /api/v1/user/me/default-tenant`
- **THEN** the server SHALL set `users.default_tenant_id` to `NULL`
- **AND** the server SHALL return `200 OK` with `{ defaultTenantId: null, defaultTenantSlug: null }`

#### Scenario: Default tenant is deleted
- **WHEN** a tenant is deleted that is referenced by one or more `users.default_tenant_id` values
- **THEN** the database FK constraint `ON DELETE SET NULL` SHALL clear those references automatically
- **AND** the affected users' next request SHALL proceed as if no default was set

### Requirement: Authentication Session Payload
The `/api/v1/auth/session` endpoint SHALL return the authenticated user's identity, tenant memberships, role assignments, and default tenant preference in a single response so clients can bootstrap their UI/CLI state without additional round trips.

#### Scenario: Session payload includes default tenant
- **WHEN** a valid bearer token is presented to `GET /api/v1/auth/session`
- **THEN** the response body SHALL include `defaultTenantId: string | null` and `defaultTenantSlug: string | null`
- **AND** the response SHALL also include `isPlatformAdmin`, `tenants: [{id, slug, name}, ...]`, and `activeTenantId` (resolved via the request-context chain for this session call)

### Requirement: Select-Tenant Error Shape
When the server determines that a tenant-scoped request cannot be resolved to a unique tenant, it SHALL return `400 Bad Request` with error code `select_tenant` and a payload that enables clients to render a picker without additional API calls.

#### Scenario: Error payload shape
- **WHEN** the server emits `select_tenant`
- **THEN** the JSON body SHALL contain: `error: "select_tenant"`, `message: <human string>`, `availableTenants: [{id, slug, name}]`, `hint: <string>`
- **AND** `availableTenants` SHALL be the caller's accessible tenants (their memberships, or all tenants if the caller is a PlatformAdmin)

#### Scenario: Legacy compatibility opt-in
- **WHEN** the request includes the header `Accept-Error-Legacy: true`
- **AND** the server would otherwise emit `select_tenant`
- **THEN** the server SHALL emit the legacy error code `tenant_required` instead
- **AND** the response body SHALL match the pre-refactor shape (`{ error: "tenant_required", message }`) without the `availableTenants` or `hint` fields
