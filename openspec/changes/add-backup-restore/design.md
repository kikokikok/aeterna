## Context

Aeterna persists enterprise AI agent data across four storage backends: PostgreSQL+pgvector (memories, knowledge metadata, governance, org units, policies, promotions), Qdrant (semantic/archival vector embeddings), DuckDB (graph nodes and edges), and Redis (working/session memory). The existing CLI exposes `aeterna admin export` and `aeterna admin import` subcommands with argument parsing for target selection, format, layer filtering, import modes, dry-run, and compression, but both commands return hardcoded "server not connected" errors or simulated analysis results.

The server already has an authenticated admin API surface under `/api/v1/admin/` with role-based access control. The CLI uses a shared authenticated client layer that resolves profiles, credentials, and server URLs. This change builds on both surfaces to deliver real export/import functionality.

This change interacts with the multi-tenant governance system: exports must respect Row-Level Security (RLS) and tenant boundaries. Cross-tenant or full-instance exports require PlatformAdmin privileges. The archive format must be self-describing so that imports can validate schema compatibility and data integrity before writing.

A critical operational requirement is that exports run online — while the server is live and serving agent traffic — with minimal impact on live performance. This drives the streaming architecture, connection pool isolation, and resource throttling decisions below.

## Goals / Non-Goals

**Goals:**
- Define a portable, versioned archive format that captures all Aeterna data types with integrity verification.
- Provide server-side asynchronous export/import APIs that coordinate reads/writes across all storage backends.
- Wire the existing CLI stubs to real backend-backed export/import operations.
- Support full, tenant-scoped, layer-scoped, and incremental (since-timestamp) export modes.
- Support merge, replace, and skip-existing import modes with dry-run validation.
- Ensure exports produce consistent snapshots across backends using streaming reads that do not block live writes.
- Minimize live server impact through dedicated connection pools, configurable batch sizes, and resource throttling.
- Emit Prometheus metrics for export/import jobs so operators can monitor impact in real-time.
- Support identity remapping for cross-instance tenant migration.
- Provide offline archive validation for DR drill verification.

**Non-Goals:**
- Continuous replication or streaming backup (CDC-based). This change covers point-in-time snapshots only.
- Backup scheduling infrastructure (cron/operator). Scheduling is a future concern; this change provides the primitives.
- Qdrant-native snapshot restore. Embeddings are exported as float vectors in NDJSON; re-indexing on import uses the existing write path.
- Redis session/working memory export. Volatile session data is excluded from backups by default; only persistent layers are exported.
- Automated DR failover orchestration. This change provides export/import primitives and validation tooling, not failover automation.
- Write quiescing or global lock modes. The design accepts the practical consistency window between PG transaction start and Qdrant snapshot creation.

## Decisions

### Use tar.gz with NDJSON data files as the archive format

**Decision:** The archive is a gzip-compressed tar containing a `manifest.json` (version, scope, checksums, metadata), one NDJSON file per entity type, and a `checksums.sha256` file. NDJSON provides streaming-friendly line-delimited records that can be processed without loading the entire file into memory.

**Why:** NDJSON enables constant-memory streaming for both export (PG cursor -> serialize -> write line) and import (read line -> deserialize -> batch write). This is essential for high-volume deployments with millions of memory entries. The manifest provides integrity verification and schema compatibility checking before any data is imported.

**Alternatives considered:**
- **Single JSON file**: Rejected because large exports (millions of memories) would require loading the entire dataset into memory for both serialization and deserialization.
- **SQLite dump**: Rejected because it couples the archive to one storage backend and does not naturally represent Qdrant or DuckDB data.
- **Protocol Buffers**: Rejected because it adds a schema compilation dependency and reduces human inspectability of archive contents.
- **Parquet**: Considered for embedding vectors (compact binary), but rejected for the general format because it requires specialized tooling to inspect and is not as universally accessible as JSON.

### Use asynchronous server-side job model for export/import

**Decision:** Export and import operations run as server-side background jobs. The CLI initiates a job via POST, polls status via GET, and downloads/confirms via separate endpoints. Jobs support cancellation via DELETE.

**Why:** Large exports can take minutes to hours. Synchronous HTTP responses would time out. The async model allows progress reporting, cancellation, and decouples the CLI session from the server-side work.

**Alternatives considered:**
- **Synchronous streaming response**: Rejected because coordinating consistent reads across PG, Qdrant, and DuckDB within a single HTTP response lifetime is fragile and prevents progress reporting or cancellation.
- **CLI-side direct database access**: Rejected because it bypasses tenant isolation, auth, and server-side coordination.

### Run exports online with dedicated connection pools and streaming reads

**Decision:** Exports run while the server is live. PostgreSQL reads use a dedicated connection pool (configurable, default 2 connections) separate from the main application pool. Data is read via `DECLARE CURSOR ... FETCH N` with configurable batch sizes. Qdrant export uses the scroll API with pagination. DuckDB reads use MVCC snapshots.

**Why:** Dedicated pools prevent export jobs from starving live agent queries of database connections. Cursor-based reads bound memory usage regardless of dataset size. PG REPEATABLE READ isolation gives the export a consistent snapshot without blocking live writes. Qdrant collection snapshots are non-blocking.

**Alternatives considered:**
- **Shared connection pool**: Rejected because a long-running export transaction holding 1+ connections from a pool of 20 could degrade live query latency under load.
- **Bulk SELECT into memory**: Rejected because it would spike memory usage proportional to dataset size, potentially OOM-killing the server for large deployments.
- **Offline-only exports (server down)**: Rejected because it requires planned downtime, which is unacceptable for enterprise BCP/DR.

### Coordinate snapshot consistency via PG serializable transaction plus Qdrant collection snapshot

**Decision:** Export jobs open a PostgreSQL REPEATABLE READ transaction for the duration of PG reads, take a Qdrant collection snapshot at job start, and read DuckDB with a tenant-scoped query. This provides practical point-in-time consistency without requiring distributed transactions.

**Why:** PG, Qdrant, and DuckDB do not share a distributed transaction protocol. The practical consistency model — PG transaction snapshot + Qdrant snapshot + DuckDB MVCC read — ensures each backend provides a self-consistent view. The manifest records snapshot timestamps for each backend so operators know the consistency window.

**Alternatives considered:**
- **Two-phase commit across all backends**: Rejected because Qdrant and DuckDB do not support 2PC.
- **No consistency guarantees**: Rejected because inconsistent exports could produce unrestorable archives where memory entries reference nonexistent knowledge items.
- **Write quiescing**: Considered as an opt-in mode but deferred — the default online mode is sufficient for BCP/DR.

### Enforce tenant scoping via RLS and explicit scope parameter

**Decision:** Export requests include an explicit scope parameter (full-instance, tenant, or layer). Tenant-scoped exports use the caller's tenant context and PostgreSQL RLS to ensure only authorized data is read. Full-instance exports require PlatformAdmin role.

**Why:** Tenant isolation is a core Aeterna invariant. The explicit scope parameter makes the export boundary visible in the manifest and audit trail.

**Alternatives considered:**
- **Always export full instance**: Rejected because it leaks cross-tenant data and violates isolation requirements.
- **Client-side filtering**: Rejected because it requires transferring all data to the client before filtering, which is both slow and insecure.

### Support three import modes with mandatory dry-run validation

**Decision:** Import supports merge (add new, update existing by newer timestamp), replace (overwrite all matching data, requires --force or prior dry-run), and skip-existing (add only new records). Dry-run produces a conflict report without writing data.

**Why:** Different scenarios need different strategies: DR restore uses replace, incremental sync uses merge, cautious migration uses skip-existing. Dry-run prevents accidental data loss by requiring operators to review conflicts before committing.

**Alternatives considered:**
- **Single import mode**: Rejected because different migration scenarios need different conflict resolution strategies.
- **Automatic conflict resolution only**: Rejected because operators need visibility into what will change before committing.

### Export embeddings as-is, re-embed on import is optional

**Decision:** Memory entry embeddings are exported as `Vec<f32>` in NDJSON. On import, embeddings are written directly to Qdrant by default. An optional `--re-embed` flag triggers re-computation using the target instance's embedding provider.

**Why:** Re-embedding millions of entries on every restore is prohibitively slow and expensive (API costs, latency). The default fast path writes archived embeddings directly. Re-embed is needed only when migrating between instances with different embedding models.

**Alternatives considered:**
- **Never export embeddings**: Rejected because re-embedding on every restore is too expensive.
- **Always re-embed on import**: Rejected for the same cost/time reason.

### Identity remapping via explicit mapping file

**Decision:** When importing into a different instance, a `--remap-file mapping.json` provides explicit ID mappings for tenant IDs, user IDs, and org unit IDs. Without a remap file, import runs in strict mode and fails on any ID mismatch.

**Why:** Silent ID mismatches would assign data to the wrong tenant or user. Explicit mapping forces the operator to declare the identity translation, preventing data corruption.

**Alternatives considered:**
- **Automatic ID generation**: Rejected because it breaks referential integrity between memories, knowledge, relations, and governance data.
- **Ignore ID mismatches**: Rejected because it could assign data to the wrong tenant or user.

## Risks / Trade-offs

- **[Risk] Large export jobs consume significant server resources** — Mitigation: dedicated connection pools (default 2 PG connections), configurable batch sizes, cursor-based streaming, Prometheus metrics for real-time monitoring. Estimated impact: ~5-15% additional PG read I/O during export, near-zero impact on write latency.
- **[Risk] Archive format versioning breaks forward compatibility** — Mitigation: manifest includes schema version; import validates version compatibility; maintain backward-compatible readers for older archive versions.
- **[Risk] Qdrant snapshot timing does not perfectly align with PG transaction start** — Mitigation: document the consistency window in the manifest with per-backend timestamps. For most BCP/DR scenarios, the sub-second gap is acceptable.
- **[Risk] Re-embedding on import with a different model produces different similarity results** — Mitigation: document that re-embed changes search behavior; log the source and target embedding model in the manifest.
- **[Risk] Importing into a tenant with existing data in replace mode causes data loss** — Mitigation: require explicit `--force` flag for replace mode; dry-run is mandatory before first non-dry-run replace import.
- **[Risk] Export job holds PG transaction open for extended duration** — Mitigation: cursor-based reads with configurable fetch size allow the transaction to make steady progress. Export can be cancelled at any time via DELETE endpoint. Monitor via `aeterna_export_duration_seconds` metric.
- **[Risk] Disk space exhaustion from large archives** — Mitigation: archives are written to a configurable temp directory; support streaming to S3/MinIO with multipart upload as a Phase 2 enhancement.

## Migration Plan

1. Define the archive format specification and backup-restore capability spec with operational guarantees.
2. Implement core archive serialization/deserialization (manifest, NDJSON streaming readers/writers, checksum validation) as a module in the storage or a new backup crate.
3. Add server-side export data access layer with dedicated PG connection pool, cursor-based streaming reads from PG, scroll-based reads from Qdrant, and tenant-scoped DuckDB reads.
4. Add server-side import data access layer with batch writes, conflict detection, and transactional rollback.
5. Add REST API endpoints for export/import job management behind the existing admin auth layer, with progress tracking and cancellation.
6. Wire CLI export/import stubs to real server-backed operations through the shared client.
7. Add incremental export, tenant migration, and identity remapping support.
8. Add governance/policy and audit trail export/import.
9. Add Prometheus metrics, DR validation tooling, and operator documentation.

## Open Questions

- Should the archive support streaming to S3/MinIO directly from the server (rather than local temp file), and should this be Phase 1 or Phase 2?
- What is the maximum supported archive size before we need chunked archives or split files?
- Should import of governance policies trigger policy conflict detection automatically, or should that be a separate post-import validation step?
- Should the archive format support partial restore (e.g., restore only knowledge items from a full backup)?
- Should we add a `--max-duration` timeout for export jobs to prevent runaway transactions?
