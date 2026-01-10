# Sync Bridge

The `sync` crate provides a pointer-based synchronization bridge between the Git-based Knowledge Repository and the Vector-based Memory Store.

## Key Features

- **Atomic Sync Cycles**: Uses an in-memory checkpoint and rollback mechanism to ensure consistency.
- **Incremental Sync**: Efficiently syncs only changed items using Git commit hashes.
- **Federation Support**: Synchronizes knowledge from multiple upstream sources.
- **Conflict Detection & Resolution**: Automatically identifies and resolves `HashMismatch`, `MissingPointer`, `OrphanedPointer`, and `DuplicatePointer` conflicts.
- **Governance Integration**: Validates all knowledge entries against organizational rules before syncing.
- **Observability**: Instrumented with `metrics` for tracking sync performance, failures, and governance blocks.

## Core Components

- `SyncManager`: The main orchestrator for sync cycles.
- `SyncState`: Maintains the current state of synchronization, including hashes and mappings.
- `SyncStatePersister`: Handles the persistence of sync state.
- `KnowledgePointer`: Metadata representing a link between a knowledge entry and a memory entry.

## Metrics

The following metrics are exposed:

- `sync.items.synced` (Counter): Total number of items successfully synchronized.
- `sync.cycles.total` (Counter): Total number of sync cycles executed.
- `sync.cycle.duration_ms` (Histogram): Duration of each sync cycle.
- `sync.governance.blocks` (Counter): Number of items blocked by governance.
- `sync.items.failed` (Gauge): Current number of failed items.
- `sync.conflicts.detected` (Counter): Number of conflicts detected.
- `sync.conflicts.resolved` (Counter): Number of successful conflict resolution cycles.

## Usage

```rust
let sync_manager = SyncManager::new(
    memory_manager,
    knowledge_repo,
    governance_engine,
    federation_manager,
    persister,
).await?;

// Run a manual sync cycle
sync_manager.run_sync_cycle(60).await?;

// Start background sync
let handle = Arc::new(sync_manager).start_background_sync(300, 60).await;
```
