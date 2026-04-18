## MODIFIED Requirements

### Requirement: Audit Log Schema
The `referential_audit_log` and `governance_audit_log` tables SHALL record both the identity of the acting user and, when applicable, the tenant in whose context the action was performed. The latter SHALL be represented by an `acting_as_tenant_id UUID NULL REFERENCES tenants(id) ON DELETE SET NULL` column on each audit table.

#### Scenario: Impersonated action record
- **WHEN** an audit event is written for a request where `RequestContext.tenant = Some(T)` and the acting user is a PlatformAdmin
- **THEN** the audit row SHALL have `actor_id = <admin-user-id>` and `acting_as_tenant_id = <T.id>`

#### Scenario: Direct tenant-member action record
- **WHEN** an audit event is written for a request by a tenant member acting within their own tenant
- **THEN** the audit row SHALL have `actor_id = <user-id>` and `acting_as_tenant_id = <user's-tenant-id>`
- **AND** consumers MAY derive `is_impersonation = false` by comparing `acting_as_tenant_id` against the user's primary tenant

#### Scenario: Platform-scoped action record
- **WHEN** an audit event is written for a request where `RequestContext.tenant = None` (e.g., a PlatformAdmin listing all tenants)
- **THEN** the audit row SHALL have `actor_id = <admin-user-id>` and `acting_as_tenant_id IS NULL`

#### Scenario: Tenant deletion preserves audit history
- **WHEN** a tenant referenced by audit rows is deleted
- **THEN** `ON DELETE SET NULL` SHALL clear `acting_as_tenant_id` to `NULL` on those rows
- **AND** the rest of the audit row (actor, action, old_values, new_values, timestamp) SHALL remain intact

### Requirement: Audit Query Filtering by Impersonation
The audit query endpoints SHALL support filtering by impersonation status so operators can review PlatformAdmin activity across tenants.

#### Scenario: Impersonation-only filter
- **WHEN** a PlatformAdmin calls `GET /api/v1/admin/audit?onlyImpersonation=true`
- **THEN** the server SHALL return audit rows where `acting_as_tenant_id` is set and does not match the actor's primary tenant

#### Scenario: Per-tenant audit with impersonation visibility
- **WHEN** a tenant administrator calls `GET /api/v1/admin/audit?tenant=<their-tenant>`
- **THEN** the server SHALL return audit rows where `acting_as_tenant_id = <their-tenant-id>`
- **AND** those rows SHALL include the `actor_id` and whether the actor is a PlatformAdmin (so tenant admins can see who impersonated their tenant)
