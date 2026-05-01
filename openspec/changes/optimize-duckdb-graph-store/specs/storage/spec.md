## ADDED Requirements

### Requirement: Graph Store Reader Pool
The DuckDB-backed graph store SHALL maintain a pool of read-only DuckDB connections in addition to the single writer connection. Read operations SHALL acquire a connection from the pool and SHALL NOT contend with mutations for the writer mutex. Write operations SHALL serialise through the writer connection.

#### Scenario: Concurrent reads do not block one another
- **WHEN** N callers concurrently invoke read methods (e.g. `get_neighbors`, `find_path`) for the same tenant
- **AND** the reader pool size is N or larger
- **THEN** the calls SHALL execute in parallel without blocking each other on a shared mutex
- **AND** the observed reader QPS SHALL scale near-linearly with N up to the pool ceiling

#### Scenario: Reads do not wait behind writes
- **WHEN** a long-running write operation holds the writer mutex
- **AND** a concurrent read is issued
- **THEN** the read SHALL acquire a reader connection from the pool and proceed
- **AND** the read SHALL observe a consistent snapshot of the database under WAL semantics

#### Scenario: Pool size is configurable
- **WHEN** `DuckDbGraphConfig::reader_pool_size` is set to an integer N at construction
- **THEN** the store SHALL build a reader pool of size N
- **AND** when unset, the store SHALL default to `min(num_cpu, 8)`

### Requirement: Graph Store Edge and Label Indexes
The `memory_edges` table SHALL carry covering indexes for tenant-scoped neighbour lookup in both directions. The `memory_nodes` table SHALL carry a covering index for tenant-scoped label-equality lookup. Indexes SHALL be created idempotently in `initialize_schema` and SHALL NOT require a data migration.

#### Scenario: Edge index covers source-side neighbour lookup
- **WHEN** a query of the shape `SELECT ... FROM memory_edges WHERE tenant_id = ? AND source_id = ?` is run on a 5M-edge fixture
- **THEN** `EXPLAIN` SHALL show an index seek using `idx_edges_tenant_source`
- **AND** the median wall-clock latency SHALL be below 5 ms

#### Scenario: Edge index covers target-side neighbour lookup
- **WHEN** a query of the shape `SELECT ... FROM memory_edges WHERE tenant_id = ? AND target_id = ?` is run on the same fixture
- **THEN** `EXPLAIN` SHALL show an index seek using `idx_edges_tenant_target`

#### Scenario: Node label index covers label-equality scans
- **WHEN** a query of the shape `SELECT ... FROM memory_nodes WHERE tenant_id = ? AND label = ?` is run
- **THEN** `EXPLAIN` SHALL show an index seek using `idx_nodes_tenant_label`

## MODIFIED Requirements

### Requirement: Storage Backend Interface
The system SHALL define a trait for all storage backend implementations. The graph storage backend SHALL additionally expose `append_event` (write path through the event log) and `last_applied_seq` (projector observability) once event-sourcing is enabled. Existing read methods of the trait SHALL be preserved unchanged so existing callers continue to compile and behave identically.

#### Scenario: StorageBackend methods
- **WHEN** implementing storage backend
- **THEN** it SHALL implement: initialize, shutdown, health_check
- **AND** it SHALL implement CRUD operations for data type
- **AND** it SHALL support transactions for atomic operations

#### Scenario: Storage backend types
- **WHEN** describing storage backends
- **THEN** system SHALL support: PostgreSQL, Qdrant, Redis
- **AND** each backend SHALL implement StorageBackend trait

#### Scenario: Graph backend exposes event-sourced methods when enabled
- **WHEN** the graph backend is constructed with `event_sourcing_enabled = true`
- **THEN** the backend SHALL expose `append_event(tenant_id, kind, payload) -> Result<seq>`
- **AND** the backend SHALL expose `last_applied_seq(tenant_id) -> i64`
- **AND** existing read methods SHALL continue to work unchanged for callers that do not use these new methods
