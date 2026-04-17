## ADDED Requirements

### Requirement: Storage Trait Pagination Primitives
The storage crate MUST define `PaginationParams { limit: usize, offset: usize }` and `PaginatedResult<T> { items: Vec<T>, total: Option<u64> }` types. All `list_*` trait methods MUST accept `PaginationParams` and return `PaginatedResult<T>`.

#### Scenario: Storage trait enforces pagination contract
- **WHEN** a new storage backend implements a `list_*` trait method
- **THEN** the method signature MUST include `params: PaginationParams` as a parameter
- **AND** the return type MUST be `PaginatedResult<T>`
- **AND** the implementation MUST apply `params.limit` and `params.offset` at the query level (SQL LIMIT/OFFSET, Redis SSCAN count, or S3 MaxKeys)

#### Scenario: Default pagination values preserve backward compatibility
- **WHEN** an existing caller does not provide pagination parameters
- **THEN** `PaginationParams::default()` MUST use `limit=1000` and `offset=0`
- **AND** existing behavior is preserved without code changes at call sites

### Requirement: PostgreSQL list_all_units Tenant Scoping
The `list_all_units` function (postgres.rs:2203) MUST add a `WHERE tenant_id = $1` clause and accept `PaginationParams`.

#### Scenario: list_all_units scoped to tenant with pagination
- **WHEN** `list_all_units` is called with `tenant_id = "acme"` and `PaginationParams { limit: 50, offset: 0 }`
- **THEN** the SQL query MUST include `WHERE tenant_id = $1 LIMIT 50 OFFSET 0`
- **AND** MUST NOT return units belonging to other tenants
- **AND** MUST return a `PaginatedResult` with `total` reflecting the count for that tenant only

### Requirement: PostgreSQL list_tenants Pagination
The `list_tenants` function (tenant_store.rs:163) MUST accept `PaginationParams` and add `LIMIT/OFFSET` to the SQL query.

#### Scenario: list_tenants with pagination
- **WHEN** `list_tenants` is called with `PaginationParams { limit: 20, offset: 40 }`
- **THEN** the SQL query MUST include `LIMIT 20 OFFSET 40`
- **AND** `total` MUST reflect the total number of tenants via `COUNT(*)`

### Requirement: Repository Manager Pagination
The `list_repositories` (repo_manager.rs:285) and `list_identities` (repo_manager.rs:1531) functions MUST accept `PaginationParams`.

#### Scenario: list_repositories bounded per tenant
- **WHEN** `list_repositories` is called for a tenant with 10,000 indexed repos and `PaginationParams { limit: 100, offset: 0 }`
- **THEN** the SQL query MUST include `LIMIT 100 OFFSET 0`
- **AND** MUST NOT load all 10,000 repositories into memory

### Requirement: Governance list_roles Pagination
The `list_roles` function (governance.rs:995) MUST accept `PaginationParams` and apply `LIMIT/OFFSET` at the SQL level.

#### Scenario: list_roles bounded query
- **WHEN** `list_roles` is called with `PaginationParams { limit: 50, offset: 0 }`
- **THEN** the SQL query MUST include `LIMIT 50 OFFSET 0`
- **AND** the query MUST filter by `revoked_at IS NULL` (active roles only) unless explicitly including revoked

### Requirement: Redis SSCAN Migration
The `RedisStore::list_all<T>` function (redis_store.rs:93) MUST replace `SMEMBERS` with `SSCAN` cursor-based iteration. The method MUST accept `PaginationParams` and return results within the requested limit.

#### Scenario: Redis list_all uses SSCAN
- **WHEN** `list_all` is called with `PaginationParams { limit: 50, offset: 0 }`
- **THEN** the implementation MUST use `SSCAN` with `COUNT` hint (not `SMEMBERS`)
- **AND** MUST NOT block the Redis event loop by loading the entire set
- **AND** results MUST be deduplicated (SSCAN may return duplicates across cursor iterations)

#### Scenario: Git provider connections use SSCAN
- **WHEN** `RedisGitProviderConnectionStore::list_connections` (git_provider_connection_store.rs:217) is called
- **THEN** it MUST delegate to the SSCAN-based `list_all` with pagination parameters

### Requirement: S3 list_snapshots Continuation Token
The `list_snapshots` function (graph_duckdb.rs:1851) MUST loop on the S3 `continuation_token` until `is_truncated == false` or a maximum page cap is reached.

#### Scenario: S3 listing with >1000 snapshots
- **WHEN** a tenant has 2,500 graph snapshots in S3
- **THEN** `list_snapshots` MUST issue 3 `ListObjectsV2` requests (1000 + 1000 + 500) using `continuation_token`
- **AND** MUST return all 2,500 snapshots

#### Scenario: S3 listing capped at max pages
- **WHEN** a tenant has >10,000 graph snapshots
- **THEN** `list_snapshots` MUST stop after 10 pages (10,000 items)
- **AND** MUST log a warning that results are truncated

### Requirement: Memory Manager list_all_from_layer SQL Enforcement
The `list_all_from_layer` function (memory/manager.rs:1059) MUST verify that the underlying Qdrant provider enforces the limit at its query level, not just at the application level.

#### Scenario: Provider enforces limit at query level
- **WHEN** `list_all_from_layer` is called with `limit=100`
- **THEN** the Qdrant provider MUST use `limit: 100` in the scroll/search request
- **AND** the application-level hard-coded `limit=1000` cap MUST be replaced with the caller-provided limit

### Requirement: Budget Storage Pagination
The `get_all_layer_usage` function (budget_storage.rs:230) MUST accept `PaginationParams` or enforce a reasonable default limit.

#### Scenario: Layer usage bounded query
- **WHEN** `get_all_layer_usage` is called for a tenant with high usage volume
- **THEN** the SQL query MUST include a LIMIT clause
- **AND** MUST NOT fetch all rows for the tenant+window_type combination unbounded
