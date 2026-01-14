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
