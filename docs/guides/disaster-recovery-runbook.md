# Disaster Recovery Runbook

## Overview

This runbook outlines the procedures for recovering the Aeterna platform from critical failures.

## Recovery Time & Point Objectives (RTO/RPO)

- **RTO (Recovery Time Objective)**: 15 minutes.
- **RPO (Recovery Point Objective)**: 5 minutes.

## Component Recovery Procedures

### 1. PostgreSQL (Metadata)

**RTO Target**: 5 minutes.

**Detection Method**:
- Patroni reports no healthy primary.
- Prometheus `up` metric for PostgreSQL instances is 0.

**Automated vs Manual Failover**:
- **Automated**: Patroni automatically elects a new primary from healthy replicas.
- **Manual**: Trigger a manual switchover if the automated failover is stuck.

**Restore Steps**:
1. Run the backup script: `infrastructure/ha/backups/backup-postgres.sh`.
2. To restore from a WAL-G or PGBackRest backup:
   ```bash
   # Restore from the latest base backup and replay WALs
   kubectl exec -it patroni-postgresql-0 -- pg_restore -d aeterna /backups/latest.dump
   ```
3. Verify data integrity using the restore test script:
   ```bash
   bash infrastructure/ha/backups/restore-test.sh postgres
   ```

### 2. Qdrant (Vector Storage)

**RTO Target**: 10 minutes.

**Detection Method**:
- Qdrant API returns 503 Service Unavailable.
- `aeterna_qdrant_cluster_status` is "Red".

**Automated vs Manual Failover**:
- **Automated**: Qdrant cluster replicates data across nodes. If one node fails, others serve the traffic.
- **Manual**: Replace the failed node and trigger a shard redistribution.

**Restore Steps**:
1. Run the Qdrant backup script: `infrastructure/ha/backups/backup-qdrant.sh`.
2. Restore from a collection snapshot:
   ```bash
   curl -X POST http://qdrant:6333/collections/{collection_name}/snapshots/recover \
     -H 'Content-Type: application/json' \
     --data '{"location": "http://backup-server/snapshots/latest.snapshot"}'
   ```
3. Test connectivity and search relevance:
   ```bash
   bash infrastructure/ha/backups/restore-test.sh qdrant
   ```

### 3. Redis (Cache & Pub/Sub)

**RTO Target**: 2 minutes.

**Detection Method**:
- Redis Sentinel reports ODown (Objective Down) for the master.
- Memory operations dashboard shows 100% cache miss rate.

**Automated vs Manual Failover**:
- **Automated**: Redis Sentinel promotes a replica to master and reconfigures others.
- **Manual**: Manually promote a replica if Sentinels fail to reach quorum.

**Restore Steps**:
1. Run the Redis backup script: `infrastructure/ha/backups/backup-redis.sh`.
2. If the persistent store (AOF/RDB) is corrupted:
   ```bash
   # Replace the dump.rdb with the latest backup
   cp /backups/redis/latest.rdb /data/dump.rdb
   kubectl delete pod redis-master-0
   ```
3. Verify cache health:
   ```bash
   bash infrastructure/ha/backups/restore-test.sh redis
   ```

## DR Drill Schedule

To ensure the effectiveness of these procedures, Aeterna teams must perform quarterly DR drills.

| Quarter | Drill Type | Scope |
|---------|------------|-------|
| Q1 | Primary Failover | PostgreSQL (Patroni) + Redis (Sentinel) |
| Q2 | Regional Outage | Full site failover to a secondary AZ |
| Q3 | Data Corruption | Restore from snapshots + WAL replay |
| Q4 | Resource Exhaustion | Scale-up and shard redistribution |

## Post-Mortem Template

After every incident or drill, complete a post-mortem report:
- **Timeline**: When was the failure detected, when was it resolved?
- **Root Cause**: What triggered the failure?
- **Resolution**: Which recovery steps were followed?
- **Lessons Learned**: How can we improve our RTO/RPO or automated failover?
- **Action Items**: New tasks to improve system resilience.
