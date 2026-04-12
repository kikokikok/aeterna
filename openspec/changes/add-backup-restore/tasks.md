## 1. Core archive format and streaming serialization

- [ ] 1.1 Define `manifest.json` schema (version, created_at, scope, source_instance, schema_version, entity counts, file checksums, embedding model metadata, per-backend snapshot timestamps, incremental flag, since-timestamp).
- [ ] 1.2 Implement streaming NDJSON writer that accepts an iterator of serializable records and writes one JSON line per record to a file without buffering the full dataset.
- [ ] 1.3 Implement streaming NDJSON reader that yields deserialized records one line at a time from a file.
- [ ] 1.4 Implement NDJSON serialization for MemoryEntry with embedding `Vec<f32>` as JSON array of floats, including all fields (id, content, embedding, layer, summaries, context_vector, importance_score, metadata, created_at, updated_at).
- [ ] 1.5 Implement NDJSON serialization for KnowledgeEntry, KnowledgeRelation, and PromotionRequest with all fields.
- [ ] 1.6 Implement NDJSON serialization for GraphNode and GraphEdge.
- [ ] 1.7 Implement NDJSON serialization for Policy, OrganizationalUnit, role assignments, and governance events.
- [ ] 1.8 Implement tar.gz archive writer (manifest + NDJSON files + checksums.sha256) with streaming file addition.
- [ ] 1.9 Implement tar.gz archive reader with manifest validation and per-file SHA256 checksum verification.
- [ ] 1.10 Add schema version compatibility checking in the archive reader (reject incompatible versions with clear error).

## 2. Server-side export data access with online operation

- [ ] 2.1 Create dedicated PG connection pool for export operations (configurable size, default 2 connections, separate from main application pool).
- [ ] 2.2 Implement cursor-based streaming reader for PostgreSQL memory entries within a REPEATABLE READ transaction, with configurable fetch size (default 1000 rows).
- [ ] 2.3 Implement cursor-based streaming reader for PostgreSQL knowledge entries, relations, and promotion requests with tenant scoping via RLS.
- [ ] 2.4 Implement cursor-based streaming reader for PostgreSQL governance data (policies, org units, role assignments).
- [ ] 2.5 Implement Qdrant collection snapshot creation at export job start, with scroll-based point extraction by tenant (configurable batch size, default 100 points).
- [ ] 2.6 Implement DuckDB graph export reader with tenant-scoped filtering for nodes and edges.
- [ ] 2.7 Implement coordinated multi-backend export session that manages the PG transaction lifetime, Qdrant snapshot lifecycle (create at start, delete after completion), and DuckDB read coordination.
- [ ] 2.8 Add incremental export filtering (since-timestamp for memories and knowledge via `WHERE updated_at > $since OR created_at > $since`).
- [ ] 2.9 Add layer-scoped export filtering (specific MemoryLayer or KnowledgeLayer).
- [ ] 2.10 Add tenant-scoped export (single tenant via RLS) and full-instance export (PlatformAdmin, all tenants with tenant_id preserved on records).

## 3. Server-side import data access

- [ ] 3.1 Implement batched memory import writer with configurable batch size (default 500 entries per PG transaction batch, 100 per Qdrant upsert batch).
- [ ] 3.2 Implement conflict detection for memory entries (match by id, compare updated_at for merge mode).
- [ ] 3.3 Implement knowledge import writer with conflict detection and resolution modes (merge, replace, skip-existing).
- [ ] 3.4 Implement graph import writer (DuckDB nodes and edges) with referential integrity checks (reject edges with dangling node references).
- [ ] 3.5 Implement governance import writer (policies, org units, role assignments, promotions) with conflict detection.
- [ ] 3.6 Implement Qdrant vector import (direct embedding write from archived vectors, or re-embed via embedding provider when `--re-embed` is specified).
- [ ] 3.7 Implement dry-run validation mode that reads the entire archive, detects conflicts against existing data, and produces a report (entity type, id, reason) without writing.
- [ ] 3.8 Implement identity remapping logic (read mapping.json, translate tenant_id, user_id, org_unit_id on all records before writing, fail if mapping is incomplete).
- [ ] 3.9 Implement transactional import with rollback on failure (PG transaction rollback; Qdrant compensating deletes for points written before failure).

## 4. Server-side REST API endpoints

- [ ] 4.1 Add `POST /api/v1/admin/export` endpoint to initiate export job (accepts scope, target, layer, format, since, compress parameters; returns job_id).
- [ ] 4.2 Add `GET /api/v1/admin/export/{job_id}` endpoint to poll export job status (pending/running/completed/failed, progress percentage, entity counts, per-backend snapshot timestamps).
- [ ] 4.3 Add `GET /api/v1/admin/export/{job_id}/download` endpoint to stream the completed archive to the client.
- [ ] 4.4 Add `DELETE /api/v1/admin/export/{job_id}` endpoint to cancel a running export job (cleanup temp files, release connections, delete Qdrant snapshot).
- [ ] 4.5 Add `POST /api/v1/admin/import` endpoint to upload archive and initiate import job (accepts mode, dry_run, remap_file, re_embed parameters; returns job_id).
- [ ] 4.6 Add `GET /api/v1/admin/import/{job_id}` endpoint to poll import job status, progress, and validation/conflict report.
- [ ] 4.7 Add `POST /api/v1/admin/import/{job_id}/confirm` endpoint to confirm import after dry-run validation (transitions job from validated to executing).
- [ ] 4.8 Wire export/import API routes into the existing admin router with PlatformAdmin/TenantAdmin role authorization.
- [ ] 4.9 Implement background job execution infrastructure (Tokio spawned task with progress tracking via shared state, cancellation via CancellationToken).

## 5. CLI wiring to real backend

- [ ] 5.1 Replace `run_export` stub in `cli/src/commands/admin.rs` with real server-backed export via shared CLI client (POST to initiate, poll status, download archive on completion).
- [ ] 5.2 Replace `run_import` stub with real server-backed import via shared CLI client (upload archive, poll status, display report).
- [ ] 5.3 Add `--since` flag to CLI export for incremental exports (ISO 8601 timestamp).
- [ ] 5.4 Add `--scope` flag to CLI export for tenant-scoped exports (`tenant:<slug>` or `full`).
- [ ] 5.5 Add `--remap-file` flag to CLI import for identity remapping (path to mapping.json).
- [ ] 5.6 Add `--re-embed` flag to CLI import for optional re-embedding on import.
- [ ] 5.7 Add `--force` flag to CLI import for replace mode without requiring prior dry-run confirmation.
- [ ] 5.8 Add `--batch-size` flag to CLI export/import for tuning throughput vs. resource impact.
- [ ] 5.9 Add progress display for long-running export/import jobs (poll loop with status bar showing entity counts and percentage).
- [ ] 5.10 Update CLI JSON output mode (`--json`) to emit structured job status and report data.
- [ ] 5.11 Add `aeterna admin backup validate <archive>` command for offline archive integrity verification (structure, manifest, checksums — no server connection required).

## 6. Governance and audit trail export/import

- [ ] 6.1 Add optional governance events (audit trail) export as `governance_events.ndjson` when `--include-audit` flag is set.
- [ ] 6.2 Add audit event import with timestamp preservation (do not regenerate event timestamps on import).
- [ ] 6.3 Add policy conflict detection as post-import validation step for imported policies (detect overlapping rules, conflicting merge strategies).

## 7. Testing and validation

- [ ] 7.1 Add unit tests for archive format serialization/deserialization (manifest parsing, NDJSON round-trip for each entity type, checksum generation and verification).
- [ ] 7.2 Add unit tests for each import conflict resolution mode (merge picks newer, replace overwrites, skip-existing preserves).
- [ ] 7.3 Add unit tests for identity remapping (complete mapping succeeds, incomplete mapping fails, nested ID references are translated).
- [ ] 7.4 Add integration tests for multi-backend coordinated export with tenant isolation (verify no cross-tenant data leaks).
- [ ] 7.5 Add integration tests for full export/import round-trip (export -> validate -> import -> verify data matches original).
- [ ] 7.6 Add integration tests for incremental export (since-timestamp filtering produces correct delta, no duplicates on merge import).
- [ ] 7.7 Add end-to-end CLI tests for `aeterna admin export` and `aeterna admin import` against a running test server.
- [ ] 7.8 Add property-based tests (proptest) for checksum integrity (corrupted archives are always detected) and NDJSON serialization round-trip.
- [ ] 7.9 Add performance tests verifying export does not exceed resource budgets (connection pool isolation, memory usage bounds, PG read I/O impact).

## 8. DR validation, observability, and documentation

- [ ] 8.1 Add Prometheus metrics for export/import jobs: `aeterna_export_rows_processed`, `aeterna_export_duration_seconds`, `aeterna_export_bytes_written`, `aeterna_import_rows_processed`, `aeterna_import_conflicts_detected`.
- [ ] 8.2 Add restore-report output summarizing what was imported (counts by entity type, conflicts resolved by mode, errors encountered, duration).
- [ ] 8.3 Document DR drill procedure in operator guide: export -> destroy test environment -> import -> validate -> report.
- [ ] 8.4 Document archive format specification for third-party tooling or manual inspection.
- [ ] 8.5 Document incremental backup strategy (full weekly + daily incrementals, retention policy, restore order).
