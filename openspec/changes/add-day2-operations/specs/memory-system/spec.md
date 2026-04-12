## MODIFIED Requirements

### Requirement: Memory Deletion
The system SHALL ensure that deleting a memory entry cascades to all storage backends.

#### Scenario: Delete cascades to Qdrant vector
- **WHEN** a memory entry is deleted from PostgreSQL
- **THEN** the system SHALL delete the corresponding vector point from Qdrant using the memory entry ID
- **AND** the system SHALL NOT leave orphaned vectors in Qdrant after deletion

#### Scenario: Delete cascades to graph node
- **WHEN** a memory entry is deleted from PostgreSQL
- **THEN** the system SHALL soft-delete the corresponding graph node and its edges in DuckDB

#### Scenario: Delete cascades to Redis cache
- **WHEN** a memory entry is deleted from PostgreSQL
- **THEN** the system SHALL delete any embedding cache keys in Redis matching the memory entry ID

## ADDED Requirements

### Requirement: Memory Importance Decay
The system SHALL apply time-based exponential decay to memory importance scores based on access patterns.

#### Scenario: Periodic importance decay
- **WHEN** the decay job runs (default: hourly)
- **THEN** the system SHALL update importance scores using the formula `new_score = score * (1 - decay_rate) ^ days_since_last_access`
- **AND** the decay rate SHALL be configurable per memory layer

#### Scenario: Access resets decay clock
- **WHEN** a memory is accessed via search or direct retrieval
- **THEN** the system SHALL update the `last_accessed_at` timestamp to the current time

### Requirement: Cold-Tier Archival
The system SHALL archive stale memories to cold storage to free hot storage resources.

#### Scenario: Archival threshold trigger
- **WHEN** a memory's importance score drops below the archival threshold (default 0.01)
- **THEN** the system SHALL move the memory content to cold storage
- **AND** the system SHALL delete the Qdrant vector
- **AND** the system SHALL retain a stub record in PostgreSQL with `archived_at` timestamp and cold-tier reference
