## Context

Aeterna's graph store backs two product surfaces: the memory consolidation
/ retrieval pipeline and the knowledge graph used by GraphRAG-style answer
synthesis. Both are read-heavy with bursty batch writes (entity extraction,
relation extraction, memory consolidation) rather than transactional UI
writes. Reads are dominated by short-hop neighbour lookups (≤3 hops) and
label-equality scans. Per-tenant graph size today is in the low millions of
edges; the design ceiling we want is ~10M edges per tenant before we should
have to revisit the substrate.

The substrate — DuckDB embedded with parquet snapshots in S3 — is the
right choice for our scale and budget. At 1000 tenants × 10 GB raw graph
each, S3 storage + requests + cold-start lazy loads cost roughly
$45–50/month. Equivalent managed graph DBs (Neo4j Aura, TigerGraph) at the
same storage scale cost two to three orders of magnitude more, plus the
operational cost of running them. This proposal does not relitigate that
choice; it makes that choice work correctly under our actual deployment
topology.

The DuckDB-over-S3 architecture as deployed today:

```
Pod (any of N replicas)
│
├─ DuckDB local file (or :memory: in tests)
│    └─ memory_nodes, memory_edges  (real tables, indexed on tenant_id)
│    └─ Arc<Mutex<Connection>>      (single-writer, single-reader gate)
│
├─ hourly: snapshot full tenant graph -> .parquet -> S3
└─ cold start: lazy-load partitions from S3 within a 3s budget
```

## Goals

- Lift read throughput per pod by 5–10x via connection pooling and edge
  indexes, with no behaviour change visible to callers.
- Bound write divergence across pods to zero by establishing a single
  authoritative log of graph mutations, regardless of which pod a write
  lands on.
- Keep S3 cost flat or lower while improving recovery time after a pod
  restart.
- Make graph health observable: lock-wait, traversal depth, projector lag,
  partition load time.

## Non-goals

- Strict cross-pod read-your-writes at zero latency. We accept up to ~200ms
  p99 of projector lag across pods.
- Multi-region active-active.
- Replacing DuckDB. The whole point is to make DuckDB scale, not to swap it.
- Changing the public `GraphStore` trait read methods. Phase 2 adds two
  methods; existing callers are unaffected.

## Phase 1 — tactical perf and indexing

### Connection pool

DuckDB connections are not thread-safe, but multiple connections to the
same database file in WAL mode coexist for concurrent readers. We replace
`Arc<Mutex<Connection>>` with:

```rust
pub struct DuckDbGraphStore {
    config: DuckDbGraphConfig,
    writer: Arc<Mutex<Connection>>,            // serialises mutations
    reader_pool: Arc<deadpool::Pool<ReaderManager>>, // N readers
}
```

Writer remains single-mutex (correct: WAL still serialises commits
logically). Reader pool sized to `min(num_cpu, 8)` per pod. All `read_*`
paths go through the pool; all `add_*` / `delete_*` paths through the
writer mutex.

### Indexes

Add in `initialize_schema()`:

```sql
CREATE INDEX IF NOT EXISTS idx_edges_tenant_source
  ON memory_edges(tenant_id, source_id);
CREATE INDEX IF NOT EXISTS idx_edges_tenant_target
  ON memory_edges(tenant_id, target_id);
CREATE INDEX IF NOT EXISTS idx_nodes_tenant_label
  ON memory_nodes(tenant_id, label);
```

No data migration needed; DuckDB rebuilds indexes on first startup after
the DDL lands.

### Snapshot strategy

Split `persist_to_s3` into two distinct operations:

- `snapshot_full(tenant_id)` — weekly compaction, full graph as today,
  named `<prefix>/<tenant>/full_<ts>.parquet`.
- `snapshot_delta(tenant_id, since_seq)` — every N events or every M
  minutes (configurable, default 5 min), exports only events newer than the
  last sequence, named `<prefix>/<tenant>/delta_<since_seq>_<ts>.parquet`.

Restore protocol: load the latest `full_*` then apply all `delta_*` files
with `since_seq >= full_seq` in seq order.

### Partitioning

Partition key derivation: `partition_key(node) = format("%Y-%m", node.created_at)`.
The existing `load_partition_data` lazy-load infrastructure already supports
this shape; what is missing is the policy that *defines* the key. We add
`PartitionKeyStrategy::ByMonth` as the default and document it.

### Iceberg

Flip `iceberg.enabled = true` in the production config layer. The code path
is already there. We get manifest-based partition pruning, schema evolution,
and time travel for free. Catalog runs alongside the existing MinIO with
REST configuration.

### Metrics

New counters and histograms in `storage/src/observability/graph_metrics.rs`:

- `graph_nodes_total{tenant_id}` (gauge)
- `graph_edges_total{tenant_id}` (gauge)
- `graph_traversal_depth{tenant_id}` (histogram)
- `graph_duckdb_lock_wait_ms` (histogram)
- `graph_partition_load_ms` (histogram)
- `graph_snapshot_bytes{kind="full"|"delta"}` (counter)

## Phase 2 — event-sourced WAL coordination

### The log of record

A new Postgres table:

```sql
CREATE TABLE graph_events (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    seq         BIGINT NOT NULL,
    kind        TEXT NOT NULL,        -- 'add_node' | 'add_edge' | ...
    payload     JSONB NOT NULL,
    created_at  TIMESTAMPTZ DEFAULT now()
);
CREATE UNIQUE INDEX idx_graph_events_tenant_seq
  ON graph_events(tenant_id, seq);
-- RLS: standard tenant_id policy applies.
```

`seq` is per-tenant monotonic, allocated by a Postgres advisory-lock-bounded
`SELECT COALESCE(MAX(seq), 0) + 1` inside the same transaction as the
INSERT. We deliberately avoid a per-tenant sequence object to keep the
schema simple at our scale.

### Write path

```
  client                pod (any)             postgres            duckdb
  ------                ---------             --------            ------
  POST /memory  ----->  validate
                        BEGIN
                          INSERT graph_events (tenant, seq, kind, payload)
                                                  ----------->
                          COMMIT
                          notify('graph_events_t-<tenant>')
                          apply event locally  ------------------>
                        return 200
```

The local apply on the writer pod is synchronous, which gives
read-your-writes on that pod. Other pods receive the NOTIFY (or the
cursor-poll picks it up), tail the new event, and apply.

### Projector

One projector task per tenant per pod. Cursor stored in-memory + flushed to
a pod-local checkpoint file every N seconds. On startup: load latest S3
full snapshot + apply deltas + apply log events from snapshot offset to
head.

Projector exposes `last_applied_seq(tenant_id)`. The pod's `/readyz` is
ungreen until `head_seq - last_applied_seq <= 100` for every active tenant.

### Snapshot + replay protocol

- Full snapshot writer records the `seq` at the time of snapshot in S3
  metadata: `metadata.snapshot_seq = <N>`.
- Delta partitions are append-only and grouped by their `seq` range.
- Restore = load full(N) + apply delta(N..M) + tail log from M to current
  head. Replay is idempotent because every event has a unique
  (tenant_id, seq).

### Failure modes

- **Postgres unavailable:** writes 503; reads continue against local DuckDB
  (best-effort, may be stale). This is the same posture as today —
  Postgres unavailability already breaks tenant CRUD.
- **Projector falls behind:** pod removes itself from `/readyz` once lag
  exceeds threshold; LB routes around. Projector continues catching up;
  pod re-adds itself when caught up.
- **DuckDB corruption on a single pod:** delete the pod-local DuckDB file,
  restart pod, cold-start protocol rebuilds from S3+log.
- **Clock skew across pods:** irrelevant; ordering is by `seq`, not
  wallclock.

## Migration from today → Phase 1 → Phase 2

1. Phase 1 ships first. No data migration. Backwards compatible.
2. Phase 2 introduces `graph_events` table empty. New writes are dual-written
   for one release: append to log + apply directly to DuckDB. Read paths
   unaffected.
3. Once the log has one full retention window of writes (default 7 days),
   flip the flag: writes go to log only; projector becomes the only thing
   that mutates DuckDB. The dual-write path is removed in the next release.
4. Roll-back at any point in step 3 is `flip the flag back`; no data loss
   because both paths were running.

## Risks

- **Projector bug → silent divergence between pods.** Mitigated by:
  (a) every event carries a content hash; projector verifies on apply,
  refuses divergent state and surfaces a metric;
  (b) periodic `verify` job that picks N random tenants, computes a
  Merkle-style digest of (sorted nodes ∥ sorted edges) on each pod, fails
  loudly if pods disagree.
- **Postgres write throughput becomes the new bottleneck.** Mitigated by:
  (a) JSONB payloads are small (~200 bytes); a single Postgres can sustain
  10k inserts/sec with no tuning;
  (b) if needed, swap the log substrate to Redis Streams in a follow-up
  proposal — the contract is the same.
- **S3 partition explosion (delta files accumulate without compaction).**
  Mitigated by the weekly `snapshot_full` job which supersedes all deltas
  before its `seq`. Add lifecycle rule on the bucket: delta partitions
  expire 14 days after their successor full snapshot.

## Performance expectations

| Workload                          | Today      | Phase 1    | Phase 2 same-pod | Phase 2 cross-pod |
|-----------------------------------|------------|------------|------------------|-------------------|
| 1-hop neighbour, 1M edges         | 1–5 ms     | 0.3–1 ms   | unchanged        | +log RTT (~2ms)   |
| 3-hop traversal, 5M edges         | 50–100 ms  | 10–30 ms   | unchanged        | +log RTT (~2ms)   |
| Concurrent reads QPS / pod        | ~500       | ~3 000–10 000 | unchanged        | unchanged         |
| Bulk insert 100k edges            | ~1 s       | ~1 s       | ~1.2 s           | ~1.2 s            |
| Cold start hydrate 5M-edge tenant | ~3 s       | ~1.5 s     | ~2 s             | ~2 s              |
| Cross-pod read-after-write p99    | divergent  | divergent  | < 5 ms           | < 200 ms          |

Numbers are estimates from the DuckDB benchmark suite + similar
event-sourced systems; we will validate against our own benchmark fixtures
as part of Phase 1 and Phase 2 acceptance.
