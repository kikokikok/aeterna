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
The system SHALL use DuckDB with DuckPGQ extension as the embedded graph database engine, requiring no external server.

#### Scenario: Zero-server deployment
- **WHEN** Aeterna is deployed as a library or single binary
- **THEN** the graph database SHALL operate without external database servers
- **AND** all graph operations SHALL be performed in-process

### Requirement: S3 Graph Persistence
The system SHALL support persisting graph data to S3-compatible storage using Parquet format.

#### Scenario: Serverless persistence
- **WHEN** running in AWS Lambda or similar serverless environment
- **THEN** the system SHALL load graph state from S3 on startup
- **AND** persist graph state to S3 on shutdown or periodic intervals
- **AND** use Parquet format for efficient columnar storage

#### Scenario: Cost-efficient idle
- **WHEN** no graph operations are being performed
- **THEN** the system SHALL incur zero compute costs
- **AND** only S3 storage costs SHALL apply

### Requirement: SQL/PGQ Query Interface
The system SHALL use SQL/PGQ (SQL:2023 standard) syntax for graph pattern matching and path finding.

#### Scenario: Pattern matching query
- **WHEN** a user queries for memories related to a topic within N hops
- **THEN** the system SHALL execute a SQL/PGQ MATCH query
- **AND** return results ordered by path distance

#### Scenario: Shortest path query
- **WHEN** a user queries for the connection between two memories
- **THEN** the system SHALL execute a SQL/PGQ SHORTEST path query
- **AND** return the minimum-hop path with intermediate nodes

### Requirement: Multi-tenant Graph Isolation
The system SHALL isolate graph data between tenants using tenant_id filtering.

#### Scenario: Tenant isolation
- **WHEN** multiple tenants share a GraphStore instance
- **THEN** all queries SHALL filter by tenant_id
- **AND** no cross-tenant data leakage SHALL occur
