## ADDED Requirements

### Requirement: Resource-Scoped Role Assignment
The system SHALL support assigning roles to users on specific resources within the organizational hierarchy, enabling fine-grained access control at the instance, tenant, organization, team, project, and session levels.

#### Scenario: Grant role on a specific team
- **WHEN** an authorized admin grants a user the TechLead role on a specific team
- **THEN** the user SHALL have TechLead permissions for that team and all projects within it
- **AND** the user SHALL NOT have TechLead permissions for other teams in the same organization

#### Scenario: Grant role on the entire tenant
- **WHEN** an authorized admin grants a user the Admin role at the tenant scope
- **THEN** the user SHALL have Admin permissions for all resources within that tenant
- **AND** the grant SHALL be stored as a Cedar entity relationship

#### Scenario: Grant role at the instance (global) scope
- **WHEN** a PlatformAdmin grants a user the PlatformAdmin role at the instance scope
- **THEN** the user SHALL have PlatformAdmin permissions across all tenants
- **AND** the grant SHALL only be creatable by an existing PlatformAdmin

### Requirement: Role Grant Inheritance Through Hierarchy
The system SHALL enforce automatic permission inheritance from higher-level resource scopes to lower-level scopes within the Cedar entity hierarchy.

#### Scenario: Organization-scoped role inherits to teams
- **WHEN** a user is granted Admin role on an Organization
- **THEN** the user SHALL have Admin permissions on all Teams within that Organization
- **AND** the user SHALL have Admin permissions on all Projects within those Teams
- **AND** this inheritance SHALL be evaluated natively by Cedar via the `in` operator

#### Scenario: Team-scoped role does not propagate upward
- **WHEN** a user is granted TechLead role on a specific Team
- **THEN** the user SHALL NOT have TechLead permissions on the parent Organization
- **AND** the user SHALL NOT have TechLead permissions on sibling Teams

### Requirement: Role Grant Management API
The system SHALL expose REST API endpoints for granting, revoking, and listing resource-scoped role assignments.

#### Scenario: Grant a scoped role
- **WHEN** `POST /api/v1/admin/roles/grant` is called with `{ user_id, role, resource_type, resource_id }`
- **AND** the caller has `AssignRoles` permission on the target resource
- **THEN** the system SHALL create the role grant as a Cedar entity relationship
- **AND** the system SHALL return the created grant details

#### Scenario: Revoke a scoped role
- **WHEN** `DELETE /api/v1/admin/roles/revoke` is called with `{ user_id, role, resource_type, resource_id }`
- **AND** the caller has `AssignRoles` permission on the target resource
- **THEN** the system SHALL remove the Cedar entity relationship for that role grant
- **AND** the user SHALL immediately lose the permissions associated with that grant

#### Scenario: List grants for a user
- **WHEN** `GET /api/v1/admin/roles/grants?user_id=alice` is called
- **THEN** the system SHALL return all active role grants for that user
- **AND** each grant SHALL include the role name, resource type, resource ID, and grant timestamp

#### Scenario: List grants on a resource
- **WHEN** `GET /api/v1/admin/roles/grants?resource_type=team&resource_id=api-team` is called
- **THEN** the system SHALL return all users with role grants on that specific resource

#### Scenario: Unauthorized grant attempt is rejected
- **WHEN** a user without `AssignRoles` permission on the target resource attempts to grant a role
- **THEN** the system SHALL return HTTP 403 Forbidden
- **AND** the system SHALL NOT create the role grant

### Requirement: Role Grant Scope Validation
The system SHALL validate that role grants are appropriate for the specified resource scope level.

#### Scenario: PlatformAdmin only grantable at instance scope
- **WHEN** an attempt is made to grant PlatformAdmin role at a team or project scope
- **THEN** the system SHALL reject the grant with a validation error
- **AND** the error SHALL indicate that PlatformAdmin is only valid at the instance scope

#### Scenario: TenantAdmin only grantable at tenant scope or higher
- **WHEN** an attempt is made to grant TenantAdmin role at a team or project scope
- **THEN** the system SHALL reject the grant with a validation error
- **AND** the error SHALL indicate that TenantAdmin is only valid at tenant scope or instance scope

#### Scenario: Developer role grantable at any scope
- **WHEN** a Developer role is granted at any scope (tenant, organization, team, project)
- **THEN** the system SHALL accept the grant
- **AND** the grant SHALL apply at that scope and all descendant resources
