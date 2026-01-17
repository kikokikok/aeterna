---
title: Memory System Specification
status: draft
version: 0.1.0
created: 2025-01-07
authors:
  - AI Systems Architecture Team
related:
  - 01-core-concepts.md
  - 04-memory-knowledge-sync.md
  - 05-adapter-architecture.md
---

## Purpose

The Memory System provides a hierarchical, provider-agnostic semantic memory store for AI agents, enabling long-term learning and knowledge retention across different scopes (agent, user, session, project, etc.).
## Requirements
### Requirement: Memory Promotion
The system SHALL support promoting memories from volatile layers (Agent, Session) to persistent layers (User, Project, Team, Org, Company) based on an importance threshold.

#### Scenario: Promote important session memory to project layer
- **WHEN** a session memory entry has an importance score >= `promotionThreshold` (default 0.8)
- **AND** the `promoteImportant` flag is enabled
- **THEN** the system SHALL create a copy of this memory in the Project layer
- **AND** link it to the original session memory via metadata

### Requirement: Importance Scoring
The system SHALL provide a default algorithm to calculate an importance score for memory entries.

#### Scenario: Score based on frequency and recency
- **WHEN** a memory is accessed or updated
- **THEN** the system SHALL update its `access_count` and `last_accessed_at` metadata
- **AND** recalculate its importance score using a combination of frequency (access count) and recency.

### Requirement: Promotion Trigger
The system SHALL trigger memory promotion checks at specific lifecycle events.

#### Scenario: Promotion check at session end
- **WHEN** a session is closed
- **THEN** the system SHALL evaluate all memories in that session for promotion.

### Requirement: PII Redaction
The system SHALL redact personally identifiable information (PII) from memory content before it is promoted to persistent layers.

#### Scenario: Redact email from content
- **WHEN** a memory is being evaluated for promotion
- **AND** the content contains an email address (e.g., "user@example.com")
- **THEN** the system SHALL replace the email with `[REDACTED_EMAIL]`

#### Scenario: Redact phone number from content
- **WHEN** a memory is being evaluated for promotion
- **AND** the content contains a phone number (e.g., "123-456-7890")
- **THEN** the system SHALL replace the phone number with `[REDACTED_PHONE]`

### Requirement: Sensitivity Check
The system SHALL prevent promotion of memories marked as sensitive or private.

#### Scenario: Block promotion of sensitive memory
- **WHEN** a memory is marked as `sensitive: true` or `private: true` in metadata
- **THEN** the system SHALL NOT promote this memory to higher layers
- **AND** record telemetry for the promotion block.

### Requirement: Performance Telemetry
The system SHALL track and emit metrics for key memory operations.

#### Scenario: Track search latency
- **WHEN** a semantic search is performed
- **THEN** the system SHALL record the operation latency and emit it to the configured metrics provider.

### Requirement: Memory Provider Adapter Trait
The system SHALL define a trait for all memory provider implementations to ensure consistent behavior.

#### Scenario: Provider trait methods
- **WHEN** implementing a memory provider
- **THEN** it SHALL implement: initialize, shutdown, health_check, add, search, get, update, delete, list, generateEmbedding
- **AND** it MAY implement: bulkAdd, bulkDelete

### Requirement: Memory Add Operation
The system SHALL provide a method to store information in memory with automatic embedding generation and governance validation.

#### Scenario: Add memory with content, layer, and tenant context
- **WHEN** adding a memory with valid content, layer, and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL validate memory content against governance policies
- **AND** system SHALL generate a unique ID
- **AND** system SHALL generate vector embedding
- **AND** system SHALL persist memory to provider with tenant isolation
- **AND** system SHALL return memory entry with all fields

#### Scenario: Add memory with missing tenant context
- **WHEN** adding a memory without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error
- **AND** error SHALL indicate TenantContext is required

#### Scenario: Add memory with missing identifier
- **WHEN** adding a memory without required layer identifier
- **THEN** system SHALL return MISSING_IDENTIFIER error
- **AND** error SHALL indicate which identifier is required

### Requirement: Memory Search Operation
The system SHALL provide semantic search across multiple memory layers with configurable parameters and tenant isolation.

#### Scenario: Search across all accessible layers with tenant context
- **WHEN** searching memories with query, layer identifiers, and valid TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL generate query embedding
- **AND** system SHALL search all accessible layers within the tenant concurrently
- **AND** system SHALL enforce tenant isolation (no cross-tenant results)
- **AND** system SHALL merge results by layer precedence
- **AND** system SHALL apply similarity threshold filtering
- **AND** system SHALL return results sorted by precedence then score

#### Scenario: Search with layer filter
- **WHEN** searching memories with specific layers parameter and TenantContext
- **THEN** system SHALL only search in specified layers within the tenant
- **AND** system SHALL skip other layers

#### Scenario: Search with threshold parameter
- **WHEN** searching memories with custom threshold and TenantContext
- **THEN** system SHALL only return results with score >= threshold
- **AND** system SHALL use threshold 0.7 if not specified

### Requirement: Memory Get Operation
The system SHALL provide a method to retrieve a memory by ID with tenant isolation.

#### Scenario: Get existing memory with tenant context
- **WHEN** getting a memory with valid ID and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL verify the memory belongs to the same tenant
- **AND** system SHALL return the memory entry with all fields

#### Scenario: Get non-existent memory
- **WHEN** getting a memory with invalid ID and TenantContext
- **THEN** system SHALL return null without error

#### Scenario: Get memory from different tenant
- **WHEN** getting a memory that belongs to a different tenant
- **THEN** system SHALL return null without revealing cross-tenant information

### Requirement: Memory Update Operation
The system SHALL provide a method to update existing memories with optional re-embedding and governance validation.

#### Scenario: Update memory content with tenant context
- **WHEN** updating a memory with new content and valid TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL validate updated content against governance policies
- **AND** system SHALL re-generate vector embedding
- **AND** system SHALL update the memory
- **AND** system SHALL update timestamp

#### Scenario: Update memory metadata only
- **WHEN** updating a memory with only metadata changes and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL NOT re-generate embedding
- **AND** system SHALL merge metadata with existing
- **AND** system SHALL update timestamp

#### Scenario: Update non-existent memory
- **WHEN** updating a memory with invalid ID and TenantContext
- **THEN** system SHALL return MEMORY_NOT_FOUND error

### Requirement: Memory Delete Operation
The system SHALL provide a method to remove memories from storage with tenant isolation.

#### Scenario: Delete existing memory with tenant context
- **WHEN** deleting a memory with valid ID and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL verify the memory belongs to the same tenant
- **AND** system SHALL remove memory from provider
- **AND** system SHALL return success: true

#### Scenario: Delete non-existent memory
- **WHEN** deleting a memory with invalid ID and TenantContext
- **THEN** system SHALL return success: true (idempotent)

### Requirement: Memory List Operation
The system SHALL provide a method to list memories with pagination, filtering, and tenant isolation.

#### Scenario: List memories with pagination and tenant context
- **WHEN** listing memories with limit parameter and TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL return up to limit results from the same tenant
- **AND** system SHALL return nextCursor if more results exist
- **AND** system SHALL return totalCount

#### Scenario: List memories with filter
- **WHEN** listing memories with metadata filter and TenantContext
- **THEN** system SHALL only return memories matching filter criteria within the tenant
- **AND** system SHALL support filtering by tags and custom metadata fields

### Requirement: Layer Precedence Merging
The system SHALL merge search results from multiple layers using precedence rules.

#### Scenario: Merge with higher priority from specific layer
- **WHEN** merging results from project and company layers
- **THEN** system SHALL sort results with project layer before company layer
- **AND** system SHALL break ties with higher similarity score

#### Scenario: Deduplicate similar results
- **WHEN** merging results with 0.95+ similarity
- **THEN** system SHALL keep only highest precedence result
- **AND** system SHALL discard lower precedence duplicates

### Requirement: Concurrent Layer Search
The system SHALL search multiple memory layers concurrently for performance.

#### Scenario: Parallel search across layers
- **WHEN** searching across 3+ layers
- **THEN** system SHALL initiate all searches concurrently
- **AND** system SHALL wait for all searches to complete
- **AND** system SHALL merge results after all layers finish

### Requirement: Provider Capabilities
The system SHALL allow providers to advertise their supported features.

#### Scenario: Provider with vector search capability
- **WHEN** checking provider capabilities
- **THEN** provider SHALL indicate vectorSearch: true
- **AND** provider SHALL indicate embeddingDimensions: number
- **AND** provider SHALL indicate distanceMetrics: array

#### Scenario: Provider without bulk operations
- **WHEN** checking provider capabilities
- **THEN** provider SHALL indicate bulkOperations: false

### Requirement: Memory Metadata Filtering
The system SHALL support filtering memories by metadata fields.

#### Scenario: Filter by tags
- **WHEN** searching memories with tags parameter
- **THEN** system SHALL only return memories with matching tags
- **AND** system SHALL support multiple tags with OR logic

#### Scenario: Filter by custom metadata
- **WHEN** searching memories with custom filter
- **THEN** system SHALL support filtering on any metadata field
- **AND** system SHALL support equality, contains, and range operators

### Requirement: Embedding Generation
The system SHALL generate vector embeddings for memory content.

#### Scenario: Generate embedding for new content
- **WHEN** adding a memory with new content
- **THEN** system SHALL generate vector embedding using configured provider
- **AND** system SHALL store embedding with memory
- **AND** system SHALL return embeddingGenerated: true

#### Scenario: Cache repeated embeddings
- **WHEN** adding memory with content seen before
- **THEN** system SHALL return cached embedding if exists
- **AND** system SHALL NOT call embedding provider again

### Requirement: Layer Access Control
The system SHALL enforce layer access based on provided identifiers and tenant context.

#### Scenario: Access layer without required identifier
- **WHEN** attempting to access session layer without sessionId and TenantContext
- **THEN** system SHALL return MISSING_IDENTIFIER error
- **AND** error SHALL specify which identifier is required

#### Scenario: Determine accessible layers from identifiers and tenant context
- **WHEN** providing userId and projectId with TenantContext
- **THEN** system SHALL grant access to: user, project, team, org, company layers within the same tenant
- **AND** system SHALL deny access to: agent, session layers

#### Scenario: Access layer without tenant context
- **WHEN** attempting to access any layer without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error

### Requirement: Memory Error Handling
The system SHALL provide specific error codes for all failure scenarios.

#### Scenario: Invalid layer error
- **WHEN** providing invalid memory layer
- **THEN** system SHALL return INVALID_LAYER error
- **AND** error SHALL be marked as non-retryable

#### Scenario: Content too long error
- **WHEN** providing content exceeding provider limit
- **THEN** system SHALL return CONTENT_TOO_LONG error
- **AND** error SHALL include max length in details

#### Scenario: Provider error with retry
- **WHEN** provider returns transient error
- **THEN** system SHALL return PROVIDER_ERROR
- **AND** error SHALL be marked as retryable
- **AND** system SHALL retry up to 3 times with exponential backoff

#### Scenario: Rate limited error
- **WHEN** provider returns rate limit error
- **THEN** system SHALL return RATE_LIMITED error
- **AND** error SHALL be marked as retryable
- **AND** system SHALL apply appropriate delay before retry

### Requirement: Memory Observability
The system SHALL emit metrics for all memory operations.

#### Scenario: Emit operation metrics
- **WHEN** performing memory operations
- **THEN** system SHALL emit counter: memory.operations.total with operation type label
- **AND** system SHALL emit histogram: memory.operations.latency with operation type label

#### Scenario: Emit search metrics
- **WHEN** performing memory search
- **THEN** system SHALL emit histogram: memory.search.results with layer labels
- **AND** system SHALL emit gauge: memory.storage.size with layer labels

#### Scenario: Emit error metrics
- **WHEN** memory operation fails
- **THEN** system SHALL emit counter: memory.operations.errors with error code label

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

### Requirement: Governance Validation
The system SHALL validate all memory operations against tenant governance policies before execution.

#### Scenario: Validate memory addition against policies
- **WHEN** adding a memory with content that violates a tenant policy
- **THEN** system SHALL reject the operation with POLICY_VIOLATION error
- **AND** error SHALL include which policy was violated

#### Scenario: Validate memory update against policies
- **WHEN** updating a memory with content that violates a tenant policy
- **THEN** system SHALL reject the operation with POLICY_VIOLATION error
- **AND** error SHALL include which policy was violated

#### Scenario: Validate memory search with policy filtering
- **WHEN** searching memories with content that matches policy-filtered terms
- **THEN** system SHALL filter out results that violate tenant policies
- **AND** system SHALL log the filtering action for audit purposes

### Requirement: Tenant Context Propagation
All memory operations SHALL require a TenantContext parameter for tenant isolation and authorization.

#### Scenario: Operation without tenant context
- **WHEN** any memory operation is attempted without TenantContext
- **THEN** system SHALL return MISSING_TENANT_CONTEXT error
- **AND** operation SHALL NOT proceed

#### Scenario: Tenant context validation
- **WHEN** TenantContext contains invalid or expired credentials
- **THEN** system SHALL return INVALID_TENANT_CONTEXT error
- **AND** operation SHALL NOT proceed

### Requirement: Tenant Isolation Enforcement
The system SHALL enforce hard tenant isolation at all memory layers.

#### Scenario: Cross-tenant memory access attempt
- **WHEN** a user from Tenant A attempts to access memory belonging to Tenant B
- **THEN** system SHALL return null or empty results
- **AND** system SHALL NOT reveal that the memory exists in another tenant
- **AND** system SHALL log the attempted cross-tenant access for security audit

#### Scenario: Tenant-specific embedding isolation
- **WHEN** performing vector similarity search
- **THEN** embeddings from other tenants SHALL NOT influence search results
- **AND** vector spaces SHALL be isolated per tenant or globally normalized with tenant filtering

## Overview

The Memory System provides:

- **Semantic storage**: Vector-based content for similarity search
- **Hierarchical scoping**: 7 layers from agent-specific to organization-wide
- **Provider abstraction**: Swap backends without code changes
- **Flexible retrieval**: Query across layers with precedence rules

```
┌─────────────────────────────────────────────────────────────────┐
│                      MEMORY SYSTEM                               │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Memory Manager                         │    │
│  │  • Coordinates all memory operations                     │    │
│  │  • Enforces layer rules                                  │    │
│  │  • Routes to provider adapter                            │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Layer Resolver                         │    │
│  │  • Determines target layers for operations               │    │
│  │  • Applies precedence rules                              │    │
│  │  • Merges results from multiple layers                   │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  Provider Adapter                        │    │
│  │  • Translates to provider-specific API                   │    │
│  │  • Handles connection, auth, retries                     │    │
│  │  • Manages embedding generation                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Provider (Mem0, Letta, etc.)                │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer Hierarchy

### The Seven Layers

Memory is organized into seven hierarchical layers, from most specific to least specific:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  LAYER          SCOPE                    EXAMPLES                │
│  ─────────────────────────────────────────────────────────────  │
│                                                                  │
│  agent    ←── Per-agent instance         Agent-specific learnings│
│    │          (most specific)            Tool preferences        │
│    │                                                             │
│  user          Per-user                  User preferences        │
│    │                                     Communication style     │
│    │                                                             │
│  session       Per-session               Current task context    │
│    │           (conversation)            Recent decisions        │
│    │                                                             │
│  project       Per-project/repo          Project conventions     │
│    │                                     Tech stack choices      │
│    │                                                             │
│  team          Per-team                  Team standards          │
│    │                                     Shared knowledge        │
│    │                                                             │
│  org           Per-organization          Org-wide policies       │
│    │                                     Compliance rules        │
│    │                                                             │
│  company  ←── Per-company/tenant         Company standards       │
│               (least specific)           Global policies         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Identifiers

Each layer requires specific identifiers to scope memory:

```typescript
interface LayerIdentifiers {
  /** Required for agent layer */
  agentId?: string;
  
  /** Required for user layer and below */
  userId?: string;
  
  /** Required for session layer and below */
  sessionId?: string;
  
  /** Required for project layer and below */
  projectId?: string;
  
  /** Required for team layer and below */
  teamId?: string;
  
  /** Required for org layer and below */
  orgId?: string;
  
  /** Required for company layer */
  companyId?: string;
}
```

### Layer Requirements Matrix

| Layer | agentId | userId | sessionId | projectId | teamId | orgId | companyId |
|-------|---------|--------|-----------|-----------|--------|-------|-----------|
| agent | ✓ | ✓ | - | - | - | - | - |
| user | - | ✓ | - | - | - | - | - |
| session | - | ✓ | ✓ | - | - | - | - |
| project | - | - | - | ✓ | - | - | - |
| team | - | - | - | - | ✓ | - | - |
| org | - | - | - | - | - | ✓ | - |
| company | - | - | - | - | - | - | ✓ |

---

## Memory Entry Schema

### Core Schema

```typescript
/**
 * A single memory entry in the system.
 */
interface MemoryEntry {
  /** Unique identifier (provider-generated or UUID) */
  id: string;
  
  /** The memory content (human-readable text) */
  content: string;
  
  /** Layer this memory belongs to */
  layer: MemoryLayer;
  
  /** Layer-specific identifiers */
  identifiers: LayerIdentifiers;
  
  /** Arbitrary metadata */
  metadata: MemoryMetadata;
  
  /** Creation timestamp (ISO 8601) */
  createdAt: string;
  
  /** Last update timestamp (ISO 8601) */
  updatedAt: string;
  
  /** Vector embedding (provider-specific format) */
  embedding?: number[];
}

type MemoryLayer = 
  | 'agent'
  | 'user'
  | 'session'
  | 'project'
  | 'team'
  | 'org'
  | 'company';
```

### Metadata Schema

```typescript
/**
 * Flexible metadata attached to memories.
 */
interface MemoryMetadata {
  /** Optional: Tags for categorization */
  tags?: string[];
  
  /** Optional: Source of this memory */
  source?: MemorySource;
  
  /** Optional: Pointer to knowledge item (see 04-memory-knowledge-sync.md) */
  knowledgePointer?: KnowledgePointer;
  
  /** Optional: Relevance score (0.0 - 1.0) */
  relevance?: number;
  
  /** Optional: Decay factor for aging */
  decayFactor?: number;
  
  /** Custom fields (string keys, JSON-serializable values) */
  [key: string]: unknown;
}

interface MemorySource {
  /** Source type */
  type: 'conversation' | 'tool_result' | 'knowledge_sync' | 'manual' | 'import';
  
  /** Optional: Reference to source (message ID, tool call ID, etc.) */
  reference?: string;
}

interface KnowledgePointer {
  /** Type of knowledge item */
  sourceType: 'adr' | 'policy' | 'pattern' | 'spec';
  
  /** ID of knowledge item */
  sourceId: string;
  
  /** Content hash at sync time */
  contentHash: string;
  
  /** Sync timestamp */
  syncedAt: string;
}
```

### Example Memory Entries

#### Agent-Level Memory

```json
{
  "id": "mem_agent_001",
  "content": "When debugging TypeScript, always check tsconfig.json first",
  "layer": "agent",
  "identifiers": {
    "agentId": "agent_debugger",
    "userId": "user_123"
  },
  "metadata": {
    "tags": ["debugging", "typescript"],
    "source": {
      "type": "conversation",
      "reference": "msg_abc123"
    }
  },
  "createdAt": "2025-01-07T10:30:00Z",
  "updatedAt": "2025-01-07T10:30:00Z"
}
```

#### Project-Level Memory (Knowledge Pointer)

```json
{
  "id": "mem_proj_042",
  "content": "Use PostgreSQL for all new services per ADR-042",
  "layer": "project",
  "identifiers": {
    "projectId": "proj_backend_api"
  },
  "metadata": {
    "tags": ["database", "architecture"],
    "source": {
      "type": "knowledge_sync"
    },
    "knowledgePointer": {
      "sourceType": "adr",
      "sourceId": "adr-042-database-selection",
      "contentHash": "sha256:abc123def456...",
      "syncedAt": "2025-01-07T09:00:00Z"
    }
  },
  "createdAt": "2025-01-07T09:00:00Z",
  "updatedAt": "2025-01-07T09:00:00Z"
}
```

---

## Core Operations

### Operation: Add Memory

Add a new memory entry to a specific layer.

```typescript
interface AddMemoryInput {
  /** Memory content (required) */
  content: string;
  
  /** Target layer (required) */
  layer: MemoryLayer;
  
  /** Layer identifiers (required fields depend on layer) */
  identifiers: LayerIdentifiers;
  
  /** Optional metadata */
  metadata?: Partial<MemoryMetadata>;
}

interface AddMemoryOutput {
  /** Created memory entry */
  memory: MemoryEntry;
  
  /** Whether embedding was generated */
  embeddingGenerated: boolean;
}
```

**Behavior:**

1. Validate `identifiers` contains required fields for `layer`
2. Generate embedding from `content` via provider
3. Persist to provider with layer isolation
4. Return created entry with generated `id`

**Errors:**

| Error | Condition |
|-------|-----------|
| `INVALID_LAYER` | Unknown layer value |
| `MISSING_IDENTIFIER` | Required identifier not provided |
| `CONTENT_TOO_LONG` | Content exceeds provider limit |
| `EMBEDDING_FAILED` | Embedding generation failed |
| `PROVIDER_ERROR` | Provider-specific error |

### Operation: Search Memory

Search for memories semantically matching a query.

```typescript
interface SearchMemoryInput {
  /** Search query (natural language) */
  query: string;
  
  /** Layers to search (default: all accessible layers) */
  layers?: MemoryLayer[];
  
  /** Layer identifiers for scoping */
  identifiers: LayerIdentifiers;
  
  /** Maximum results per layer (default: 10) */
  limit?: number;
  
  /** Minimum similarity threshold (0.0 - 1.0, default: 0.7) */
  threshold?: number;
  
  /** Optional: Filter by metadata */
  filter?: MetadataFilter;
}

interface MetadataFilter {
  /** Match any of these tags */
  tags?: string[];
  
  /** Match source type */
  sourceType?: MemorySource['type'];
  
  /** Only knowledge pointers */
  hasKnowledgePointer?: boolean;
  
  /** Custom field filters */
  custom?: Record<string, unknown>;
}

interface SearchMemoryOutput {
  /** Search results, ordered by relevance */
  results: MemorySearchResult[];
  
  /** Total results before limit */
  totalCount: number;
  
  /** Layers that were searched */
  searchedLayers: MemoryLayer[];
}

interface MemorySearchResult {
  /** The memory entry */
  memory: MemoryEntry;
  
  /** Similarity score (0.0 - 1.0) */
  score: number;
  
  /** Layer this result came from */
  layer: MemoryLayer;
}
```

**Behavior:**

1. Generate embedding for `query`
2. For each layer in `layers`:
   a. Verify `identifiers` provides required fields
   b. Execute vector similarity search
   c. Apply `threshold` filter
   d. Apply `filter` if provided
3. Merge results using layer precedence (see [Layer Resolution](#layer-resolution))
4. Return top `limit` results

**Errors:**

| Error | Condition |
|-------|-----------|
| `INVALID_LAYER` | Unknown layer in `layers` array |
| `MISSING_IDENTIFIER` | Required identifier for layer not provided |
| `QUERY_TOO_LONG` | Query exceeds embedding limit |
| `PROVIDER_ERROR` | Provider-specific error |

### Operation: Get Memory

Retrieve a specific memory by ID.

```typescript
interface GetMemoryInput {
  /** Memory ID */
  id: string;
}

interface GetMemoryOutput {
  /** The memory entry, or null if not found */
  memory: MemoryEntry | null;
}
```

**Behavior:**

1. Look up memory by `id` in provider
2. Return entry or null

### Operation: Update Memory

Update an existing memory's content or metadata.

```typescript
interface UpdateMemoryInput {
  /** Memory ID */
  id: string;
  
  /** New content (optional, triggers re-embedding) */
  content?: string;
  
  /** Metadata updates (merged with existing) */
  metadata?: Partial<MemoryMetadata>;
}

interface UpdateMemoryOutput {
  /** Updated memory entry */
  memory: MemoryEntry;
  
  /** Whether embedding was regenerated */
  embeddingRegenerated: boolean;
}
```

**Behavior:**

1. Fetch existing memory by `id`
2. If `content` changed, regenerate embedding
3. Merge `metadata` with existing (shallow merge)
4. Update `updatedAt` timestamp
5. Persist to provider

**Errors:**

| Error | Condition |
|-------|-----------|
| `MEMORY_NOT_FOUND` | No memory with given ID |
| `CONTENT_TOO_LONG` | New content exceeds limit |
| `EMBEDDING_FAILED` | Re-embedding failed |

### Operation: Delete Memory

Remove a memory from the system.

```typescript
interface DeleteMemoryInput {
  /** Memory ID */
  id: string;
}

interface DeleteMemoryOutput {
  /** Whether deletion succeeded */
  success: boolean;
}
```

**Behavior:**

1. Remove memory from provider
2. Return success status

### Operation: List Memories

List memories in a specific layer with pagination.

```typescript
interface ListMemoriesInput {
  /** Target layer */
  layer: MemoryLayer;
  
  /** Layer identifiers */
  identifiers: LayerIdentifiers;
  
  /** Pagination cursor */
  cursor?: string;
  
  /** Page size (default: 50, max: 100) */
  limit?: number;
  
  /** Optional: Filter by metadata */
  filter?: MetadataFilter;
}

interface ListMemoriesOutput {
  /** Memories in this page */
  memories: MemoryEntry[];
  
  /** Cursor for next page (null if no more) */
  nextCursor: string | null;
  
  /** Total count in layer */
  totalCount: number;
}
```

---

## Layer Resolution

### Precedence Rules

When searching across multiple layers, results are merged using these rules:

```
┌─────────────────────────────────────────────────────────────────┐
│                   LAYER PRECEDENCE                               │
│                                                                  │
│  1. agent    (highest priority - most specific)                 │
│  2. user                                                        │
│  3. session                                                     │
│  4. project                                                     │
│  5. team                                                        │
│  6. org                                                         │
│  7. company  (lowest priority - least specific)                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Merge Algorithm

```typescript
function mergeSearchResults(
  resultsByLayer: Map<MemoryLayer, MemorySearchResult[]>,
  limit: number
): MemorySearchResult[] {
  // 1. Flatten all results
  const allResults: MemorySearchResult[] = [];
  for (const [layer, results] of resultsByLayer) {
    allResults.push(...results);
  }
  
  // 2. Sort by: layer precedence (primary), score (secondary)
  allResults.sort((a, b) => {
    const layerDiff = getLayerPrecedence(a.layer) - getLayerPrecedence(b.layer);
    if (layerDiff !== 0) return layerDiff;
    return b.score - a.score; // Higher score first
  });
  
  // 3. Deduplicate by content similarity (optional)
  const deduped = deduplicateBySimilarity(allResults, 0.95);
  
  // 4. Return top N
  return deduped.slice(0, limit);
}

function getLayerPrecedence(layer: MemoryLayer): number {
  const precedence: Record<MemoryLayer, number> = {
    agent: 1,
    user: 2,
    session: 3,
    project: 4,
    team: 5,
    org: 6,
    company: 7
  };
  return precedence[layer];
}
```

### Override Behavior

More specific layers **override** less specific layers when content conflicts:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  company layer: "Use spaces for indentation"                    │
│                              │                                   │
│                              ▼                                   │
│  project layer: "Use tabs for indentation"  ◄── WINS            │
│                                                                  │
│  Result: Agent uses tabs (project overrides company)            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Access Control

Layers are only searchable if appropriate identifiers are provided:

```typescript
function getAccessibleLayers(identifiers: LayerIdentifiers): MemoryLayer[] {
  const layers: MemoryLayer[] = [];
  
  // Always accessible with company ID
  if (identifiers.companyId) layers.push('company');
  
  // Org requires org ID
  if (identifiers.orgId) layers.push('org');
  
  // Team requires team ID
  if (identifiers.teamId) layers.push('team');
  
  // Project requires project ID
  if (identifiers.projectId) layers.push('project');
  
  // Session requires user + session ID
  if (identifiers.userId && identifiers.sessionId) layers.push('session');
  
  // User requires user ID
  if (identifiers.userId) layers.push('user');
  
  // Agent requires agent + user ID
  if (identifiers.agentId && identifiers.userId) layers.push('agent');
  
  return layers;
}
```

---

## Provider Adapter Interface

### Interface Definition

All memory providers must implement this interface:

```typescript
/**
 * Memory provider adapter interface.
 * Implement this to add support for a new storage backend.
 */
interface MemoryProviderAdapter {
  /** Provider name (e.g., "mem0", "letta", "chroma") */
  readonly name: string;
  
  /** Provider version */
  readonly version: string;
  
  /** Initialize the provider connection */
  initialize(config: ProviderConfig): Promise<void>;
  
  /** Clean up resources */
  shutdown(): Promise<void>;
  
  /** Health check */
  healthCheck(): Promise<HealthCheckResult>;
  
  // Core operations
  add(input: AddMemoryInput): Promise<AddMemoryOutput>;
  search(input: SearchMemoryInput): Promise<SearchMemoryOutput>;
  get(input: GetMemoryInput): Promise<GetMemoryOutput>;
  update(input: UpdateMemoryInput): Promise<UpdateMemoryOutput>;
  delete(input: DeleteMemoryInput): Promise<DeleteMemoryOutput>;
  list(input: ListMemoriesInput): Promise<ListMemoriesOutput>;
  
  // Embedding operations
  generateEmbedding(content: string): Promise<number[]>;
  
  // Bulk operations (optional)
  bulkAdd?(inputs: AddMemoryInput[]): Promise<AddMemoryOutput[]>;
  bulkDelete?(ids: string[]): Promise<{ deleted: number; failed: string[] }>;
}

interface ProviderConfig {
  /** Provider-specific configuration */
  [key: string]: unknown;
}

interface HealthCheckResult {
  /** Overall health status */
  status: 'healthy' | 'degraded' | 'unhealthy';
  
  /** Latency in milliseconds */
  latencyMs: number;
  
  /** Optional: Detailed component health */
  components?: Record<string, {
    status: 'healthy' | 'degraded' | 'unhealthy';
    message?: string;
  }>;
}
```

### Layer Isolation

Providers MUST ensure layer isolation. Implementation strategies:

#### Strategy 1: Namespace by Layer

```
Collection: memories_agent_{agentId}_{userId}
Collection: memories_user_{userId}
Collection: memories_session_{userId}_{sessionId}
Collection: memories_project_{projectId}
...
```

#### Strategy 2: Metadata Filtering

```json
{
  "content": "...",
  "metadata": {
    "_layer": "project",
    "_projectId": "proj_123"
  }
}
```

Query with filter: `metadata._layer == "project" AND metadata._projectId == "proj_123"`

#### Strategy 3: Tenant Partitioning

Use provider's native multi-tenancy:
- Qdrant: Separate collections per layer
- Pinecone: Namespaces per layer
- Chroma: Collections per layer

---

## Memory Lifecycle

### State Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│                        ┌─────────┐                              │
│                        │ CREATED │                              │
│                        └────┬────┘                              │
│                             │                                    │
│                             ▼                                    │
│                        ┌─────────┐                              │
│              ┌────────►│ ACTIVE  │◄────────┐                    │
│              │         └────┬────┘         │                    │
│              │              │              │                    │
│              │    ┌─────────┴─────────┐    │                    │
│              │    │                   │    │                    │
│              │    ▼                   ▼    │                    │
│         ┌────┴────┐             ┌─────────┐                     │
│         │ UPDATED │             │ DECAYED │                     │
│         └─────────┘             └────┬────┘                     │
│                                      │                          │
│                                      ▼                          │
│                                ┌──────────┐                     │
│                                │ ARCHIVED │                     │
│                                └────┬─────┘                     │
│                                     │                           │
│                                     ▼                           │
│                                ┌─────────┐                      │
│                                │ DELETED │                      │
│                                └─────────┘                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Memory Decay (Optional)

Providers MAY support memory decay to reduce old memory relevance:

```typescript
interface DecayConfig {
  /** Enable decay */
  enabled: boolean;
  
  /** Decay rate per day (0.0 - 1.0) */
  ratePerDay: number;
  
  /** Minimum relevance before archival */
  archiveThreshold: number;
  
  /** Layers exempt from decay */
  exemptLayers: MemoryLayer[];
}
```

**Decay Formula:**

```
relevance(t) = initial_relevance * (1 - rate)^days_since_creation
```

### Memory Consolidation (Optional)

Providers MAY support consolidation to merge similar memories:

```typescript
interface ConsolidationConfig {
  /** Enable consolidation */
  enabled: boolean;
  
  /** Similarity threshold for merging (0.0 - 1.0) */
  similarityThreshold: number;
  
  /** Maximum memories before triggering consolidation */
  maxMemoriesBeforeTrigger: number;
  
  /** Layers to consolidate */
  targetLayers: MemoryLayer[];
}
```

### Session Memory Cleanup

Session-layer memories have special lifecycle:

```typescript
interface SessionCleanupConfig {
  /** Auto-delete session memories after session ends */
  autoDelete: boolean;
  
  /** Retention period after session end (e.g., "7d", "30d") */
  retentionPeriod?: string;
  
  /** Promote important memories to user layer */
  promoteImportant: boolean;
  
  /** Threshold for promotion (0.0 - 1.0) */
  promotionThreshold?: number;
}
```

---

## Error Handling

### Error Response Format

```typescript
interface MemoryError {
  /** Error code */
  code: MemoryErrorCode;
  
  /** Human-readable message */
  message: string;
  
  /** Operation that failed */
  operation: string;
  
  /** Additional context */
  details?: Record<string, unknown>;
  
  /** Whether operation can be retried */
  retryable: boolean;
}

type MemoryErrorCode =
  | 'INVALID_LAYER'
  | 'MISSING_IDENTIFIER'
  | 'MEMORY_NOT_FOUND'
  | 'CONTENT_TOO_LONG'
  | 'QUERY_TOO_LONG'
  | 'EMBEDDING_FAILED'
  | 'PROVIDER_ERROR'
  | 'RATE_LIMITED'
  | 'UNAUTHORIZED'
  | 'CONFIGURATION_ERROR';
```

### Error Handling Guidelines

| Error Code | Recommended Action |
|------------|-------------------|
| `INVALID_LAYER` | Fix input, do not retry |
| `MISSING_IDENTIFIER` | Add required identifier, do not retry |
| `MEMORY_NOT_FOUND` | Check ID, may be deleted |
| `CONTENT_TOO_LONG` | Truncate or split content |
| `QUERY_TOO_LONG` | Shorten query |
| `EMBEDDING_FAILED` | Retry with backoff |
| `PROVIDER_ERROR` | Retry with backoff, check provider status |
| `RATE_LIMITED` | Retry after delay (use `Retry-After` if provided) |
| `UNAUTHORIZED` | Check credentials, do not retry |
| `CONFIGURATION_ERROR` | Fix configuration, do not retry |

### Retry Strategy

```typescript
interface RetryConfig {
  /** Maximum retry attempts */
  maxAttempts: number;
  
  /** Initial delay in milliseconds */
  initialDelayMs: number;
  
  /** Maximum delay in milliseconds */
  maxDelayMs: number;
  
  /** Backoff multiplier */
  backoffMultiplier: number;
  
  /** Error codes to retry */
  retryableCodes: MemoryErrorCode[];
}

const defaultRetryConfig: RetryConfig = {
  maxAttempts: 3,
  initialDelayMs: 1000,
  maxDelayMs: 30000,
  backoffMultiplier: 2,
  retryableCodes: ['EMBEDDING_FAILED', 'PROVIDER_ERROR', 'RATE_LIMITED']
};
```

---

## Implementation Notes

### Thread Safety

Memory operations MUST be thread-safe. Implementations should:

1. Use atomic operations where possible
2. Implement optimistic locking for updates
3. Handle concurrent access to same memory gracefully

### Caching

Implementations MAY cache:

- Embeddings for frequently accessed content
- Layer resolution results
- Provider connection metadata

Implementations MUST NOT cache:

- Memory content (may be stale)
- Search results (query-dependent)

### Observability

Implementations SHOULD emit metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `memory.operations.total` | Counter | Total operations by type |
| `memory.operations.errors` | Counter | Failed operations by error code |
| `memory.operations.latency` | Histogram | Operation latency in ms |
| `memory.search.results` | Histogram | Number of results per search |
| `memory.storage.size` | Gauge | Total memories by layer |

---

**Next**: [03-knowledge-repository.md](./03-knowledge-repository.md) - Knowledge Repository Specification
