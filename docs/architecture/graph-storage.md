# Graph Storage

## Partitioning contract

The DuckDB graph cold-start path uses month-based partition keys.

- Partition strategy: `YYYY-MM`
- Partition derivation source: node `created_at`
- Example: a node created at `2026-05-19T08:45:00Z` maps to partition `2026-05`

## Operational rules

- Partitions are treated as immutable once a month closes.
- Live writes always belong to the current month partition.
- Cold-start prewarm and lazy-load paths normalize date-like inputs to the
  canonical `YYYY-MM` partition key before reading from object storage.

## Why month partitions

Month partitions keep the S3/object-store keyspace predictable, align with the
 existing cold-start lazy-loading model, and avoid unbounded partition fan-out
 from per-day or per-event naming.

## Phase 2: Event-sourced WAL coordination

When `providers.graph.event_sourcing_enabled = true`, writes are dual-written
to a Postgres event log (`graph_events` table) and the local DuckDB store.

### Write path

1. Client sends mutation (add_node, add_edge, soft_delete)
2. Pod appends event to `graph_events` with advisory-lock-allocated `seq`
3. Pod applies event locally to DuckDB (read-your-writes on writer pod)
4. Other pods' projectors tail the log and apply asynchronously

### Projector

One tokio task per tenant per pod. Polls `graph_events` every 100ms.
Stores `last_applied_seq` in a DuckDB table `graph_projector_state`.

Pod `/readyz` is ungreen when `head_seq - last_applied_seq > 100` (configurable).

### Snapshot + replay

- `snapshot_full` records `snapshot_seq` in S3 metadata
- `restore_from_s3` applies full + deltas, then replays log from `snapshot_seq` to head
- Replay is idempotent: every event has unique `(tenant_id, seq)`

### Divergence verification

`graph_verify::compute_digest(tenant_id)` produces a SHA-256 over sorted
serialized nodes and edges. Pods can be compared via the
`GET /api/v1/internal/graph/digest` endpoint.

### Feature flags and metrics

| Flag / Metric | Description |
|---|---|
| `providers.graph.event_sourcing_enabled` | Enable dual-write to Postgres event log (default: ON) |
| `graph_nodes_total` | Gauge: total live nodes |
| `graph_edges_total` | Gauge: total live edges |
| `graph_traversal_depth` | Histogram: traversal depth per query |
| `graph_duckdb_lock_wait_ms` | Histogram: writer lock acquisition time |
| `graph_partition_load_ms` | Histogram: cold-start partition load time |
| `graph_snapshot_bytes` | Counter: bytes written in snapshots |
