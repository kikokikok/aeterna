# governance Specification

## Purpose
TBD - created by archiving change refactor-enterprise-architecture. Update Purpose after archive.
## Requirements
### Requirement: High Availability Policy Engine
The governance engine (OPAL Server and Cedar components) SHALL operate in a highly available, multi-replica architecture without unsafe secret rotation or permissive production defaults.

#### Scenario: OPAL Pod Failure
- **WHEN** an OPAL server pod crashes
- **THEN** authorization decisions must continue unhindered via local caches and remaining replicas
- **AND** the system must utilize an HA Redis backend for PubSub state

#### Scenario: OPAL secret reuse during upgrade
- **WHEN** OPAL components are upgraded without an explicit credential change
- **THEN** master and client tokens SHALL remain stable across the upgrade
- **AND** connected agents and fetchers SHALL NOT be invalidated by chart-generated secret churn

### Requirement: Policy Conflict Detection
The governance system MUST detect and block conflicting policy deployments before runtime, including conflicts across the expanded set of Cedar actions.

#### Scenario: Opposing Rules
- **WHEN** an admin submits a Cedar policy allowing an action that another policy explicitly denies
- **THEN** the `aeterna_policy_validate` analyzer must reject the proposal

#### Scenario: Expanded action conflict detection
- **WHEN** a new Cedar policy is submitted that affects tenant management, session, sync, graph, or CCA actions
- **THEN** the policy conflict detector SHALL evaluate the new policy against all existing permit and forbid rules for those action domains
- **AND** conflicting rules SHALL be reported before the policy is applied

### Requirement: Production Authentication Defaults
The governance and agent-facing runtime surfaces SHALL enforce fail-closed authentication behavior in production-capable deployments.

#### Scenario: Auth enabled without valid configuration
- **WHEN** authentication is enabled for an agent-facing service
- **AND** no valid API key, JWT verifier, or equivalent auth backend is configured
- **THEN** service startup or request processing SHALL fail closed
- **AND** the system SHALL NOT treat missing auth configuration as authenticated access

#### Scenario: JWT-backed request context
- **WHEN** a request presents a JWT-backed Authorization header
- **THEN** the system SHALL validate the token before deriving tenant or user context
- **AND** invalid or unimplemented JWT handling SHALL return an authentication error

#### Scenario: Production CORS behavior
- **WHEN** production deployment mode is configured
- **THEN** CORS origins, methods, and headers SHALL be restricted to configured allowlists
- **AND** wildcard permissive defaults SHALL NOT be used in production

#### Scenario: Production-capable mode does not use allow-all auth by accident
- **WHEN** a production-capable deployment starts without an explicitly supported permissive development mode
- **THEN** the runtime SHALL NOT default to allow-all authorization for tenant-scoped surfaces
- **AND** the operator SHALL receive an actionable configuration error instead

### Requirement: Expanded Cedar Policy Coverage
The governance system SHALL maintain Cedar policies that cover all authorization actions across all domains (memory, knowledge, policy, governance, organization, agent, tenant, session, sync, graph, CCA, user management, admin operations), with explicit permit and forbid rules for each role.

#### Scenario: Cedar policies cover all defined actions
- **WHEN** the Cedar policy bundle is loaded
- **THEN** the bundle SHALL contain permit rules mapping each of the 8 roles to their authorized actions
- **AND** the bundle SHALL contain forbid rules for cross-tenant access and privilege escalation
- **AND** every Cedar action defined in the schema SHALL have at least one permit rule and be tested

#### Scenario: New Cedar actions validated by tests
- **WHEN** RBAC integration tests run against the Cedar policy bundle
- **THEN** the tests SHALL verify all role-action combinations for the expanded action set
- **AND** the tests SHALL cover both positive (permit) and negative (deny) cases for each role

### Requirement: Promotion Review Decisions
The governance system SHALL support structured decisions for promotion requests.

#### Scenario: Approve as specialization
- **WHEN** a reviewer determines that shared content should be promoted while local detail remains below
- **THEN** the reviewer SHALL be able to approve the request as Specialization
- **AND** the system SHALL preserve the lower-layer residual item

#### Scenario: Retarget promotion
- **WHEN** a reviewer determines the requested target layer is too broad
- **THEN** the reviewer SHALL be able to retarget the request to a narrower higher layer
- **AND** the request SHALL remain reviewable without losing history

#### Scenario: Reject promotion
- **WHEN** reviewers reject a promotion request
- **THEN** the system SHALL preserve the original lower-layer item unchanged
- **AND** the request SHALL transition to Rejected

### Requirement: Authorization by Lifecycle Action
The governance system SHALL authorize promotion actions by action type and target layer.

#### Scenario: Approver lacks authority for target layer
- **WHEN** a reviewer attempts to approve a promotion to a layer beyond their authority
- **THEN** the system SHALL reject the action
- **AND** the promotion request SHALL remain pending

### Requirement: Idempotent Governance Decisions
The governance system SHALL make promotion decision endpoints idempotent.

#### Scenario: Duplicate approval retry
- **WHEN** an approval request is retried after the promotion request has already been approved
- **THEN** the system SHALL not create duplicate relations, notifications, or promoted items
- **AND** the system SHALL return the existing outcome

### Requirement: Stale Review Rejection
The governance system SHALL fail closed on stale promotion reviews.

#### Scenario: Source knowledge changed during review
- **WHEN** the source or target canonical item changes after a promotion request was submitted
- **THEN** the system SHALL reject approval of the stale request
- **AND** the reviewer SHALL be prompted to refresh or resubmit
