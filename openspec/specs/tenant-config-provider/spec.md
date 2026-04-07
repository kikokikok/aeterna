# tenant-config-provider Specification

## Purpose
TBD - created by archiving change add-tenant-config-provider. Update Purpose after archive.
## Requirements
### Requirement: Canonical Tenant Configuration Provider
The system SHALL define a canonical tenant configuration contract and manage it through a provider abstraction.

#### Scenario: Tenant config separates non-secret config from secret references
- **WHEN** the system persists or returns tenant configuration
- **THEN** non-secret tenant config SHALL be stored separately from raw secret values
- **AND** the config SHALL contain only logical secret references or metadata rather than raw secret material

#### Scenario: Provider abstraction governs tenant config operations
- **WHEN** the control plane reads, writes, lists, or validates tenant configuration
- **THEN** those operations SHALL execute through the supported tenant configuration provider abstraction
- **AND** provider implementations SHALL enforce the canonical tenant config contract consistently

#### Scenario: Tenant config references shared Git provider connectivity by identifier
- **WHEN** a tenant is configured to use a supported Git provider connection such as GitHub App connectivity
- **THEN** the tenant configuration SHALL reference a platform-managed connection identifier rather than embedding provider certificate material in tenant-owned config
- **AND** the control plane SHALL preserve the underlying connection secret material outside the tenant-owned configuration document

### Requirement: Kubernetes Tenant Config Provider
The system SHALL provide a Kubernetes-backed tenant configuration provider that stores tenant config in a ConfigMap and tenant secret values in a paired Secret keyed by the stable tenant identifier.

#### Scenario: Kubernetes provider materializes tenant config containers
- **WHEN** a tenant is provisioned or updated through the supported control plane for the Kubernetes provider
- **THEN** the provider SHALL create or update a ConfigMap named from the unique tenant identifier
- **AND** the provider SHALL create or update a paired Secret named from the same tenant identifier for tenant secret values

#### Scenario: Tenant config references only paired tenant secret entries
- **WHEN** a tenant configuration document references secret material under the Kubernetes provider
- **THEN** each reference SHALL resolve only to keys within that tenant's paired Secret
- **AND** the provider SHALL reject references that target another tenant's secret container or unsupported locations

### Requirement: Tenant Configuration Ownership Segregation
The system SHALL segregate GlobalAdmin-managed and TenantAdmin-managed tenant configuration surfaces.

#### Scenario: Global admin bootstraps tenant config surface
- **WHEN** a GlobalAdmin creates or bootstraps tenant configuration
- **THEN** the system SHALL permit creation of the tenant-scoped config and secret containers plus platform-owned configuration fields
- **AND** the resulting configuration SHALL record ownership metadata for auditable segregation

#### Scenario: Tenant admin is limited to tenant-owned config
- **WHEN** a TenantAdmin updates tenant configuration or tenant secret entries
- **THEN** the system SHALL permit mutations only to tenant-owned fields for that tenant
- **AND** the system SHALL reject attempts to mutate platform-owned fields or any other tenant's configuration

#### Scenario: Tenant admin may use only visible shared Git provider connections
- **WHEN** a TenantAdmin assigns or updates Git provider connectivity for a tenant
- **THEN** the system SHALL permit references only to platform-managed connections explicitly visible to that tenant
- **AND** the system SHALL reject references to connections that are not assigned to or shared with that tenant

### Requirement: Secret Redaction and Honest Admin Surfaces
The system SHALL expose tenant configuration and tenant secret administration through supported surfaces without disclosing raw secret values.

#### Scenario: Tenant config inspection redacts secret values
- **WHEN** an operator inspects tenant configuration through the supported API or CLI
- **THEN** the response SHALL include config fields, logical secret names, and validation state
- **AND** it SHALL NOT include raw tenant secret values

#### Scenario: Invalid config or secret mutation fails honestly
- **WHEN** an operator submits invalid tenant config or an invalid tenant secret reference
- **THEN** the system SHALL reject the request with actionable validation output
- **AND** the previous valid provider state SHALL remain unchanged

