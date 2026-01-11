## ADDED Requirements

### Requirement: Memory Layer Types
The system SHALL define a seven-layer memory hierarchy for storing agent experiences with different scopes and lifetimes.

#### Scenario: Memory layer enumeration
- **WHEN** the system is initialized
- **THEN** the following memory layers SHALL be defined: agent, user, session, project, team, org, company
- **AND** each layer SHALL have a unique precedence value (1-7)

### Requirement: Knowledge Type Definitions
The system SHALL support four types of knowledge entries with distinct characteristics.

#### Scenario: Knowledge type enumeration
- **WHEN** creating knowledge items
- **THEN** the following types SHALL be supported: adr (Architecture Decision Record), policy, pattern, spec (technical specification)
- **AND** each type SHALL have a file extension and display name

### Requirement: Knowledge Layer Hierarchy
The system SHALL support a four-layer multi-tenant knowledge hierarchy for organizational structure.

#### Scenario: Knowledge layer enumeration
- **WHEN** the system is initialized
- **THEN** the following knowledge layers SHALL be defined: company, org, team, project
- **AND** project SHALL have highest precedence, company SHALL have lowest precedence

### Requirement: Constraint Severity Levels
The system SHALL define three enforcement levels for constraints to guide agent behavior.

#### Scenario: Constraint severity enumeration
- **WHEN** defining constraints
- **THEN** the following severity levels SHALL be supported: info, warn, block
- **AND** block SHALL stop the action, warn SHALL allow continuation with warning, info SHALL only log

### Requirement: Constraint Operators
The system SHALL support six constraint operators for enforcing organizational rules.

#### Scenario: Constraint operator enumeration
- **WHEN** creating constraints
- **THEN** the following operators SHALL be supported: must_use, must_not_use, must_match, must_not_match, must_exist, must_not_exist

### Requirement: Layer Identifiers
The system SHALL require specific identifiers for each memory layer to ensure proper scoping.

#### Scenario: Agent layer requires agentId
- **WHEN** creating a memory at the agent layer
- **THEN** the system SHALL require agentId and userId identifiers

#### Scenario: User layer requires userId
- **WHEN** creating a memory at the user layer
- **THEN** the system SHALL require userId identifier

#### Scenario: Session layer requires sessionId
- **WHEN** creating a memory at the session layer
- **THEN** the system SHALL require userId and sessionId identifiers

### Requirement: Memory Entry Structure
The system SHALL define a complete memory entry structure with all required fields.

#### Scenario: Memory entry with all fields
- **WHEN** a memory entry is created
- **THEN** it SHALL contain: id, content, layer, identifiers, metadata, createdAt, updatedAt
- **AND** it MAY contain an optional embedding field

### Requirement: Knowledge Item Structure
The system SHALL define a complete knowledge item structure with all required fields.

#### Scenario: Knowledge item with all fields
- **WHEN** a knowledge item is created
- **THEN** it SHALL contain: id, type, layer, title, summary, content, contentHash, status, severity, constraints, tags, metadata, createdAt, updatedAt
- **AND** it MAY contain optional version and supersedes fields

### Requirement: Content Hash Computation
The system SHALL compute SHA-256 hashes of knowledge item content for change detection.

#### Scenario: Hash computation includes all fields
- **WHEN** computing a knowledge item hash
- **THEN** the hash SHALL include: content, constraints, and status
- **AND** the result SHALL be a hexadecimal SHA-256 string

### Requirement: UUID Generation
The system SHALL generate unique identifiers for all entities.

#### Scenario: UUID generation for new entities
- **WHEN** creating a new memory or knowledge item
- **THEN** the system SHALL generate a unique UUID v4 identifier

### Requirement: Layer Precedence Function
The system SHALL provide a function to determine the precedence order of memory and knowledge layers.

#### Scenario: Precedence ordering for memory layers
- **WHEN** comparing two memory layers
- **THEN** agent SHALL have precedence 1, user SHALL have precedence 2
- **AND** lower precedence numbers SHALL indicate higher priority

#### Scenario: Precedence ordering for knowledge layers
- **WHEN** comparing two knowledge layers
- **THEN** project SHALL have precedence 1, company SHALL have precedence 4
- **AND** the system SHALL use precedence for conflict resolution

### Requirement: Validation Functions
The system SHALL provide validation functions for all user inputs.

#### Scenario: Memory layer validation
- **WHEN** validating a memory layer string
- **THEN** the system SHALL return true for valid layers (agent, user, session, project, team, org, company)
- **AND** it SHALL return false for invalid values

#### Scenario: Knowledge type validation
- **WHEN** validating a knowledge type string
- **THEN** the system SHALL return true for valid types (adr, policy, pattern, spec)
- **AND** it SHALL return false for invalid values

### Requirement: Configuration Structure
The system SHALL define a comprehensive configuration structure for all system components.

#### Scenario: Configuration includes all components
- **WHEN** the system reads configuration
- **THEN** it SHALL support configuration for: memory provider, knowledge repository, sync, tool interface, storage adapters
- **AND** all configuration SHALL be validated at startup

### Requirement: Error Types
The system SHALL define specific error types for each component.

#### Scenario: Memory error codes
- **WHEN** a memory operation fails
- **THEN** the error SHALL include a code from: INVALID_LAYER, MISSING_IDENTIFIER, MEMORY_NOT_FOUND, CONTENT_TOO_LONG, QUERY_TOO_LONG, EMBEDDING_FAILED, PROVIDER_ERROR, RATE_LIMITED, UNAUTHORIZED, CONFIGURATION_ERROR

#### Scenario: Knowledge error codes
- **WHEN** a knowledge operation fails
- **THEN** the error SHALL include a code from: ITEM_NOT_FOUND, INVALID_TYPE, INVALID_LAYER, INVALID_STATUS, CONSTRAINT_VIOLATION, GIT_ERROR, MANIFEST_CORRUPTED
