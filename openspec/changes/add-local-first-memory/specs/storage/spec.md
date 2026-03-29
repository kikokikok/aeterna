## ADDED Requirements

### Requirement: SQLite Storage Backend
The storage layer SHALL support SQLite (via `better-sqlite3` in TypeScript, `rusqlite` in Rust) as an embedded storage backend for local-first memory operations.

#### Scenario: SQLite backend initialization
- **WHEN** a local memory store is initialized
- **THEN** the system SHALL create or open a SQLite database with WAL journal mode
- **AND** the system SHALL create the required tables (`memories`, `sync_queue`, `sync_cursors`) idempotently
- **AND** the database file SHALL be a single `.db` file at the configured path

#### Scenario: SQLite vector search
- **WHEN** a vector similarity search is requested on the local store
- **AND** stored memories have embeddings (float32 arrays stored as BLOBs)
- **THEN** the system SHALL load embeddings and compute cosine similarity in application code
- **AND** the system SHALL return the top-k results sorted by similarity score

#### Scenario: SQLite text search fallback
- **WHEN** a search is requested and embeddings are not available
- **THEN** the system SHALL perform text matching using SQL `LIKE` operators
- **AND** results SHALL be sorted by `updated_at` descending

#### Scenario: SQLite concurrent access
- **WHEN** multiple operations access the SQLite database concurrently (sync engine + tool calls)
- **THEN** the system SHALL use WAL mode to allow concurrent reads with a single writer
- **AND** write contention SHALL be handled via SQLite's built-in busy timeout (default 5000ms)
