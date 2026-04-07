## ADDED Requirements

### Requirement: PostgreSQL Tenant Isolation Enforcement
The PostgreSQL storage layer SHALL enforce tenant isolation for all tenant-scoped tables using database-enforced controls.

#### Scenario: Tenant-scoped governance table protected by RLS
- **WHEN** a tenant-scoped governance, administration, or control-plane table is created or migrated in PostgreSQL
- **THEN** row-level security SHALL be enabled for that table
- **AND** the active tenant session context SHALL be required for queries against that table

#### Scenario: Malformed query path cannot cross tenants
- **WHEN** an application query path omits an expected tenant filter for a tenant-scoped PostgreSQL table
- **THEN** the database's tenant isolation policy SHALL still prevent cross-tenant reads or writes
- **AND** the operation SHALL fail rather than returning another tenant's rows

### Requirement: Qdrant Tenant Isolation Enforcement
The Qdrant storage layer SHALL enforce tenant isolation through explicit tenant-scoped routing.

#### Scenario: Upsert vector uses tenant-scoped routing
- **WHEN** the system stores a tenant-scoped vector in Qdrant
- **THEN** it SHALL place the vector into the tenant's scoped collection, apply the tenant payload metadata, or both according to the configured isolation strategy
- **AND** later queries for other tenants SHALL NOT see that vector

#### Scenario: Search query without tenant scoping is rejected
- **WHEN** a Qdrant search path is invoked without the tenant-scoping information required by the configured isolation strategy
- **THEN** the storage layer SHALL reject the request
- **AND** it SHALL NOT issue a cross-tenant search against the shared backend

### Requirement: Redis Tenant Isolation Enforcement
The Redis storage layer SHALL isolate tenant-scoped data through namespaced keys and controlled access wrappers.

#### Scenario: Tenant-scoped key namespace
- **WHEN** the system stores tenant-scoped Redis data such as working memory, session state, checkpoints, streams, or caches
- **THEN** the key names SHALL include the tenant namespace defined by the storage layer
- **AND** the storage API SHALL construct those keys centrally rather than requiring callers to concatenate tenant prefixes manually

#### Scenario: Cross-tenant key access attempt
- **WHEN** a caller attempts to read or mutate Redis data for a different tenant without an explicit authorized tenant context
- **THEN** the storage layer SHALL reject the request or resolve only the caller's authorized tenant namespace
- **AND** the operation SHALL NOT return another tenant's data
