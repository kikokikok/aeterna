## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Policy Conflict Detection
The governance system MUST detect and block conflicting policy deployments before runtime, including conflicts across the expanded set of Cedar actions.

#### Scenario: Opposing Rules
- **WHEN** an admin submits a Cedar policy allowing an action that another policy explicitly denies
- **THEN** the `aeterna_policy_validate` analyzer must reject the proposal

#### Scenario: Expanded action conflict detection
- **WHEN** a new Cedar policy is submitted that affects tenant management, session, sync, graph, or CCA actions
- **THEN** the policy conflict detector SHALL evaluate the new policy against all existing permit and forbid rules for those action domains
- **AND** conflicting rules SHALL be reported before the policy is applied
