# storage Specification

## Purpose
TBD - created by archiving change add-storage-layer. Update Purpose after archive.
## Requirements
### Requirement: Storage Backend Interface
The system SHALL define a trait for all storage backend implementations.

#### Scenario: StorageBackend methods
- **WHEN** implementing storage backend
- **THEN** it SHALL implement: initialize, shutdown, health_check
- **AND** it SHALL implement CRUD operations for data type
- **AND** it SHALL support transactions for atomic operations

#### Scenario: Storage backend types
- **WHEN** describing storage backends
- **THEN** system SHALL support: PostgreSQL, Qdrant, Redis
- **AND** each backend SHALL implement StorageBackend trait

### Requirement: PostgreSQL Implementation
The system SHALL implement a PostgreSQL backend for structured data storage.

#### Scenario: Initialize PostgreSQL connection
- **WHEN** system starts
- **THEN** backend SHALL initialize PostgreSQL client using sqlx
- **AND** backend SHALL create connection pool with deadpool
- **AND** backend SHALL run health check

#### Scenario: Create schema for episodic memories
- **WHEN** creating episodic memory table
- **THEN** system SHALL create table with fields: id, content, layer, identifiers, metadata, createdAt, updatedAt
- **AND** system SHALL add indexes on layer and timestamps

#### Scenario: Create schema for procedural memories
- **WHEN** creating procedural memory table
- **THEN** system SHALL create table with fields: id, fact, confidence, layer, metadata, createdAt, updatedAt
- **AND** system SHALL add indexes on layer and fact

#### Scenario: Create schema for user personal memories
- **WHEN** creating user memory table
- **THEN** system SHALL create table with fields: id, userId, content, embedding, metadata, createdAt, updatedAt
- **AND** system SHALL add pgvector index for semantic search

#### Scenario: Create schema for organization data
- **WHEN** creating organization table
- **THEN** system SHALL create table with fields: orgId, type, data, metadata, createdAt, updatedAt
- **AND** system SHALL add indexes on orgId and type

#### Scenario: Insert episodic memory
- **WHEN** storing episodic memory
- **THEN** system SHALL insert into PostgreSQL table
- **AND** system SHALL generate ID
- **AND** system SHALL set timestamps

#### Scenario: Query episodic memories
- **WHEN** querying episodic memories with filters
- **THEN** system SHALL return memories matching filters
- **AND** system SHALL support pagination with limit and offset
- **AND** system SHALL complete in < 50ms (P95)

#### Scenario: pgvector similarity search
- **WHEN** searching user memories semantically
- **THEN** system SHALL use pgvector cosine similarity
- **AND** system SHALL return top N results by score
- **AND** system SHALL complete in < 100ms (P95)

### Requirement: Qdrant Implementation
The system SHALL implement a Qdrant backend for vector storage and search.

#### Scenario: Initialize Qdrant client
- **WHEN** system starts
- **THEN** backend SHALL initialize Qdrant client
- **AND** backend SHALL create collections for semantic and archival layers
- **AND** backend SHALL run health check

#### Scenario: Create semantic memory collection
- **WHEN** creating semantic memory collection
- **THEN** system SHALL create Qdrant collection with vector dimensions
- **AND** system SHALL set distance metric to cosine
- **AND** system SHALL configure indexing for performance

#### Scenario: Upsert vector to Qdrant
- **WHEN** storing memory with embedding
- **THEN** system SHALL upsert point to Qdrant
- **AND** system SHALL include vector, payload, id
- **AND** system SHALL complete in < 20ms (P95)

#### Scenario: Vector similarity search
- **WHEN** searching semantic memories
- **THEN** system SHALL query Qdrant with search vector
- **AND** system SHALL return top N results by score
- **AND** system SHALL apply similarity threshold
- **AND** system SHALL complete in < 200ms (P95)

#### Scenario: Metadata filtering
- **WHEN** searching with metadata filters
- **THEN** system SHALL apply Qdrant filter to query
- **AND** system SHALL filter by tags and custom metadata fields
- **AND** system SHALL only return matching results

#### Scenario: Batch operations
- **WHEN** storing multiple vectors
- **THEN** system SHALL use Qdrant batch upsert
- **AND** system SHALL handle > 1000 vectors efficiently
- **AND** system SHALL complete in < 500ms (P95)

### Requirement: Redis Implementation
The system SHALL implement a Redis backend for working and session memory.

#### Scenario: Initialize Redis connection
- **WHEN** system starts
- **THEN** backend SHALL initialize Redis client with connection pool
- **AND** backend SHALL run health check

#### Scenario: Store working memory
- **WHEN** storing working memory (in-memory, microseconds)
- **THEN** system SHALL store value in Redis without TTL
- **AND** system SHALL complete in < 5ms (P95)

#### Scenario: Store session memory with TTL
- **WHEN** storing session memory
- **THEN** system SHALL store value in Redis with TTL
- **AND** system SHALL expire after session timeout
- **AND** system SHALL complete in < 20ms (P95)

#### Scenario: Retrieve working memory
- **WHEN** retrieving working memory
- **THEN** system SHALL get value from Redis by key
- **AND** system SHALL return null if not exists
- **AND** system SHALL complete in < 5ms (P95)

#### Scenario: Cache embeddings in Redis
- **WHEN** caching embedding for content
- **THEN** system SHALL store content hash -> embedding mapping
- **AND** system SHALL set TTL for cache (e.g., 24 hours)
- **AND** system SHALL avoid redundant embedding generation

### Requirement: Storage Factory
The system SHALL provide a factory for creating storage backends from configuration.

#### Scenario: Create PostgreSQL backend
- **WHEN** config specifies PostgreSQL
- **THEN** factory SHALL create PostgreSQL backend instance
- **AND** factory SHALL pass connection string and config

#### Scenario: Create Qdrant backend
- **WHEN** config specifies Qdrant
- **THEN** factory SHALL create Qdrant backend instance
- **AND** factory SHALL pass endpoint and collection name

#### Scenario: Create Redis backend
- **WHEN** config specifies Redis
- **THEN** factory SHALL create Redis backend instance
- **AND** factory SHALL pass connection URL and config

### Requirement: Connection Pooling
The system SHALL use connection pooling for efficient database connections.

#### Scenario: Configure PostgreSQL pool
- **WHEN** creating PostgreSQL backend
- **THEN** system SHALL configure deadpool with appropriate size
- **AND** system SHALL set min and max connections
- **AND** system SHALL handle connection timeouts gracefully

#### Scenario: Configure Redis pool
- **WHEN** creating Redis backend
- **THEN** system SHALL configure connection pool
- **AND** system SHALL set max connections based on expected load
- **AND** system SHALL recycle idle connections

### Requirement: Storage Health Checks
The system SHALL provide health check endpoints for all storage backends.

#### Scenario: PostgreSQL health check
- **WHEN** checking PostgreSQL health
- **THEN** backend SHALL execute simple query (SELECT 1)
- **AND** backend SHALL return healthy if query succeeds
- **AND** backend SHALL return unhealthy with error if query fails

#### Scenario: Qdrant health check
- **WHEN** checking Qdrant health
- **THEN** backend SHALL call Qdrant collections API
- **AND** backend SHALL return healthy if API responds
- **AND** backend SHALL return unhealthy with error if API fails

#### Scenario: Redis health check
- **WHEN** checking Redis health
- **THEN** backend SHALL execute PING command
- **AND** backend SHALL return healthy if PONG received
- **AND** backend SHALL return unhealthy with error if timeout

### Requirement: Storage Migrations
The system SHALL support schema migrations for PostgreSQL backend.

#### Scenario: Run initial migration
- **WHEN** system starts with no migrations run
- **THEN** system SHALL create all tables and indexes
- **AND** system SHALL record migration version
- **AND** system SHALL succeed without errors

#### Scenario: Run incremental migration
- **WHEN** new migration is available
- **THEN** system SHALL apply migration to schema
- **AND** system SHALL verify migration success
- **AND** system SHALL update migration version

#### Scenario: Migration rollback
- **WHEN** migration fails
- **THEN** system SHALL rollback to previous version
- **AND** system SHALL log rollback reason
- **AND** system SHALL notify admin of failure

### Requirement: Storage Metrics and Observability
The system SHALL emit metrics for all storage operations.

#### Scenario: Emit PostgreSQL metrics
- **WHEN** PostgreSQL operation completes
- **THEN** system SHALL emit counter: storage.postgres.queries.total
- **AND** system SHALL emit histogram: storage.postgres.query.latency
- **AND** system SHALL emit gauge: storage.postgres.connections.active

#### Scenario: Emit Qdrant metrics
- **WHEN** Qdrant operation completes
- **THEN** system SHALL emit counter: storage.qdrant.operations.total
- **AND** system SHALL emit histogram: storage.qdrant.search.latency
- **AND** system SHALL emit gauge: storage.qdrant.collections.size

#### Scenario: Emit Redis metrics
- **WHEN** Redis operation completes
- **THEN** system SHALL emit counter: storage.redis.operations.total
- **AND** system SHALL emit histogram: storage.redis.get.latency
- **AND** system SHALL emit gauge: storage.redis.connections.active

### Requirement: Storage Error Handling
The system SHALL provide specific error codes for all storage failures.

#### Scenario: Connection error
- **WHEN** storage backend cannot connect
- **THEN** system SHALL return STORAGE_CONNECTION_ERROR
- **AND** error SHALL include backend name
- **AND** error SHALL be retryable

#### Scenario: Query error
- **WHEN** storage query fails
- **THEN** system SHALL return STORAGE_QUERY_ERROR
- **AND** error SHALL include query and cause
- **AND** error SHALL be retryable

#### Scenario: Serialization error
- **WHEN** data cannot be serialized
- **THEN** system SHALL return STORAGE_SERIALIZATION_ERROR
- **AND** error SHALL not be retryable

### Requirement: Iceberg Catalog Storage
The system SHALL persist DuckDB Knowledge Graph nodes and edges to an Apache Iceberg table in object storage.

#### Scenario: Transactional Write
- **WHEN** multiple memory modifications occur within a session
- **THEN** the graph updates must be committed atomically to the Iceberg catalog
- **AND** the snapshot must be recoverable via time-travel queries

### Requirement: Cross-Tenant Data Deletion (GDPR)
The system MUST support cascading soft-deletes across vector databases, relational states, and DuckDB Iceberg tables for a specific tenant or user.

#### Scenario: User Data Wipe
- **WHEN** a tenant initiates a data deletion request for user X
- **THEN** all graph nodes, edges, vectors, and relational states owned by user X are marked as deleted and cascade.

