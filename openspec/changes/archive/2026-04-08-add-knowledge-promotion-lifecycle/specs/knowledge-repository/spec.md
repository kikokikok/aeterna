## ADDED Requirements

### Requirement: Knowledge Promotion Request Lifecycle
The system SHALL treat promotion as a first-class lifecycle request distinct from generic knowledge CRUD.

#### Scenario: Create promotion request
- **WHEN** a user requests promotion of an accepted knowledge item to a higher layer
- **THEN** the system SHALL create a PromotionRequest
- **AND** the request SHALL capture target layer, promotion mode, shared content, residual content, residual role, and justification
- **AND** the request SHALL enter PendingReview status

### Requirement: Partial Promotion Preservation
The system SHALL preserve lower-layer specificity when only part of a knowledge item is promoted.

#### Scenario: Promote shared core and retain specialization
- **WHEN** a user promotes only the shared portion of a project-layer knowledge item
- **THEN** the system SHALL create a higher-layer canonical knowledge item
- **AND** the lower-layer item SHALL remain available
- **AND** the lower-layer item SHALL be marked with an appropriate semantic role such as Specialization, Applicability, Exception, or Clarification
- **AND** the items SHALL be linked by explicit semantic relations

### Requirement: Full Promotion Replacement
The system SHALL allow full promotion when higher-layer content fully replaces lower-layer content.

#### Scenario: Full-equivalence promotion
- **WHEN** reviewers approve a promotion as a full replacement
- **THEN** the promoted higher-layer item SHALL become canonical
- **AND** the lower-layer item SHALL transition to Superseded
- **AND** the system SHALL record explicit supersession relations

### Requirement: Knowledge Semantic Relations
The system SHALL persist explicit semantic relations between knowledge items.

#### Scenario: Create specialization relation
- **WHEN** a promoted item leaves project-specific detail behind
- **THEN** the system SHALL record a Specializes relation from the lower-layer item to the higher-layer canonical item

### Requirement: Deterministic Resolution Precedence
The system SHALL resolve canonical and residual knowledge deterministically.

#### Scenario: Resolve canonical and specialization together
- **WHEN** a query matches both a higher-layer canonical item and a local specialization
- **THEN** the system SHALL return the canonical item as shared truth
- **AND** the system SHALL return the local specialization as linked residual context
- **AND** the response SHALL preserve relation metadata

### Requirement: Confidential Promotion Validation
The system SHALL prevent confidential or scope-inappropriate content from being promoted to broader layers.

#### Scenario: Reject confidential promotion
- **WHEN** a promotion request contains content that violates confidentiality policy for the target layer
- **THEN** the system SHALL reject the promotion request
- **AND** the source lower-layer item SHALL remain unchanged

### Requirement: Tenant Boundary Enforcement
The system SHALL enforce tenant boundaries across promotion workflows.

#### Scenario: Cross-tenant promotion forbidden
- **WHEN** a promotion request attempts to promote knowledge across tenants without explicit federation support
- **THEN** the system SHALL reject the request
- **AND** no promoted item or relation SHALL be created

### Requirement: Promotion Lifecycle Auditability
The system SHALL audit all promotion lifecycle transitions.

#### Scenario: Audit promotion approval
- **WHEN** a promotion request is approved
- **THEN** the system SHALL record the approver identity, reviewer decision, resulting item IDs, relation changes, and timestamp
