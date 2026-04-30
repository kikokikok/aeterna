## 1. Phase 1 — Connection pool

- [ ] 1.1 Introduce `ReaderManager` implementing `deadpool::Manager` for read-only DuckDB connections opened against the same database file.
- [ ] 1.2 Refactor `DuckDbGraphStore` to hold `writer: Arc<Mutex<Connection>>` and `reader_pool: Arc<deadpool::Pool<ReaderManager>>`. Pool size = `min(num_cpu, 8)` with explicit override via `DuckDbGraphConfig::reader_pool_size`.
- [ ] 1.3 Route every `add_*`, `update_*`, `delete_*`, `soft_delete_*` method through `writer`. Route every `get_*`, `find_*`, `search_*`, `list_*` method through `reader_pool`.
- [ ] 1.4 Open the database in WAL mode (DuckDB's equivalent setting) so concurrent readers see a consistent snapshot.
- [ ] 1.5 Update existing unit tests; add a new test that fires N concurrent readers + 1 writer and asserts reader QPS scales near-linearly with pool size.
- [ ] 1.6 Bench: capture before/after numbers for the workloads in the design doc's performance table; commit to `storage/benches/graph_pool_bench.rs`.

## 2. Phase 1 — Indexes

- [ ] 2.1 Add `idx_edges_tenant_source`, `idx_edges_tenant_target`, `idx_nodes_tenant_label` to `initialize_schema()` in `graph_duckdb.rs`.
- [ ] 2.2 Verify `EXPLAIN` plan for `get_neighbors`, `find_path`, label search now uses the index seek path (assert in a test).
- [ ] 2.3 Bench: 1-hop and 3-hop traversal on the 5M-edge fixture; record before/after.

## 3. Phase 1 — Snapshot strategy split

- [ ] 3.1 Add `snapshot_full(tenant_id)` to `DuckDbGraphStore` (existing `persist_to_s3` becomes its body, key prefix becomes `full_<ts>.parquet`).
- [ ] 3.2 Add `snapshot_delta(tenant_id, since_seq) -> DeltaSnapshotResult`. Exports rows with `seq > since_seq` as parquet, key `delta_<since_seq>_<ts>.parquet`. Records `since_seq` and `to_seq` in S3 object metadata.
- [ ] 3.3 Update `load_from_s3` to discover the latest `full_*` and apply all subsequent `delta_*` in seq order, verifying checksums on each.
- [ ] 3.4 Add `SnapshotPolicy` config: `full_interval_secs` (default 604800 = weekly), `delta_interval_secs` (default 300 = 5min). Bootstrap reads from `config.providers.graph.snapshot_*`.
- [ ] 3.5 Tests: round-trip full + N deltas restore, delta with no events is a no-op, mixed full/delta order in S3 still restores correctly.

## 4. Phase 1 — Partitioning policy

- [ ] 4.1 Define `PartitionKeyStrategy::ByMonth` in `graph_duckdb.rs` with a `partition_key_for(node) -> String` method (`YYYY-MM`).
- [ ] 4.2 Update `load_partition_data` to call this for the cold-start path.
- [ ] 4.3 Document the partition key contract in `docs/architecture/graph-storage.md` (new file): partitions are immutable once a month closes; live writes always go to the current partition.
- [ ] 4.4 Tests: cold-start with N month-partitions in S3 lazy-loads only the prewarm count (config: 5) and defers the rest.

## 5. Phase 1 — Iceberg in production

- [ ] 5.1 Update prod Helm chart values to set `providers.graph.iceberg.enabled = true` with REST catalog pointing at the existing MinIO.
- [ ] 5.2 Add a smoke test in `storage/tests/iceberg_smoke.rs` that exercises put + get under the iceberg-enabled flag with a MinIO testcontainer.
- [ ] 5.3 Document in `docs/operations/graph-storage.md` how to inspect Iceberg manifests for a tenant (debugging recipe).

## 6. Phase 1 — Metrics

- [ ] 6.1 New `storage/src/observability/graph_metrics.rs` with the six metric handles listed in design.md (`graph_nodes_total`, `graph_edges_total`, `graph_traversal_depth`, `graph_duckdb_lock_wait_ms`, `graph_partition_load_ms`, `graph_snapshot_bytes`).
- [ ] 6.2 Wire each metric at its production call site (lock acquisition, traversal recursion, partition load, snapshot upload).
- [ ] 6.3 Grafana dashboard JSON committed under `ops/grafana/graph-storage.json` with panels for each metric.
- [ ] 6.4 Alert rules: traversal_depth_p99 > 5 (likely a runaway recursion), partition_load_ms_p99 > 5000 (cold-start budget bust), snapshot_bytes per delta growing > 10x baseline (compaction overdue).

## 7. Phase 2 — Event log table and writer

- [ ] 7.1 New migration `storage/migrations/NNN_graph_events.sql` creating `graph_events` table with the schema in design.md. Include RLS policies mirroring `tenants` (tenant_id-based). Include `idx_graph_events_tenant_seq` UNIQUE index.
- [ ] 7.2 New `storage/src/graph_event_log.rs` exposing `GraphEventLog::append(tenant_id, kind, payload) -> Result<i64>` (returns assigned seq) and `GraphEventLog::tail(tenant_id, from_seq, batch_size) -> Vec<Event>`.
- [ ] 7.3 Implement seq allocation via Postgres advisory lock keyed on hash(tenant_id) within the same transaction as the INSERT. Verify under concurrent load test that no gaps and no duplicates are produced.
- [ ] 7.4 Integration test: 100 concurrent appenders for the same tenant produce 100 events with seq 1..100 in commit order.

## 8. Phase 2 — Projector

- [ ] 8.1 New `storage/src/graph_projector.rs` implementing one tokio task per (pod, tenant) that tails the log via LISTEN/NOTIFY (preferred) or cursor poll fallback (every 100ms when LISTEN is unavailable).
- [ ] 8.2 Apply each event idempotently: derive a deterministic primary key from (tenant_id, seq); INSERT...ON CONFLICT DO NOTHING into DuckDB.
- [ ] 8.3 Maintain `last_applied_seq` per tenant in a small DuckDB table `graph_projector_state(tenant_id, last_applied_seq)`.
- [ ] 8.4 Expose `last_applied_seq(tenant_id) -> i64` on the projector handle.
- [ ] 8.5 `/readyz` integration: pod is ready iff `head_seq(t) - last_applied_seq(t) <= projector_lag_threshold` for every active tenant. Threshold is config; default 100 events.
- [ ] 8.6 Test: kill the projector mid-replay, restart, assert recovery from `last_applied_seq` with no double-apply.

## 9. Phase 2 — Wire into write path

- [ ] 9.1 Extend the `GraphStore` trait with `append_event(...)` and `last_applied_seq(...)`. Default impl on the trait can panic; concrete `DuckDbGraphStore` implements both.
- [ ] 9.2 Refactor `add_node`, `add_edge`, soft-delete operations to (a) build an event payload, (b) call `GraphEventLog::append`, (c) on the local pod also synchronously apply the event into DuckDB.
- [ ] 9.3 Feature flag `providers.graph.event_sourcing_enabled`. Default OFF for one release window. When ON, dual-write becomes log-only; the synchronous local apply still fires for read-your-writes.
- [ ] 9.4 Once dual-write window has elapsed in production with no divergence, default-flip to ON. Remove dual-write code in the following release.

## 10. Phase 2 — Snapshot + replay update

- [ ] 10.1 Update `snapshot_full` to record `seq` at snapshot time in S3 object metadata as `snapshot_seq`.
- [ ] 10.2 Update `snapshot_delta` to be driven by event-log seq ranges (not wallclock).
- [ ] 10.3 Update `load_from_s3` cold-start path: after applying full + deltas, replay log events from `snapshot_seq` to head before the pod marks itself ready.
- [ ] 10.4 Test: cold-start a fresh pod against a non-empty log + S3 snapshot; assert it converges to the same digest as a long-running pod.

## 11. Phase 2 — Divergence verification

- [ ] 11.1 New `storage/src/graph_verify.rs` exposing `compute_digest(tenant_id) -> [u8; 32]`: SHA-256 over (sorted serialised nodes ∥ sorted serialised edges) for that tenant.
- [ ] 11.2 New endpoint `GET /api/v1/internal/graph/digest?tenant_id=X` (PlatformAdmin only, internal network only) returning the digest from the local pod.
- [ ] 11.3 New cron job `verify_graph_consistency` that picks N random tenants, calls each pod's digest endpoint, alerts loudly if any pair diverges.
- [ ] 11.4 Test: inject a known divergence (manually mutate one pod's DuckDB), assert the verify job catches it.

## 12. Phase 2 — Failure mode tests

- [ ] 12.1 Test: Postgres becomes unavailable mid-write — client gets 503, no partial state in DuckDB, retry after Postgres recovery succeeds.
- [ ] 12.2 Test: projector lag exceeds threshold — pod removes itself from `/readyz`, recovers when caught up.
- [ ] 12.3 Test: pod-local DuckDB file is deleted while pod runs — graceful detection, pod restart triggers cold-start protocol.
- [ ] 12.4 Chaos test: 3-pod deployment, random pod kills under sustained mixed read/write load; assert no event loss and digest convergence within 5 minutes after chaos ends.

## 13. Documentation

- [ ] 13.1 New `docs/architecture/graph-storage.md` with the diagrams and protocols from design.md.
- [ ] 13.2 New `docs/operations/graph-recovery-runbook.md` covering pod-local corruption, projector stuck, log compaction overdue, full S3 restore.
- [ ] 13.3 Update `AGENTS.md` (or equivalent) with the new feature flags and metric names so future agents know about them.

## 14. Removal of dead code

- [ ] 14.1 Remove `impl GraphStore for PostgresBackend` from `storage/src/postgres.rs` (dead code; references non-existent tables). This is a sibling commit but listed here for completeness.
