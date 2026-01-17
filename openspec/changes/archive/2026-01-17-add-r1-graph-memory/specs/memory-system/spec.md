## ADDED Requirements

### Requirement: Memory-R1 Pruning
The system SHALL support reinforcement learning-driven pruning of memory entries based on their contribution to successful task outcomes.

#### Scenario: Pruning useless memories
- **WHEN** a memory entry consistently fails to contribute to correct answers (negative reward)
- **THEN** it SHALL be marked for pruning or compression
- **AND** the system SHALL remove it from the semantic search index to reduce noise

### Requirement: Dynamic Graph Reasoning
The system SHALL maintain a dynamic knowledge graph of entities and relationships extracted from memory entries.

#### Scenario: Entity Relation Traversal
- **WHEN** a query requires linking two disparate concepts (e.g., 'Project A' and 'Memory Leak')
- **THEN** the system SHALL traverse the relationship graph to find common nodes
- **AND** return a reasoning path explaining the link

### Requirement: Embedded Graph Storage
The system SHALL use DuckDB as an embedded graph database requiring no separate server process.

#### Scenario: Zero-server graph operations
- **WHEN** the system initializes the graph store
- **THEN** it SHALL create or open a local DuckDB database file
- **AND** no external database server connection SHALL be required

#### Scenario: In-process graph queries
- **WHEN** an agent queries the graph
- **THEN** all processing SHALL occur within the Rust process
- **AND** latency SHALL be sub-millisecond for local operations

### Requirement: S3 Graph Persistence
The system SHALL support persisting graph data to S3-compatible storage using Parquet format for serverless deployments.

#### Scenario: Export graph to S3
- **WHEN** the system triggers a persistence checkpoint
- **THEN** it SHALL export all graph tables to Parquet files
- **AND** upload them to the configured S3 bucket with tenant-prefixed paths

#### Scenario: Load graph from S3
- **WHEN** the system initializes in a serverless environment (e.g., Lambda)
- **THEN** it SHALL download Parquet files from S3
- **AND** hydrate the in-memory DuckDB instance
- **AND** be ready to serve queries within cold-start budget (<3 seconds)

#### Scenario: Incremental persistence
- **WHEN** changes occur between checkpoints
- **THEN** the system SHALL track dirty pages
- **AND** only persist modified partitions to minimize S3 writes

### Requirement: SQL/PGQ Query Interface
The system SHALL support SQL:2023 Property Graph Queries (PGQ) via DuckPGQ extension for graph traversal operations.

#### Scenario: Define property graph
- **WHEN** the graph store initializes
- **THEN** it SHALL create property graph definitions using `CREATE PROPERTY GRAPH` syntax
- **AND** map memory nodes and edges to graph vertices and edges

#### Scenario: N-hop neighbor traversal
- **WHEN** an agent requests related memories within N hops
- **THEN** the system SHALL execute a `GRAPH_TABLE` query with path pattern `-[r]->{1,N}`
- **AND** return memories ordered by hop distance

#### Scenario: Shortest path query
- **WHEN** an agent requests the connection between two memories
- **THEN** the system SHALL execute a `MATCH SHORTEST` path query
- **AND** return the path with minimum hops and intermediate nodes

### Requirement: Multi-tenant Graph Isolation
The system SHALL enforce tenant isolation at the graph layer using tenant_id column filtering.

#### Scenario: Tenant-scoped queries
- **WHEN** an agent queries the graph
- **THEN** all queries SHALL include `WHERE tenant_id = $current_tenant` filter
- **AND** no cross-tenant data SHALL be returned

#### Scenario: Tenant-prefixed S3 storage
- **WHEN** graph data is persisted to S3
- **THEN** it SHALL use path prefix `s3://{bucket}/{tenant_id}/graph/`
- **AND** IAM policies SHALL restrict access to tenant-owned prefixes

### Requirement: LLM-based Entity Extraction
The system SHALL use LLM providers to extract entities and relationships from memory content.

#### Scenario: Extract entities from memory
- **WHEN** a new memory is stored
- **THEN** the system SHALL invoke the configured LLM to extract named entities
- **AND** create entity nodes linked to the memory node

#### Scenario: Extract relationships
- **WHEN** multiple entities are extracted from a memory
- **THEN** the system SHALL invoke the LLM to identify relationships between entities
- **AND** create typed edges (e.g., `WORKS_ON`, `CAUSED_BY`, `RELATES_TO`)

### Requirement: Cascading Graph Deletion
The system SHALL automatically remove orphaned graph data when memory entries are deleted to prevent data accumulation and maintain referential integrity.

#### Scenario: Memory deletion cascades to graph nodes
- **WHEN** a memory entry is deleted via `memory_delete`
- **THEN** all associated nodes in `memory_nodes` SHALL be removed
- **AND** all edges in `memory_edges` referencing the deleted node SHALL be removed
- **AND** all entities in `entities` linked to the memory SHALL be removed
- **AND** all entity edges derived from the memory SHALL be removed

#### Scenario: Batch deletion with graph cleanup
- **WHEN** multiple memories are deleted in a batch operation
- **THEN** graph cleanup SHALL be performed transactionally
- **AND** partial failures SHALL trigger rollback of the entire batch

#### Scenario: Soft delete with deferred cleanup
- **WHEN** a memory is soft-deleted (marked for deletion)
- **THEN** associated graph data SHALL be marked with `deleted_at` timestamp
- **AND** a background cleanup job SHALL permanently remove marked data after retention period

### Requirement: Application-Level Referential Integrity
The system SHALL enforce referential integrity at the application layer since DuckDB does not enforce foreign key constraints.

#### Scenario: Edge creation validates node existence
- **WHEN** an edge is created between two nodes
- **THEN** the system SHALL verify both source and target nodes exist
- **AND** reject the edge creation if either node is missing
- **AND** return an error indicating which node reference is invalid

#### Scenario: Entity edge validates entity existence
- **WHEN** an entity edge is created
- **THEN** the system SHALL verify both source and target entities exist in the `entities` table
- **AND** reject creation with clear error if validation fails

#### Scenario: Periodic integrity scan
- **WHEN** the integrity scan job runs (configurable interval, default: daily)
- **THEN** the system SHALL identify orphaned edges with missing nodes
- **AND** log violations to the audit log
- **AND** optionally auto-repair by removing orphaned records

### Requirement: Write Coordination for Single-Writer Constraint
The system SHALL implement write coordination to handle DuckDB's single-writer limitation in concurrent environments.

#### Scenario: Serialized writes via queue
- **WHEN** multiple concurrent write requests arrive
- **THEN** the system SHALL serialize writes through a Redis-backed queue
- **AND** process writes in FIFO order
- **AND** return success only after write is committed

#### Scenario: Lambda cold start lock acquisition
- **WHEN** a Lambda function cold starts and needs write access
- **THEN** it SHALL acquire a distributed lock (Redis SETNX) before initializing DuckDB
- **AND** wait with exponential backoff if lock is held
- **AND** timeout after configurable duration (default: 30s)

#### Scenario: Write contention metrics
- **WHEN** write contention occurs
- **THEN** the system SHALL emit metrics for queue depth, wait time, and timeout rate
- **AND** alert if contention exceeds threshold

### Requirement: Transactional S3 Persistence
The system SHALL implement transactional semantics for S3 persistence to prevent partial export failures.

#### Scenario: Two-phase commit for export
- **WHEN** the system triggers a persistence checkpoint
- **THEN** it SHALL write all Parquet files to a temporary S3 prefix
- **AND** validate checksums for all exported files
- **AND** atomically rename (copy + delete) to final path only after all files succeed

#### Scenario: Export failure recovery
- **WHEN** an export fails mid-operation
- **THEN** temporary files SHALL be cleaned up
- **AND** the previous consistent snapshot SHALL remain intact
- **AND** the failure SHALL be logged with details for debugging

#### Scenario: Checksum validation on load
- **WHEN** the system loads graph data from S3
- **THEN** it SHALL validate checksums of all Parquet files
- **AND** reject corrupted files with clear error
- **AND** fall back to previous snapshot if available

### Requirement: Composite Index Optimization
The system SHALL create composite indexes to optimize multi-tenant graph queries.

#### Scenario: Tenant-scoped edge queries use index
- **WHEN** a query filters by `tenant_id` and `source_id`
- **THEN** the query SHALL use the `idx_edges_tenant_source` composite index
- **AND** avoid full table scans

#### Scenario: Index creation on initialization
- **WHEN** the graph store initializes
- **THEN** it SHALL create indexes if not present:
  - `idx_edges_tenant_source ON memory_edges(tenant_id, source_id)`
  - `idx_edges_tenant_target ON memory_edges(tenant_id, target_id)`
  - `idx_nodes_tenant ON memory_nodes(tenant_id)`
  - `idx_entities_tenant ON entities(tenant_id)`

### Requirement: Graph Query Observability
The system SHALL emit telemetry for all graph operations to enable performance monitoring and debugging.

#### Scenario: Span creation for graph traversal
- **WHEN** `find_related()` or `shortest_path()` is called
- **THEN** the system SHALL create an OpenTelemetry span
- **AND** record attributes: query type, tenant_id, hop count, result count, duration_ms

#### Scenario: Metrics export
- **WHEN** graph operations complete
- **THEN** the system SHALL emit Prometheus metrics:
  - `graph_query_duration_seconds` (histogram)
  - `graph_query_result_count` (histogram)
  - `graph_cache_hit_ratio` (gauge)
  - `graph_traversal_depth` (histogram)

### Requirement: Tenant Query Isolation
The system SHALL enforce strict tenant isolation at the query layer to prevent cross-tenant data access.

#### Scenario: Parameterized tenant filter
- **WHEN** any graph query is executed
- **THEN** the tenant_id filter SHALL be applied via parameterized query
- **AND** never via string interpolation
- **AND** the tenant context SHALL be validated before query execution

#### Scenario: Query validation layer
- **WHEN** a query is submitted
- **THEN** the system SHALL parse and validate the query structure
- **AND** reject queries attempting to bypass tenant filters
- **AND** log rejected queries with full context for security audit

### Requirement: Automated Graph Backups
The system SHALL support automated backup of graph data with point-in-time recovery capability.

#### Scenario: Scheduled S3 snapshots
- **WHEN** the backup schedule triggers (configurable, default: every 6 hours)
- **THEN** the system SHALL create a versioned snapshot in S3
- **AND** retain snapshots according to retention policy (default: 7 days)
- **AND** emit metrics for backup duration and size

#### Scenario: Point-in-time recovery
- **WHEN** a recovery is requested with a timestamp
- **THEN** the system SHALL locate the nearest snapshot before the timestamp
- **AND** restore graph state from that snapshot
- **AND** log recovery operation with before/after state

### Requirement: Multi-Table Transaction Atomicity
The system SHALL ensure atomicity for operations spanning multiple graph tables.

#### Scenario: Atomic node and edge creation
- **WHEN** a memory is added with entities and relationships
- **THEN** all inserts (memory_nodes, memory_edges, entities, entity_edges) SHALL be wrapped in a single transaction
- **AND** failure in any insert SHALL rollback all changes

#### Scenario: Transaction isolation level
- **WHEN** concurrent reads and writes occur
- **THEN** the system SHALL use SERIALIZABLE isolation level
- **AND** readers SHALL not see partial writes

### Requirement: Lambda Cold Start Optimization
The system SHALL optimize cold start performance for serverless deployments.

#### Scenario: Lazy partition loading
- **WHEN** the graph store initializes in Lambda
- **THEN** it SHALL load only index metadata initially
- **AND** load data partitions on-demand as queries access them
- **AND** track partition access patterns for pre-warming hints

#### Scenario: Cold start budget enforcement
- **WHEN** initialization approaches the cold start budget (3 seconds)
- **THEN** the system SHALL complete with available data
- **AND** continue loading remaining partitions asynchronously
- **AND** log cold start duration and loaded partition count

#### Scenario: Warm pool strategy
- **WHEN** Lambda provisioned concurrency is configured
- **THEN** the system SHALL pre-warm graph data during initialization
- **AND** maintain warm connection to avoid repeated S3 fetches

### Requirement: Graph Health Checks
The system SHALL provide health check endpoints for monitoring graph store connectivity and status.

#### Scenario: Health endpoint returns status
- **WHEN** `/health/graph` is called
- **THEN** it SHALL verify DuckDB connection is alive
- **AND** verify S3 bucket is accessible (if configured)
- **AND** return status with latency measurements

#### Scenario: Readiness check
- **WHEN** `/ready/graph` is called
- **THEN** it SHALL verify graph data is loaded and queryable
- **AND** return ready status only when graph operations can succeed

### Requirement: Schema Migration Support
The system SHALL support versioned schema migrations for graph tables.

#### Scenario: Schema version tracking
- **WHEN** the graph store initializes
- **THEN** it SHALL check the `schema_version` table for current version
- **AND** apply pending migrations in order
- **AND** update version after successful migration

#### Scenario: Migration rollback
- **WHEN** a migration fails
- **THEN** the system SHALL rollback to previous schema state
- **AND** log failure details with migration step that failed
- **AND** prevent startup until migration issue is resolved

#### Scenario: Backward compatible migrations
- **WHEN** a new schema version is deployed
- **THEN** migrations SHALL be backward compatible (additive only)
- **AND** support rolling deployments without downtime
