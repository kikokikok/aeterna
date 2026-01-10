## MODIFIED Requirements
### Requirement: Memory Promotion
The system SHALL support promoting memories from volatile layers (Agent, Session) to persistent layers (User, Project, Team, Org, Company) based on an importance threshold.

#### Scenario: Promote important session memory to project layer
- **WHEN** a session memory entry has an importance score >= `promotionThreshold` (default 0.8)
- **AND** the `promoteImportant` flag is enabled
- **THEN** the system SHALL create a copy of this memory in the Project layer
- **AND** link it to the original session memory via metadata

### Requirement: Importance Scoring
The system SHALL provide a default algorithm to calculate an importance score for memory entries.

#### Scenario: Score based on frequency and recency
- **WHEN** a memory is accessed or updated
- **THEN** the system SHALL update its `access_count` and `last_accessed_at` metadata
- **AND** recalculate its importance score using a combination of frequency (access count) and recency.

### Requirement: PII Redaction
The system SHALL redact personally identifiable information (PII) from memory content before it is promoted to persistent layers.

#### Scenario: Redact email from content
- **WHEN** a memory is being evaluated for promotion
- **AND** the content contains an email address (e.g., "user@example.com")
- **THEN** the system SHALL replace the email with `[REDACTED_EMAIL]`

#### Scenario: Redact phone number from content
- **WHEN** a memory is being evaluated for promotion
- **AND** the content contains a phone number (e.g., "123-456-7890")
- **THEN** the system SHALL replace the phone number with `[REDACTED_PHONE]`
