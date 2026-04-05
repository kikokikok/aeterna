## MODIFIED Requirements

### Requirement: Relationship-Based Access Control

The system SHALL implement relationship-based and role-based authorization using Cedar policies and hierarchy-aware role resolution for fine-grained permissions within a tenant.

Supported roles:
- **Developer**: Can add memories, propose knowledge, view resources
- **Tech Lead**: Can approve promotions, manage team knowledge
- **Architect**: Can reject proposals, force corrections, review drift
- **Admin**: Full tenant management access
- **Agent**: Inherits permissions from the user it acts on behalf of

Effective role computation SHALL include role inheritance through the Company → Organization → Team → Project hierarchy by evaluating ancestor assignments at the requested scope.

#### Scenario: Role-based knowledge approval
- **WHEN** a Developer proposes promoting a memory to team knowledge
- **THEN** a Tech Lead or Architect from that team or higher hierarchy MUST approve
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

The system SHALL propagate tenant context through all operations using a TenantContext structure, and the context resolution MUST support project-level scope (not only company-level defaults).

```rust
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub hierarchy_path: HierarchyPath,
}
```

The `hierarchy_path` field MUST resolve to the actual target scope, including project scope when requests operate on project resources.

#### Scenario: Context injection
- **WHEN** a request arrives at any API endpoint
- **THEN** the system SHALL extract and validate TenantContext from the request
- **AND** the context SHALL be available to all downstream operations

#### Scenario: Context validation failure
- **WHEN** a request lacks valid TenantContext
- **THEN** the system SHALL reject the request with 401 Unauthorized
- **AND** the system SHALL NOT process any data operations
