## ADDED Requirements

### Requirement: Tenant List Pagination
The `GET /api/v1/admin/tenants` endpoint MUST accept `limit` and `offset` query parameters and return a `PaginatedResponse` envelope.

#### Scenario: Paginated tenant list
- **WHEN** a client calls `GET /api/v1/admin/tenants?limit=20&offset=0`
- **THEN** the response MUST be `{ "items": [...tenants...], "total": N, "limit": 20, "offset": 0 }`
- **AND** the previous `{ "success": true, "tenants": [...] }` wrapper MUST be replaced with the standard paginated envelope

### Requirement: Show Tenant Response Contract
The `GET /api/v1/admin/tenants/{tenant}` endpoint returns `{ "success": true, "tenant": TenantRecord }`. Clients MUST unwrap the `tenant` field.

#### Scenario: Frontend correctly unwraps tenant detail
- **WHEN** the admin UI calls `GET /api/v1/admin/tenants/acme-corp`
- **THEN** the response is `{ "success": true, "tenant": { "id": "...", "slug": "acme-corp", "name": "Acme", ... } }`
- **AND** the admin UI MUST access `response.tenant.slug`, not `response.slug`

### Requirement: Hierarchy Endpoints Pagination
The `GET /api/v1/admin/hierarchy` and related endpoints MUST use SQL-level tenant-scoped queries with LIMIT/OFFSET instead of `list_all_units()` with Rust-side filtering.

#### Scenario: Hierarchy list is SQL-bounded
- **WHEN** a client calls `GET /api/v1/admin/hierarchy?limit=50`
- **THEN** the SQL query MUST include `WHERE tenant_id = $1 LIMIT 50`
- **AND** MUST NOT load all units across all tenants into memory

### Requirement: Git Provider Connections Pagination
The `GET /api/v1/admin/git-provider-connections` endpoint MUST accept pagination parameters.

#### Scenario: Paginated git connections
- **WHEN** a client calls `GET /api/v1/admin/git-provider-connections?limit=20`
- **THEN** the response MUST return at most 20 connections in a `PaginatedResponse` envelope
