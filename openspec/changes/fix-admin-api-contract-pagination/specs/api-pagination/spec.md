## ADDED Requirements

### Requirement: Paginated Response Envelope
The system SHALL provide a standard `PaginatedResponse<T>` envelope for all list endpoints. The envelope MUST contain `items` (array of results), `total` (grand total count from database, not `items.len()`), `limit` (page size used), and `offset` (starting position used).

#### Scenario: Paginated list returns envelope with correct total
- **WHEN** a client calls `GET /api/v1/user?limit=10&offset=0` and there are 250 users
- **THEN** the response MUST be `{ "items": [...10 users...], "total": 250, "limit": 10, "offset": 0 }`
- **AND** `items.length` MUST be 10
- **AND** `total` MUST be 250 (the grand total, not 10)

#### Scenario: Paginated list with offset beyond total
- **WHEN** a client calls `GET /api/v1/user?limit=10&offset=300` and there are 250 users
- **THEN** the response MUST be `{ "items": [], "total": 250, "limit": 10, "offset": 300 }`

### Requirement: Pagination Query Parameters
All list endpoints MUST accept `limit` and `offset` query parameters via a shared `PaginationParams` extractor.

#### Scenario: Default pagination when no params provided
- **WHEN** a client calls a list endpoint without `limit` or `offset`
- **THEN** the system MUST default to `limit=50` and `offset=0`

#### Scenario: Limit cap enforcement
- **WHEN** a client provides `limit=5000`
- **THEN** the system MUST cap `limit` to 200
- **AND** return the capped value in the response `limit` field

#### Scenario: Invalid offset rejected
- **WHEN** a client provides `offset=-1`
- **THEN** the system MUST return HTTP 400 with a descriptive error

### Requirement: SQL-Level Pagination
All list endpoints MUST apply LIMIT and OFFSET at the SQL query level, not by fetching all rows and truncating in Rust.

#### Scenario: Database query includes LIMIT/OFFSET
- **WHEN** a list endpoint is called with `limit=20&offset=40`
- **THEN** the SQL query MUST include `LIMIT 20 OFFSET 40`
- **AND** a separate `SELECT COUNT(*)` query (or window function) MUST provide the grand total

#### Scenario: Memory list pushes limit to storage layer
- **WHEN** `POST /api/v1/memory/list` is called with `{ "layer": "User", "limit": 20 }`
- **THEN** the storage layer MUST query with `LIMIT 20`, not fetch all entries and truncate in Rust

### Requirement: Sync Pull Max Cap
The `GET /api/v1/sync/pull` endpoint MUST enforce a maximum limit cap to prevent abuse.

#### Scenario: Sync pull limit capped
- **WHEN** a client calls `GET /api/v1/sync/pull?limit=99999`
- **THEN** the system MUST cap the limit to 500
- **AND** return at most 500 items
