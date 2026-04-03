## ADDED Requirements

### Requirement: Administrative Authority Boundaries
The system SHALL distinguish cross-tenant platform administration from tenant-scoped administration without weakening tenant isolation.

#### Scenario: Platform admin manages tenant lifecycle without implicit tenant content access
- **WHEN** a platform administrator creates, updates, or deactivates a tenant
- **THEN** the system SHALL allow the lifecycle mutation across tenants
- **AND** the platform administrator SHALL NOT implicitly gain read or write access to that tenant's memory or knowledge content outside an explicit tenant-scoped request path

#### Scenario: Tenant admin is limited to one tenant
- **WHEN** a tenant administrator attempts a cross-tenant lifecycle or scoped-administration action
- **THEN** the system SHALL reject the request with an authorization error
- **AND** the audit trail SHALL record the denied cross-tenant attempt

### Requirement: Verified Tenant Resolution for Administrative Onboarding
The system SHALL resolve tenant association for onboarding and administrative bootstrap from explicit tenant selection, sync-derived mappings, or admin-approved verified mappings.

#### Scenario: Verified domain mapping resolves a tenant
- **WHEN** an onboarding or bootstrap flow matches exactly one admin-approved email-domain mapping for a tenant
- **THEN** the system SHALL allow that mapping to select the tenant automatically
- **AND** the audit trail SHALL record the mapping source used for the tenant resolution

#### Scenario: Ambiguous or missing mapping fails closed
- **WHEN** no verified tenant mapping exists or multiple mappings match the same onboarding request
- **THEN** the system SHALL require explicit tenant selection or administrator intervention
- **AND** the system SHALL NOT infer a tenant from an unverified email suffix alone

### Requirement: Canonical Administrative Role Catalog
The system SHALL maintain one canonical administrative role catalog across runtime types, CLI validation, API schemas, and authorization policy bundles.

#### Scenario: Role catalog inspection is consistent
- **WHEN** an operator inspects the supported role catalog through the API, CLI, or policy-inspection surface
- **THEN** each surface SHALL report the same role identifiers and scope rules
- **AND** the catalog SHALL include any special cross-tenant or read-only administrative roles supported by the deployment

#### Scenario: Policy bundle with unknown role is rejected
- **WHEN** the authorization policy bundle references a role that is not part of the canonical role catalog
- **THEN** policy validation SHALL fail before the bundle becomes active
- **AND** the validation error SHALL identify the unknown role reference

## MODIFIED Requirements

### Requirement: Relationship-Based Access Control
The system SHALL implement relationship- and policy-based access control using the supported authorization stack for fine-grained permissions within a tenant.

Supported roles:
- **PlatformAdmin**: Can manage tenant lifecycle, tenant bindings, and cross-tenant control-plane workflows without implicit tenant content access
- **Admin**: Can manage hierarchy, members, tenant configuration, and governance within a single tenant
- **Architect**: Can reject proposals, force corrections, and review drift within authorized scopes
- **Tech Lead**: Can approve promotions and manage team knowledge within authorized scopes
- **Developer**: Can add memories, propose knowledge, and view resources within authorized scopes
- **Viewer**: Can view authorized resources without mutation rights
- **Agent**: Inherits permissions from the user or principal it acts on behalf of

#### Scenario: Role-based knowledge approval
- **WHEN** a Developer proposes promoting a memory to team knowledge
- **THEN** a Tech Lead, Architect, or Admin from that team or higher hierarchy MUST approve
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

#### Scenario: Platform admin requires explicit tenant context for tenant data access
- **WHEN** a platform administrator attempts a tenant-content operation without an explicit authorized tenant context
- **THEN** the system SHALL reject the request
- **AND** the denial SHALL be auditable as a boundary-enforcement event

#### Scenario: Viewer remains non-mutating across scopes
- **WHEN** a principal holds only the Viewer role at a scope
- **THEN** the system SHALL allow read access permitted by policy at that scope
- **AND** the system SHALL deny role mutation, knowledge mutation, and governance approval actions for that principal
