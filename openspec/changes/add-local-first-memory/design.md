## Context

Aeterna currently operates as a **remote-only** system. The OpenCode plugin (`packages/opencode-plugin/`) is a thin HTTP client that proxies all memory/knowledge/graph operations to the Aeterna server. This means:

- Every memory read requires a network round-trip (~50-200ms depending on latency)
- Plugin is completely non-functional when the server is unreachable
- Personal layers (agent, user, session) have no reason to traverse the network for single-user data
- Cross-device continuity of personal memories requires explicit server availability

The Rust codebase already uses DuckDB extensively for the graph layer (`storage/src/graph_duckdb.rs` — 3,900+ lines with tenant isolation, schema versioning, backup/restore). The plugin is TypeScript (Node.js) communicating over HTTP.

**Key constraint**: The plugin runs inside the OpenCode process (Node.js). Any local store must work in-process or as a lightweight companion, not as a separate server.

## Goals / Non-Goals

**Goals:**
- Sub-millisecond reads for agent/user/session memories during interactive coding
- Offline-first: personal layers always available, even without network
- Background sync: local changes push to remote for backup; shared layers pull for caching
- Zero-config default: local store auto-initializes on first use, sync starts when server URL is configured
- Cross-device continuity: personal memories sync via remote server to other devices
- Graceful degradation: shared layers return cached results when offline, empty (not error) if never cached

**Non-Goals:**
- Running the full Aeterna server locally (k3s/docker-compose — explicitly rejected)
- Real-time collaboration on personal layers (eventual consistency is fine)
- Conflict resolution for shared layers (remote is authoritative; local only caches)
- Local knowledge repository (knowledge remains server-side, git-versioned)
- Embedding generation locally (use remote embeddings, cache results)
- Supporting non-OpenCode clients in v1 (CLI support is stretch goal)

## Decisions

### Decision 1: SQLite (better-sqlite3) for local store, not DuckDB

**Choice**: Use `better-sqlite3` in the TypeScript plugin for the local memory store.

**Rationale**:
- The plugin is TypeScript/Node.js. `better-sqlite3` is the gold standard for embedded SQLite in Node — synchronous API, zero-config, battle-tested, 7M+ weekly downloads
- DuckDB's Node.js binding (`duckdb-node`) is less mature for embedded use and much heavier (~50MB binary)
- SQLite is perfect for key-value/relational local storage with FTS5 for text search
- Vector search for local memories: store embeddings as BLOBs, compute cosine similarity in JS for small datasets (<100K memories per user) — no need for a vector extension
- The Rust side continues using DuckDB for the graph layer (server-side) — no conflict

**Alternatives considered**:
- DuckDB Node.js binding — heavier, less mature in Node ecosystem, overkill for local cache
- LevelDB/RocksDB — no SQL, harder to query, no FTS
- IndexedDB — browser-only, not available in Node.js
- Plain JSON files — no query capability, no concurrent access safety

### Decision 2: Layer ownership model

**Choice**: Hard partition — local store OWNS agent/user/session; remote OWNS project/team/org/company.

| Layer | Owner | Read | Write | Sync Direction |
|-------|-------|------|-------|----------------|
| agent | local | local | local | push to remote (backup) |
| user | local | local | local | push to remote (backup + cross-device) |
| session | local | local | local | push to remote (backup) |
| project | remote | local cache | remote | pull from remote |
| team | remote | local cache | remote | pull from remote |
| org | remote | local cache | remote | pull from remote |
| company | remote | local cache | remote | pull from remote |

**Rationale**:
- Personal layers are single-writer (one user, one device at a time) — no multi-writer conflicts
- Shared layers are multi-writer (many users) — local should never authoritatively write
- Clean partition means sync conflicts only happen on personal layers during cross-device use (rare, last-writer-wins is fine)

### Decision 3: Sync protocol — HTTP batch with cursor

**Choice**: Simple HTTP batch push/pull using server-assigned cursors, not WebSocket or CRDT.

**Push** (local → remote):
```
POST /api/v1/sync/push
Body: { entries: MemoryEntry[], device_id: string, last_push_cursor: string }
Response: { cursor: string, conflicts: ConflictEntry[] }
```

**Pull** (remote → local):
```
GET /api/v1/sync/pull?since_cursor={cursor}&layers=project,team,org,company&limit=100
Response: { entries: MemoryEntry[], cursor: string, has_more: boolean }
```

**Rationale**:
- HTTP batch is simple, works through all proxies/firewalls, no connection state
- Cursor-based pagination handles large backlogs gracefully
- Push interval: 30s (configurable) or on plugin shutdown (flush)
- Pull interval: 60s (configurable) for shared layers
- No CRDT complexity — personal layers use last-writer-wins with device_id + timestamp; shared layers are read-only locally

**Alternatives considered**:
- WebSocket streaming — unnecessary complexity for eventual consistency, connection management burden
- CRDTs — massive implementation cost, not justified for single-writer personal layers
- Git-based sync — great for knowledge (already used), wrong abstraction for high-frequency memory writes

### Decision 4: Local store schema

**Choice**: Three tables — `memories`, `sync_queue`, `sync_cursors`.

```sql
-- Local memories (both owned and cached)
CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  layer TEXT NOT NULL,           -- agent|user|session|project|team|org|company
  ownership TEXT NOT NULL,       -- 'local' or 'cached'
  embedding BLOB,               -- float32 array, nullable
  tags TEXT,                     -- JSON array
  metadata TEXT,                 -- JSON object
  importance_score REAL DEFAULT 0.0,
  tenant_context TEXT,           -- JSON: {company, org, team, project, user, agent, session}
  device_id TEXT,
  created_at INTEGER NOT NULL,   -- unix millis
  updated_at INTEGER NOT NULL,
  synced_at INTEGER,             -- null = never synced
  deleted_at INTEGER             -- soft delete for sync
);

CREATE INDEX idx_memories_layer ON memories(layer);
CREATE INDEX idx_memories_ownership ON memories(ownership);
CREATE INDEX idx_memories_updated ON memories(updated_at);

-- Outbound sync queue (local changes not yet pushed)
CREATE TABLE sync_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  memory_id TEXT NOT NULL,
  operation TEXT NOT NULL,       -- 'upsert' or 'delete'
  queued_at INTEGER NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);

-- Sync cursors per remote server
CREATE TABLE sync_cursors (
  server_url TEXT NOT NULL,
  direction TEXT NOT NULL,       -- 'push' or 'pull'
  cursor TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (server_url, direction)
);
```

**Rationale**:
- Single `memories` table for both local-owned and cached entries (distinguished by `ownership` column)
- `sync_queue` tracks local mutations that haven't been pushed — survives plugin restarts
- Soft deletes (`deleted_at`) so deletions can be synced to remote
- No FTS5 index in v1 — text search uses `LIKE` for small local datasets; add FTS5 if needed later

### Decision 5: Plugin architecture — LocalMemoryManager class

**Choice**: New `LocalMemoryManager` class that wraps `better-sqlite3` and exposes the same interface as the HTTP client for local layers.

```
AeternaClient (existing)
├── LocalMemoryManager (NEW) — handles agent/user/session layers
│   ├── better-sqlite3 database
│   ├── SyncEngine — background push/pull loop
│   └── EmbeddingCache — stores embeddings received from remote
└── HTTP client (existing) — handles project/team/org/company layers + knowledge + graph + CCA
```

**Read path**:
1. Determine layer from request
2. If personal layer (agent/user/session) → read from `LocalMemoryManager`
3. If shared layer (project/team/org/company) → read from local cache first, fallback to HTTP
4. Return results

**Write path**:
1. If personal layer → write to local SQLite + enqueue to `sync_queue`
2. If shared layer → send to remote via HTTP (no local write authority)

**Rationale**:
- Existing tool interfaces (`aeterna_memory_add`, `aeterna_memory_search`, etc.) don't change
- Router logic is transparent — tools don't need to know about local vs remote
- `SyncEngine` runs as a `setInterval` background loop, no extra processes

### Decision 6: Embedding strategy for local search

**Choice**: Cache embeddings received from remote; compute cosine similarity in JS for local search.

- When a memory is added locally, the embedding is `null` initially
- On next sync push, the remote server generates the embedding and returns it in the push response
- Local search before embedding exists: fall back to text `LIKE` matching
- Local search with embedding: brute-force cosine similarity over all local memories (fast for <100K entries)
- Shared layer cache entries already have embeddings from the pull

**Rationale**:
- No local embedding model required — avoids 500MB+ model downloads
- Brute-force cosine similarity over 10K float32 vectors takes <10ms in Node.js
- Text fallback ensures search works immediately, improves after first sync

**Alternatives considered**:
- Local embedding model (e.g., ONNX) — too heavy for a plugin, 500MB+ download
- No local search at all — defeats the purpose of local-first

### Decision 7: Local store file location

**Choice**: `~/.aeterna/local.db` (default), configurable via `AETERNA_LOCAL_DB_PATH` or `.aeterna/config.toml`.

```toml
[local]
enabled = true                    # default: true
db_path = "~/.aeterna/local.db"   # default
sync_push_interval_ms = 30000     # default: 30s
sync_pull_interval_ms = 60000     # default: 60s
max_cached_entries = 50000        # default: 50K shared layer entries
```

**Rationale**:
- User home directory is the standard location for per-user application data
- Single file (SQLite) — easy to backup, move, delete
- Per-user, not per-project — personal memories span projects

## Risks / Trade-offs

- **Stale shared layer cache** → Mitigation: 60s pull interval, manual refresh via `aeterna_sync_status` tool, cache entries include `cached_at` timestamp so tools can warn on old data
- **Cross-device conflict on personal layers** → Mitigation: Last-writer-wins with `device_id` + `updated_at`; conflicts are rare (single user, usually one active device). Future: add conflict log for manual resolution
- **SQLite file corruption** → Mitigation: WAL mode, proper shutdown flush, automatic backup before schema migrations. SQLite is extremely resilient.
- **Plugin startup latency** → Mitigation: SQLite opens in <5ms, schema creation is idempotent and fast. No impact on OpenCode startup.
- **Large local databases** → Mitigation: Auto-prune session memories older than `storage_ttl_hours` (default 24h). Agent/user memories persist indefinitely but are typically small (<10K entries per user).
- **Embedding sync delay** → Mitigation: Text search works immediately. Embeddings arrive after first push cycle (30s). Display "semantic search available after sync" message.

## Migration Plan

**This is a net-new capability — no data migration required.**

1. **Phase 1 — Local store**: Add `LocalMemoryManager` + SQLite to plugin. Personal layer reads/writes go local. No sync yet. Plugin works offline for personal layers.
2. **Phase 2 — Sync protocol**: Add `/api/v1/sync/push` and `/api/v1/sync/pull` endpoints to server. Add `SyncEngine` to plugin. Personal memories backup to remote. Shared layers cache locally.
3. **Phase 3 — Polish**: Add `aeterna_sync_status` tool enhancements (show local/remote counts, last sync time, pending queue size). Add config options. Add CLI support as stretch goal.

**Rollback**: Delete `~/.aeterna/local.db`. Plugin falls back to remote-only mode (existing behavior). Zero risk.

## Open Questions

1. **Should session memories auto-expire locally?** Proposed: yes, after `storage_ttl_hours` (default 24h). But users may want to keep session memories longer for continuity. Could make it per-layer configurable.
2. **Should the sync push response include embeddings?** Proposed: yes, so local search improves immediately after sync. Alternative: separate endpoint to fetch embeddings in batch.
3. **CLI support in v1?** The Rust CLI could also embed a local SQLite store using the `rusqlite` crate. Same schema, same sync protocol. Decision: defer to v1.1 unless it falls naturally out of the implementation.
