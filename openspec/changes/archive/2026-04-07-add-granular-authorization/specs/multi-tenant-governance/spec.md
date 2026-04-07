## MODIFIED Requirements

### Requirement: Relationship-Based Access Control

The system SHALL implement Cedar-based Role-Based Access Control with 8 roles (Viewer, Agent, Developer, TechLead, Architect, Admin, TenantAdmin, PlatformAdmin) for fine-grained permissions within and across tenants.

Supported roles:
- **Viewer**: Read-only access to memories, knowledge, policies, governance requests, and organization structure
- **Agent**: Inherits permissions from the user it acts on behalf of
- **Developer**: Can add memories, propose knowledge, view resources, register and delegate to agents
- **Tech Lead**: Can approve promotions, manage team knowledge, manage members
- **Architect**: Can reject proposals, force corrections, review drift, create policies
- **Admin**: Full tenant management access (legacy alias for TenantAdmin)
- **TenantAdmin**: Explicit full tenant-scoped administration, tenant config/secrets, role delegation within tenant
- **PlatformAdmin**: Cross-tenant super admin, tenant lifecycle, shared Git provider connections

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

#### Scenario: Viewer role has read-only access
- **WHEN** a user has the Viewer role
- **THEN** the user SHALL be able to view memories, knowledge, policies, governance requests, and organization structure
- **AND** the user SHALL NOT be able to create, modify, delete, or approve any resource

#### Scenario: TenantAdmin manages tenant-scoped resources
- **WHEN** a user has the TenantAdmin role on a specific tenant
- **THEN** the user SHALL have full administrative access within that tenant
- **AND** the user SHALL be able to manage tenant config, secrets, hierarchy, and role assignments within the tenant
- **AND** the user SHALL NOT have access to other tenants or platform-wide operations

### Requirement: Tenant Context Propagation

The system SHALL propagate tenant context through all operations using a TenantContext structure.

```rust
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub hierarchy_path: HierarchyPath,
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

#### Scenario: MCP tool invocation validates tenant context
- **WHEN** an MCP tool is invoked with tenant context in the JSON payload
- **THEN** the runtime SHALL validate that the supplied tenant context matches the authenticated identity
- **AND** the request SHALL be rejected if the supplied tenant context cannot be verified

### Requirement: Tenant Context Safety (MT-H3)
The system SHALL enforce mandatory tenant context propagation.

#### Scenario: Middleware Enforcement
- **WHEN** requests are processed
- **THEN** middleware MUST require valid TenantContext
- **AND** requests without context MUST be rejected immediately

#### Scenario: Fail-Closed Policy
- **WHEN** TenantContext extraction fails
- **THEN** system MUST fail closed (reject request)
- **AND** MUST NOT fall back to default tenant
- **AND** MUST NOT assign a synthetic system user as a replacement caller identity

#### Scenario: Context Audit Trail
- **WHEN** operations are performed
- **THEN** TenantContext MUST be logged with each operation
- **AND** audit logs MUST enable forensic reconstruction
