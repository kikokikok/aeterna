## ADDED Requirements

### Requirement: Canonical Tenant Repository Binding
The system SHALL maintain a canonical knowledge repository binding for each tenant and use that binding to resolve tenant knowledge operations.

#### Scenario: Tenant binding determines repository resolution
- **WHEN** a knowledge read or write is executed for a tenant
- **THEN** the system SHALL resolve the backing repository from that tenant's configured repository binding
- **AND** the system SHALL NOT fall back to a process-global repository default that is shared implicitly across tenants

#### Scenario: Missing tenant binding fails closed
- **WHEN** a tenant-scoped knowledge operation requires a repository binding and no valid binding exists for that tenant
- **THEN** the system SHALL reject the operation with a configuration error
- **AND** the system SHALL NOT route the operation to another tenant's repository or to an unspecified default repository

#### Scenario: Tenant binding isolation
- **WHEN** one tenant updates its repository binding
- **THEN** the change SHALL affect only that tenant's knowledge resolution
- **AND** knowledge operations for other tenants SHALL continue using their own bindings unchanged

#### Scenario: Sync-managed tenant preserves admin-managed binding
- **WHEN** an IdP or hierarchy sync updates a tenant or its synced hierarchy records
- **THEN** the sync SHALL NOT overwrite an admin-managed tenant repository binding unless an explicit binding-ownership policy allows it
- **AND** tenant knowledge operations SHALL continue using the last valid binding for that tenant

### Requirement: Tenant Repository Binding Validation
The system SHALL validate tenant repository bindings before accepting configuration changes.

#### Scenario: Valid tenant repository binding
- **WHEN** an administrator configures a tenant repository binding with valid repository settings and credential references
- **THEN** the system SHALL persist the binding
- **AND** the binding SHALL be available for subsequent knowledge operations and admin inspection

#### Scenario: Invalid tenant repository binding
- **WHEN** an administrator configures a tenant repository binding with invalid structure, unsupported repository mode, or missing credential references
- **THEN** the system SHALL reject the binding update
- **AND** the error SHALL identify which binding fields failed validation

#### Scenario: Binding stores secret references instead of raw secrets
- **WHEN** an administrator configures a tenant repository binding that requires credentials
- **THEN** the system SHALL persist secret references or handles rather than raw secret material in the binding record
- **AND** binding inspection responses SHALL redact or omit secret-reference resolution details that would expose sensitive values
