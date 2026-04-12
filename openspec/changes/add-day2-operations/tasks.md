## 1. Remediation request system

- [ ] 1.1 Create `remediation_requests` PostgreSQL migration table (id, request_type, risk_tier, entity_type, entity_ids, description, proposed_action, detected_by, status, created_at, reviewed_by, reviewed_at, resolution_notes).
- [ ] 1.2 Define `RemediationRequest` struct in `mk_core/src/types.rs` with risk tiers: AutoExecute, NotifyAndExecute, RequireApproval.
- [ ] 1.3 Implement `RemediationStore` in `storage/src/` with CRUD operations (create, list pending, approve, reject, auto-expire after 7 days).
- [ ] 1.4 Add `POST /api/v1/admin/lifecycle/remediations` — list pending remediation requests with filters (risk tier, entity type, status).
- [ ] 1.5 Add `POST /api/v1/admin/lifecycle/remediations/{id}/approve` — approve and execute a remediation.
- [ ] 1.6 Add `POST /api/v1/admin/lifecycle/remediations/{id}/reject` — reject with reason.
- [ ] 1.7 Add escalation: emit alert/notification when remediation request is pending > 48 hours.
- [ ] 1.8 Add auto-expiry: requests pending > 7 days are logged and skipped.

## 2. Cascading delete completion

- [ ] 2.1 Fix memory delete to cascade to Qdrant: when `DELETE FROM memory_entries` executes, also call Qdrant `delete_points()` with matching IDs.
- [ ] 2.2 Fix memory delete to cascade to DuckDB graph: call `soft_delete_nodes_by_source_memory_id()` for deleted memory IDs.
- [ ] 2.3 Fix memory delete to cascade to Redis: delete embedding cache keys matching `embed:*:{memory_id}`.
- [ ] 2.4 Fix knowledge item delete to cascade to promotion requests: delete all `PromotionRequest` records referencing the deleted knowledge item.
- [ ] 2.5 Fix knowledge item delete to cascade to knowledge relations: delete all `KnowledgeRelation` records where source_id or target_id matches deleted item.
- [ ] 2.6 Fix user deactivation (GDPR flow) to include role assignment cleanup: `DELETE FROM user_roles WHERE user_id = $1`.
- [ ] 2.7 Fix user deactivation to include governance event anonymization: replace actor with "[deleted]" in governance events authored by deleted user.
- [ ] 2.8 Fix org unit delete to cascade to role assignments: `DELETE FROM user_roles WHERE resource_id = $1`.
- [ ] 2.9 Fix tenant deactivation to schedule full data purge after configurable quarantine period (default 30 days).
- [ ] 2.10 Implement tenant purge job that runs after quarantine: delete all PG data (memories, knowledge, org units, policies, roles, audit logs), Qdrant collection, DuckDB graph data, Redis keys for the tenant.
- [ ] 2.11 Create `RequireApproval` remediation request for tenant purge before execution.

## 3. Cross-layer reconciliation

- [ ] 3.1 Implement PG↔Qdrant reconciliation: sample N% of memory IDs from PG, verify corresponding Qdrant point exists. Log orphaned PG records (missing vector) and orphaned Qdrant points (missing PG record).
- [ ] 3.2 Implement PG↔DuckDB reconciliation: sample N% of memory IDs from PG, verify corresponding graph node exists. Log orphaned records.
- [ ] 3.3 Implement Qdrant→PG reconciliation (reverse): sample N% of Qdrant points, verify PG record exists. Detect orphaned vectors from incomplete deletes.
- [ ] 3.4 For each detected orphan set, create a `RequireApproval` remediation request with details (orphan count, affected tenant, sample IDs, proposed action).
- [ ] 3.5 Implement remediation executor for approved PG→Qdrant cleanup: delete orphaned Qdrant points.
- [ ] 3.6 Implement remediation executor for approved Qdrant→PG cleanup: re-embed and re-insert missing PG records, or delete orphaned vectors (operator chooses).
- [ ] 3.7 Implement remediation executor for approved graph cleanup: hard-delete orphaned graph nodes.
- [ ] 3.8 Add configurable sampling rate (default 5%) and schedule (default every 6 hours) via env vars.
- [ ] 3.9 Add Prometheus metrics: `aeterna_reconciliation_orphans_detected`, `aeterna_reconciliation_repairs_executed`, `aeterna_reconciliation_duration_seconds`.

## 4. Retention policies

- [ ] 4.1 Implement audit log hard-purge: after `audit_retention` archives to S3, delete PG records older than configurable retention (default 90 days).
- [ ] 4.2 Implement governance event retention: delete events older than configurable TTL (default 180 days) — auto-execute tier.
- [ ] 4.3 Implement drift result retention: delete results older than configurable TTL (default 30 days) — auto-execute tier.
- [ ] 4.4 Implement soft-delete hard-purge: reduce graph soft-delete retention from 30 days to configurable (default 7 days), then hard-delete.
- [ ] 4.5 Implement export/import job cleanup: delete completed/failed job records after 24 hours, clean temp archive files after 1 hour — auto-execute tier.
- [ ] 4.6 Implement promotion request cleanup: delete rejected/abandoned promotion requests older than 30 days — auto-execute tier.
- [ ] 4.7 Add per-entity-type retention config via env vars or tenant config fields.

## 5. Storage quota enforcement

- [ ] 5.1 Define well-known tenant config fields for quotas: `storage_quota_memories_max`, `storage_quota_knowledge_max`, `storage_quota_vectors_max`.
- [ ] 5.2 Implement `TenantStorageUsage` query: COUNT(*) per table per tenant, cached in DashMap with 5-minute TTL.
- [ ] 5.3 Implement soft-limit check (80% of quota): log warning, emit metric, continue accepting writes.
- [ ] 5.4 Implement hard-limit check (100% of quota): reject new writes with HTTP 429, emit metric, create `NotifyAndExecute` remediation request.
- [ ] 5.5 Wire quota check into memory add and knowledge create handlers.
- [ ] 5.6 Add `GET /api/v1/admin/tenants/{tenant}/storage-usage` endpoint returning current usage vs quota.
- [ ] 5.7 Add Prometheus metrics: `aeterna_tenant_storage_usage_count{tenant, entity_type}`, `aeterna_tenant_storage_quota_max{tenant, entity_type}`.

## 6. Memory importance decay and cold-tier archival

- [ ] 6.1 Add `last_accessed_at` column to `memory_entries` table (migration). Update on search hit.
- [ ] 6.2 Implement periodic decay job (hourly): `new_score = score * (1 - decay_rate) ^ days_since_last_access`. Configurable decay rate per layer.
- [ ] 6.3 Implement archival threshold: memories with importance < 0.01 after decay are candidates for cold-tier archival.
- [ ] 6.4 Implement cold-tier archival: move memory content to cold storage (S3 Parquet or separate PG partition), remove Qdrant vector, keep PG stub with `archived_at` timestamp and cold-tier reference.
- [ ] 6.5 Implement cold-tier search: when search results include archived memories, return stub with "archived" flag — client can request full content from cold tier on demand.
- [ ] 6.6 Create `NotifyAndExecute` remediation request for archival batches (> 100 memories) so operators are aware.

## 7. Dead-letter queue for failed sync/promotion

- [ ] 7.1 Add `dead_letter_items` PostgreSQL table (id, item_type, item_id, tenant_id, error, retry_count, first_failed_at, last_failed_at, status).
- [ ] 7.2 Modify sync bridge to move items to dead-letter after configurable max retries (default 5).
- [ ] 7.3 Modify promotion flow to move failed promotions to dead-letter after max retries.
- [ ] 7.4 Add `GET /api/v1/admin/lifecycle/dead-letter` — list dead-letter items.
- [ ] 7.5 Add `POST /api/v1/admin/lifecycle/dead-letter/{id}/retry` — manual retry of a dead-letter item.
- [ ] 7.6 Add `POST /api/v1/admin/lifecycle/dead-letter/{id}/discard` — permanently discard with `RequireApproval` remediation request.
- [ ] 7.7 Add Prometheus metric: `aeterna_dead_letter_count{item_type, tenant}`.

## 8. Lifecycle manager bootstrap

- [ ] 8.1 Create `LifecycleManager` struct that holds references to all lifecycle tasks and schedules.
- [ ] 8.2 Spawn lifecycle tasks at server startup in `bootstrap.rs`:
  - Reconciliation: every 6 hours
  - Retention purge: daily at 03:00 UTC
  - Quota check: every 5 minutes
  - Importance decay: hourly
  - Job cleanup: hourly
  - Dead-letter retry: every 30 minutes
  - Remediation auto-expiry: daily
- [ ] 8.3 Add graceful shutdown: cancel all lifecycle tasks on SIGTERM, wait for in-progress tasks to complete.
- [ ] 8.4 Add feature flag `AETERNA_LIFECYCLE_ENABLED` (default true) to disable all lifecycle tasks.
- [ ] 8.5 Add per-task schedule override via env vars (e.g., `AETERNA_LIFECYCLE_RECONCILIATION_INTERVAL_SECS`).

## 9. Admin UI lifecycle dashboard

- [ ] 9.1 Create `src/pages/admin/LifecyclePage.tsx` — unified lifecycle operations dashboard.
- [ ] 9.2 Add remediation queue widget: list pending remediations with approve/reject actions.
- [ ] 9.3 Add reconciliation status widget: last run, orphan count, next scheduled run.
- [ ] 9.4 Add storage quota widget per tenant: bar charts showing usage vs quota.
- [ ] 9.5 Add dead-letter queue widget: list items with retry/discard actions.
- [ ] 9.6 Add retention status widget: last purge run, records purged, next scheduled run.

## 10. Observability and alerting

- [ ] 10.1 Add Prometheus metrics for all lifecycle operations (orphan counts, quota usage, reconciliation status, cleanup throughput, remediation pending count, dead-letter depth).
- [ ] 10.2 Add Grafana dashboard template for lifecycle operations.
- [ ] 10.3 Add alerting rules: remediation pending > 48h, quota > 90%, reconciliation orphans > threshold, dead-letter depth > threshold.
- [ ] 10.4 Add structured logging for all lifecycle actions (JSON, includes tenant_id, action, entity_count, duration).

## 11. Sync bridge lifecycle hardening

- [ ] 11.1 Wire sync bridge delete path through `cascade_delete_memories` so Qdrant vectors and graph nodes are cleaned when sync removes a memory entry.
- [ ] 11.2 Acquire distributed lock (Redis-based via existing `distributed-lock` crate) at the start of `run_sync_cycle()` to prevent concurrent sync corruption in multi-instance deployments.
- [ ] 11.3 Wire automatic checkpoint rollback: wrap `run_sync_cycle()` body in a guard that calls `rollback()` on any error before returning.
- [ ] 11.4 Move failed sync items to dead-letter queue after 5 retries instead of accumulating in `failed_items` forever.
- [ ] 11.5 Wire summary invalidation cascade: when a parent-layer summary is invalidated, walk child depths and mark them stale too.
- [ ] 11.6 Apply `redact_pii()` to memory content in the `/sync/pull` device endpoint response.
- [ ] 11.7 Wire `ResolutionPromoted` hindsight events to trigger knowledge entry creation so promoted resolutions sync back to the knowledge layer.
- [ ] 11.8 Add sync health metrics: `aeterna_sync_drift_score`, `aeterna_sync_failed_items_count`, `aeterna_sync_last_cycle_duration_ms`, `aeterna_sync_orphaned_pointers_count`.
- [ ] 11.9 Make orphaned pointer cleanup eager (on knowledge delete) rather than lazy (next sync cycle): call pointer cleanup in the cascading delete for knowledge items.
- [ ] 11.10 Add reconciliation check for sync state consistency: verify all pointer_mapping entries point to existing memory and knowledge records.

## 12. Testing

- [ ] 12.1 Unit tests for cascading delete logic (mock PG + Qdrant + graph, verify all layers cleaned).
- [ ] 12.2 Unit tests for reconciliation sampling and orphan detection.
- [ ] 12.3 Unit tests for remediation request lifecycle (create → approve → execute, create → reject, auto-expire).
- [ ] 12.4 Unit tests for storage quota enforcement (soft limit, hard limit, burst allowance).
- [ ] 12.5 Unit tests for importance decay formula (verify exponential decay, access reset, archival threshold).
- [ ] 12.6 Unit tests for dead-letter queue (max retries, discard, manual retry).
- [ ] 12.7 Integration tests for cross-layer reconciliation (testcontainers PG + Qdrant).
- [ ] 12.8 Integration tests for full tenant purge cascade (all backends cleaned).
- [ ] 12.9 Unit tests for sync bridge lifecycle (distributed lock, checkpoint rollback, eager orphan cleanup).
- [ ] 12.10 Integration tests for sync-delete cascade (delete knowledge → verify memory + Qdrant + graph cleaned).
