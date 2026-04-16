## 0. Storage Trait Pagination Primitives

- [x] 0.1 Define `PaginationParams { limit: usize, offset: usize }` in `mk_core/src/pagination.rs` with `Default` impl (`limit=50, offset=0`), capped at MAX_LIMIT=200
- [x] 0.2 Define `PaginatedResult<T> { items: Vec<T>, total: Option<u64> }` in `mk_core/src/pagination.rs` with `map()`, `with_total()`, `without_total()` helpers
- [x] 0.3 Update `StorageBackend` trait: add `list_units_paginated` with default impl calling `list_all_units()` + Rust-side filtering
- [x] 0.4 Implement `list_tenants_paginated` in `storage/src/tenant_store.rs` with SQL LIMIT/OFFSET + COUNT(*)
- [x] 0.5 Implement `list_audit_logs_paginated` and `list_roles_paginated` in `storage/src/governance.rs` with SQL LIMIT/OFFSET + COUNT(*)
- [x] 0.6 Implement `list_units_paginated` override in `storage/src/postgres.rs` with SQL LIMIT/OFFSET + COUNT(*) + tenant_id WHERE clause
- [x] 0.7 Implement `list_paginated<T>` in `storage/src/redis_store.rs` using SSCAN cursor iteration + deduplication
- [x] 0.8 Re-export primitives from `storage/src/pagination.rs` and `cli/src/server/pagination.rs`
- [x] 0.9 Unit tests for PaginationParams (default, cap, sql_fragment), PaginatedResult (map, with/without_total), ApiPaginationParams (defaults, cap), PaginatedResponse (camelCase serialization, from_result)

## 1. Shared API Pagination Primitives

- [x] 1.1 Create `cli/src/server/pagination.rs` with `ApiPaginationParams` (Query extractor, limit default 50 max 200, offset default 0)
- [x] 1.2 Create `PaginatedResponse<T: Serialize>` with `items`, `total: Option<u64>`, `limit`, `offset`; `#[serde(rename_all = "camelCase")]`; `from_result()` and `new()` constructors
- [x] 1.3 SQL helper: `PaginationParams::sql_fragment()` appends `LIMIT $N OFFSET $M` with bind index tracking
- [x] 1.4 Module declared and re-exported in both `storage/src/lib.rs` and `cli/src/server/mod.rs`
- [x] 1.5 Unit tests for API pagination (defaults, cap, serialization)
- [x] 1.6 Unit tests for PaginatedResponse serialization (camelCase keys, from_result)

## 2. Serde Standardization (BREAKING)

- [x] 2.1 Add `#[serde(rename_all = "camelCase")]` to `ApprovalRequest` in `storage/src/governance.rs`
- [x] 2.2 Add `#[serde(rename_all = "camelCase")]` to `GovernanceAuditEntry` in `storage/src/governance.rs`
- [x] 2.3 Add `#[serde(rename_all = "camelCase")]` to `GovernanceRole` in `storage/src/governance.rs`
- [x] 2.4 `mk_core/src/types.rs` already has `rename_all = "camelCase"` on 60+ structs including `KnowledgeEntry`, `TenantRecord`, `OrganizationalUnit`, `MemoryEntry`, etc.
- [x] 2.5 Audit `RemediationRequest` and lifecycle types for missing camelCase — verify all response types are covered
- [x] 2.6 Update CLI display commands (`aeterna query knowledge`, `aeterna govern list`, `aeterna govern audit`) to handle camelCase field names if they deserialize JSON
- [x] 2.7 Add BREAKING CHANGE entry to CHANGELOG.md documenting the wire format change for governance types

## 3. Backend Response Shape Fixes

- [x] 3.1 Add `GET /api/v1/knowledge/{id}` handler (`get_by_id_handler`) returning full `KnowledgeItem` with content, metadata, tags, variant_role, layer
- [x] 3.2 Replace `find_entry_by_id` 4-layer sequential `list()` scan with a direct `WHERE path = $1` SQL query
- [x] 3.3 Add `POST /api/v1/memory/{id}/feedback` route alias (`feedback_by_id_handler`) that extracts ID from path and delegates to existing handler; made `memory_id` field default-able
- [x] 3.4 Fix `MemorySearchResponse.total` to return grand total from `COUNT(*)`, not `items.len()`
- [x] 3.5 Fix `KnowledgeQueryResponse.total` to return grand total from `COUNT(*)`, not `items.len()`
- [x] 3.6 Change `GET /api/v1/govern/policies` to return paginated envelope with `items` key (not `policies` key)
- [x] 3.7 `GET /api/v1/govern/audit` already returns `{ items, total, limit, offset }` envelope via `list_audit_logs_paginated`
- [x] 3.8 Write integration tests for `GET /knowledge/{id}` endpoint (200 found, 404 not found)
- [x] 3.9 Write integration tests for `POST /memory/{id}/feedback` route alias

## 4. Storage-Layer Fixes — Critical Unbounded Functions

- [x] 4.1 `list_units_paginated` (postgres.rs) — SQL `WHERE tenant_id = $1` + optional `unit_type` filter + `LIMIT/OFFSET` + `COUNT(*)`
- [x] 4.2 `list_tenants_paginated` (tenant_store.rs) — SQL `LIMIT/OFFSET` + `COUNT(*)`
- [x] 4.3 Fix `list_repositories` (repo_manager.rs:285) — add `LIMIT/OFFSET` to SQL
- [x] 4.4 Fix `list_identities` (repo_manager.rs:1531) — add `LIMIT/OFFSET` to SQL
- [x] 4.5 `list_roles_paginated` (governance.rs) — SQL `LIMIT/OFFSET` + `COUNT(*)`
- [x] 4.6 Fix `list_children` (postgres.rs:962) — add `LIMIT/OFFSET` to SQL
- [x] 4.7 Fix `list_unit_roles` (postgres.rs:1277) — add `LIMIT/OFFSET` to SQL
- [x] 4.8 Fix `list_unit_members` (postgres.rs:1957) and `list_project_team_assignments` (postgres.rs:2011) — add `LIMIT/OFFSET`
- [x] 4.9 Fix `get_all_layer_usage` (budget_storage.rs:230) — add `LIMIT` clause
- [x] 4.10 Fix `list_suppressions` (postgres.rs:2266) — add `LIMIT/OFFSET`
- [x] 4.11 Write unit tests for each fixed storage function

## 4b. Storage-Layer Fixes — Redis SSCAN Migration

- [x] 4b.1 `RedisStore::list_paginated<T>` uses `SSCAN` cursor iteration instead of `SMEMBERS`
- [x] 4b.2 Deduplication via `HashSet<String>` for SSCAN batches
- [x] 4b.3 Update `RedisGitProviderConnectionStore::list_connections` to use `list_paginated`
- [x] 4b.4 Update Redis `list_all_units` stub — implement or document as unsupported
- [x] 4b.5 Write integration tests for SSCAN pagination

## 4c. Storage-Layer Fixes — S3 and Memory Provider

- [x] 4c.1 Fix `list_snapshots` (graph_duckdb.rs:1851) — add `continuation_token` loop; cap at 10 pages
- [x] 4c.2 Verify Qdrant provider pushes `limit` into scroll/search request
- [x] 4c.3 Verify pgvector provider includes `LIMIT` in SQL
- [x] 4c.4 Replace hard-coded `limit=1000` in `list_all_from_layer` (memory/manager.rs:1059) with caller-provided limit
- [x] 4c.5 Write integration tests for S3 continuation token handling

## 5. API-Layer Pagination — Critical Unbounded Endpoints

- [x] 5.1 `GET /api/v1/user` — `PaginationParams` + COUNT(*) + LIMIT/OFFSET SQL + envelope response
- [x] 5.2 `GET /api/v1/org` — wired to `list_units_paginated` with `unit_type="organization"` + envelope response
- [x] 5.3 `GET /api/v1/team` — wired to `list_units_paginated` with `unit_type="team"` + envelope response
- [x] 5.4 `GET /api/v1/project` — wired to `list_units_paginated` with `unit_type="project"` + envelope response
- [x] 5.5 `GET /api/v1/roles/grants` — wire to paginated storage; fix N+1 unit type lookups
- [x] 5.6 `POST /api/v1/memory/list` — wire to updated `list_all_from_layer` with pagination
- [x] 5.7 `GET /api/v1/admin/hierarchy` — uses `list_units_paginated` (same as org/team/project pattern)
- [x] 5.8 Write integration tests for each paginated API endpoint

## 6. API-Layer Pagination — High Priority Endpoints

- [x] 6.1 `GET /api/v1/govern/audit` — `PaginationParams` + `list_audit_logs_paginated` + envelope
- [x] 6.2 `GET /api/v1/govern/roles` — `ApiPaginationParams` + `list_roles_paginated` + envelope
- [x] 6.3 `GET /api/v1/govern/policies` — returns `{ items, total, limit, offset }` envelope
- [x] 6.4 `GET /api/v1/knowledge/promotions` — add `PaginationParams` to `PromotionsQuery`; wire to storage
- [x] 6.5 `GET /api/v1/admin/tenants` — wired to `list_tenants_paginated` + envelope response
- [x] 6.6 `GET /api/v1/admin/git-provider-connections` — add `PaginationParams`; wire to SSCAN-based list

## 7. API-Layer Pagination — Medium Priority (Offset/Cursor Additions)

- [x] 7.1 `GET /api/v1/govern/pending` — replace hardcoded `limit: Some(100)` with `PaginationParams`; add offset; return envelope
- [x] 7.2 `POST /api/v1/knowledge/query` — add `offset` field to request body; fix `total` to be grand total
- [x] 7.3 `POST /api/v1/memory/search` — add `offset` field to request body; fix `total` to be grand total
- [x] 7.4 `GET /api/v1/admin/exports` — add `PaginationParams`
- [x] 7.5 `GET /api/v1/admin/imports` — add `PaginationParams`
- [x] 7.6 `GET /api/v1/sync/pull` — add max cap of 500 to existing limit parameter

## 8. Admin UI TypeScript Fixes

- [x] 8.1 Update `admin-ui/src/api/types.ts` — fix `GovernanceAuditEntry` to use camelCase fields
- [x] 8.2 Update `admin-ui/src/api/types.ts` — fix `KnowledgeItem` to match backend fields
- [x] 8.3 Update `admin-ui/src/api/types.ts` — fix `UserRole` to use `{ role, scope, unitId }`
- [x] 8.4 Update `admin-ui/src/api/types.ts` — add `PaginatedResponse<T>` generic type
- [x] 8.5 Fix `TenantDetailPage` — unwrap `response.tenant` from envelope
- [x] 8.6 Fix `AuditLogPage` — consume `PaginatedResponse<GovernanceAuditEntry>`
- [x] 8.7 Fix `PolicyListPage` — consume `PaginatedResponse<PolicyRecord>` via `data.items`
- [x] 8.8 Fix `LifecyclePage` — consume `{ items, count }` wrapper; use camelCase fields
- [x] 8.9 Fix `UserDetailPage` — use `scope`/`unitId`; send `{ role, scope }` for grant-role
- [x] 8.10 Fix `KnowledgeSearchPage` — display actual `KnowledgeItem` fields
- [x] 8.11 Fix `MemorySearchPage` — call `POST /memory/{id}/feedback` with correct body
- [x] 8.12 Fix `KnowledgeDetailPage` — use `GET /knowledge/{id}` endpoint

## 9. Admin UI Pagination Controls

- [x] 9.1 Create shared `<PaginationBar>` component
- [x] 9.2 Add pagination to `UserListPage`
- [x] 9.3 Add pagination to `AuditLogPage`
- [x] 9.4 Add pagination to `PolicyListPage`
- [x] 9.5 Add pagination to `TenantListPage`
- [x] 9.6 Add pagination to `KnowledgeSearchPage` results
- [x] 9.7 Add pagination to `MemorySearchPage` results
- [x] 9.8 Add pagination to org/team/project list pages

## 10. OpenAPI Spec & Validation

- [x] 10.1 Author `aeterna-openapi-spec.yaml` (OpenAPI 3.1) covering all endpoints
- [x] 10.2 Add CI step to validate `aeterna-openapi-spec.yaml` syntax
- [x] 10.3 Add integration test that validates response shapes against spec
- [x] 10.4 Add migration notes to README and CHANGELOG
