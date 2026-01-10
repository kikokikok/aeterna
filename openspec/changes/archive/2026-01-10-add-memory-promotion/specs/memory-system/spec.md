## ADDED Requirements

### Requirement: Memory Promotion
The system SHALL support promoting memories from volatile layers (Agent, Session) to persistent layers (User, Project, Team, Org, Company) based on an importance threshold.

#### Scenario: Promote important session memory to user layer
- **WHEN** a session memory entry has an importance score >= `promotionThreshold` (default 0.8)
- **AND** the `promoteImportant` flag is enabled
- **THEN** the system SHALL create a copy of this memory in the User layer
- **AND** link it to the original session memory via metadata

### Requirement: Importance Scoring
The system SHALL provide a default algorithm to calculate an importance score for memory entries.

#### Scenario: Score based on frequency and recency
- **WHEN** a memory is accessed or updated
- **THEN** the system SHALL update its `access_count` and `last_accessed_at` metadata
- **AND** recalculate its importance score using a combination of frequency (access count) and recency.

### Requirement: Promotion Trigger
The system SHALL trigger memory promotion checks at specific lifecycle events.

#### Scenario: Promotion check at session end
- **WHEN** a session is closed
- **THEN** the system SHALL evaluate all memories in that session for promotion.
