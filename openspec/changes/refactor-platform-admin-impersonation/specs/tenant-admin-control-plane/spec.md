## MODIFIED Requirements

### Requirement: Platform-Wide Tenant Administration Endpoints
The endpoints `GET /api/v1/admin/tenants`, `POST /api/v1/admin/tenants`, `POST /api/v1/admin/tenants/provision`, and any other admin endpoints declared as platform-scoped SHALL require PlatformAdmin role but SHALL NOT require the `X-Tenant-ID` header. This enables first-tenant provisioning on fresh deployments.

#### Scenario: Fresh-deploy first tenant provisioning
- **WHEN** a PlatformAdmin calls `POST /api/v1/admin/tenants/provision` with a valid tenant manifest
- **AND** no tenants currently exist in the database
- **AND** no `X-Tenant-ID` header is provided
- **THEN** the server SHALL return `201 Created` with the provisioned tenant
- **AND** the new tenant SHALL be visible via subsequent `GET /api/v1/admin/tenants`

#### Scenario: List tenants without tenant context
- **WHEN** a PlatformAdmin calls `GET /api/v1/admin/tenants` with no `X-Tenant-ID` header
- **THEN** the server SHALL return `200 OK` with the full list of tenants

#### Scenario: Non-admin calls platform-wide endpoint
- **WHEN** a user without the PlatformAdmin role calls any endpoint declared platform-scoped (e.g., `GET /api/v1/admin/tenants`)
- **THEN** the server SHALL return `403 Forbidden` with error code `platform_admin_required`

### Requirement: Cross-Tenant Listing for PlatformAdmin
Selected read endpoints SHALL accept a `?tenant=*` query parameter that, when provided by a PlatformAdmin, returns results across all tenants. Response items SHALL include `tenantId` and `tenantSlug` columns identifying each row's tenant.

#### Scenario: Cross-tenant user listing
- **WHEN** a PlatformAdmin calls `GET /api/v1/admin/users?tenant=*`
- **THEN** the server SHALL return users from all tenants in a single response
- **AND** each user record SHALL include `tenantId` and `tenantSlug`

#### Scenario: Cross-tenant listing by non-admin
- **WHEN** a non-PlatformAdmin calls `GET /api/v1/admin/users?tenant=*`
- **THEN** the server SHALL return `403 Forbidden`

#### Scenario: Single-tenant listing preserved
- **WHEN** any authenticated caller calls `GET /api/v1/admin/users` without `?tenant=*` and with `X-Tenant-ID: <slug>`
- **THEN** the server SHALL return only users of that tenant (existing behavior preserved)

### Requirement: Tenant Provisioning Manifest Contract
The `POST /api/v1/admin/tenants/provision` endpoint SHALL continue to accept a `TenantManifest` body and produce an idempotent, atomic materialization of the tenant, its hierarchy, users, roles, configuration, and secrets. The change in this capability is solely about the authorization pre-conditions (X-Tenant-ID no longer required).

#### Scenario: Re-running the same manifest
- **WHEN** a PlatformAdmin calls `POST /api/v1/admin/tenants/provision` twice with an identical manifest
- **THEN** the first call SHALL return `201 Created`
- **AND** the second call SHALL return `200 OK` with no additional writes (idempotent)

#### Scenario: Dry-run provisioning
- **WHEN** a PlatformAdmin calls `POST /api/v1/admin/tenants/provision?dryRun=true`
- **THEN** the server SHALL validate the manifest and return the planned diff without persisting any changes
