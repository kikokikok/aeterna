## Why

Aeterna is deployed as a Kubernetes ReplicaSet with multiple replicas. Several runtime stores use in-memory data structures (e.g. `HashMap`, `DashMap`, `Vec`) that are local to each process. When more than one replica runs, these stores diverge immediately: writes on replica A are invisible to replicas B and C. This causes silent data loss, inconsistent API responses depending on which replica handles a request, and duplicate lifecycle task execution (every replica runs its own retention purge, decay cycle, etc.).

Affected areas:
1. **RemediationStore** -- in-memory `DashMap` of remediation requests.
2. **DeadLetterQueue** -- in-memory `DashMap` of failed sync/promotion items.
3. **RefreshTokenStore** -- in-memory `DashMap` of plugin-auth refresh tokens.
4. **Rate-limit / circuit-breaker state** -- per-process counters and windows.
5. **WebSocket session registry** -- in-memory map of connected agents per replica.
6. **Lifecycle task scheduling** -- every replica independently runs the same periodic tasks (retention purge, decay, reconciliation) without distributed coordination.

## What Changes

- Migrate the six in-memory stores listed above to Redis, using the existing Redis connection already available in the runtime for working/session memory.
- Introduce a distributed lock (Redis `SET NX EX` or Redlock) for lifecycle tasks so that only one replica executes each periodic job per interval.
- Ensure all stores fall back gracefully to in-memory mode when Redis is unavailable (single-replica / development mode).

## Impact

- **Breaking**: None. The migration is internal; API contracts are unchanged.
- **Operational**: Redis becomes a hard runtime dependency for multi-replica deployments (it is already required for working/session memory).
- **Performance**: Negligible. The affected stores handle low-throughput control-plane data (remediation requests, refresh tokens, dead letters), not hot-path memory reads.

## Capabilities

### Modified Capabilities
- `runtime-operations`: Stores become Redis-backed; lifecycle tasks use distributed locks.
