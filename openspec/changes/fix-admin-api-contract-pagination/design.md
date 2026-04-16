## Context

Aeterna exposes 70+ REST endpoints through an Axum 0.8 HTTP server. The admin UI (React 18 + TypeScript + TanStack Query) was built against manually-maintained TypeScript interfaces that have drifted from the Rust serde serialization. The backend has three competing response envelope patterns, two competing field-naming conventions (some types use `#[serde(rename_all = "camelCase")]`, others default to snake_case), and 15 of 32 list endpoints perform unbounded full-table scans.

The `add-admin-web-ui` change design noted "No automated TypeScript type generation" as a known trade-off and "API type drift between Rust types and TypeScript types" as a risk. This change resolves that technical debt.

## Goals / Non-Goals

**Goals:**
- Fix all 11 identified frontend/backend contract mismatches so every admin UI page renders correctly.
- Standardize serde serialization to camelCase for all JSON API response types.
- Add SQL-level pagination (LIMIT/OFFSET) to all 15 unbounded list endpoints.
- Introduce a `PaginatedResponse<T>` standard envelope and `PaginationParams` extractor.
- Fix the misleading `total` semantics in memory and knowledge responses.
- Add the missing `GET /knowledge/{id}` endpoint.
- Fix the `/memory/feedback` route and request body contract.
- Produce a machine-readable OpenAPI 3.1 spec that documents the actual wire format.

**Non-Goals:**
- Auto-generating TypeScript types from Rust (future improvement; this change fixes the manual types).
- Adding cursor-based pagination to all endpoints (only sync/pull uses cursors; LIMIT/OFFSET is sufficient for admin UI use cases).
- Redesigning the authentication flow or adding new auth mechanisms.
- Adding WebSocket real-time updates or Server-Sent Events to list endpoints.
- Refactoring the storage layer abstractions (only adding LIMIT/OFFSET parameters to existing query methods).

## Decisions

### Standardize on camelCase for all JSON API responses

**Decision:** Add `#[serde(rename_all = "camelCase")]` to every struct that serializes to a JSON API response. This includes `ApprovalRequest`, `GovernanceAuditEntry`, `KnowledgeItem`, `PolicyRecord`, `RemediationRequest`, and all pagination wrapper types.

**Why:** The admin UI (and any JavaScript consumer) conventionally uses camelCase. The types that already work correctly (`MemoryEntry`, `UserRoleResponse`, `TenantRecord`) all use camelCase. The types that are broken (`ApprovalRequest`, `GovernanceAuditEntry`, `KnowledgeItem`) either use default snake_case or no rename. Standardizing eliminates the per-type guessing game.

**Alternatives considered:**
- **Standardize on snake_case:** Would require changing the already-working memory and tenant types, breaking the admin UI pages that currently work. More churn for the same result.
- **Add a JSON middleware that converts all keys:** Fragile and hard to debug. Serde is the right layer for serialization control.
- **Keep mixed and fix only broken types:** Leaves a maintenance trap; any new type added without rename_all will silently break.

**Breaking:** CLI consumers and plugin adapters that parse governance, knowledge, or policy responses will see camelCase field names instead of snake_case. Migration note required.

### Use LIMIT/OFFSET pagination (not cursor-based) for admin endpoints

**Decision:** All admin-facing list endpoints use `?limit=N&offset=N` query parameters with a standard `PaginatedResponse<T>` envelope containing `{ items, total, limit, offset }`. The `total` field is a true `COUNT(*)` from the database, not `items.len()`.

**Why:** Admin UI pagination uses page numbers ("Page 1 of 5"), which maps directly to LIMIT/OFFSET. Cursor-based pagination is more efficient for infinite scroll but adds complexity and doesn't provide total counts for page navigation. The sync/pull endpoint already uses cursors and keeps that pattern.

**Alternatives considered:**
- **Cursor-based everywhere:** Better for large datasets but the admin UI needs total counts for page indicators. Cursor pagination doesn't naturally provide totals.
- **Keyset pagination (WHERE id > last_id):** Efficient but requires a stable sort column and complicates filtering. LIMIT/OFFSET is simpler for datasets under 100K rows (admin use case).
- **No standard envelope, use X-Total-Count header:** Less discoverable and harder for TanStack Query to consume. The envelope keeps everything in the JSON body.

### Fix frontend to match backend (not vice versa) for response shapes

**Decision:** For the 11 contract mismatches, the frontend TypeScript types are updated to match the backend\'s actual response shapes. The backend response shapes are NOT changed except where they are objectively wrong (e.g., missing endpoint, misleading `total` semantics).

**Why:** The backend contracts are already consumed by the CLI, plugins, and potentially external integrations. Changing backend shapes to match incorrect frontend assumptions would break those other consumers. The frontend is the newer, less-integrated consumer.

**Exceptions (backend changes):**
- Add `GET /knowledge/{id}` (missing endpoint).
- Fix `total` to be grand total, not `items.len()`.
- Fix `/memory/feedback` route to also accept `POST /memory/{id}/feedback` as an alias.
- Standardize serde to camelCase (changes wire format of some types).

### Add PaginationParams as an Axum extractor

**Decision:** Create a shared `PaginationParams` struct that implements Axum\'s `FromRequestParts` (via `Query` extraction) with validation: `limit` defaults to 50, max 200; `offset` defaults to 0, must be >= 0.

**Why:** Every list endpoint currently re-implements limit handling differently (some hardcode 100, some have no default, some accept but ignore). A shared extractor enforces consistent defaults and caps.

### Migrate Redis SMEMBERS to SSCAN for unbounded sets

**Decision:** Replace `RedisStore::list_all<T>` (which uses `SMEMBERS` to load entire index sets) with `SSCAN`-based cursor iteration that returns paginated results. The `list_all` method signature changes to accept `PaginationParams` and return `PaginatedResult<T>`.

**Why:** `SMEMBERS` blocks the Redis event loop for the duration of the read on large sets. `SSCAN` is non-blocking and returns items in cursor-sized batches. All consumers of `list_all` (git-provider-connections, dead letters, remediations) are affected.

**Alternatives considered:**
- **Keep SMEMBERS with application-level truncation:** Still blocks Redis; the read is the expensive part, not the Rust processing.
- **Switch to Redis Sorted Sets (ZRANGEBYSCORE with LIMIT):** Better pagination semantics but requires migrating existing data and changing write paths.
- **Move these stores to PostgreSQL:** Eliminates the Redis problem but changes deployment dependencies. Out of scope for this change.

### Add S3 continuation token loop for list_snapshots

**Decision:** The `list_snapshots` function in `graph_duckdb.rs` currently calls `ListObjectsV2` once and ignores `continuation_token`. Fix to loop until `is_truncated == false` or add a max-pages cap (e.g., 10 pages = 10,000 snapshots).

**Why:** S3 silently caps `ListObjectsV2` at 1,000 objects per request. Tenants with >1,000 graph snapshots would get silently truncated results.

### Introduce PaginationParams/PaginatedResult at the storage trait level

**Decision:** Add `PaginationParams { limit: usize, offset: usize }` and `PaginatedResult<T> { items: Vec<T>, total: Option<u64> }` to the storage crate. All `list_*` trait methods accept `PaginationParams` and return `PaginatedResult<T>`. The `total` is `Option` because some backends (Qdrant vector search, S3 listing) cannot efficiently provide exact counts.

**Why:** The current situation has each storage function handling limits differently (some ignore them, some hard-code them, some accept but don't pass to SQL). A trait-level contract ensures every implementation must handle pagination.

**Alternatives considered:**
- **Only add pagination at the API handler level:** Leaves the storage layer unbounded; a bug in any handler could still trigger a full table scan.
- **Use a trait with default implementation:** Risky; default impls would silently do nothing, masking the problem.

### Produce OpenAPI 3.1 spec as a checked-in YAML file

**Decision:** Maintain a hand-authored `openapi.yaml` at the project root. Do not auto-generate from Rust code in V1.

**Why:** Auto-generation via utoipa or aide requires annotating every handler with proc macros, which is a large refactor. A hand-authored spec can be validated against the implementation via integration tests (`assert_eq!(actual_response_shape, expected_from_spec)`).

**Alternatives considered:**
- **utoipa (proc-macro based):** Excellent for greenfield; too invasive for 70+ existing handlers.
- **aide (axum-native):** Requires wrapping every handler in `aide::axum::ApiRouter`; significant refactor.
- **Generate from tests:** Integration tests already exercise the endpoints; adding schema assertions is lighter than proc-macro annotation.

## Risks / Trade-offs

- **[Risk] camelCase migration breaks CLI consumers** → Mitigation: The CLI uses `serde_json::Value` for most display commands, so field name changes only affect structured parsing. Add a migration note to CHANGELOG. The CLI\'s `aeterna query knowledge` and `aeterna govern list` commands need field name updates.
- **[Risk] LIMIT/OFFSET performance on large tables** → Mitigation: For the admin UI use case (browsing, not analytics), OFFSET up to ~10K rows is fast enough on PostgreSQL with indexed columns. If needed, add keyset pagination later for specific endpoints.
- **[Risk] COUNT(*) on every list request adds latency** → Mitigation: PostgreSQL\'s planner uses index-only scans for COUNT(*) on well-indexed tables. The `total` field is optional in the response; endpoints can omit it if the query is expensive (e.g., vector search).
- **[Trade-off] Hand-authored OpenAPI spec can drift from implementation** → Accepted for V1. Spec drift is caught by integration tests that validate response shapes against the spec.
- **[Trade-off] LIMIT/OFFSET is less efficient than cursor-based for deep pages** → Accepted. Admin UI rarely pages beyond page 20. The sync/pull endpoint keeps cursor-based pagination.
- **[Risk] Redis SSCAN migration changes iteration guarantees** → Mitigation: SSCAN may return duplicates across cursor iterations. Deduplicate in the application layer. For the small sets we have today (git connections, remediations), this is negligible.
- **[Risk] S3 continuation token loop could be slow for very large snapshot histories** → Mitigation: Cap at 10 pages (10K snapshots). If a tenant has >10K snapshots, the list is truncated with a warning log.
- **[Risk] Storage trait change breaks all backend implementations** → Mitigation: Add `PaginationParams` with default values (`limit=1000, offset=0`) so existing callers continue to work. Update implementations incrementally.

## Migration Plan

1. **Phase 0 — Storage trait primitives (non-breaking):** Add `PaginationParams` and `PaginatedResult<T>` to the storage crate. Update trait method signatures with default pagination values so existing callers compile without changes.
1. **Phase 1 — Shared API primitives (non-breaking):** Add `PaginatedResponse<T>`, `PaginationParams`, and SQL helper functions to a new `cli/src/server/pagination.rs` module. No endpoint changes yet.
2. **Phase 2 — serde standardization (breaking wire format):** Add `#[serde(rename_all = "camelCase")]` to all response structs. Update admin UI TypeScript types simultaneously. Update CLI display commands.
3. **Phase 3 — Response shape fixes (frontend):** Fix all 11 contract mismatches in the admin UI. Update TanStack Query hooks to unwrap response envelopes correctly.
4. **Phase 4 — Missing endpoints:** Add `GET /knowledge/{id}`. Add `/memory/{id}/feedback` route alias.
5. **Phase 5 — Pagination (backend + storage):** Roll out `PaginationParams` to all 15 unbounded API endpoints AND 9 unbounded storage functions in priority order (list_all_units → list_tenants → list_repositories → list_roles → RedisStore::list_all SSCAN migration → memory list_all_from_layer → list_snapshots S3 fix → budget get_all_layer_usage).
6. **Phase 6 — Frontend pagination UI:** Add pagination controls to all list pages using the new `PaginatedResponse` envelope.
7. **Phase 7 — OpenAPI spec:** Author `openapi.yaml` reflecting the final API surface. Add CI validation.

Rollback: Each phase is independently deployable. Phase 2 is the only breaking change; rollback requires reverting both backend serde and frontend types.

## Open Questions

- Should `PaginatedResponse<T>` include a `nextCursor` field for future cursor migration, or keep it strictly LIMIT/OFFSET?
- Should the OpenAPI spec be generated from integration test recordings rather than hand-authored?
- Should we add response shape validation middleware (debug mode only) that warns when a response doesn\'t match the OpenAPI spec?
- Should the `total` field in paginated responses be mandatory or optional (omittable for expensive queries like vector search)?
