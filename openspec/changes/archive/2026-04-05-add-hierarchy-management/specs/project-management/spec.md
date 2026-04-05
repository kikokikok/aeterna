## ADDED Requirements

### Requirement: Project CRUD
The system SHALL provide REST API endpoints for project lifecycle management at `/api/v1/projects` supporting POST, GET, PUT, and DELETE operations, and the implementation SHALL follow the established `org_api.rs` API pattern for router structure, middleware application, and handler composition.

#### Scenario: Create and read project lifecycle resource
- **WHEN** an authorized client sends `POST /api/v1/projects` with valid project payload
- **THEN** the system SHALL create the project under the requested hierarchy scope and return the created resource
- **AND** a subsequent `GET /api/v1/projects/{project_id}` SHALL return the same persisted project details

### Requirement: Project Member Management
The system SHALL support adding and removing members for projects with explicit role assignments.

#### Scenario: Add and remove project member role assignment
- **WHEN** an authorized client assigns a member role to a project and later removes that assignment
- **THEN** the system SHALL persist the role membership change
- **AND** subsequent project membership queries SHALL reflect the updated assignment state

### Requirement: Team-Project Assignments
The system SHALL support many-to-many team-project relationships with assignment types `owner` and `contributor`.

#### Scenario: Assign owner and contributor teams to one project
- **WHEN** an authorized client links multiple teams to the same project using assignment types `owner` and `contributor`
- **THEN** the system SHALL persist each team-project assignment edge with its assignment type
- **AND** project/team relationship queries SHALL return the complete set of assignment edges

### Requirement: Project-Scoped Authorization
The system SHALL enforce Cedar authorization actions for all project operations, including the existing `CreateProject` action.

#### Scenario: Enforce Cedar checks on project create
- **WHEN** a client invokes project create without permission for the `CreateProject` action
- **THEN** the system SHALL deny the operation
- **AND** no project record SHALL be created

### Requirement: Project ID Consistency
The system SHALL use a single canonical project identifier across all subsystems.

#### Scenario: Canonical project ID used across API and governance stores
- **WHEN** a project is created and later referenced by governance or drift-related subsystems
- **THEN** all persisted references SHALL use the canonical organizational-unit UUID project identifier
- **AND** cross-subsystem joins SHALL resolve without slug-to-UUID translation ambiguity
