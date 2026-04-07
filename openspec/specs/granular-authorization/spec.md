# granular-authorization Specification

## Purpose
TBD - created by archiving change add-granular-authorization. Update Purpose after archive.
## Requirements
### Requirement: Complete Route-to-Action Permission Mapping
The system SHALL map every REST API endpoint and MCP tool invocation to a specific Cedar authorization action, ensuring no functionality is accessible without an explicit permission check when authentication is enabled.

#### Scenario: Protected REST endpoint requires Cedar action check
- **WHEN** a request arrives at any non-health, non-auth-bootstrap REST endpoint
- **AND** authentication is enabled
- **THEN** the system SHALL evaluate the corresponding Cedar action against the authenticated principal and target resource
- **AND** the system SHALL reject the request with 403 Forbidden if the Cedar evaluation denies the action

#### Scenario: MCP tool invocation maps to domain Cedar action
- **WHEN** an MCP tool is invoked via the tool dispatcher
- **AND** authentication is enabled
- **THEN** the system SHALL map the tool name to its corresponding domain Cedar action (e.g., `memory_add` → `CreateMemory`, `knowledge_query` → `SearchKnowledge`)
- **AND** the system SHALL evaluate the Cedar action against the authenticated principal before executing the tool

#### Scenario: Unrecognized tool falls back to generic permission
- **WHEN** an MCP tool name does not have an explicit Cedar action mapping
- **THEN** the system SHALL require the `InvokeMcpTool` action as a fallback
- **AND** the system SHALL log a warning indicating the unmapped tool name

### Requirement: Expanded Cedar Action Set
The system SHALL define Cedar actions covering all authorization domains: memory, knowledge, policy, governance, organization management, agent management, tenant management, tenant configuration, repository binding, git provider connections, session management, sync operations, graph operations, CCA operations, user management, and admin operations.

#### Scenario: Tenant management actions exist in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include actions: `ListTenants`, `CreateTenant`, `ViewTenant`, `UpdateTenant`, `DeactivateTenant`
- **AND** each action SHALL apply to the appropriate principal and resource types

#### Scenario: Session management actions exist in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include actions: `CreateSession`, `ViewSession`, `EndSession`

#### Scenario: Sync actions exist in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include actions: `TriggerSync`, `ViewSyncStatus`, `ResolveConflict`

#### Scenario: Extended memory actions exist in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include actions: `SearchMemory`, `ListMemory`, `OptimizeMemory`, `ReasonMemory`, `CloseMemory`, `FeedbackMemory` in addition to the existing `ViewMemory`, `CreateMemory`, `UpdateMemory`, `DeleteMemory`, `PromoteMemory`

#### Scenario: Graph and CCA actions exist in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include actions: `QueryGraph`, `ModifyGraph`, `InvokeCCA`

#### Scenario: Admin sync action exists in Cedar schema
- **WHEN** the Cedar schema is loaded
- **THEN** the schema SHALL include the action `AdminSyncGitHub` applicable to PlatformAdmin principals

### Requirement: Role Permission Matrix
The system SHALL maintain an authoritative role-to-permission matrix that maps each of the 8 roles (Viewer, Agent, Developer, TechLead, Architect, Admin, TenantAdmin, PlatformAdmin) to their permitted Cedar actions, kept in sync with the Cedar RBAC policy files.

#### Scenario: Permission matrix includes all roles
- **WHEN** the permission matrix API endpoint is queried
- **THEN** the response SHALL include entries for all 8 roles
- **AND** each role entry SHALL list every Cedar action that role is permitted to perform

#### Scenario: Permission matrix matches Cedar policies
- **WHEN** RBAC integration tests run
- **THEN** the Rust `role_permission_matrix()` output SHALL exactly match the permits defined in `policies/cedar/rbac.cedar`
- **AND** any mismatch SHALL cause the test to fail

#### Scenario: Viewer role has read-only permissions
- **WHEN** the permission matrix is evaluated for the Viewer role
- **THEN** the permitted actions SHALL include only View* actions, `SimulatePolicy`, and `ViewSyncStatus`
- **AND** the permitted actions SHALL NOT include any Create*, Update*, Delete*, Approve*, or management actions

#### Scenario: TenantAdmin has full tenant-scoped permissions
- **WHEN** the permission matrix is evaluated for the TenantAdmin role
- **THEN** the permitted actions SHALL include all tenant-scoped actions
- **AND** the permitted actions SHALL NOT include cross-tenant actions like `ListTenants` or `ManageGitProviderConnections`

### Requirement: Global Authentication Middleware
The system SHALL enforce authentication via a global Axum tower layer on all protected route groups, rather than per-handler authentication checks.

#### Scenario: Authentication layer validates bearer token
- **WHEN** a request arrives at a protected route group
- **AND** authentication is enabled
- **THEN** the authentication layer SHALL validate the `Authorization: Bearer <token>` header
- **AND** the layer SHALL extract and inject `TenantContext` as a request extension for downstream handlers

#### Scenario: Authentication layer passes through when disabled
- **WHEN** a request arrives at any route
- **AND** authentication is disabled (`pluginAuth.enabled: false`)
- **THEN** the authentication layer SHALL pass the request through without token validation
- **AND** the system SHALL use the legacy header-based identity extraction

#### Scenario: Health and auth bootstrap routes are excluded
- **WHEN** a request arrives at `/health`, `/live`, `/ready`, or `/api/v1/auth/plugin/*`
- **THEN** the authentication layer SHALL NOT require a bearer token
- **AND** the request SHALL proceed regardless of authentication configuration

#### Scenario: Missing or invalid token returns 401
- **WHEN** a request arrives at a protected route
- **AND** the `Authorization` header is missing, malformed, or contains an invalid/expired token
- **THEN** the authentication layer SHALL return HTTP 401 Unauthorized
- **AND** the response SHALL NOT reveal whether the route exists

### Requirement: RBAC Matrix Documentation
The system SHALL maintain an auto-generated `docs/security/rbac-matrix.md` document that accurately reflects the current role-to-permission matrix, including all 8 roles, all Cedar actions, and the MCP tool-to-action mapping.

#### Scenario: RBAC matrix includes all roles and actions
- **WHEN** the RBAC matrix document is generated
- **THEN** it SHALL list all 8 roles with their precedence levels
- **AND** it SHALL include permission tables for every Cedar action domain (memory, knowledge, governance, tenant, session, sync, graph, CCA, admin)

#### Scenario: RBAC matrix includes MCP tool mapping
- **WHEN** the RBAC matrix document is generated
- **THEN** it SHALL include a section mapping each MCP tool name to its corresponding Cedar action

