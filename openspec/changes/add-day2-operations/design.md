## Context

Aeterna persists enterprise AI agent data across PostgreSQL (relational, metadata, governance), Qdrant (vector search), DuckDB (graph), and Redis (session/working memory). Operations that span multiple backends — memory add (PG + Qdrant + graph), memory delete (PG only, Qdrant orphaned), tenant deactivation (flag only, no data cleanup), user removal (partial GDPR flow) — have no unified lifecycle coordination. A codebase audit identified 15 critical gaps that cause silent data divergence, unbounded storage growth, orphaned references, and incomplete cleanup cascades.

The existing codebase has fragments of lifecycle management: soft-delete with `deleted_at` timestamps in graph tables (30-day TTL), Redis TTL on session keys, GDPR deletion flow for user data (incomplete), audit retention archival to S3 (no purge from PG), and drift detection (no auto-remediation). These fragments need to be unified into a coherent system that handles the full data lifecycle autonomously while providing operator visibility and approval gates for destructive actions.

The server already has background task infrastructure (Tokio spawned tasks for export jobs, sync cycles, and metric recording) and a governance approval system (pending requests → approve/reject workflow) that can be extended for remediation approvals.

## Goals / Non-Goals

**Goals:**
- Ensure every deletion cascades to all backends (PG, Qdrant, DuckDB, Redis) with no orphaned references.
- Detect and repair cross-layer divergence (PG-Qdrant reconciliation, graph-memory consistency) automatically, with human approval for destructive repairs.
- Enforce configurable retention policies for audit logs, governance events, drift results, soft-deleted records, and completed jobs.
- Enforce per-tenant storage quotas with soft/hard limits and alerting.
- Age memories via importance score decay and automatic cold-tier archival.
- Provide a dead-letter queue for permanently failed sync/promotion items with manual retry.
- Surface all detected issues and proposed remediations via a remediation request system that operators can review and approve.
- Emit Prometheus metrics for all lifecycle operations (orphan counts, quota usage, reconciliation status, cleanup throughput).

**Non-Goals:**
- Cross-region replication or disaster recovery (covered by `add-backup-restore`).
- Real-time streaming cleanup (batch jobs on configurable schedules are sufficient).
- Tenant data migration between clusters (covered by import/export).
- Compliance-specific retention (GDPR right-to-delete is already partially implemented; this change improves completeness, not compliance certification).

## Decisions

### Unified lifecycle manager as a background task coordinator

**Decision:** A `LifecycleManager` struct runs as a set of periodic Tokio tasks spawned at server startup. Each task handles one lifecycle concern (reconciliation, retention, quota enforcement, aging) on an independent schedule. Tasks share read access to `AppState` and emit remediation requests for destructive actions.

**Why:** Keeps lifecycle logic independent of request handling. Each task can run at its own cadence (reconciliation every 6 hours, retention daily, aging hourly) without blocking API traffic. Reuses existing Tokio spawned-task pattern from export jobs.

**Alternatives considered:**
- **Kubernetes CronJobs for each task**: Rejected because they require separate container images, can't access in-process state (DashMap caches, in-memory job stores), and add deployment complexity.
- **Single monolithic cleanup loop**: Rejected because different concerns have different cadences and failure modes — reconciliation is expensive (cross-backend queries), retention is cheap (single DELETE), aging is medium (UPDATE importance scores).

### Human-in-the-loop remediation approval for destructive actions

**Decision:** When a lifecycle task detects an issue that requires destructive action (deleting orphaned vectors, purging stale records, hard-deleting soft-deleted nodes), it creates a `RemediationRequest` in PostgreSQL rather than executing immediately. Operators review and approve/reject via API or admin UI. Low-risk actions (expired TTL purge, completed job cleanup) execute automatically. High-risk actions (cross-layer reconciliation deletes, tenant data purge) require approval.

**Why:** Autonomous cleanup that silently deletes data is dangerous in enterprise deployments. The governance approval system already exists and can be extended. Operators need visibility into what the system wants to do before it does it.

**Alternatives considered:**
- **Fully automatic cleanup**: Rejected because silent deletion of seemingly-orphaned data could destroy valid records if the detection logic has a bug.
- **Fully manual cleanup**: Rejected because it defeats the purpose of autonomous operations — the system should detect and propose, not wait for operators to hunt.
- **Approval per-item**: Rejected because a reconciliation run might find 10,000 orphaned vectors — item-by-item approval is impractical. Approve the batch instead.

### Risk-tiered action classification

**Decision:** Lifecycle actions are classified into three risk tiers:

- **Auto-execute (no approval)**: Expired TTL cleanup (soft-deleted records past retention, completed export jobs past 24h, Redis key expiry). These are deterministic and time-based — no judgment call.
- **Auto-execute with notification**: Storage quota soft-limit enforcement (reject new writes, emit alert), importance score decay, audit log archival to S3. These are non-destructive but operationally significant.
- **Require approval**: Cross-layer reconciliation deletes (orphaned Qdrant vectors, orphaned graph nodes), tenant data purge after deactivation, dead-letter item permanent discard, promotion orphan cleanup. These are destructive and potentially irreversible.

**Why:** Balances autonomy with safety. Time-based cleanup is deterministic and safe. Cross-layer deletes require human judgment because the detection logic could have false positives.

**Alternatives considered:**
- **Two tiers only (auto vs manual)**: Rejected because the middle tier (notify but proceed) handles important cases like quota enforcement where blocking writes is time-sensitive.
- **Configurable per-action**: Deferred to Phase 2 — the initial tier assignment is hardcoded based on risk analysis.

### Cross-layer reconciliation via sampling, not full scan

**Decision:** Reconciliation jobs sample a configurable percentage of records (default 5%) rather than scanning every record. Full scans are available via API trigger but not scheduled by default.

**Why:** A full PG-Qdrant reconciliation on a million records would take hours and consume significant I/O. Sampling catches systemic issues (e.g., Qdrant delete never called) quickly. If sampling finds orphans, it triggers a targeted full scan of the affected time window.

**Alternatives considered:**
- **Full scan every cycle**: Rejected for performance reasons — a million-record cross-join between PG and Qdrant is prohibitive.
- **Event-driven reconciliation**: Considered but deferred — would require a write-ahead log or change-data-capture stream from PG, which adds infrastructure.

### Per-tenant storage quotas via PostgreSQL check + Qdrant collection info

**Decision:** Storage quota enforcement checks `COUNT(*)` and `pg_total_relation_size()` in PostgreSQL per tenant, plus Qdrant collection point count per tenant prefix. Quotas are stored as tenant config fields (`storage_quota_memories_max`, `storage_quota_knowledge_max`, `storage_quota_vectors_max`). Soft limit warns, hard limit rejects writes.

**Why:** Uses existing infrastructure (tenant config fields, PG queries, Qdrant collection info API). No new storage backend needed. Quota checks are cached in DashMap for 5 minutes to avoid per-request COUNT queries.

**Alternatives considered:**
- **Real-time accounting via event streaming**: More accurate but adds significant complexity and latency.
- **Prometheus-based alerting only**: Rejected because alerting without enforcement doesn't prevent resource exhaustion.

### Memory importance decay as exponential decay on access pattern

**Decision:** A periodic job (hourly) applies exponential decay to importance scores: `new_score = score * (1 - decay_rate)^days_since_last_access`. Memories below a threshold (0.01) after decay are candidates for cold-tier archival. The decay rate is configurable per layer (default 0.05/day for session, 0.01/day for project, 0.005/day for org).

**Why:** The decay formula already exists in `memory/src/promotion/mod.rs` for hindsight promotion but is not applied to general memories. Extending it to all memories ensures stale data naturally drops in priority and can be archived.

**Alternatives considered:**
- **Fixed TTL per layer**: Rejected because a frequently-accessed memory should stay hot regardless of age.
- **Manual archival only**: Rejected because operators won't manually review millions of memories.

## Risks / Trade-offs

- **[Risk] False positive orphan detection deletes valid data** — Mitigation: human approval required for reconciliation deletes; sampling before full scan; 7-day quarantine before hard-delete of detected orphans.
- **[Risk] Quota enforcement rejects legitimate writes during burst** — Mitigation: soft limit at 80% (warn only), hard limit at 100%; burst allowance of 10% above hard limit before reject; quota cached for 5 minutes to smooth spikes.
- **[Risk] Reconciliation job consumes excessive I/O** — Mitigation: sampling (5% default), configurable schedule (default 6h), dedicated connection pool, rate-limited queries.
- **[Risk] Importance decay archives frequently-needed old memories** — Mitigation: access-based decay (accessed memories reset decay clock); configurable per-layer decay rates; archived memories remain searchable via cold-tier with higher latency.
- **[Risk] Remediation request backlog grows if operators don't review** — Mitigation: auto-expire remediation requests after 7 days with no action (log and skip); escalation notification after 48h; dashboard widget showing pending count.
- **[Trade-off] Sampling-based reconciliation may miss isolated orphans** — Accepted for V1; systematic orphans (broken delete cascade) will be caught; isolated orphans (single failed request) may survive until full scan.

## Migration Plan

1. Create remediation request table in PostgreSQL and API endpoints for listing/approving/rejecting.
2. Implement cascading delete completion for memory (PG → Qdrant → graph → Redis), knowledge (PG → promotions → relations), user (GDPR + roles + graph), and tenant (all backends).
3. Implement cross-layer reconciliation jobs (PG↔Qdrant, PG↔graph) with sampling and remediation request creation.
4. Implement retention policies (audit log purge, governance event TTL, soft-delete hard-purge, job cleanup).
5. Implement storage quota enforcement (tenant config fields, cached counts, soft/hard limits).
6. Implement memory importance decay and cold-tier archival trigger.
7. Implement dead-letter queue for failed sync/promotion items.
8. Wire lifecycle manager into server bootstrap as periodic background tasks.
9. Add admin UI lifecycle dashboard (remediation queue, quota usage, reconciliation status).
10. Add Prometheus metrics for all lifecycle operations.

## Open Questions

- Should reconciliation repairs be applied immediately after approval, or batched for a maintenance window?
- Should quota limits be per-tenant only, or also per-org/team/project for finer-grained control?
- Should the cold tier be Qdrant with a separate collection, S3 with Parquet, or PostgreSQL with a separate partition?
- Should remediation requests use the existing governance approval system, or a separate approval workflow?
- How should the system handle "Qdrant vector exists but PG record was intentionally deleted by GDPR flow" — is the Qdrant vector an orphan or a compliance gap?
