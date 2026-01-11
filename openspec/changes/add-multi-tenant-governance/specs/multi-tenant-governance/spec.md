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
