## ADDED Requirements

### Requirement: Promotion Lifecycle API
The server SHALL provide first-class lifecycle APIs for knowledge promotion workflows.

#### Scenario: Preview promotion split
- **WHEN** a client requests a promotion preview
- **THEN** the server SHALL return suggested shared content and residual content
- **AND** the response SHALL include a suggested residual semantic role

#### Scenario: Create promotion request
- **WHEN** a client submits a promotion request
- **THEN** the server SHALL persist the request
- **AND** the server SHALL return a stable promotion request identifier

#### Scenario: Approve promotion request
- **WHEN** an authorized reviewer approves a promotion request
- **THEN** the server SHALL apply the reviewed decision
- **AND** the server SHALL create or update the resulting items and relations atomically or with safe compensation

### Requirement: Additive Backward Compatibility
The server SHALL preserve existing knowledge CRUD and governance routes during rollout.

#### Scenario: Legacy client continues to operate
- **WHEN** an older client continues to use the existing knowledge CRUD endpoints
- **THEN** the server SHALL continue to support those endpoints
- **AND** promotion-specific lifecycle behavior SHALL remain accessible through additive endpoints

### Requirement: Promotion Event Emission
The server SHALL emit promotion lifecycle events for audit and real-time monitoring.

#### Scenario: Emit event on promotion apply
- **WHEN** a promotion request is successfully applied
- **THEN** the server SHALL emit a `KnowledgePromotionApplied` event
- **AND** the event SHALL include source item ID, resulting item IDs, target layer, and request ID
