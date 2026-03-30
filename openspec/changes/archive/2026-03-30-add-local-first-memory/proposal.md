## Why

AI coding assistants (OpenCode, Claude Code, Cursor) need sub-millisecond access to agent, user, and session memories during interactive conversations. Today Aeterna only runs as a remote server, meaning every memory read adds network latency, requires server availability, and fails completely when offline. The real user experience should be: **local-first for personal layers, remote for shared layers, bidirectional sync to bridge them.**

This is the capability that was identified when the Tier 1.2 "local docker-compose" approach was cancelled — the actual need is not running the server locally, but having an embedded local memory store that syncs to the remote Aeterna instance.

## What Changes

- Introduce an **embedded local memory store** (DuckDB) that lives inside the OpenCode plugin process (or a lightweight CLI sidecar)
- Local store owns **agent, user, and session layers** — reads are instant, writes are local-first
- Remote Aeterna server owns **project, team, org, and company layers** — the shared organizational knowledge
- **Bidirectional sync** reconciles local ↔ remote:
  - Local writes to agent/user/session layers queue for async push to remote (backup + cross-device continuity)
  - Remote writes to project/team/org/company layers pull down to local cache (offline access to shared knowledge)
  - Conflict resolution: last-writer-wins with vector clock, local always wins for personal layers
- Plugin/CLI detects whether remote Aeterna is reachable and **degrades gracefully** — personal layers always work, shared layers return cached or empty
- **No new server-side deployment** required — this is purely a client-side (plugin/CLI) capability with sync protocol additions to the existing MCP/HTTP API

## Capabilities

### New Capabilities
- `local-memory-store`: Embedded DuckDB-based memory store for agent/user/session layers running in-process within the plugin or CLI, with vector search via DuckDB's VSS extension
- `memory-sync-protocol`: Bidirectional sync protocol between local store and remote Aeterna server — push local changes, pull shared layer updates, conflict resolution, offline queue

### Modified Capabilities
- `memory-system`: Add layer ownership model (local-owned vs remote-owned layers), add sync metadata fields (vector clock, sync status, origin device ID)
- `opencode-integration`: Plugin initializes local store on startup, reads personal layers locally, falls back to remote for shared layers, runs background sync loop
- `storage`: Add DuckDB as a supported storage backend for embedded/local use (alongside PostgreSQL for server, Qdrant for vectors)

## Impact

- **Plugin (`packages/opencode-plugin/`)**: Major — needs embedded DuckDB, local memory manager, sync loop, offline detection
- **CLI (`cli/`)**: Moderate — local store can also be used by CLI for direct memory access without server
- **MCP API (`tools/`)**: Minor — add sync endpoints (push batch, pull since timestamp, conflict report)
- **Memory crate (`memory/`)**: Moderate — layer ownership model, sync metadata on MemoryEntry
- **Storage crate (`storage/`)**: Moderate — DuckDB backend implementation (VSS for vectors, FTS for text search)
- **Server (`cli/src/server/`)**: Minor — new sync HTTP endpoints for batch push/pull
- **No breaking changes** to existing remote-only deployments — sync is opt-in, triggered by clients that have local stores
