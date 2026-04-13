## Why

Aeterna stores data across four backends (PostgreSQL, Qdrant, DuckDB, Redis) with no unified data lifecycle management. A codebase audit reveals 15 critical day-2 operational gaps that cause unbounded storage growth, orphaned cross-layer references, incomplete deletion cascades, and silent data inconsistency. Specifically:

1. **Orphaned vectors**: Deleting a memory from PostgreSQL does NOT delete its Qdrant vector — vectors accumulate forever, leaking storage and polluting search results.
2. **Incomplete cascading deletes**: Deleting a user, tenant, or org unit leaves residual data in role assignments, graph nodes, promotion requests, and governance events.
3. **No cross-layer reconciliation**: PostgreSQL, Qdrant, DuckDB, and Redis can diverge silently with no detection or repair mechanism.
4. **Unbounded audit/event growth**: Governance events, audit logs, drift results, and soft-deleted records accumulate indefinitely with no retention or purge policy.
5. **No storage quotas**: A single tenant can consume all cluster resources with no enforcement or visibility.
6. **No memory aging**: Importance scores never decay, old memories never archive to cold storage, and stale data never expires.
7. **Partial failure leaves inconsistent state**: Multi-step operations (PG insert + Qdrant upsert + graph node) have no compensation on partial failure.
8. **Tenant deactivation is incomplete**: Deactivated tenants leave data in all backends with no purge timeline.
9. **Promotion orphans**: Deleted memories leave orphaned promotion requests in the knowledge repository.
10. **No human-in-the-loop for risky remediations**: Auto-cleanup and drift repair run without operator review for destructive actions.

## What Changes

- Add a unified **data lifecycle manager** that coordinates deletion, archival, and cleanup across all backends with cascading rules and cross-layer consistency checks.
- Add **periodic reconciliation jobs** that detect and repair divergence between PostgreSQL, Qdrant, DuckDB, and Redis — with human-in-the-loop approval for destructive repairs.
- Add **retention policies** for audit logs, governance events, drift results, soft-deleted records, and completed export/import jobs — with configurable TTLs per entity type.
- Add **storage quota enforcement** per tenant with usage monitoring, alerts, and soft/hard limits.
- Add **memory aging** with importance score decay, staleness detection, and automatic archival to cold tier after configurable inactivity period.
- Add **cascading delete completion** for user deactivation, tenant purge, org unit removal, and knowledge item deletion — ensuring all cross-layer references are cleaned.
- Add **dead-letter queue** for permanently failed sync/promotion items with alerting and manual retry surface.
- Add **remediation request system** where the server detects issues, proposes fixes, and queues them for operator approval before executing destructive actions.

## Capabilities

### New Capabilities
- `day2-operations`: Unified data lifecycle management, cross-layer reconciliation, retention policies, storage quotas, memory aging, dead-letter queues, and human-in-the-loop remediation approval system.

### Modified Capabilities
- `memory-system`: Add importance score decay, staleness TTL, cold-tier archival trigger, and Qdrant cascade on delete.
- `knowledge-repository`: Add promotion orphan cleanup, stale proposal garbage collection, and cascade on knowledge item deletion.
- `storage`: Add cross-layer reconciliation, per-tenant storage quota enforcement, audit log hard-purge after archive, and soft-delete TTL enforcement.
- `runtime-operations`: Add lifecycle manager background tasks, health-check-driven cleanup triggers, and remediation request API.

## Impact

- Affected code: `storage/src/` (reconciliation, quotas, retention), `memory/src/` (aging, cold-tier trigger, Qdrant cascade), `knowledge/src/` (promotion GC), `sync/src/` (dead-letter queue), `cli/src/server/` (remediation API, lifecycle manager startup), new `lifecycle/` module or crate.
- Affected APIs: New `GET/POST /api/v1/admin/lifecycle/*` endpoints for remediation requests, quota management, and reconciliation status.
- Affected systems: All four storage backends, cron/background task scheduling, observability (new metrics for orphan counts, quota usage, reconciliation status).
