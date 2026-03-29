## ADDED Requirements

### Requirement: Sync-to-Governance Bridge

The system SHALL automatically bridge IdP sync results to the governance authorization system after each successful sync operation.

When a GitHub Org Sync (or any IdP sync) completes, the system MUST:
- Map synced users with team memberships to `governance_roles` entries
- Map synced users to `user_roles` entries with appropriate role levels
- Use `ON CONFLICT DO UPDATE` for full idempotency
- Assign org-level members a baseline `viewer` role
- Assign team members the role mapped from their GitHub team role

#### Scenario: Post-sync governance bridge populates roles
- **WHEN** a GitHub org sync completes with 10 users across 3 teams
- **THEN** the system SHALL create/update `governance_roles` entries for each user-team membership
- **AND** the system SHALL create/update `user_roles` entries for each user
- **AND** existing role assignments not from the sync SHALL NOT be affected

#### Scenario: Idempotent re-sync does not duplicate roles
- **WHEN** a sync runs twice with identical GitHub org data
- **THEN** the second run SHALL produce the same `governance_roles` and `user_roles` state
- **AND** no duplicate entries SHALL be created

### Requirement: OPAL Authorization View Layer

The system SHALL maintain PostgreSQL views that the OPAL fetcher queries to generate Cedar entities for authorization evaluation.

Required views:
- `v_hierarchy`: Flattened Company → Organization → Team → Project hierarchy with slugs and UUIDs
- `v_user_permissions`: User-team memberships with roles, resolved to company/org/team UUIDs and slugs
- `v_agent_permissions`: Agent delegation chains with capabilities and allowed scope

#### Scenario: OPAL fetcher reads v_hierarchy
- **WHEN** the OPAL fetcher queries `SELECT * FROM v_hierarchy`
- **THEN** it SHALL receive rows with `company_id`, `company_slug`, `company_name`, `org_id`, `org_slug`, `org_name`, `team_id`, `team_slug`, `team_name`, `project_id`, `project_slug`, `project_name`, `git_remote`
- **AND** all ID columns SHALL be UUID type

#### Scenario: OPAL fetcher reads v_user_permissions
- **WHEN** the OPAL fetcher queries `SELECT * FROM v_user_permissions`
- **THEN** it SHALL receive rows with `user_id`, `email`, `user_name`, `user_status`, `team_id`, `role`, `permissions`, `org_id`, `company_id`, `company_slug`, `org_slug`, `team_slug`
- **AND** permissions SHALL be a JSONB array of permission strings

#### Scenario: OPAL fetcher reads v_agent_permissions
- **WHEN** the OPAL fetcher queries `SELECT * FROM v_agent_permissions`
- **THEN** it SHALL receive rows matching the `AgentPermissionRow` struct in `opal-fetcher/src/entities.rs`

### Requirement: Real-Time OPAL Notification

The system SHALL use PostgreSQL NOTIFY triggers to inform the OPAL fetcher of entity changes in real time.

#### Scenario: User created triggers notification
- **WHEN** a new user is inserted into the `users` table
- **THEN** the system SHALL emit `NOTIFY aeterna_entity_change` with payload `{"type":"user","id":"<user_id>"}`
- **AND** the OPAL fetcher's listener SHALL receive the notification within 1 second

#### Scenario: Membership changed triggers notification
- **WHEN** a membership is inserted or updated in the `memberships` table
- **THEN** the system SHALL emit `NOTIFY aeterna_entity_change` with payload `{"type":"membership","id":"<membership_id>"}`

### Requirement: Cedar Entity Loading from OPAL

The `CedarAuthorizer` SHALL load actual Cedar entities from the OPAL fetcher instead of using `Entities::empty()`.

#### Scenario: Authorization with real entities
- **WHEN** a user requests access to a resource
- **THEN** the `CedarAuthorizer` SHALL fetch entities from the OPAL fetcher HTTP endpoints
- **AND** the authorization decision SHALL be based on the user's actual roles and memberships
- **AND** entities SHALL be cached with a configurable TTL (default 30 seconds)

#### Scenario: OPAL fetcher unavailable falls back gracefully
- **WHEN** the OPAL fetcher is unreachable during entity fetch
- **THEN** the `CedarAuthorizer` SHALL use the last cached entities if available
- **AND** SHALL deny access if no cached entities exist
- **AND** SHALL log a warning

## MODIFIED Requirements

### Requirement: Hierarchical Organization Structure

The system SHALL support a four-level organizational hierarchy within each tenant:
1. Company (tenant root)
2. Organization (business unit)
3. Team (working group)
4. Project (codebase/repository)

Each level MUST have:
- A UUID identifier (generated deterministically from TEXT IDs where needed)
- A human-readable `slug` for display and URL purposes
- A `name` for display
- A reference to its parent level

#### Scenario: Hierarchy navigation
- **WHEN** a user queries for knowledge at the Team level
- **THEN** the system SHALL include inherited knowledge from Organization and Company levels
- **AND** the system SHALL mark each result with its originating hierarchy level

#### Scenario: Hierarchy creation
- **WHEN** an admin creates a new Team under an Organization
- **THEN** the Team SHALL inherit default policies from the parent Organization
- **AND** the Team SHALL be visible to all Organization members with appropriate permissions

#### Scenario: Hierarchy populated from IdP sync
- **WHEN** a GitHub org sync creates organizational units
- **THEN** each unit SHALL have a `slug` column populated from its GitHub slug
- **AND** each unit SHALL have an `external_id` column linking to the GitHub entity
- **AND** the `v_hierarchy` view SHALL expose units with proper UUID IDs and slugs
