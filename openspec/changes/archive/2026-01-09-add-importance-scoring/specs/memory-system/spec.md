## MODIFIED Requirements
### Requirement: Importance Scoring
The system SHALL provide a default algorithm to calculate an importance score for memory entries.

#### Scenario: Score based on frequency and recency
- **WHEN** a memory is accessed or updated
- **THEN** the system SHALL update its `access_count` and `last_accessed_at` metadata
- **AND** recalculate its importance score using a weighted combination:
  - Explicit Score: 60%
  - Frequency (Access Count): 30%
  - Recency (Time since last access): 10%
