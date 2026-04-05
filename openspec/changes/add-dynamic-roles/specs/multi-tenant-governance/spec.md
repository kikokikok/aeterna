## MODIFIED Requirements

### Requirement: Relationship-Based Access Control

The system SHALL implement ReBAC (Relationship-Based Access Control) using Cedar for fine-grained permissions within a tenant.

Supported roles:
- **CompanyAdmin**: Full company-level management access
- **OrgAdmin**: Organization-level administration and governance management
- **TeamAdmin**: Team-level administration and role delegation within assigned teams
- **ProjectAdmin**: Project-level administration and policy management for assigned projects
- **Architect**: Can reject proposals, force corrections, review drift
- **TechLead**: Can approve promotions, manage team knowledge
- **Developer**: Can add memories, propose knowledge, view resources
- **Viewer**: Read-only access to permitted resources
- **Agent**: Inherits permissions from the user it acts on behalf of

Roles SHALL be defined and resolved through Cedar entity membership, and custom roles SHALL be configurable through Cedar/OPAL entity data without Rust recompilation.

#### Scenario: Role-based knowledge approval
- **WHEN** a Developer proposes promoting a memory to team knowledge
- **THEN** a TechLead or Architect from that team or higher hierarchy MUST approve
- **AND** the Developer SHALL NOT be able to self-approve their own proposals

#### Scenario: Architect rejection with feedback
- **WHEN** an Architect rejects a knowledge proposal
- **THEN** the system SHALL require a rejection reason
- **AND** the system SHALL notify the proposer with the rejection reason
- **AND** the proposal status SHALL change to "rejected"

#### Scenario: LLM agent as architect
- **WHEN** an LLM agent is configured with Architect role
- **THEN** the agent SHALL be able to approve or reject proposals programmatically
- **AND** all agent actions SHALL be audited with the agent's identity

### Requirement: Tenant Context Propagation

The system SHALL propagate tenant context through all operations using a TenantContext structure.

```rust
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<RoleIdentifier>,
    pub hierarchy_path: HierarchyPath,
}
```

#### Scenario: Context injection
- **WHEN** a request arrives at any API endpoint
- **THEN** the system SHALL extract and validate TenantContext from the request
- **AND** the context SHALL be available to all downstream operations

#### Scenario: Context validation failure
- **WHEN** a request lacks valid TenantContext
- **THEN** the system SHALL reject the request with 401 Unauthorized
- **AND** the system SHALL NOT process any data operations

### Requirement: RBAC Policy Testing (MT-C2)
The system SHALL have comprehensive automated testing for role-based access control.

Permission matrix artifacts SHALL be derived from Cedar policies rather than manually maintained in static Rust data, and contract tests SHALL verify the end-to-end authorization pipeline (`DB row -> OPAL entity -> Cedar decision`).

#### Scenario: RBAC Integration Tests
- **WHEN** CI runs
- **THEN** integration tests MUST verify all role-action-resource combinations
- **AND** tests MUST cover positive and negative authorization cases

#### Scenario: Permission Matrix Validation
- **WHEN** RBAC policies are modified
- **THEN** a permission matrix MUST be generated from Cedar policies
- **AND** matrix MUST be reviewed before deployment

#### Scenario: Role Escalation Prevention
- **WHEN** testing role permissions
- **THEN** tests MUST verify privilege escalation is not possible
- **AND** tests MUST verify role hierarchy is enforced correctly
