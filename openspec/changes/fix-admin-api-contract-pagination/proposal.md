## Why

The admin UI is broken across 6 of 10 page areas due to systematic mismatches between the frontend TypeScript interfaces and the actual Rust API response shapes. Additionally, a comprehensive pagination audit reveals that 15 of 32 list endpoints perform unbounded full-table scans with no limit, offset, or cursor support, posing a production scalability risk as tenant/user/knowledge/memory data grows. These two categories of defects share a root cause: the API surface was never formally specified, so frontend and backend evolved independently without a contract.

## What Changes

### Response contract alignment (11 bugs)

- **Fix TenantDetailPage response unwrapping**: `show_tenant` returns `{ success, tenant }` but frontend treats it as a flat `TenantRecord`. All detail tabs (config, providers, repository-binding) pass `undefined` slug.
- **Fix AuditLogPage double mismatch**: Backend returns a plain array; frontend expects `{ items }`. Field names also diverge (`actor_email`/`target_type`/`created_at` vs `actor`/`resource_type`/`timestamp`).
- **Fix PolicyListPage wrapper key**: Backend returns `{ policies }`, frontend unwraps `items` or plain array.
- **Fix LifecyclePage remediation list shape**: Backend returns `{ items, count }`, frontend expects a plain `RemediationRequest[]`.
- **Fix LifecyclePage field naming**: Backend serializes with camelCase (`requestType`, `riskTier`), frontend interface uses snake_case (`request_type`, `risk_tier`).
- **Fix LifecyclePage status shape**: Backend returns `{ lifecycle_manager, tasks: [...] }`, frontend expects `{ enabled, tasks: Record<...>, remediation_summary }`.
- **Fix UserDetailPage role field names**: Backend returns `{ role, scope, unitId }` (camelCase), frontend expects `{ role, resource_type, resource_id }` (snake_case).
- **Fix UserDetailPage grant-role request body**: Frontend sends `{ role, resource_type, resource_id }`, backend expects `{ role, scope }`.
- **Fix KnowledgeSearchPage item field mismatch**: Backend `KnowledgeItem` has `tags`, `variant_role`, `relations`; frontend expects `kind`, `status`, `author`, `commit_hash`, `updated_at`.
- **Fix MemorySearchPage feedback endpoint**: Frontend calls `POST /memory/{id}/feedback` with `{ feedback }`, backend expects `POST /memory/feedback` with `{ memoryId, layer, rewardType, score, reasoning }`.
- **Add missing `GET /knowledge/{id}` endpoint**: Frontend `KnowledgeDetailPage` calls `GET /knowledge/{id}` but only `PUT` and `DELETE` exist on that path. Returns 405.

### Serde serialization standardization

- **BREAKING**: Standardize all API response types to use `#[serde(rename_all = "camelCase")]`. Currently mixed: `MemoryEntry` and `UserRoleResponse` use camelCase, `ApprovalRequest` and `GovernanceAuditEntry` use default snake_case, `KnowledgeItem` uses no rename. This is the single highest-impact fix for frontend/backend drift.

### Pagination (15 unbounded API endpoints, 9 unbounded storage functions)

- **Add `PaginatedResponse<T>` standard envelope**: `{ items: Vec<T>, total: i64, limit: usize, offset: usize }` with optional `next_cursor` for cursor-based endpoints.
- **Fix misleading `total` field**: `MemorySearchResponse.total` and `KnowledgeQueryResponse.total` currently return `items.len()` (count of returned items), not the grand total. Rename to `returned` or fix to actual `COUNT(*)`.
- **Add SQL-level LIMIT/OFFSET to all list endpoints** (priority order):
  1. `GET /user` — unbounded full users table scan
  2. `GET /org`, `GET /team`, `GET /project` — `list_all_units()` loads every unit across all tenants, then filters in Rust
  3. `GET /roles/grants` — full `user_roles` table scan with N+1 unit type lookups
  4. `POST /memory/list` — fetches ALL entries from a layer, truncates in Rust
  5. `GET /admin/hierarchy` — same `list_all_units()` pattern
  6. `GET /govern/audit`, `/roles`, `/policies` — no limit whatsoever
  7. `GET /knowledge/promotions` — unbounded, grows with all promotion history
  8. `GET /admin/tenants` — could have thousands in multi-tenant deployments
  9. `GET /admin/git-provider-connections` — unbounded
- **Add offset/cursor to already-bounded endpoints**: `GET /govern/pending` (hardcoded 100, no offset), `POST /knowledge/query` (has limit, no offset), `POST /memory/search` (has limit, no offset), `GET /admin/exports`/`imports`.
- **Cap sync/pull**: Add max limit (500) to prevent `limit=99999` abuse.
- **Fix `find_entry_by_id`**: Replace 4-layer sequential `list()` scan with direct ID lookup query.

### Storage-layer unbounded query fixes (9 critical functions)

- **Fix `list_all_units` (postgres.rs:2203)**: Most dangerous — no `WHERE tenant_id` clause. Returns every org unit across ALL tenants. Add `WHERE tenant_id = $1` and `LIMIT $2 OFFSET $3`.
- **Fix `list_tenants` (tenant_store.rs:163)**: Full table scan, no LIMIT. Add `LIMIT/OFFSET` parameters to the trait and PostgreSQL implementation.
- **Fix `list_repositories` / `list_identities` (repo_manager.rs:285/1531)**: Per-tenant but unbounded. A tenant with 10K indexed repos will OOM the server.
- **Fix `list_roles` (governance.rs:995)**: `governance_roles` is append-only (soft-deleted via `revoked_at`), grows without bound. Add LIMIT.
- **Fix `RedisStore::list_all<T>` (redis_store.rs:93)**: Uses `SMEMBERS` on unbounded sets. Migrate to `SSCAN` with cursor pagination. Affects git-provider-connections, dead letters, and remediations.
- **Fix `list_all_from_layer` (memory/manager.rs:1059)**: Hard-codes `limit=1000` at application level but the underlying provider `list()` call may not push this to SQL/vector-DB. Verify Qdrant and pgvector providers enforce the limit.
- **Fix `list_snapshots` (graph_duckdb.rs:1851)**: S3 `ListObjectsV2` caps at 1000 objects by default but does not loop on `continuation_token`. Silent data loss for large snapshot histories.
- **Fix `get_all_layer_usage` (budget_storage.rs:230)**: Fetches all rows for tenant+window_type with no LIMIT.
- **Add `PaginationParams` / `PaginatedResult<T>` to storage trait**: Introduce `PaginationParams { limit: usize, offset: usize }` and `PaginatedResult<T> { items: Vec<T>, total: Option<u64> }` at the storage trait level.

### Response envelope standardization

- Adopt a single convention: plain arrays for list endpoints (with pagination headers or wrapper), `{ success, <entity> }` for single-entity endpoints. Currently mixed across 3 patterns.

## Capabilities

### New Capabilities

- `api-pagination`: Standard pagination primitives (`PaginatedResponse<T>`, `PaginationParams`, SQL helpers) shared across all list endpoints. Defines the contract for limit/offset and cursor-based pagination, default limits, max caps, and `X-Total-Count` header convention.

### Modified Capabilities

- `admin-dashboard`: Fix all 11 frontend/backend response shape mismatches. Update TypeScript interfaces to match actual backend serialization. Add missing browse/detail flows.
- `server-runtime`: Standardize serde serialization to camelCase across all response types. Add `GET /knowledge/{id}` endpoint. Standardize response envelope pattern. Add `PaginatedResponse<T>` to router-level types.
- `storage`: Add `PaginationParams` and `PaginatedResult<T>` to storage traits. Fix all 9 unbounded storage functions. Migrate Redis `SMEMBERS` to `SSCAN`. Fix S3 continuation token handling.
- `memory-system`: Fix `/memory/feedback` route path and request body contract. Fix `total` semantics in `MemorySearchResponse`. Add SQL-level LIMIT to `list_all_from_layer()`. Add offset/cursor to search and list.
- `knowledge`: Add `GET /knowledge/{id}` handler. Fix `total` semantics in `QueryResponse`. Add offset to query. Fix `find_entry_by_id` to use direct lookup. Add pagination to promotions list.
- `governance`: Add pagination params (limit/offset) to audit, roles, policies, and pending request list endpoints. Standardize response shapes. Fix unbounded `list_roles` at storage level.
- `tenant-admin-control-plane`: Add pagination to tenant list, hierarchy, git-provider-connections. Fix show-tenant response unwrapping contract. Fix unbounded `list_tenants` at storage level.
- `runtime-operations`: Add pagination to export/import job lists. Cap sync/pull limit.
- `user-auth`: Add SQL-level LIMIT/OFFSET to user list. Fix role list field naming. Fix grant-role request body contract.
- `granular-authorization`: Add pagination to `GET /roles/grants`. Fix N+1 unit type lookups.
- `resource-scoped-roles`: Fix role response field naming (`scope`/`unitId` vs `resource_type`/`resource_id` alignment).

## Impact

- **Affected code (backend)**: `memory_api.rs`, `knowledge_api.rs`, `govern_api.rs`, `user_api.rs`, `tenant_api.rs`, `org_api.rs`, `team_api.rs`, `project_api.rs`, `backup_api.rs`, `sync.rs`, `role_grants.rs`, `lifecycle_api.rs`, `router.rs`, plus new shared pagination module.
- **Affected code (frontend)**: All 10 page components in `admin-ui/src/pages/`, `admin-ui/src/api/types.ts`, `admin-ui/src/api/client.ts`.
- **Affected storage layer**: `storage/src/postgres.rs` (list_all_units:2203, list_children:962, list_unit_roles:1277, list_unit_members:1957, list_project_team_assignments:2011, list_suppressions:2266), `storage/src/governance.rs` (list_pending_requests:703, list_audit_logs:905, list_roles:995), `storage/src/tenant_store.rs` (list_tenants:163), `storage/src/repo_manager.rs` (list_repositories:285, list_identities:1531), `storage/src/redis_store.rs` (list_all:93 — SMEMBERS→SSCAN migration), `storage/src/git_provider_connection_store.rs` (list_connections:217), `storage/src/budget_storage.rs` (get_all_layer_usage:230), `storage/src/graph_duckdb.rs` (list_snapshots:1851 — S3 continuation token), `memory/src/manager.rs` (list_all_from_layer:1059), `mk_core/src/types.rs` (add serde rename_all where missing).
- **Breaking changes**: serde rename_all standardization changes the wire format of `ApprovalRequest`, `GovernanceAuditEntry`, `KnowledgeItem`, and `PolicyRecord` from snake_case to camelCase. All API consumers (CLI, plugins, admin UI) must update. The admin UI update ships in the same commit; CLI and plugin consumers need a migration note.
- **Affected APIs**: Every list endpoint gains `?limit=N&offset=N` query params. Response shapes change for governance, policies, lifecycle, and knowledge endpoints. New `GET /knowledge/{id}` endpoint added.
- **Dependencies**: No new crate dependencies. Uses existing `serde`, `axum`, `sqlx` features.
