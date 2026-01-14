# Multi-Tenant Governance Specification

## ADDED Requirements

### Requirement: Tenant Isolation

The system SHALL enforce hard isolation at the company (tenant) boundary for all memory and knowledge operations.

Each tenant MUST have:
- Unique tenant identifier
- Isolated data storage (logical or physical)
- Independent configuration

#### Scenario: Tenant boundary enforcement
- **WHEN** a user from Company A attempts to access data belonging to Company B
- **THEN** the system SHALL reject the request with an authorization error
- **AND** the system SHALL NOT reveal whether the target resource exists

#### Scenario: Cross-tenant search isolation
- **WHEN** a user performs a vector similarity search
- **THEN** the system SHALL only return results from the user's tenant
- **AND** embeddings from other tenants SHALL NOT influence the search results

### Requirement: Hierarchical Organization Structure

The system SHALL support a four-level organizational hierarchy within each tenant:
1. Company (tenant root)
2. Organization (business unit)
3. Team (working group)
4. Project (codebase/repository)

#### Scenario: Hierarchy navigation
- **WHEN** a user queries for knowledge at the Team level
- **THEN** the system SHALL include inherited knowledge from Organization and Company levels
- **AND** the system SHALL mark each result with its originating hierarchy level

#### Scenario: Hierarchy creation
- **WHEN** an admin creates a new Team under an Organization
- **THEN** the Team SHALL inherit default policies from the parent Organization
- **AND** the Team SHALL be visible to all Organization members with appropriate permissions

### Requirement: Relationship-Based Access Control

The system SHALL implement ReBAC (Relationship-Based Access Control) using OpenFGA for fine-grained permissions within a tenant.

Supported roles:
- **Developer**: Can add memories, propose knowledge, view resources
- **Tech Lead**: Can approve promotions, manage team knowledge
- **Architect**: Can reject proposals, force corrections, review drift
- **Admin**: Full tenant management access
- **Agent**: Inherits permissions from the user it acts on behalf of

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

### Requirement: Governance Event Streaming

The system SHALL emit real-time events for all governance-related actions via Redis Streams.

Event types:
- `KnowledgeProposed`: New knowledge proposal submitted
- `KnowledgeApproved`: Proposal approved by authorized role
- `KnowledgeRejected`: Proposal rejected with reason
- `MemoryPromoted`: Memory promoted to higher layer
- `DriftDetected`: Semantic drift identified
- `PolicyViolation`: Policy compliance failure

#### Scenario: Real-time governance notification
- **WHEN** a knowledge proposal is submitted
- **THEN** the system SHALL emit a `KnowledgeProposed` event within 100ms
- **AND** all subscribed governance dashboards SHALL receive the event

#### Scenario: Event persistence
- **WHEN** a governance event is emitted
- **THEN** the event SHALL be persisted for audit purposes
- **AND** the event SHALL include timestamp, actor, action, and affected resources

### Requirement: Semantic Drift Detection

The system SHALL detect semantic drift between project-level knowledge/memories and higher-level organizational standards.

Drift types:
- **Contradicting**: Project knowledge contradicts organization policy
- **Missing**: Required policies not present at project level
- **Stale**: References to deprecated or outdated knowledge
- **Pattern Deviation**: Significant deviation from approved patterns

#### Scenario: Real-time contradiction detection
- **WHEN** a new knowledge item is created that contradicts an existing higher-level policy
- **THEN** the system SHALL flag the item with a drift warning
- **AND** the system SHALL calculate a drift score based on semantic similarity

#### Scenario: Missing policy detection
- **WHEN** a project lacks required policies defined at the organization level
- **THEN** the system SHALL identify the missing policies during sync
- **AND** the system SHALL generate a compliance report

#### Scenario: Drift score calculation
- **WHEN** drift detection runs for a project
- **THEN** the system SHALL calculate: `drift_score = sum(severity_weight * item_drift) / total_items`
- **AND** `item_drift = 1 - cosine_similarity(project_embedding, reference_embedding)`

### Requirement: Batch Governance Analysis

The system SHALL perform scheduled batch analysis for complex governance checks that require LLM-based semantic comparison.

Schedule:
- **Hourly**: Quick drift scan for active projects
- **Daily**: Full drift analysis with LLM semantic comparison
- **Weekly**: Comprehensive governance report generation

#### Scenario: Hourly drift scan
- **WHEN** the hourly batch job runs
- **THEN** the system SHALL scan all projects modified in the last hour
- **AND** the system SHALL perform vector-based drift detection only (no LLM calls)

#### Scenario: Daily semantic analysis
- **WHEN** the daily batch job runs
- **THEN** the system SHALL perform LLM-based semantic comparison for flagged items
- **AND** the system SHALL update drift scores with refined analysis
- **AND** the system SHALL cache results to avoid redundant LLM calls

#### Scenario: Weekly governance report
- **WHEN** the weekly batch job runs
- **THEN** the system SHALL generate a governance report per organization
- **AND** the report SHALL include drift trends, approval rates, and policy compliance

### Requirement: Deployment Mode Support

The system SHALL support three deployment modes to accommodate different usage patterns:

| Mode | Local Components | Central Components |
|------|------------------|-------------------|
| **Local** | All (Redis, PG, Qdrant, Aeterna) | None |
| **Hybrid** | Working/Session memory only | Episodic+, Knowledge, Governance |
| **Remote** | None | All |

#### Scenario: Hybrid mode sync
- **WHEN** running in Hybrid mode
- **THEN** local Working and Session memories SHALL sync to central Episodic memory
- **AND** central Knowledge queries SHALL be available locally
- **AND** governance events SHALL originate from the central server only

#### Scenario: Local mode isolation
- **WHEN** running in Local mode
- **THEN** all operations SHALL be self-contained
- **AND** no external network calls SHALL be made for governance
- **AND** drift detection SHALL run against local knowledge only

#### Scenario: Remote mode delegation
- **WHEN** running in Remote mode
- **THEN** all operations SHALL delegate to the central server
- **AND** the local instance SHALL act as a thin client
- **AND** authentication SHALL be required for all operations

### Requirement: Governance Dashboard API

The system SHALL expose REST API endpoints for governance dashboards to query and visualize governance data.

Endpoints:
- `GET /api/v1/governance/drift/{project_id}` - Project drift status
- `GET /api/v1/governance/proposals` - Pending proposals list
- `GET /api/v1/governance/reports/{org_id}` - Organization reports
- `POST /api/v1/governance/proposals/{id}/approve` - Approve proposal
- `POST /api/v1/governance/proposals/{id}/reject` - Reject proposal

#### Scenario: Drift status query
- **WHEN** a dashboard queries drift status for a project
- **THEN** the system SHALL return current drift score, flagged items, and trend data
- **AND** the response SHALL include last scan timestamp

#### Scenario: Proposal approval via API
- **WHEN** an authorized user approves a proposal via API
- **THEN** the system SHALL validate the user's role permissions
- **AND** the system SHALL emit a `KnowledgeApproved` event
- **AND** the system SHALL update the knowledge repository

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
- **WHEN** a request arrives at any API endpoint
- **THEN** the system SHALL extract and validate TenantContext from the request
- **AND** the context SHALL be available to all downstream operations

#### Scenario: Context validation failure
- **WHEN** a request lacks valid TenantContext
- **THEN** the system SHALL reject the request with 401 Unauthorized
- **AND** the system SHALL NOT process any data operations

### Requirement: Policy Inheritance

The system SHALL support policy inheritance from higher hierarchy levels to lower levels.

Policies can be:
- **Mandatory**: Must be present at all child levels (inherited automatically)
- **Optional**: Can be overridden at child levels
- **Forbidden**: Cannot be overridden (enforced from parent)

#### Scenario: Mandatory policy inheritance
- **WHEN** a Company defines a mandatory security policy
- **THEN** all Organizations, Teams, and Projects under that Company SHALL inherit the policy
- **AND** the policy SHALL NOT be deletable at lower levels

#### Scenario: Optional policy override
- **WHEN** a Team overrides an optional Organization policy
- **THEN** the Team's policy version SHALL take precedence for that Team
- **AND** the override SHALL be tracked for audit purposes

#### Scenario: Forbidden policy enforcement
- **WHEN** a lower level attempts to create a policy that contradicts a forbidden parent policy
- **THEN** the system SHALL reject the creation
- **AND** the system SHALL return the conflicting parent policy in the error response

### Requirement: Tenant Data Isolation Security (MT-C1)
The system SHALL implement defense-in-depth for tenant data isolation.

#### Scenario: Query Parameterization
- **WHEN** database queries are executed
- **THEN** all queries MUST use parameterized statements
- **AND** tenant_id MUST be included in all WHERE clauses

#### Scenario: Row-Level Security
- **WHEN** PostgreSQL tables contain multi-tenant data
- **THEN** row-level security (RLS) policies MUST be enabled
- **AND** RLS MUST enforce tenant isolation at the database level

#### Scenario: Penetration Testing
- **WHEN** new tenant isolation features are deployed
- **THEN** penetration testing MUST verify cross-tenant isolation
- **AND** test results MUST be documented

### Requirement: RBAC Policy Testing (MT-C2)
The system SHALL have comprehensive automated testing for role-based access control.

#### Scenario: RBAC Integration Tests
- **WHEN** CI runs
- **THEN** integration tests MUST verify all role-action-resource combinations
- **AND** tests MUST cover positive and negative authorization cases

#### Scenario: Permission Matrix Validation
- **WHEN** RBAC policies are modified
- **THEN** a permission matrix MUST be generated
- **AND** matrix MUST be reviewed before deployment

#### Scenario: Role Escalation Prevention
- **WHEN** testing role permissions
- **THEN** tests MUST verify privilege escalation is not possible
- **AND** tests MUST verify role hierarchy is enforced correctly

### Requirement: Drift Detection Tuning (MT-C3)
The system SHALL provide controls to reduce drift detection false positives.

#### Scenario: Configurable Drift Threshold
- **WHEN** drift detection runs
- **THEN** drift threshold MUST be configurable per project
- **AND** default threshold MUST be conservative (0.2 similarity difference)

#### Scenario: Drift Suppression Rules
- **WHEN** a known legitimate drift pattern exists
- **THEN** suppression rules MUST allow marking it as acceptable
- **AND** suppressed drifts MUST be tracked separately in reports

#### Scenario: Drift Confidence Scoring
- **WHEN** drift is detected
- **THEN** system MUST provide confidence score for each drift item
- **AND** low-confidence drifts MUST be flagged for manual review

### Requirement: Event Streaming Reliability (MT-H1)
The system SHALL implement reliable delivery for governance events.

#### Scenario: Event Persistence
- **WHEN** governance events are emitted
- **THEN** events MUST be persisted to durable storage before acknowledgment
- **AND** persistence failure MUST be retried with exponential backoff

#### Scenario: At-Least-Once Delivery
- **WHEN** consumers receive events
- **THEN** delivery MUST be at-least-once with idempotency keys
- **AND** consumers MUST handle duplicate events gracefully

#### Scenario: Dead Letter Queue
- **WHEN** event delivery fails after max retries
- **THEN** events MUST be moved to dead letter queue
- **AND** DLQ MUST be monitored with alerts

### Requirement: Batch Job Coordination (MT-H2)
The system SHALL prevent batch job scheduling conflicts.

#### Scenario: Distributed Locking
- **WHEN** batch jobs are scheduled
- **THEN** distributed locks MUST prevent concurrent execution
- **AND** locks MUST have TTL to prevent deadlocks

#### Scenario: Job Deduplication
- **WHEN** a job is triggered while previous instance is running
- **THEN** system MUST skip the duplicate run
- **AND** skip event MUST be logged for monitoring

#### Scenario: Job Timeout
- **WHEN** a batch job exceeds timeout (default: 30 minutes)
- **THEN** job MUST be terminated gracefully
- **AND** partial results MUST be persisted

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

#### Scenario: Context Audit Trail
- **WHEN** operations are performed
- **THEN** TenantContext MUST be logged with each operation
- **AND** audit logs MUST enable forensic reconstruction

### Requirement: Authorization Fallback (MT-H4)
The system SHALL implement fallback for external authorization service failures.

#### Scenario: Local Policy Cache
- **WHEN** Permit.io/external auth service is unavailable
- **THEN** system MUST use locally cached policies
- **AND** cache MUST be refreshed when service recovers

#### Scenario: OPA/Cedar Fallback
- **WHEN** configuring authorization
- **THEN** system MUST support local OPA or Cedar as fallback
- **AND** policy sync between Permit.io and local MUST be automated

#### Scenario: Graceful Degradation
- **WHEN** authorization is degraded
- **THEN** system MUST log degradation mode
- **AND** read operations MAY continue with cached permissions

### Requirement: Dashboard API Security (MT-H5)
The system SHALL secure dashboard API endpoints.

#### Scenario: JWT Validation
- **WHEN** dashboard API requests are received
- **THEN** JWT tokens MUST be validated
- **AND** expired tokens MUST be rejected

#### Scenario: OPAL Integration
- **WHEN** using OPAL for policy updates
- **THEN** dashboard auth MUST integrate with OPAL auth
- **AND** API keys MUST be rotatable

#### Scenario: CORS Configuration
- **WHEN** dashboard frontends access API
- **THEN** CORS MUST be configured with allowed origins
- **AND** production MUST not allow wildcard origins
