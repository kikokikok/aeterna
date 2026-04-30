## Why

The DuckDB-backed `GraphStore` (`storage/src/graph_duckdb.rs`) is the actual
graph backend in production for memory + knowledge linking. The Postgres
`impl GraphStore for PostgresBackend` is dead code referencing tables that
no migration creates and is removed in a sibling commit.

The DuckDB design has three real fitness gaps that bound how far we can scale
before we hit either a perf cliff or a correctness problem:

1. **Single-mutex serialisation.** Every read goes through
   `Arc<Mutex<Connection>>`. That is correct (DuckDB connections are not
   thread-safe) but wastes 5–10x of available read throughput. The same lock
   forces traversal queries to wait behind point writes.
2. **Missing edge indexes on the hot lookup paths.** Schema has
   `(tenant_id)` and `(tenant_id, deleted_at)` on nodes; edges have no
   composite covering index for neighbour lookup. Multi-hop traversals scan
   edge ranges they should seek into.
3. **Per-pod local file in a stateless multi-replica architecture.** Each
   pod has its own DuckDB; two pods writing the same tenant produce divergent
   graphs. Today we get away with this because writes are batch-y and rare
   and clients tend to be sticky, but it is structurally incorrect and the
   moment we add a second writer the silently-diverging graph is a
   correctness bug. Hourly full-tenant snapshots to S3 are also a write
   amplifier and a freshness ceiling for cold restarts.

## What Changes

This change is staged. Phase 1 is tactical, ships in v1.6.0, and removes
the perf gap without touching the architecture. Phase 2 is the strategic
move that fixes the multi-pod coordination problem, ships in v1.6.x or v1.7,
and requires a migration but no API change.

### Phase 1 — tactical performance and indexing (v1.6.0)

- Replace `Arc<Mutex<Connection>>` with a connection pool: one writer + N
  readers (DuckDB WAL mode supports concurrent readers).
- Add composite covering indexes to `memory_edges`:
  `(tenant_id, source_id)`, `(tenant_id, target_id)`,
  and a label index on `memory_nodes(tenant_id, label)`.
- Replace hourly full-tenant parquet snapshots with incremental delta
  partitions (per-day or per-N-events), full snapshots only weekly for
  compaction. Drops S3 PUT amplification and shrinks restore lag.
- Define a partition-key policy for `memory_nodes` / `memory_edges`
  partitioned by `created_at` month, so the existing `load_partition_data`
  cold-start path can lazy-load older partitions on demand.
- Turn Iceberg ON in production (the code path exists, the catalog config is
  default-off). This buys us partition pruning, schema evolution, and
  manifest-based time travel without any code change.
- Add structural metrics: edges_total, nodes_total, traversal_depth_p99,
  duckdb_lock_wait_ms_p99, partition_load_ms_p99. We currently observe none
  of these.

### Phase 2 — event-sourced WAL coordination (v1.6.x → v1.7)

- Introduce a shared `graph_events` append-only log in PostgreSQL as the
  source of truth for all graph mutations (per-tenant monotonic sequence,
  RLS-scoped). Writes from any pod APPEND to the log and return.
- Each pod runs a per-tenant projector that tails the log
  (LISTEN/NOTIFY + cursor) and applies events into its local DuckDB.
- Snapshots to S3 are now compaction checkpoints carrying a sequence offset;
  cold start = load latest snapshot + replay events from offset to head.
- Read-your-writes within a pod is synchronous on commit; across pods the
  bound is the projector lag (target p99 < 200ms).
- The `GraphStore` trait grows two methods: `append_event(...)` (write path)
  and `last_applied_seq(tenant_id)` (observability). Existing read methods
  are unchanged.

### Explicitly rejected alternatives

- **Redis lease as tenant-write-master selector.** Rejected: adds a
  request-time hop on 50% of writes, exposes a 5–30s lease-TTL availability
  gap on pod failover, and does not solve cross-pod read-your-writes (still
  need projection / invalidation). The lease pattern fits when a single
  authoritative copy MUST exist; in our model the log is the truth.
- **Raft / consensus over the edge-log.** Rejected: DuckDB is not
  replication-aware, so we would still be replicating an external log and
  projecting it locally — which is Phase 2 with extra steps. Raft buys
  cross-region strict ordering we do not need.
- **Postgres-as-graph-source-of-truth (DuckDB as cache only).** Rejected
  short-term: round-trips every traversal to Postgres, which loses the
  reason we chose DuckDB. Reconsider only if Phase 2 reveals projection
  drift bugs we cannot bound.
- **StatefulSet single-writer pod.** Rejected: does not scale, makes the
  writer a bottleneck and a SPOF.
- **Sticky LB routing (consistent hash tenant→pod).** Rejected: a tenant's
  pod going down takes the tenant offline until reschedule.

## Capabilities

### Modified Capabilities
- `storage`: graph store gains a connection pool and additional indexes;
  snapshot strategy becomes incremental + weekly compaction; partitioning
  policy becomes explicit; the trait grows event-sourced methods in Phase 2.

### New Capabilities
- `graph-coordination`: defines the event-sourced WAL contract — the
  `graph_events` log shape, the projector's read-your-writes guarantees,
  the snapshot+replay restore protocol, the per-pod projector lag SLO, and
  the failure semantics when projection falls behind.

## Impact

- **Affected code (Phase 1)**:
  - `storage/src/graph_duckdb.rs` — connection-pool refactor; index DDL in
    `initialize_schema`; snapshot strategy split into `snapshot_full` (weekly)
    and `snapshot_delta` (per-N-events); explicit partition-key derivation.
  - `storage/src/observability/graph_metrics.rs` (new) — metrics handles.
  - `cli/src/server/bootstrap.rs` — wire pool size + iceberg defaults.
- **Affected code (Phase 2)**:
  - `storage/migrations/NNN_graph_events.sql` (new) — append-only log table
    with RLS, per-tenant monotonic sequence.
  - `storage/src/graph_event_log.rs` (new) — log writer + tail iterator.
  - `storage/src/graph_projector.rs` (new) — tails the log, applies into
    DuckDB, exposes `last_applied_seq`.
  - `storage/src/graph.rs` — trait additions.
  - `cli/src/server/bootstrap.rs` — spawn projector per tenant; expose
    projector-lag in /healthz.
- **Out of scope** (deliberately):
  - Multi-region active-active. Phase 2's log is single-region; multi-region
    is a separate proposal that would build on this one.
  - Migrating other backends (Qdrant, Postgres KV) onto the same WAL
    pattern. They have different consistency properties; out of scope.
  - Replacing DuckDB with a managed graph DB. The whole point of this
    proposal is to make DuckDB scale; that decision is deferred until we
    actually exhaust this design.
