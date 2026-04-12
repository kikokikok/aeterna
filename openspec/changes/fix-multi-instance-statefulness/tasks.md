## 1. Migrate in-memory stores to Redis

- [ ] 1.1 Migrate `RemediationStore` from `DashMap` to Redis hashes (`aeterna:remediations:{id}`). Preserve existing API (`create`, `list_pending`, `approve`, `reject`, `expire_stale`, `cleanup_old`). Fall back to in-memory when Redis is unavailable.
- [ ] 1.2 Migrate `DeadLetterQueue` from `DashMap` to Redis sorted sets (`aeterna:dlq:{tenant}`). Preserve existing API (`enqueue`, `list_active`, `discard`, `retry`, `cleanup_discarded`, `active_count`). Fall back to in-memory when Redis is unavailable.
- [ ] 1.3 Migrate `RefreshTokenStore` from `DashMap` to Redis keys with TTL (`aeterna:refresh:{token_hash}`). Preserve single-use rotation semantics. Fall back to in-memory when Redis is unavailable.
- [ ] 1.4 Migrate rate-limit / circuit-breaker counters to Redis (`aeterna:ratelimit:{key}`) with `INCR` + `EXPIRE`. Fall back to per-process counters when Redis is unavailable.
- [ ] 1.5 Migrate WebSocket session registry to Redis set (`aeterna:ws:sessions:{tenant}`) so any replica can list connected agents. Fall back to in-memory when Redis is unavailable.

## 2. Add distributed locking for lifecycle tasks

- [ ] 2.1 Implement `DistributedLock` trait with a Redis `SET NX EX` backend (`aeterna:lock:{task_name}`, TTL = task interval + margin). Provide a no-op in-memory implementation for single-replica mode.
- [ ] 2.2 Wrap each lifecycle task (`retention_purge`, `job_cleanup`, `remediation_expiry`, `dead_letter_cleanup`, `importance_decay`) with lock acquisition so only one replica executes per interval.
- [ ] 2.3 Add lock health metrics (acquisitions, contention, expirations) to the Prometheus `/metrics` endpoint.

## 3. Testing and rollout

- [ ] 3.1 Add integration tests (testcontainers) that start two lifecycle managers against the same Redis and verify only one executes each task per interval.
- [ ] 3.2 Add integration tests verifying store convergence: write on one simulated replica, read from another.
- [ ] 3.3 Update Helm chart values to document the Redis dependency for multi-replica deployments.
