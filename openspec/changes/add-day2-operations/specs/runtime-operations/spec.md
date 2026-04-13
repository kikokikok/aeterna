## ADDED Requirements

### Requirement: Lifecycle Manager Background Tasks
The server SHALL spawn periodic background tasks for data lifecycle management.

#### Scenario: Tasks start with server
- **WHEN** the server starts and lifecycle management is enabled
- **THEN** the system SHALL spawn background tasks for reconciliation, retention, quota enforcement, importance decay, job cleanup, dead-letter processing, and remediation auto-expiry

#### Scenario: Graceful task shutdown
- **WHEN** the server receives a shutdown signal
- **THEN** the system SHALL cancel pending lifecycle tasks and wait for in-progress tasks to complete

#### Scenario: Lifecycle feature flag
- **WHEN** `AETERNA_LIFECYCLE_ENABLED` is set to false
- **THEN** no lifecycle tasks SHALL be spawned and the server SHALL start without lifecycle management

### Requirement: Remediation API
The server SHALL provide REST endpoints for managing remediation requests.

#### Scenario: List pending remediations
- **WHEN** an administrator requests the remediation list
- **THEN** the server SHALL return all pending remediation requests with risk tier, entity type, and proposed action

#### Scenario: Approve remediation
- **WHEN** an administrator approves a remediation request
- **THEN** the server SHALL execute the proposed action and update the request status

#### Scenario: Reject remediation
- **WHEN** an administrator rejects a remediation request with a reason
- **THEN** the server SHALL mark the request as rejected and log the reason

### Requirement: Dead-Letter Queue API
The server SHALL provide REST endpoints for managing the dead-letter queue.

#### Scenario: List dead-letter items
- **WHEN** an administrator requests the dead-letter list
- **THEN** the server SHALL return all dead-letter items with error details, retry count, and timestamps

#### Scenario: Retry dead-letter item
- **WHEN** an administrator triggers a retry for a dead-letter item
- **THEN** the server SHALL re-attempt the original operation

#### Scenario: Discard dead-letter item
- **WHEN** an administrator discards a dead-letter item
- **THEN** the server SHALL create a RequireApproval remediation request before permanent deletion
