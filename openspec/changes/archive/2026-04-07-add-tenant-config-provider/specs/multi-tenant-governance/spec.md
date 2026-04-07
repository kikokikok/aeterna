## MODIFIED Requirements

### Requirement: Tenant Isolation

The system SHALL enforce hard isolation at the company (tenant) boundary for all memory and knowledge operations.

Each tenant MUST have:
- Unique tenant identifier
- Isolated data storage (logical or physical)
- Independent configuration

#### Scenario: Tenant boundary enforcement
- **WHEN** a user from Company A attempts to access data belonging to Company B
- **THEN** the system SHALL reject the request with an authorization error
- **AND** the system SHALL NOT reveal whether the target resource exists

#### Scenario: Cross-tenant search isolation
- **WHEN** a user performs a vector similarity search
- **THEN** the system SHALL only return results from the user's tenant
- **AND** embeddings from other tenants SHALL NOT influence the search results

#### Scenario: Tenant configuration segregation
- **WHEN** the system stores or mutates tenant-specific configuration and secret references
- **THEN** each tenant SHALL have an independently addressable configuration surface bound to that tenant's unique identifier
- **AND** GlobalAdmin and TenantAdmin mutations SHALL be restricted to the tenant and ownership scope authorized for the caller
- **AND** the system SHALL reject tenant configuration references that cross tenant boundaries or expose raw secret values outside the tenant's approved secret container

#### Scenario: Shared provider connections respect tenant visibility boundaries
- **WHEN** the system exposes a platform-managed Git provider connection to one or more tenants
- **THEN** only explicitly allowed tenants SHALL be able to reference that connection in tenant configuration
- **AND** tenants that are not granted visibility SHALL receive an authorization or validation error without disclosure of hidden connection details
