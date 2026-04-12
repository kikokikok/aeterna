## Why

Aeterna stores critical enterprise data across multiple backends (PostgreSQL+pgvector, Qdrant, DuckDB, Redis) with no supported path for exporting, backing up, restoring, or migrating that data between environments. The CLI already stubs `aeterna admin export` and `aeterna admin import` commands, but both return "server not connected" errors with simulated analysis data. This gap blocks three enterprise requirements:

1. **Business Continuity / Disaster Recovery (BCP/DR)**: Operators have no way to create point-in-time backups or restore a tenant's data after a failure.
2. **Data Portability and Tenant Migration**: Moving a tenant between Aeterna instances (e.g., staging to production, region migration) requires manual database dumps with no integrity verification.
3. **Compliance and Audit**: Regulated environments require demonstrated ability to export, archive, and restore organizational knowledge and governance metadata on demand.

## What Changes

- Add a new `backup-restore` capability specifying the archive format, export/import lifecycle, integrity guarantees, tenant scoping rules, online operation guarantees, and resource throttling.
- Add server-side REST API endpoints for asynchronous export and import job management, including job status polling, archive download, archive upload, dry-run validation, post-dry-run confirmation, and job cancellation.
- Implement a portable archive format (tar.gz with manifest.json, NDJSON data files per entity type, and SHA256 checksums) that captures memories, knowledge, graph data, governance metadata, and optionally audit events.
- Wire the existing CLI `aeterna admin export` and `aeterna admin import` stubs to real server-backed operations through the shared CLI client layer.
- Add incremental export support (since-timestamp filtering) and tenant-scoped export/import with identity remapping.
- Coordinate export consistency across PostgreSQL, Qdrant, DuckDB using streaming reads (PG cursors, Qdrant scroll API) with dedicated connection pools to minimize live server impact.
- Add a dedicated `aeterna admin backup validate` command for offline archive integrity verification without server connectivity.

## Capabilities

### New Capabilities
- `backup-restore`: Archive format specification, export/import lifecycle, server-side async job API, integrity validation, tenant scoping, incremental export, identity remapping, online operation with resource throttling, Prometheus observability, and DR validation tooling.

### Modified Capabilities
- `memory-system`: Add export serialization contract for MemoryEntry records including embeddings and layer metadata.
- `knowledge-repository`: Add export serialization contract for KnowledgeEntry records, KnowledgeRelation records, and promotion history.
- `storage`: Add coordinated multi-backend snapshot semantics, dedicated export connection pools, streaming data access interfaces, and import write coordination with rollback.
- `runtime-operations`: Replace stubbed export/import CLI behavior with real backend-backed execution.

## Impact

- Affected code: `cli/src/commands/admin.rs` (export/import stubs), server router (new admin API routes), new backup_api module, `storage/src/` (export/import data access layer), `mk_core/src/types.rs` (serialization contracts).
- Affected APIs: New `/api/v1/admin/export` and `/api/v1/admin/import` endpoint families under the existing authenticated admin API surface.
- Affected systems: PostgreSQL, Qdrant, DuckDB data access for coordinated reads/writes; CLI client layer for authenticated job management; tenant isolation (RLS) enforcement during scoped exports; observability stack for export job metrics.
