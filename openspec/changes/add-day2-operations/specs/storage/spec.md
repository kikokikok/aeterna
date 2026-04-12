## ADDED Requirements

### Requirement: Cross-Layer Reconciliation
The system SHALL periodically detect divergence between PostgreSQL, Qdrant, and DuckDB.

#### Scenario: Sampling-based reconciliation
- **WHEN** the reconciliation job runs on schedule
- **THEN** the system SHALL sample a configurable percentage of records (default 5%)
- **AND** the system SHALL check for matching records in the counterpart backend

#### Scenario: Targeted full scan on orphan detection
- **WHEN** the sampling detects orphans
- **THEN** the system SHALL trigger a targeted full scan of the affected time window to determine the full scope

### Requirement: Per-Tenant Storage Quotas
The system SHALL enforce configurable storage limits per tenant.

#### Scenario: Quota enforcement on write
- **WHEN** a write operation would cause a tenant to exceed their hard quota limit
- **THEN** the system SHALL reject the write with HTTP 429
- **AND** the system SHALL emit a metric and create a notification

#### Scenario: Quota usage caching
- **WHEN** the system checks quota for a tenant
- **THEN** the system SHALL cache the count results for a configurable period (default 5 minutes)

### Requirement: Retention Hard-Purge
The system SHALL hard-delete records that have been archived and exceeded their retention period.

#### Scenario: Audit log purge
- **WHEN** audit log records have been archived to S3 and are older than the retention period
- **THEN** the system SHALL DELETE them from PostgreSQL

#### Scenario: Soft-delete hard-purge
- **WHEN** soft-deleted records are older than the configured TTL
- **THEN** the system SHALL hard-delete them permanently

### Requirement: Tenant Data Quarantine and Purge
The system SHALL quarantine deactivated tenant data before permanent deletion.

#### Scenario: Quarantine period
- **WHEN** a tenant is deactivated
- **THEN** the system SHALL retain all tenant data for the configured quarantine period (default 30 days)
- **AND** the system SHALL reject all write operations for the deactivated tenant

#### Scenario: Purge after quarantine
- **WHEN** the quarantine period has elapsed
- **THEN** the system SHALL create a RequireApproval remediation request for full tenant data purge across all backends
