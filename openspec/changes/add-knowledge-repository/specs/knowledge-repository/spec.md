## ADDED Requirements

### Requirement: Knowledge Item Creation
The system SHALL provide a method to propose new knowledge items with automatic ID generation.

#### Scenario: Create knowledge item with valid data
- **WHEN** proposing a knowledge item with valid type, title, summary, content
- **THEN** system SHALL generate a unique ID
- **AND** system SHALL set initial status to 'draft'
- **AND** system SHALL create Git commit with type='create'
- **AND** system SHALL return the created item

#### Scenario: Create knowledge item with invalid type
- **WHEN** proposing a knowledge item with invalid type
- **THEN** system SHALL return INVALID_TYPE error
- **AND** error SHALL list valid types (adr, policy, pattern, spec)

### Requirement: Knowledge Query Operation
The system SHALL provide a method to query knowledge items with flexible filtering.

#### Scenario: Query all knowledge items
- **WHEN** querying knowledge without filters
- **THEN** system SHALL return all accessible items
- **AND** system SHALL include item summaries (not full content)
- **AND** system SHALL include totalCount

#### Scenario: Query with type filter
- **WHEN** querying knowledge with type='adr'
- **THEN** system SHALL only return ADR items

#### Scenario: Query with layer filter
- **WHEN** querying knowledge with layer='project'
- **THEN** system SHALL only return project-level knowledge

#### Scenario: Query with status filter
- **WHEN** querying knowledge with status=['accepted']
- **THEN** system SHALL only return accepted items
- **AND** system SHALL default to ['accepted'] if not specified

#### Scenario: Query with tags filter
- **WHEN** querying knowledge with tags=['security', 'database']
- **THEN** system SHALL only return items with matching tags
- **AND** system SHALL use OR logic for multiple tags

### Requirement: Knowledge Get Operation
The system SHALL provide a method to retrieve full knowledge item details.

#### Scenario: Get existing knowledge item
- **WHEN** getting a knowledge item with valid ID
- **THEN** system SHALL return full item with all fields
- **AND** system SHALL include constraints if includeConstraints=true
- **AND** system SHALL include commit history if includeHistory=true

#### Scenario: Get non-existent knowledge item
- **WHEN** getting a knowledge item with invalid ID
- **THEN** system SHALL return null without error

### Requirement: Constraint Check Operation
The system SHALL provide a method to evaluate constraints against a context.

#### Scenario: Check constraints with valid context
- **WHEN** checking constraints with files and dependencies
- **THEN** system SHALL load all applicable constraints from knowledge
- **AND** system SHALL evaluate each constraint
- **AND** system SHALL aggregate violations by severity
- **AND** system SHALL return result with passed flag
- **AND** system SHALL return all violations with locations

#### Scenario: Check constraints with specific knowledge items
- **WHEN** checking constraints with knowledgeItemIds
- **THEN** system SHALL only check constraints from specified items
- **AND** system SHALL ignore other knowledge items

#### Scenario: Check constraints with minimum severity
- **WHEN** checking constraints with minSeverity='block'
- **THEN** system SHALL only check block-level constraints
- **AND** system SHALL skip warn and info constraints

#### Scenario: Constraint violation severity
- **WHEN** a constraint with severity='block' is violated
- **THEN** system SHALL set passed=false in result
- **AND** system SHALL increment block count in summary

#### Scenario: Constraint violation with warn severity
- **WHEN** a constraint with severity='warn' is violated
- **THEN** system SHALL set passed=true in result
- **AND** system SHALL increment warn count in summary

### Requirement: Status Update Operation
The system SHALL provide a method to update knowledge item status.

#### Scenario: Update status to accepted
- **WHEN** updating status to 'accepted' from 'proposed'
- **THEN** system SHALL validate transition is allowed
- **AND** system SHALL update item status
- **AND** system SHALL create Git commit with type='status'
- **AND** system SHALL return updated item

#### Scenario: Update status to deprecated
- **WHEN** updating status to 'deprecated' from 'accepted'
- **THEN** system SHALL validate transition is allowed
- **AND** system SHALL create Git commit with type='status'
- **AND** system SHALL keep item accessible but marked deprecated

#### Scenario: Invalid status transition
- **WHEN** updating status from 'draft' to 'deprecated'
- **THEN** system SHALL return INVALID_STATUS error
- **AND** error SHALL explain allowed transitions

### Requirement: Manifest Generation
The system SHALL maintain an index of all knowledge items for fast lookups.

#### Scenario: Generate manifest after commit
- **WHEN** a knowledge commit is created
- **THEN** system SHALL regenerate manifest
- **AND** manifest SHALL include all items with metadata
- **AND** manifest SHALL group items by layer
- **AND** manifest SHALL group items by type
- **AND** manifest SHALL group items by status
- **AND** manifest SHALL store current Git commit hash

#### Scenario: Load existing manifest
- **WHEN** system starts
- **THEN** system SHALL load manifest from Git repository
- **AND** system SHALL validate manifest integrity
- **AND** system SHALL use manifest for fast queries

### Requirement: Git Commit Model
The system SHALL use immutable Git commits to track all knowledge changes.

#### Scenario: Create knowledge commit
- **WHEN** a knowledge change occurs
- **THEN** system SHALL create Git commit
- **AND** commit SHALL include affected item IDs
- **AND** commit SHALL include change type (create, update, delete, supersede, status, federation)
- **AND** commit SHALL include manifest snapshot
- **AND** commit SHALL include author and timestamp

#### Scenario: Commit immutability
- **WHEN** a commit exists
- **THEN** system SHALL never modify the commit
- **AND** system SHALL only create new commits

#### Scenario: Get commit history
- **WHEN** requesting knowledge history
- **THEN** system SHALL return all commits for item
- **AND** system SHALL order commits by timestamp (newest first)

### Requirement: Constraint DSL Parsing
The system SHALL parse constraint definitions from knowledge item content.

#### Scenario: Parse must_use constraint
- **WHEN** parsing "must_use: React"
- **THEN** system SHALL create Constraint with operator='must_use'
- **AND** system SHALL set target='dependency'
- **AND** system SHALL set pattern='react'
- **AND** system SHALL set severity from item's severity

#### Scenario: Parse must_not_use constraint
- **WHEN** parsing "must_not_use: eval()"
- **THEN** system SHALL create Constraint with operator='must_not_use'
- **AND** system SHALL set target='code'
- **AND** system SHALL set pattern='eval\(\)'

#### Scenario: Parse must_match constraint with appliesTo
- **WHEN** parsing "must_match: '*.ts' appliesTo: ['src/**']"
- **THEN** system SHALL create Constraint with operator='must_match'
- **AND** system SHALL set target='file'
- **AND** system SHALL set pattern='*.ts'
- **AND** system SHALL set appliesTo=['src/**']

#### Scenario: Invalid constraint syntax
- **WHEN** parsing constraint with invalid syntax
- **THEN** system SHALL return CONSTRAINT_SYNTAX_ERROR
- **AND** error SHALL indicate which part is invalid

### Requirement: Constraint Evaluation
The system SHALL evaluate constraints against provided context.

#### Scenario: Evaluate must_use constraint
- **WHEN** checking must_use for 'react' in dependencies
- **THEN** system SHALL verify 'react' exists in dependencies
- **AND** system SHALL create violation if not found

#### Scenario: Evaluate must_not_use constraint
- **WHEN** checking must_not_use for 'eval()' in code
- **THEN** system SHALL search code for 'eval(' pattern
- **AND** system SHALL create violation if pattern found

#### Scenario: Evaluate must_match constraint
- **WHEN** checking must_match for '*.ts' in files
- **THEN** system SHALL verify all files match pattern
- **AND** system SHALL create violation if file doesn't match

#### Scenario: Evaluate must_exist constraint
- **WHEN** checking must_exist for 'README.md'
- **THEN** system SHALL verify file exists
- **AND** system SHALL create violation if not found

### Requirement: Multi-Tenant Federation
The system SHALL support syncing knowledge from upstream repositories.

#### Scenario: Fetch upstream manifest
- **WHEN** federating from upstream repository
- **THEN** system SHALL fetch upstream manifest
- **AND** system SHALL compare with local manifest
- **AND** system SHALL compute delta

#### Scenario: Apply federation changes
- **WHEN** applying upstream changes
- **THEN** system SHALL create new items for additions
- **AND** system SHALL update items for modifications
- **AND** system SHALL delete items removed upstream
- **AND** system SHALL create commit with type='federation'

#### Scenario: Layer precedence in federation
- **WHEN** conflicting items exist in project and org layers
- **THEN** system SHALL keep project item (higher precedence)
- **AND** system SHALL ignore org item for that ID

### Requirement: Knowledge Error Handling
The system SHALL provide specific error codes for all failure scenarios.

#### Scenario: Item not found error
- **WHEN** getting non-existent knowledge item
- **THEN** system SHALL return ITEM_NOT_FOUND error
- **AND** error SHALL include the requested ID

#### Scenario: Invalid type error
- **WHEN** creating knowledge item with invalid type
- **THEN** system SHALL return INVALID_TYPE error
- **AND** error SHALL list valid types

#### Scenario: Invalid layer error
- **WHEN** creating knowledge item with invalid layer
- **THEN** system SHALL return INVALID_LAYER error
- **AND** error SHALL list valid layers

#### Scenario: Git operation error
- **WHEN** Git operation fails
- **THEN** system SHALL return GIT_ERROR
- **AND** error SHALL include Git error message
- **AND** error SHALL be marked as retryable

#### Scenario: Manifest corrupted error
- **WHEN** manifest fails validation
- **THEN** system SHALL return MANIFEST_CORRUPTED error
- **AND** system SHALL attempt to regenerate from Git history
