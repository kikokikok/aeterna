## MODIFIED Requirements

### Requirement: Tenant Context Propagation
The system SHALL propagate tenant context through all operations using a TenantContext structure.

```rust
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub hierarchy_path: HierarchyPath,
    pub agent_id: Option<String>,
}
```

#### Scenario: Context injection
- **WHEN** a request arrives at any tenant-scoped API or tool boundary
- **THEN** the system SHALL extract and validate TenantContext from authenticated identity or trusted boundary data
- **AND** the context SHALL include the caller's effective roles and hierarchy path for downstream authorization decisions

#### Scenario: Context validation failure
- **WHEN** a request lacks valid TenantContext
- **THEN** the system SHALL reject the request with 401 Unauthorized
- **AND** the system SHALL NOT process any data operations

### Requirement: Relationship-Based Access Control
The system SHALL implement relationship- and policy-based access control using the supported authorization stack for fine-grained permissions within a tenant.

#### Scenario: Role lookup reflects persisted assignments
- **WHEN** an authorized caller requests a principal's roles at a scope
- **THEN** the system SHALL return the persisted effective role assignments for that principal and scope
- **AND** the response SHALL use the canonical role catalog for the deployment

#### Scenario: Self-approval is denied
- **WHEN** a requestor attempts to approve their own governance request
- **THEN** the system SHALL reject the approval attempt
- **AND** the system SHALL record an auditable denial event for the violation
