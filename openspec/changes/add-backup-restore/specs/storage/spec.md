## ADDED Requirements

### Requirement: Coordinated Multi-Backend Export Access
The system SHALL provide a coordinated data access interface for reading from multiple storage backends during export operations with bounded resource consumption.

#### Scenario: PostgreSQL export with REPEATABLE READ isolation
- **WHEN** an export job reads memory, knowledge, governance, or organizational data from PostgreSQL
- **THEN** the read operations SHALL execute within a REPEATABLE READ transaction on the dedicated export connection pool
- **AND** the transaction SHALL remain open for the duration of all PostgreSQL reads within the export job
- **AND** the export connection pool SHALL be separate from the main application pool to prevent resource contention with live queries

#### Scenario: Cursor-based streaming from PostgreSQL
- **WHEN** an export job reads a table from PostgreSQL
- **THEN** the system SHALL use server-side cursors (`DECLARE CURSOR ... FETCH N`) with configurable fetch size
- **AND** each fetched batch SHALL be serialized and written to the archive before the next batch is fetched
- **AND** the system SHALL NOT accumulate the full result set in memory

#### Scenario: Qdrant export with collection snapshot
- **WHEN** an export job reads vector data from Qdrant
- **THEN** the system SHALL create a Qdrant collection snapshot at the beginning of the export
- **AND** vector data SHALL be read from the snapshot via the scroll API with configurable batch size
- **AND** the snapshot SHALL be deleted after export completes or is cancelled

#### Scenario: DuckDB export with tenant scoping
- **WHEN** an export job reads graph data from DuckDB
- **THEN** the system SHALL filter graph nodes and edges by the export tenant scope
- **AND** the system SHALL NOT read data belonging to other tenants

### Requirement: Multi-Backend Import Access
The system SHALL provide a coordinated data access interface for writing to multiple storage backends during import operations with transactional safety.

#### Scenario: Transactional import with rollback
- **WHEN** an import job writes data across PostgreSQL and Qdrant
- **THEN** PostgreSQL writes SHALL execute within a transaction that rolls back on any failure
- **AND** Qdrant writes that were committed before a PostgreSQL rollback SHALL be compensated by deleting the imported points

#### Scenario: Batched import writes
- **WHEN** an import job writes data to storage backends
- **THEN** the system SHALL process records in configurable batches (default 500 for PG, 100 for Qdrant)
- **AND** each batch SHALL be committed independently to bound transaction size and memory usage

#### Scenario: DuckDB import with integrity checks
- **WHEN** an import job writes graph nodes and edges to DuckDB
- **THEN** the system SHALL verify that all edge source_id and target_id references resolve to existing nodes (either already in DuckDB or imported in the same job)
- **AND** the system SHALL reject edges with dangling node references
