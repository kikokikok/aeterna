## ADDED Requirements

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
