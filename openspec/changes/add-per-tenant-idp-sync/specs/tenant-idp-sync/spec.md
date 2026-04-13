# tenant-idp-sync Specification

## Purpose
Define how each tenant configures and triggers its own Identity Provider (IdP) synchronization,
independent of other tenants and without requiring platform-level environment variable changes.

## Requirements

### Requirement: Per-Tenant GitHub App Configuration
Each tenant SHALL be able to store its own GitHub App credentials and organization reference
in its tenant config document, using well-known config key names.

#### Scenario: Tenant with GitHub config triggers sync
- **GIVEN** a tenant has `github.org_name`, `github.app_id`, `github.installation_id` in its
  config fields and `github.app_pem` as a tenant secret
- **WHEN** `POST /admin/tenants/{tenant}/sync/github` is called by a PlatformAdmin or TenantAdmin
- **THEN** the server SHALL resolve credentials from the tenant config and secret
- **AND** SHALL execute the GitHub organization sync against the configured org
- **AND** SHALL return a sync report with users_created, users_updated, groups_synced

#### Scenario: Tenant without GitHub config falls back to env vars
- **GIVEN** a tenant has no `github.org_name` in its config
- **WHEN** `POST /admin/tenants/{tenant}/sync/github` is called
- **THEN** the server SHALL attempt to resolve credentials from environment variables
- **AND** SHALL fail with a clear error if env vars are also absent

### Requirement: Multi-Tenant Fan-Out
The platform-wide sync endpoint SHALL fan out to all tenants that have GitHub config present,
running each tenant sync independently.

#### Scenario: Fan-out returns per-tenant results
- **GIVEN** multiple tenants each have `github.org_name` configured
- **WHEN** `POST /admin/sync/github` is called by a PlatformAdmin (no tenant scoping)
- **THEN** the server SHALL sync each configured tenant in sequence
- **AND** SHALL return `{ results: [{ tenant_id, status, report | error }] }` for each

#### Scenario: Fan-out partial failure is non-blocking
- **GIVEN** tenant A has valid GitHub config and tenant B has invalid config
- **WHEN** `POST /admin/sync/github` fan-out runs
- **THEN** tenant A SHALL be synced successfully
- **AND** tenant B's failure SHALL be recorded in results but SHALL NOT abort tenant A's sync

### Requirement: Concurrency Protection
Per-tenant sync SHALL be protected against concurrent execution for the same tenant.
Platform-wide fan-out SHALL be protected against overlapping fan-out runs.

#### Scenario: Concurrent per-tenant sync is rejected
- **GIVEN** a sync for tenant X is already running
- **WHEN** a second `POST /admin/tenants/{tenant}/sync/github` arrives for tenant X
- **THEN** the server SHALL return `409 Conflict` with `{ "error": "sync_in_progress" }`

### Requirement: Authorization
Per-tenant sync SHALL be accessible to PlatformAdmin and TenantAdmin roles for that tenant.
The fan-out endpoint SHALL require PlatformAdmin.

#### Scenario: TenantAdmin can trigger their own tenant sync
- **GIVEN** a user with TenantAdmin role for tenant X
- **WHEN** they call `POST /admin/tenants/X/sync/github`
- **THEN** the request SHALL succeed (subject to GitHub config being present)

#### Scenario: TenantAdmin cannot trigger other tenant sync
- **GIVEN** a user with TenantAdmin role for tenant X
- **WHEN** they call `POST /admin/tenants/Y/sync/github` (different tenant)
- **THEN** the server SHALL return `403 Forbidden`
