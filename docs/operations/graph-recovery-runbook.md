# Graph Storage Recovery Runbook

## Pod-local DuckDB corruption

1. Delete the corrupted DuckDB file from the pod's local storage
2. Restart the pod — cold-start protocol triggers automatically
3. Pod restores from S3: latest `full_*.parquet` + subsequent `delta_*.parquet`
4. If `event_sourcing_enabled`, event log replay fills the gap from `snapshot_seq` to head
5. Pod marks itself ready when projector lag ≤ threshold (default: 100 events)

## Projector stuck (lag growing)

Symptoms: pod drops from `/readyz`, `head_seq - last_applied_seq` growing.

1. Check projector logs for repeated errors on a specific event
2. If a single event is poison: skip it by manually advancing `graph_projector_state.last_applied_seq` in DuckDB
3. If Postgres connectivity issue: resolve Postgres, projector auto-recovers
4. Nuclear option: delete DuckDB, restart pod for full cold-start rebuild

## Log compaction overdue (delta files accumulating)

1. Trigger a manual `snapshot_full` for affected tenants
2. All deltas before the full snapshot's `snapshot_seq` are now superseded
3. Configure S3 lifecycle rule: delete delta files 14 days after successor full snapshot

## Full S3 restore (disaster recovery)

1. Ensure S3 bucket contains at least one `full_*.parquet` per tenant
2. Deploy fresh pods with empty local storage
3. Each pod calls `restore_from_s3(tenant_id)` on startup
4. Restore order: full snapshot → deltas (sorted by timestamp) → event log replay
5. Verify convergence: call `GET /api/v1/internal/graph/digest?tenant_id=X` on each pod, compare digests

## Consistency verification (automated)

The `verify_graph_consistency` cron job runs hourly and checks all active tenants:

1. Enumerates tenants via `tenant_store.list_tenants(false)`
2. For each tenant, computes a SHA-256 digest of sorted nodes + edges using `graph_verify::compute_digest_hex()`
3. On digest mismatch or computation failure, increments the `graph_consistency_divergences_total` Prometheus counter and logs at `error` level
4. Healthy tenants are logged at `debug` level

**Manual trigger**: restart the pod or call the digest endpoint directly:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  "https://aeterna.example.com/api/v1/internal/graph/digest?tenant_id=<tenant-id>"
```

**Alert on**: `graph_consistency_divergences_total` increasing. Investigate with per-pod digest comparison, then follow the "Pod-local DuckDB corruption" runbook above if needed.
