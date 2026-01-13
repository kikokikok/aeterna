# Deployment Spec Delta: UX-First Architecture

## ADDED Requirements

### Requirement: Zero-Friction Organization Bootstrap

The system SHALL provide a zero-friction bootstrap process that initializes a complete organizational hierarchy from a single command.

#### Scenario: Bootstrap from single command
- **WHEN** admin runs `aeterna init` with company name and admin email
- **THEN** system creates company entity with all required infrastructure
- **AND** creates default organization and team structure
- **AND** applies selected governance level defaults
- **AND** completes in under 60 seconds

#### Scenario: Resume interrupted bootstrap
- **WHEN** bootstrap is interrupted partway through
- **THEN** system detects partial state on next run
- **AND** offers to resume or restart
- **AND** cleans up partial state if restart selected

### Requirement: Project Auto-Discovery

The system SHALL support automatic project discovery and registration from git repositories.

#### Scenario: Discover project from git remote
- **WHEN** user runs project init in git repository
- **THEN** system extracts project identity from remote URL
- **AND** matches to existing team based on remote patterns
- **AND** creates project entity if not already registered

#### Scenario: Discover project from monorepo
- **WHEN** user runs project init in monorepo subdirectory
- **THEN** system detects monorepo structure
- **AND** creates project scoped to subdirectory
- **AND** supports multiple projects in same repository

### Requirement: CLI Installation Experience

The system SHALL provide a streamlined CLI installation experience with automatic shell integration.

#### Scenario: Install CLI with package manager
- **WHEN** user installs via cargo install aeterna-cli
- **THEN** CLI is available in path
- **AND** shell completions are generated for bash/zsh/fish
- **AND** first run triggers optional setup wizard

#### Scenario: First run setup wizard
- **WHEN** user runs aeterna for first time
- **THEN** CLI offers interactive setup
- **AND** detects existing context if available
- **AND** guides user through minimal required configuration

### Requirement: Agent Token Provisioning

The system SHALL provide secure agent token provisioning for CI/CD and automation use cases.

#### Scenario: Provision CI agent token
- **WHEN** admin creates agent with CI scope
- **THEN** system generates secure token
- **AND** token has configurable expiration
- **AND** token can be revoked immediately

#### Scenario: Rotate agent token
- **WHEN** admin requests token rotation
- **THEN** system generates new token
- **AND** old token remains valid for grace period (configurable)
- **AND** logs rotation in audit trail

### Requirement: Multi-Region Deployment Support

The system SHALL support multi-region deployment for enterprise customers with data residency requirements.

#### Scenario: Configure regional data residency
- **WHEN** admin configures organization for EU data residency
- **THEN** system ensures all data for org stays in EU region
- **AND** prevents cross-region data transfer for org resources
- **AND** supports read replicas for performance

### Requirement: Self-Service Team Onboarding

The system SHALL support self-service team onboarding where team leads can create and configure their teams.

#### Scenario: Team lead creates team
- **WHEN** user with tech_lead role creates new team
- **THEN** system creates team within user's org
- **AND** automatically assigns creator as team lead
- **AND** inherits org governance without admin intervention

#### Scenario: Team lead invites members
- **WHEN** team lead invites user to team
- **THEN** system sends invitation
- **AND** creates membership upon acceptance
- **AND** applies default role based on org settings

### Requirement: Project Template Support

The system SHALL support project templates for standardized project initialization.

#### Scenario: Initialize from template
- **WHEN** user runs project init with --template flag
- **THEN** system applies template configuration
- **AND** creates standard .aeterna/ structure
- **AND** applies template-defined policies and knowledge

#### Scenario: Create project template
- **WHEN** architect creates project template for team
- **THEN** system stores template with policies and config
- **AND** template is available for team projects
- **AND** template versioning is supported

### Requirement: Health Check Endpoint

The system SHALL provide health check endpoints and CLI commands for operational monitoring.

#### Scenario: CLI health check
- **WHEN** user runs aeterna admin health
- **THEN** CLI checks connectivity to all required services
- **AND** reports status of each service
- **AND** shows overall health status

#### Scenario: Detailed health check
- **WHEN** user runs aeterna admin health --verbose
- **THEN** CLI shows response times for each service
- **AND** shows recent error counts
- **AND** shows resource utilization metrics

### Requirement: OPAL Infrastructure Integration

The system SHALL integrate with OPAL (Open Policy Administration Layer) for organizational referential and real-time policy sync.

#### Scenario: Deploy OPAL Server
- **WHEN** administrator deploys Aeterna stack
- **THEN** OPAL Server is deployed as infrastructure component
- **AND** connects to PostgreSQL for data broadcast
- **AND** connects to Git repository for Cedar policies
- **AND** provides WebSocket endpoint for real-time sync

#### Scenario: Deploy Cedar Agent via OPAL
- **WHEN** OPAL Server is running
- **THEN** Cedar Agent connects via OPAL Client
- **AND** receives organizational data and policies
- **AND** provides authorization API on port 8180
- **AND** updates in real-time when data changes

#### Scenario: OPAL data fetcher deployment
- **WHEN** administrator deploys Aeterna stack
- **THEN** custom OPAL data fetcher is deployed
- **AND** connects to PostgreSQL referential
- **AND** formats hierarchy/users/agents for Cedar
- **AND** publishes updates via OPAL PubSub

### Requirement: Organizational Referential Database

The system SHALL maintain a PostgreSQL database as the authoritative source for organizational topology.

#### Scenario: Create referential schema
- **WHEN** administrator initializes Aeterna
- **THEN** system creates companies, organizations, teams, projects tables
- **AND** creates users, agents, memberships tables
- **AND** creates git_remote_patterns and email_domain_patterns tables
- **AND** creates views for OPAL data fetcher consumption

#### Scenario: Referential triggers real-time sync
- **WHEN** organizational data changes in PostgreSQL
- **THEN** database triggers fire pg_notify events
- **AND** OPAL data fetcher receives notifications
- **AND** publishes updates to OPAL Server
- **AND** Cedar Agents receive updated data within 1 second

### Requirement: Cedar Agent Availability

The system SHALL ensure Cedar Agent is highly available for authorization decisions.

#### Scenario: Cedar Agent failover
- **WHEN** primary Cedar Agent becomes unavailable
- **THEN** Aeterna fails over to secondary Cedar Agent
- **AND** uses cached authorization data if all agents unavailable
- **AND** logs degraded mode operation

#### Scenario: Cedar Agent horizontal scaling
- **WHEN** authorization load increases
- **THEN** additional Cedar Agents can be deployed
- **AND** all receive identical data from OPAL
- **AND** any agent can serve authorization requests

### Requirement: IdP Synchronization

The system SHALL support synchronization of users and groups from identity providers.

#### Scenario: Sync users from Okta
- **WHEN** IdP sync runs for Okta
- **THEN** system fetches all users from Okta API
- **AND** creates or updates users in PostgreSQL
- **AND** maps Okta groups to team memberships
- **AND** triggers OPAL update for Cedar Agents

#### Scenario: Sync users from Azure AD
- **WHEN** IdP sync runs for Azure AD
- **THEN** system fetches users via Microsoft Graph
- **AND** creates or updates users in PostgreSQL
- **AND** maps Azure AD groups to team memberships
- **AND** handles nested groups correctly

#### Scenario: Real-time IdP webhook
- **WHEN** IdP sends user change webhook
- **THEN** system processes webhook immediately
- **AND** updates user/membership in PostgreSQL
- **AND** Cedar Agent has updated data within 2 seconds

### Requirement: Migration from Non-OPAL Deployment

The system SHALL support migration from heuristic-based context to OPAL-backed context.

#### Scenario: Phase 1 parallel deployment
- **WHEN** existing Aeterna deployment adds OPAL
- **THEN** OPAL can run alongside heuristic context
- **AND** both produce same context results (verified)
- **AND** no disruption to existing operations

#### Scenario: Phase 2 switchover
- **WHEN** administrator switches to OPAL context
- **THEN** Aeterna uses Cedar Agent for all context resolution
- **AND** falls back to cache on Cedar Agent failure
- **AND** audit log shows context source change

#### Scenario: Phase 3 enforcement
- **WHEN** administrator enables OPAL enforcement
- **THEN** all authorization decisions go through Cedar
- **AND** denied requests are logged with policy reason
- **AND** circuit breaker prevents cascade failures
