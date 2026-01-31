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

### Requirement: OPAL Server High Availability
The system SHALL deploy OPAL Server in high availability mode to prevent authorization decision outages.

#### Scenario: OPAL Server HA deployment
- **WHEN** administrator deploys Aeterna in production mode
- **THEN** OPAL Server SHALL be deployed with minimum 3 replicas
- **AND** replicas SHALL be distributed across availability zones
- **AND** load balancer SHALL distribute traffic across healthy replicas

#### Scenario: OPAL Server failover
- **WHEN** an OPAL Server replica becomes unavailable
- **THEN** traffic SHALL be automatically routed to healthy replicas
- **AND** no authorization decisions SHALL be blocked during failover
- **AND** system SHALL alert on replica failure

#### Scenario: Local policy cache with TTL
- **WHEN** all OPAL Server replicas are unavailable
- **THEN** Cedar Agents SHALL use local policy cache
- **AND** cache SHALL have configurable TTL (default: 5 minutes)
- **AND** system SHALL operate in degraded mode with cached policies
- **AND** alert SHALL be raised for manual intervention

### Requirement: Cedar Policy Conflict Detection
The system SHALL detect and prevent conflicting Cedar policies before deployment.

#### Scenario: Conflict detection on policy proposal
- **WHEN** a new policy is proposed via `aeterna_policy_validate`
- **THEN** the system SHALL analyze for conflicts with existing policies
- **AND** detect explicit allow + deny conflicts for same action/resource
- **AND** detect implicit conflicts from different policy priorities
- **AND** return detailed conflict report

#### Scenario: Block conflicting policy deployment
- **WHEN** policy conflict is detected
- **THEN** the system SHALL reject policy proposal
- **AND** provide clear explanation of conflict
- **AND** suggest resolution options (modify, override, remove)

#### Scenario: Policy conflict audit
- **WHEN** conflict detection runs
- **THEN** analysis results SHALL be logged to audit trail
- **AND** include policy IDs, conflict type, and resolution path

### Requirement: PostgreSQL Referential Integrity
The system SHALL enforce referential integrity in the organizational hierarchy database.

#### Scenario: Foreign key constraint enforcement
- **WHEN** database schema is created
- **THEN** foreign key constraints SHALL be defined for all relationships:
  - organizations.company_id → companies.id
  - teams.org_id → organizations.id
  - projects.team_id → teams.id
  - memberships.user_id → users.id
  - memberships.team_id → teams.id

#### Scenario: Cascading soft-delete
- **WHEN** an organization is deleted
- **THEN** all child teams SHALL be soft-deleted
- **AND** all child projects SHALL be soft-deleted
- **AND** memberships to deleted teams SHALL be soft-deleted
- **AND** cleanup job SHALL permanently remove after retention period

#### Scenario: Orphan detection
- **WHEN** integrity scan runs (daily)
- **THEN** system SHALL detect orphaned records (missing parent)
- **AND** log orphans to audit trail
- **AND** optionally auto-repair by reassigning or removing

### Requirement: WebSocket PubSub Reliability
The system SHALL ensure reliable delivery of policy updates via OPAL WebSocket PubSub.

#### Scenario: Reconnection with exponential backoff
- **WHEN** Cedar Agent loses WebSocket connection to OPAL Server
- **THEN** agent SHALL reconnect with exponential backoff (1s, 2s, 4s, 8s, max 30s)
- **AND** emit metrics for reconnection attempts
- **AND** log reconnection events

#### Scenario: Full resync on reconnect
- **WHEN** Cedar Agent reconnects after disconnection
- **THEN** agent SHALL request full policy and data resync
- **AND** verify data consistency with checksum
- **AND** log any detected drift

#### Scenario: Connection health monitoring
- **WHEN** WebSocket connection is active
- **THEN** system SHALL emit heartbeat metrics (latency, drop count)
- **AND** alert on high latency (>1s) or frequent drops (>3/minute)

### Requirement: IdP Synchronization Timeliness
The system SHALL ensure timely synchronization of user and group changes from identity providers.

#### Scenario: Webhook-based real-time sync
- **WHEN** IdP sends webhook for user/group change
- **THEN** system SHALL process webhook within 5 seconds
- **AND** update PostgreSQL referential
- **AND** trigger OPAL update
- **AND** Cedar Agents SHALL have updated data within 10 seconds total

#### Scenario: Scheduled pull sync fallback
- **WHEN** webhook delivery fails or is delayed
- **THEN** scheduled sync job (configurable, default: 15 minutes) SHALL catch up
- **AND** log delta between webhook and pull sync

#### Scenario: Sync lag alerting
- **WHEN** IdP changes are detected older than threshold (default: 30 minutes)
- **THEN** system SHALL alert on sync lag
- **AND** log affected users/groups

### Requirement: CLI Offline Mode
The system SHALL support offline operation of CLI commands when Aeterna server is unreachable.

#### Scenario: Local policy cache for offline
- **WHEN** CLI detects server is unreachable
- **THEN** CLI SHALL use local policy cache for read operations
- **AND** display warning about cached data age
- **AND** refuse write operations that require server

#### Scenario: Operation queue for later sync
- **WHEN** CLI is used offline for write operations
- **THEN** operations SHALL be queued in local SQLite database
- **AND** CLI SHALL attempt sync when server becomes reachable
- **AND** conflict resolution SHALL be prompted if conflicts detected

#### Scenario: Server reachability check
- **WHEN** CLI starts
- **THEN** CLI SHALL check server health endpoint
- **AND** cache result for session
- **AND** display connection status

### Requirement: Policy Rollback Mechanism
The system SHALL support rollback of policy deployments when issues are detected.

#### Scenario: Manual policy rollback
- **WHEN** administrator runs `aeterna policy rollback --version <version>`
- **THEN** system SHALL revert to specified policy version
- **AND** propagate change to all Cedar Agents
- **AND** log rollback in audit trail

#### Scenario: Automatic rollback on error rate
- **WHEN** authorization error rate exceeds threshold (configurable, default: 5% in 5 minutes)
- **THEN** system SHALL automatically rollback to previous policy version
- **AND** alert administrators
- **AND** log incident with error details

#### Scenario: Policy version history
- **WHEN** policy is deployed
- **THEN** system SHALL store version with timestamp and deployer
- **AND** retain configurable number of versions (default: 10)
- **AND** support diff between versions

### Requirement: LLM Translation Determinism
The system SHALL ensure deterministic policy translation from natural language to Cedar.

#### Scenario: Prompt caching for consistency
- **WHEN** same natural language input is provided multiple times
- **THEN** system SHALL return cached translation if available
- **AND** cache SHALL have configurable TTL (default: 24 hours)
- **AND** cache key SHALL include input hash and context

#### Scenario: Few-shot example templates
- **WHEN** natural language input matches known pattern
- **THEN** system SHALL use template-based translation (no LLM)
- **AND** templates SHALL cover 80% of common use cases
- **AND** fall back to LLM only for complex patterns

#### Scenario: Translation audit trail
- **WHEN** translation is performed
- **THEN** system SHALL log input, output, method (template/LLM), and confidence
- **AND** support review of translations for quality assurance

### Requirement: Approval Workflow Timeout
The system SHALL enforce timeouts on pending approval workflows to prevent stuck proposals.

#### Scenario: Configurable approval timeout
- **WHEN** proposal is submitted for approval
- **THEN** system SHALL apply timeout (configurable per governance level)
- **AND** send reminder at 50% and 75% of timeout
- **AND** expire proposal if timeout reached

#### Scenario: Escalation on timeout
- **WHEN** proposal timeout is approaching (75%)
- **THEN** system SHALL escalate to next approver tier
- **AND** notify original approvers of escalation
- **AND** log escalation event

#### Scenario: Auto-close expired proposals
- **WHEN** proposal expires
- **THEN** system SHALL close proposal with "expired" status
- **AND** notify proposer of expiration
- **AND** proposer can resubmit if still needed

### Requirement: Governance Audit Log Retention
The system SHALL manage retention of governance audit logs to prevent unbounded growth.

#### Scenario: Configurable retention policy
- **WHEN** audit log retention is configured (default: 90 days)
- **THEN** logs older than retention period SHALL be archived to cold storage
- **AND** archived logs SHALL be available for compliance queries
- **AND** hot storage SHALL contain only recent logs

#### Scenario: Audit log archival
- **WHEN** archival job runs (daily)
- **THEN** system SHALL move expired logs to S3 cold storage
- **AND** maintain index for searchability
- **AND** emit metrics for archived log count and size

#### Scenario: Compliance export
- **WHEN** compliance audit requires historical logs
- **THEN** system SHALL support export from archive
- **AND** export SHALL include full event details
- **AND** export SHALL be filterable by time range and entity
