## ADDED Requirements

### Requirement: Knowledge Export Serialization
The system SHALL support serializing KnowledgeEntry, KnowledgeRelation, and PromotionRequest records for portable export via the backup-restore capability.

#### Scenario: Export knowledge entries with metadata
- **WHEN** the backup system exports knowledge data
- **THEN** each KnowledgeEntry SHALL be serialized as a complete JSON object including path, content, layer, kind, status, summaries, metadata, commit_hash, author, and updated_at
- **AND** each KnowledgeRelation SHALL be serialized including id, source_id, target_id, relation_type, tenant_id, created_by, created_at, and metadata

#### Scenario: Export promotion history
- **WHEN** the backup system exports knowledge data
- **THEN** each PromotionRequest SHALL be serialized including all fields (id, source/target layers, content, status, decisions, requested_by, tenant_id, timestamps)
- **AND** the export SHALL preserve the full promotion decision chain for audit purposes

#### Scenario: Import knowledge entries with relation integrity
- **WHEN** the backup system imports KnowledgeEntry and KnowledgeRelation records
- **THEN** the system SHALL validate that all relation source_id and target_id references resolve to entries present in either the archive or the target store
- **AND** the system SHALL fail the import if referential integrity cannot be established

#### Scenario: Streaming knowledge export
- **WHEN** the backup system exports knowledge data from PostgreSQL
- **THEN** the system SHALL read entries via cursor-based pagination within the same REPEATABLE READ transaction used for memory export
- **AND** the system SHALL serialize and write each batch to the NDJSON file before fetching the next batch
