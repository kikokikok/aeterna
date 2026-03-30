## 1. Local Memory Store — Plugin Foundation

- [ ] 1.1 Add `better-sqlite3` and `@types/better-sqlite3` as dependencies to `packages/opencode-plugin/package.json`
- [ ] 1.2 Create `packages/opencode-plugin/src/local/schema.ts` — export `SCHEMA_VERSION`, `CREATE_TABLE` SQL constants for `memories`, `sync_queue`, `sync_cursors` tables (WAL mode, indexes)
- [ ] 1.3 Create `packages/opencode-plugin/src/local/db.ts` — `LocalDatabase` class wrapping `better-sqlite3`: open/create DB at path, apply schema idempotently, verify schema version, WAL mode, busy timeout 5000ms
- [ ] 1.4 Create `packages/opencode-plugin/src/local/manager.ts` — `LocalMemoryManager` class: constructor takes db path + config, exposes `add()`, `update()`, `delete()`, `search()`, `getById()` for local-owned memories
- [ ] 1.5 Implement `LocalMemoryManager.add()` — insert into `memories` table with `ownership='local'`, enqueue to `sync_queue`, generate UUID for id, set timestamps
- [ ] 1.6 Implement `LocalMemoryManager.update()` — update `memories` row, bump `updated_at`, enqueue `upsert` to `sync_queue`
- [ ] 1.7 Implement `LocalMemoryManager.delete()` — soft delete (set `deleted_at`), enqueue `delete` to `sync_queue`
- [ ] 1.8 Implement `LocalMemoryManager.search()` — cosine similarity over embeddings (float32 BLOB decode), fallback to `LIKE` when embeddings missing, return sorted results
- [ ] 1.9 Implement `LocalMemoryManager.getById()` — single row lookup by id, return null if not found or soft-deleted
- [ ] 1.10 Create `packages/opencode-plugin/src/local/config.ts` — `LocalConfig` type: `enabled`, `db_path`, `sync_push_interval_ms`, `sync_pull_interval_ms`, `max_cached_entries`, `session_storage_ttl_hours`; parse from env vars + `.aeterna/config.toml`

## 2. Shared Layer Cache

- [ ] 2.1 Add `upsertCached()` method to `LocalMemoryManager` — insert/update with `ownership='cached'`, set `synced_at`
- [ ] 2.2 Add `searchCached()` method to `LocalMemoryManager` — query cached entries by layer, support same similarity/text-fallback search as local
- [ ] 2.3 Add `evictOldCached()` method to `LocalMemoryManager` — delete oldest cached entries when count exceeds `max_cached_entries`
- [ ] 2.4 Add `expireSessionMemories()` method to `LocalMemoryManager` — delete session-layer local memories older than `session_storage_ttl_hours`

## 3. Sync Engine — Client Side

- [ ] 3.1 Create `packages/opencode-plugin/src/local/sync.ts` — `SyncEngine` class: constructor takes `LocalMemoryManager`, `AeternaClient` (HTTP), `LocalConfig`
- [ ] 3.2 Implement device ID generation and persistence — generate UUID v4 on first init, store in `sync_cursors` or metadata row, read on subsequent starts
- [ ] 3.3 Implement `SyncEngine.pushCycle()` — read `sync_queue`, batch entries, POST to `/api/v1/sync/push` with `entries`, `device_id`, `last_push_cursor`; on success remove from queue and update cursor; on conflict overwrite local with remote version
- [ ] 3.4 Implement embedding response handling — when push response includes `embeddings{}`, update local `memories.embedding` BLOB for each returned memory
- [ ] 3.5 Implement `SyncEngine.pullCycle()` — GET `/api/v1/sync/pull?since_cursor={}&layers=project,team,org,company&limit=100`, upsert results via `upsertCached()`, paginate while `has_more=true` (max 10 pages), update pull cursor
- [ ] 3.6 Implement push exponential backoff — on failure: 30s, 60s, 120s, max 300s; reset on success
- [ ] 3.7 Implement `SyncEngine.start()` / `stop()` — `setInterval` for push (30s default) and pull (60s default), call `expireSessionMemories()` and `evictOldCached()` during pull cycle
- [ ] 3.8 Implement `SyncEngine.flushOnShutdown()` — attempt final push with 5s timeout, close DB cleanly

## 4. Memory Layer Router — Plugin Integration

- [ ] 4.1 Create `packages/opencode-plugin/src/local/router.ts` — `MemoryRouter` class: determines whether a memory operation goes to local or remote based on layer
- [ ] 4.2 Implement read routing — personal layers (agent/user/session) → `LocalMemoryManager.search()`; shared layers → check local cache first (< 60s), fall back to HTTP client
- [ ] 4.3 Implement write routing — personal layers → `LocalMemoryManager.add()`; shared layers → HTTP client only (no local write)
- [ ] 4.4 Wire `MemoryRouter` into existing `AeternaClient` — replace direct HTTP calls for memory operations with router dispatch; keep all non-memory operations (knowledge, graph, CCA) as HTTP-only
- [ ] 4.5 Update plugin entry point (`src/index.ts`) to initialize `LocalMemoryManager` → `SyncEngine` → `MemoryRouter` on startup; start sync engine if server URL configured
- [ ] 4.6 Update plugin shutdown hook to call `SyncEngine.flushOnShutdown()`

## 5. Sync Status Tool

- [ ] 5.1 Add `aeterna_sync_status` tool (or extend existing tool) — return pending push count, last push/pull timestamps, server connectivity, local store size (entry counts per layer)
- [ ] 5.2 Add `source: 'local' | 'cache' | 'remote'` metadata to memory search responses so tools can indicate data provenance
- [ ] 5.3 Add cache staleness warning — if cached shared-layer results are older than 10 minutes, include warning in response metadata

## 6. Server-Side Sync Endpoints

- [ ] 6.1 Add `POST /api/v1/sync/push` handler in `cli/src/server/` — accept `{ entries, device_id, last_push_cursor }`, upsert into memory store, generate embeddings for entries missing them, detect conflicts (same id, remote newer), return `{ cursor, conflicts, embeddings }`
- [ ] 6.2 Add `GET /api/v1/sync/pull` handler in `cli/src/server/` — accept `since_cursor`, `layers`, `limit` query params; return entries updated after cursor position ordered by `updated_at` ASC; enforce tenant isolation; return `{ entries, cursor, has_more }`
- [ ] 6.3 Add `device_id` column to server-side memory table (or metadata) — record origin device on synced entries
- [ ] 6.4 Add authentication check on sync endpoints — reject unauthenticated requests with HTTP 401

## 7. Tests — Plugin Local Store

- [ ] 7.1 Unit tests for `LocalDatabase` — schema creation, idempotent re-open, WAL mode verification, migration path
- [ ] 7.2 Unit tests for `LocalMemoryManager` CRUD — add/update/delete/getById with assertion on `sync_queue` entries
- [ ] 7.3 Unit tests for `LocalMemoryManager.search()` — cosine similarity with mock embeddings, text fallback, empty result
- [ ] 7.4 Unit tests for shared-layer cache — `upsertCached()`, `searchCached()`, `evictOldCached()`, session expiration
- [ ] 7.5 Unit tests for `SyncEngine` push cycle — mock HTTP, verify queue drain, cursor update, conflict handling, embedding storage
- [ ] 7.6 Unit tests for `SyncEngine` pull cycle — mock HTTP, verify cache upsert, pagination, cursor update
- [ ] 7.7 Unit tests for `MemoryRouter` — personal layer → local, shared layer → cache + remote fallback, write routing
- [ ] 7.8 Unit tests for `LocalConfig` — env var override, config file parsing, defaults

## 8. Tests — Server Sync Endpoints

- [ ] 8.1 Integration test for `POST /api/v1/sync/push` — testcontainers PostgreSQL, push batch, verify stored, verify embedding generation, verify conflict detection
- [ ] 8.2 Integration test for `GET /api/v1/sync/pull` — testcontainers PostgreSQL, seed data, verify cursor pagination, layer filtering, tenant isolation
- [ ] 8.3 Integration test for auth rejection — verify 401 on unauthenticated sync requests

## 9. Configuration & Documentation

- [ ] 9.1 Update `packages/opencode-plugin/README.md` — document local-first architecture, config options (`local.enabled`, `db_path`, sync intervals), offline behavior
- [ ] 9.2 Add local store config section to `.aeterna/config.toml` example in docs — show all configurable fields with defaults
- [ ] 9.3 Update INSTALL.md — add section on local-first memory, explain that personal layers work offline, sync is automatic when server is configured
