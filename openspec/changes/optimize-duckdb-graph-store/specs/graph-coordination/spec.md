## ADDED Requirements

### Requirement: Graph Event Log of Record
The platform SHALL maintain a single append-only log of graph mutations in PostgreSQL as the source of truth for graph state. Every graph mutation (add_node, add_edge, soft_delete_node, soft_delete_edge, update_node, update_edge) SHALL be recorded as one row in `graph_events` before being applied to any pod-local DuckDB. The log SHALL be the only authoritative ordering of mutations across pods.

#### Scenario: Per-tenant monotonic sequence
- **WHEN** N concurrent writers append events for the same tenant
- **THEN** each event SHALL be assigned a strictly monotonic `seq` value within that tenant
- **AND** there SHALL be no gaps and no duplicates in the per-tenant `seq` series
- **AND** the per-tenant ordering SHALL match the order in which the appending transactions committed

#### Scenario: Sequences are independent across tenants
- **WHEN** writers append events for two different tenants concurrently
- **THEN** each tenant's `seq` series SHALL be allocated independently
- **AND** activity in tenant A SHALL NOT introduce gaps in tenant B's `seq`

#### Scenario: Log writes are RLS-scoped
- **WHEN** a request authenticated under tenant A attempts to append an event with `tenant_id = B`
- **THEN** the database SHALL reject the write under the existing tenant_id RLS policy
- **AND** no row SHALL be inserted into `graph_events`

### Requirement: Per-Pod Projector
Each pod SHALL run a projector task that tails `graph_events` and applies events into the pod-local DuckDB. The projector SHALL maintain a `last_applied_seq` per tenant and SHALL apply events idempotently so that re-running the projector cannot double-apply any event.

#### Scenario: Projector applies events in seq order
- **WHEN** events with `seq = 1..N` are appended for a tenant
- **THEN** the projector on every pod SHALL apply them to its local DuckDB in increasing `seq` order
- **AND** `last_applied_seq` SHALL increase monotonically toward N

#### Scenario: Projector restart does not double-apply
- **WHEN** the projector is killed and restarted while applying events
- **THEN** on restart the projector SHALL resume from `last_applied_seq + 1`
- **AND** no event SHALL be applied twice
- **AND** the resulting graph state on that pod SHALL be identical to the state on a pod that never restarted

#### Scenario: Same-pod read-your-writes
- **WHEN** a client appends an event via pod P and immediately issues a read against pod P
- **THEN** the read SHALL observe the effect of the just-appended event
- **AND** the latency from append-commit to read-visible SHALL be below 5 ms p99

#### Scenario: Cross-pod read-your-writes within bounded lag
- **WHEN** a client appends an event via pod A and immediately issues a read against pod B
- **THEN** the read SHALL observe the effect of the appended event within the projector lag SLO
- **AND** the projector lag p99 across pods SHALL be below 200 ms under steady-state load

### Requirement: Snapshot and Replay Restore Protocol
The pod cold-start path SHALL reconstruct the local DuckDB graph for a tenant by (a) loading the latest full snapshot from S3, (b) applying any newer delta snapshots in seq order, and (c) replaying log events from the snapshot's `snapshot_seq` to the current head before the pod marks itself ready for that tenant.

#### Scenario: Cold start converges to canonical state
- **WHEN** a fresh pod with no local DuckDB starts up against a tenant that has events in the log and snapshots in S3
- **THEN** the pod SHALL load the latest full snapshot, apply newer deltas in order, and replay log events from `snapshot_seq` to head
- **AND** after this protocol completes, the pod's graph digest for that tenant SHALL equal the digest computed on any other up-to-date pod

#### Scenario: Replay is idempotent under partial failure
- **WHEN** the cold-start protocol is interrupted and re-run
- **THEN** the resulting state SHALL be identical to a single uninterrupted run
- **AND** no event SHALL be applied twice and none SHALL be skipped

### Requirement: Projector Lag Gates Pod Readiness
The pod's `/readyz` endpoint SHALL report the pod as ready for a tenant only when the projector's `last_applied_seq` is within a configurable threshold of the log's head sequence for that tenant. Pods that fall behind SHALL be removed from the load-balancer's rotation until they catch up.

#### Scenario: Pod is unready while projector is behind
- **WHEN** `head_seq(tenant) - last_applied_seq(tenant) > projector_lag_threshold`
- **THEN** the pod's `/readyz` SHALL report not-ready
- **AND** the load balancer SHALL stop routing traffic for that tenant to the pod

#### Scenario: Pod becomes ready when projector catches up
- **WHEN** the projector applies enough events to bring lag back within the threshold
- **THEN** `/readyz` SHALL report ready
- **AND** the load balancer SHALL resume routing traffic to the pod

### Requirement: Cross-Pod Divergence Detection
The platform SHALL provide a mechanism to detect divergence between pods' local DuckDB state for the same tenant. Each pod SHALL expose an internal endpoint that returns a deterministic digest of (sorted nodes ∥ sorted edges) for a given tenant. A periodic verification job SHALL sample random tenants, query every pod's digest, and alert when digests diverge.

#### Scenario: Identical pods produce identical digests
- **WHEN** two pods are both fully caught up on the log for a tenant
- **THEN** the digest endpoint SHALL return identical bytes from both pods

#### Scenario: Verification job alerts on divergence
- **WHEN** the verify job queries N pods for a tenant and at least two return different digests
- **THEN** the job SHALL emit an alert with the involved pod ids and the tenant id
- **AND** the metric `graph_pod_divergence_detected_total` SHALL increment
