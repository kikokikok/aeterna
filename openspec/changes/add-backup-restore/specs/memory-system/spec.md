## ADDED Requirements

### Requirement: Memory Export Serialization
The system SHALL support serializing MemoryEntry records for portable export via the backup-restore capability.

#### Scenario: Export memory entries with embeddings
- **WHEN** the backup system exports memory data
- **THEN** each MemoryEntry SHALL be serialized as a complete JSON object including id, content, embedding (as a JSON array of floats), layer, summaries, context_vector, importance_score, metadata, created_at, and updated_at
- **AND** the serialization format SHALL preserve all fields required to reconstruct the entry on import without data loss

#### Scenario: Export memories scoped by layer
- **WHEN** the backup system exports memories with a layer filter
- **THEN** only MemoryEntry records matching the specified MemoryLayer SHALL be included in the export
- **AND** the export SHALL respect tenant isolation regardless of layer filter

#### Scenario: Streaming memory export
- **WHEN** the backup system exports memory data from PostgreSQL
- **THEN** the system SHALL read entries via cursor-based pagination within a REPEATABLE READ transaction
- **AND** the system SHALL serialize and write each batch to the NDJSON file before fetching the next batch
- **AND** peak memory usage SHALL be proportional to the configured batch size, not the total number of memories

#### Scenario: Import memory entries into existing store
- **WHEN** the backup system imports MemoryEntry records
- **THEN** the system SHALL write entries to the appropriate storage backend (PostgreSQL for metadata, Qdrant for embeddings) using the existing memory write path
- **AND** the system SHALL apply the specified conflict resolution mode (merge, replace, skip-existing) for entries with matching identifiers
