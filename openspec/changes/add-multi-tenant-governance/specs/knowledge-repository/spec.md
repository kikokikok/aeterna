## MODIFIED Requirements

### Requirement: Knowledge Item Creation
The system SHALL provide a method to propose new knowledge items with automatic ID generation and governance validation.

#### Scenario: Create knowledge item with valid data and tenant context
- **WHEN** proposing a knowledge item with valid type, title, summary, content, and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL validate knowledge content against governance policies
- **AND** system SHALL generate a unique ID
- **AND** system SHALL set initial status to 'draft'
- **AND** system SHALL create Git commit with type='create' and tenant metadata
- **AND** system SHALL return the created item

#### Scenario: Create knowledge item with invalid type
- **WHEN** proposing a knowledge item with invalid type and TenantContext
- **THEN** system SHALL return INVALID_TYPE error
- **AND** error SHALL list valid types (adr, policy, pattern, spec)

#### Scenario: Create knowledge item without tenant context
- **WHEN** proposing a knowledge item without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Knowledge Query Operation
The system SHALL provide a method to query knowledge items with flexible filtering and tenant isolation.

#### Scenario: Query all knowledge items with tenant context
- **WHEN** querying knowledge without filters but with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL return all accessible items within the tenant
- **AND** system SHALL include item summaries (not full content)
- **AND** system SHALL include totalCount

#### Scenario: Query with type filter
- **WHEN** querying knowledge with type='adr' and TenantContext
- **THEN** system SHALL only return ADR items within the tenant

#### Scenario: Query with layer filter
- **WHEN** querying knowledge with layer='project' and TenantContext
- **THEN** system SHALL only return project-level knowledge within the tenant

#### Scenario: Query with status filter
- **WHEN** querying knowledge with status=['accepted'] and TenantContext
- **THEN** system SHALL only return accepted items within the tenant
- **AND** system SHALL default to ['accepted'] if not specified

### Requirement: Knowledge Get Operation
The system SHALL provide a method to retrieve a knowledge item by ID with tenant isolation.

#### Scenario: Get existing knowledge item with tenant context
- **WHEN** getting a knowledge item with valid ID and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL verify the item belongs to the same tenant
- **AND** system SHALL return the full item content

#### Scenario: Get non-existent knowledge item
- **WHEN** getting a knowledge item with invalid ID and TenantContext
- **THEN** system SHALL return null without error

#### Scenario: Get knowledge item from different tenant
- **WHEN** getting a knowledge item that belongs to a different tenant
- **THEN** system SHALL return null without revealing cross-tenant information

### Requirement: Constraint Check Operation
The system SHALL validate knowledge items against defined constraints with tenant-specific policy enforcement.

#### Scenario: Check constraint with tenant context
- **WHEN** checking a knowledge item against constraints with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL apply tenant-specific policy constraints
- **AND** system SHALL return constraint violations if any

#### Scenario: Check constraint without tenant context
- **WHEN** checking a knowledge item without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Status Update Operation
The system SHALL provide a method to update knowledge item status with governance approval workflows.

#### Scenario: Update status with tenant context and authorization
- **WHEN** updating knowledge item status with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL verify user has appropriate role (TechLead, Architect, Admin)
- **AND** system SHALL enforce governance approval workflow
- **AND** system SHALL create Git commit with status change
- **AND** system SHALL emit governance event (KnowledgeApproved/KnowledgeRejected)

#### Scenario: Update status without required role
- **WHEN** updating knowledge item status with insufficient role permissions
- **THEN** system SHALL return INSUFFICIENT_PERMISSIONS error
- **AND** status SHALL NOT be changed

### Requirement: Multi-Tenant Federation
The system SHALL support syncing knowledge from upstream repositories while managing local overrides and conflicts with tenant isolation.

#### Scenario: Sync from upstream with tenant context
- **WHEN** synchronizing from upstream repository with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** new and updated items from acceptable layers SHALL be merged into the local repository with tenant isolation
- **AND** conflicts SHALL be resolved according to tenant-specific conflict resolution policies

#### Scenario: Sync without tenant context
- **WHEN** synchronizing from upstream repository without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Hierarchical Scoping
The system SHALL support multiple knowledge layers (Company, Org, Team, Project) with explicit precedence rules and tenant isolation.

#### Scenario: Project-specific override with tenant context
- **WHEN** a Project-level policy conflicts with a Company-level policy within the same tenant
- **THEN** the Project-level policy SHALL take precedence during evaluation for that project
- **AND** precedence SHALL be evaluated within tenant boundaries only

#### Scenario: Cross-tenant hierarchy access attempt
- **WHEN** attempting to access hierarchy levels from another tenant
- **THEN** system SHALL return empty hierarchy
- **AND** system SHALL NOT reveal cross-tenant structure

## ADDED Requirements

### Requirement: Tenant Context Propagation
All knowledge operations SHALL require a TenantContext parameter for tenant isolation and authorization.

#### Scenario: Operation without tenant context
- **WHEN** any knowledge operation is attempted without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error
- **AND** operation SHALL NOT proceed

#### Scenario: Tenant context validation
- **WHEN** TenantContext contains invalid or expired credentials
- **THEN** system SHALL return INVALID_TENANT_CONTEXT error
- **AND** operation SHALL NOT proceed

### Requirement: Governance Policy Validation
The system SHALL validate all knowledge operations against tenant governance policies before execution.

#### Scenario: Validate knowledge creation against policies
- **WHEN** creating a knowledge item that violates a tenant policy
- **THEN** system SHALL reject the operation with POLICY_VIOLATION error
- **AND** error SHALL include which policy was violated

#### Scenario: Validate knowledge update against policies
- **WHEN** updating a knowledge item with content that violates a tenant policy
- **THEN** system SHALL reject the operation with POLICY_VIOLATION error
- **AND** error SHALL include which policy was violated

### Requirement: Governance Event Emission
Knowledge operations SHALL emit governance events for audit and real-time monitoring.

#### Scenario: Emit event on knowledge proposal
- **WHEN** a knowledge item is proposed
- **THEN** system SHALL emit a `KnowledgeProposed` event with tenant context
- **AND** event SHALL be published to Redis Streams for real-time consumption

#### Scenario: Emit event on knowledge approval
- **WHEN** a knowledge item is approved
- **THEN** system SHALL emit a `KnowledgeApproved` event with tenant context
- **AND** event SHALL include approver identity and timestamp