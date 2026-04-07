## MODIFIED Requirements

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

#### Scenario: Tenant-scoped runtime connection
- **WHEN** PostgreSQL queries execute against tenant-scoped tables in runtime hot paths
- **THEN** the runtime SHALL activate the database tenant context required by row-level security policies before executing the query
- **AND** queries SHALL continue to include explicit tenant filters as an application-layer defense in depth
