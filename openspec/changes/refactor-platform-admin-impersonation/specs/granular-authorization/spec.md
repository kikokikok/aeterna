## MODIFIED Requirements

### Requirement: PlatformAdmin Tenant Impersonation
A user with the PlatformAdmin role SHALL be authorized to act in the context of any existing tenant via the `X-Tenant-ID` header, with full access equivalent to a tenant administrator of the targeted tenant. Absent the header, a PlatformAdmin request is considered platform-scoped and SHALL be authorized only for endpoints declared as platform-wide.

#### Scenario: PlatformAdmin impersonates any existing tenant
- **WHEN** a PlatformAdmin sends a tenant-scoped request with `X-Tenant-ID: <valid-slug>`
- **THEN** the server SHALL authorize the request as if the user were the tenant administrator of that tenant
- **AND** the audit log SHALL record `actor_user_id` (the PlatformAdmin) and `acting_as_tenant_id` (the impersonated tenant)

#### Scenario: PlatformAdmin calls platform-scoped endpoint without tenant header
- **WHEN** a PlatformAdmin sends a request to a platform-scoped endpoint (e.g., `GET /api/v1/admin/tenants`, `POST /api/v1/admin/tenants/provision`) with no `X-Tenant-ID` header
- **THEN** the server SHALL authorize the request
- **AND** the request SHALL succeed without requiring a tenant selection

#### Scenario: PlatformAdmin calls tenant-scoped endpoint without tenant header
- **WHEN** a PlatformAdmin sends a request to a tenant-scoped endpoint (e.g., `GET /api/v1/user`) with no `X-Tenant-ID` header and no `default_tenant_id` set
- **THEN** the server SHALL return `400 select_tenant`
- **AND** the payload's `availableTenants` list SHALL contain every tenant in the system (PlatformAdmin can target any)

### Requirement: Tenant Membership Authorization
A non-PlatformAdmin user SHALL be authorized only within tenants where the user has an explicit role assignment. Attempting to target a foreign tenant via `X-Tenant-ID` SHALL be rejected.

#### Scenario: Non-admin targets foreign tenant
- **WHEN** a user without the PlatformAdmin role sends a request with `X-Tenant-ID: <valid-slug>`
- **AND** the user has no role assignments in that tenant
- **THEN** the server SHALL return `403 Forbidden` with error code `forbidden_tenant`
- **AND** the response body SHALL NOT confirm or deny the existence of the tenant beyond what is already known from the 403 vs 404 distinction

#### Scenario: Non-admin within own tenant
- **WHEN** a user without the PlatformAdmin role sends a request with `X-Tenant-ID: <valid-slug>` matching one of their role assignments
- **THEN** the server SHALL authorize the request according to the roles the user holds in that tenant

### Requirement: Audit Logging for Impersonation
Every authenticated request that mutates state or accesses tenant-scoped data SHALL be recorded in the appropriate audit log with both the acting user and, when applicable, the tenant being acted upon.

#### Scenario: Impersonated mutation recorded
- **WHEN** a PlatformAdmin performs a write action on resources of tenant `T` via `X-Tenant-ID: T`
- **THEN** the `referential_audit_log` or `governance_audit_log` entry SHALL have `actor_user_id = <admin-user-id>` and `acting_as_tenant_id = <T's-id>`

#### Scenario: Direct action by tenant member
- **WHEN** a tenant member performs a write action within their own tenant
- **THEN** the audit log entry SHALL have `actor_user_id = <their-id>` and `acting_as_tenant_id = <their-tenant-id>` (matching, not impersonation)
