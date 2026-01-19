## ADDED Requirements

### Requirement: Hindsight Knowledge Type
The repository SHALL support 'hindsight' as a new knowledge type for storing error-resolution patterns.

#### Scenario: Store hindsight item
- **WHEN** creating knowledge item with type='hindsight'
- **THEN** system SHALL validate error_signature and resolutions metadata
- **AND** system SHALL store as versioned Markdown in hindsight/ directory
- **AND** system SHALL generate unique ID with hindsight- prefix

#### Scenario: Query hindsight items
- **WHEN** querying knowledge with type='hindsight'
- **THEN** system SHALL return matching hindsight items
- **AND** system SHALL support filtering by error_type in metadata
- **AND** system SHALL support semantic search on error patterns

#### Scenario: Hindsight lifecycle
- **WHEN** hindsight item reaches success threshold
- **THEN** system MAY auto-transition from draft to proposed
- **AND** system SHALL require approval for accepted status

### Requirement: Summary Storage for Knowledge Items
The repository SHALL store pre-computed summaries for knowledge items to support efficient context assembly.

#### Scenario: Store item summary
- **WHEN** knowledge item is created or updated
- **AND** summary generation is enabled
- **THEN** system SHALL generate summaries at configured depths
- **AND** system SHALL store summaries in item metadata
- **AND** system SHALL include token counts and source hash

#### Scenario: Retrieve item summaries
- **WHEN** querying knowledge items for context assembly
- **THEN** system SHALL return summaries instead of full content
- **AND** system SHALL return appropriate depth based on token budget
- **AND** system SHALL return full content reference for deep inspection

### Requirement: Knowledge Type Configuration Extension
The system SHALL extend type configuration to include hindsight type with specific requirements.

#### Scenario: Hindsight type configuration
- **WHEN** validating hindsight knowledge item
- **THEN** system SHALL require error_signature field in metadata
- **AND** system SHALL require at least one resolution in metadata
- **AND** system SHALL validate error_signature schema (error_type, message_pattern)

#### Scenario: Hindsight file structure
- **WHEN** storing hindsight knowledge item
- **THEN** system SHALL store in {layer}/hindsight/{id}.md
- **AND** system SHALL include structured YAML frontmatter with error metadata

### Requirement: Constraint Support for Hindsight
The system SHALL support constraints derived from hindsight patterns.

#### Scenario: Generate constraint from hindsight
- **WHEN** hindsight item has high success_rate resolution
- **THEN** system MAY auto-generate must_not_match constraint for error pattern
- **AND** system SHALL set constraint severity based on hindsight confidence

#### Scenario: Link constraint to resolution
- **WHEN** constraint from hindsight is violated
- **THEN** system SHALL include resolution suggestion in violation message
- **AND** system SHALL include reference to source hindsight item
