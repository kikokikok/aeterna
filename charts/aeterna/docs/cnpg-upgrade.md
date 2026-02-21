# CloudNativePG Upgrade Procedure

This guide covers upgrading CloudNativePG (CNPG), the PostgreSQL operator managing database clusters in Aeterna deployments. CNPG handles both operator upgrades and PostgreSQL instance version management.

## Version Compatibility

The Aeterna chart pins CloudNativePG to version **0.23.x** series. Check your current version:

```bash
kubectl get deployment -n cnpg-system cnpg-controller-manager -o jsonpath='{.spec.template.spec.containers[0].image}'
# Output example: ghcr.io/cloudnative-pg/cloudnative-pg:1.23.1
```

Verify compatibility with your PostgreSQL instances:
- CNPG 0.23.x supports PostgreSQL 12, 13, 14, 15, 16
- Extensions (pgvector, PostGIS, etc.) must match PostgreSQL minor version

## Pre-Upgrade Checklist

### 1. Backup Production Databases

Create a backup before any upgrade:

```bash
# Trigger immediate backup via CNPG API
kubectl patch cluster aeterna-db -n aeterna \
  -p '{"spec":{"backup":{"immediateBackup":true}}}' --type=merge

# Wait for backup to complete
kubectl get backup -n aeterna --watch

# Verify backup was stored
kubectl describe cluster aeterna-db -n aeterna | grep -A5 "Last Successful Backup"
```

### 2. Test Upgrade in Staging

Never upgrade production directly. Create a staging environment:

```bash
# Clone PostgreSQL instance from production backup
kubectl apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: aeterna-db-staging
  namespace: aeterna
spec:
  instances: 3
  postgresql:
    parameters:
      shared_preload_libraries: "pgvector"
  bootstrap:
    recovery:
      source: aeterna-db-prod-backup
      recoveryTarget:
        timeline: latest
EOF

# Wait for recovery to complete
kubectl logs -n aeterna aeterna-db-staging-1 -f | grep "consistent recovery point reached"
```

### 3. Check Cluster Health

Verify all replicas are healthy:

```bash
kubectl get pod -n aeterna -l cnpg.io/cluster=aeterna-db
# All pods should show STATUS: Running

# Verify streaming replication is active
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "\x" -c "SELECT * FROM pg_stat_replication;"
# Should show all replicas in "streaming" state

# Check extension versions
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "SELECT name, version FROM pg_available_extensions WHERE name IN ('pgvector', 'uuid-ossp');"
```

## Operator Upgrade Steps

### Step 1: Update Helm Values

Edit `values-prod.yaml` to pin the new CNPG version:

```yaml
postgresql:
  cloudnativepg:
    enabled: true
    # Helm chart version for CNPG operator
    version: "0.23.x"  # e.g., 0.23.3
    # Repository URL (CNPG hosts its own chart)
    repository: "https://cloudnative-pg.github.io/charts"
    # Image tag for operator
    operatorImage:
      tag: "1.23.1"  # Matches version above
```

### Step 2: Dry-Run Upgrade

Always test before applying:

```bash
helm upgrade aeterna-db ./charts/aeterna \
  -n aeterna \
  -f values-prod.yaml \
  --dry-run \
  --debug > upgrade-plan.yaml

# Review the manifest
cat upgrade-plan.yaml | grep -A10 "kind: Deployment" | head -20
```

### Step 3: Execute Upgrade

Perform the actual upgrade with a 30-second timeout (CNPG restarts quickly):

```bash
helm upgrade aeterna-db ./charts/aeterna \
  -n aeterna \
  -f values-prod.yaml \
  --timeout 5m \
  --wait

# Monitor operator restart
kubectl rollout status deployment/cnpg-controller-manager -n cnpg-system --timeout=3m
```

### Step 4: Verify Operator is Ready

```bash
# Check operator pod is running
kubectl get pod -n cnpg-system -l app.kubernetes.io/name=cloudnative-pg
# STATUS should be Running and READY should be 1/1

# Verify operator logs show no errors
kubectl logs -n cnpg-system -l app.kubernetes.io/name=cloudnative-pg --tail=50 | grep -i error
# Should return no results
```

## Post-Upgrade Validation

### 1. Database Accessibility

Test connections to ensure databases are still accessible:

```bash
# Port-forward to primary pod
kubectl port-forward -n aeterna aeterna-db-1 5432:5432 &

# Connect using psql
psql -h localhost -U postgres -d postgres -c "SELECT version();"
# Output should show PostgreSQL version unchanged (operator doesn't auto-upgrade PG)

psql -h localhost -U postgres -d postgres -c "SELECT name, installed_version FROM pg_available_extensions WHERE installed_version IS NOT NULL;"
```

### 2. Extension Verification

Critical for pgvector and other extensions:

```bash
# Check pgvector extension is available
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -d aeterna -c "CREATE EXTENSION IF NOT EXISTS pgvector;"

# Verify vector operations work
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -d aeterna -c "SELECT (ARRAY[1,2,3]::vector) <-> (ARRAY[1,1,1]::vector) AS distance;"
```

### 3. Replication Status

Ensure all secondary replicas caught up:

```bash
# On primary
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "
SELECT 
  client_addr,
  state,
  write_lag,
  flush_lag,
  replay_lag
FROM pg_stat_replication;"

# All replicas should have write_lag, flush_lag, replay_lag < 1s
```

## Rollback Procedure

If issues occur immediately after upgrade:

### Quick Rollback (within 30 minutes)

```bash
# Revert Helm chart to previous version
helm rollback aeterna-db -n aeterna 1

# Monitor operator restart
kubectl rollout status deployment/cnpg-controller-manager -n cnpg-system

# Verify cluster reconciliation
kubectl describe cluster aeterna-db -n aeterna | tail -20
```

### Full Rollback (after 30 minutes)

If quick rollback fails, restore from backup:

```bash
# Delete failed cluster
kubectl delete cluster aeterna-db -n aeterna

# Restore from backup taken before upgrade
kubectl apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: aeterna-db
  namespace: aeterna
spec:
  instances: 3
  bootstrap:
    recovery:
      source: aeterna-db-backup  # Your pre-upgrade backup
      recoveryTarget:
        timeline: latest
EOF

# Monitor recovery
kubectl logs -n aeterna aeterna-db-1 -f
```

## Major PostgreSQL Version Upgrades

When upgrading PostgreSQL itself (e.g., 14 → 15), CNPG uses `pg_upgrade`:

### 1. Prepare Cluster for Upgrade

```yaml
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: aeterna-db
  namespace: aeterna
spec:
  postgresql:
    version: 15  # Change from 14 to 15
  # CNPG will trigger pg_upgrade automatically
```

### 2. Monitor pg_upgrade Process

```bash
# Watch cluster status during upgrade
kubectl describe cluster aeterna-db -n aeterna --watch

# Monitor pg_upgrade logs (takes 5-30 minutes depending on data size)
kubectl logs -n aeterna aeterna-db-1 -f | grep -E "pg_upgrade|copy|link|Performing"

# Check progress via psql
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "SELECT version();"
# Should show new PostgreSQL version once upgrade completes
```

### 3. Verify Post-Upgrade

```bash
# Check all replicas upgraded
kubectl exec -it aeterna-db-2 -n aeterna -- psql -U postgres -c "SELECT version();"

# Analyze tables after major version upgrade (required)
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -d aeterna -c "ANALYZE;"

# Check extension compatibility with new version
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "SELECT * FROM pg_extension WHERE extname = 'pgvector';"
```

## Monitoring During Upgrade

### Key Metrics to Watch

```bash
# Pod restarts (should be 0)
kubectl get pod -n aeterna -l cnpg.io/cluster=aeterna-db \
  --custom-columns=NAME:.metadata.name,RESTARTS:.status.containerStatuses[0].restartCount

# Connection count (should not spike)
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "SELECT count(*) FROM pg_stat_activity;"

# Replication lag (should stay < 1s)
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c "SELECT EXTRACT(EPOCH FROM now() - pg_last_xact_replay_timestamp()) as replication_lag_seconds;"
```

### Prometheus Queries (if monitoring enabled)

```promql
# Operator reconciliation duration
cnpg_operator_reconciliation_duration_seconds

# Database connection count
pg_stat_activity_count

# Replication lag in seconds
pg_replication_lag_seconds
```

## Troubleshooting

### Operator Stuck in CrashLoopBackOff

```bash
# Check logs for errors
kubectl logs -n cnpg-system deployment/cnpg-controller-manager --tail=100

# Common cause: incompatible CNPG/PostgreSQL versions
# Solution: verify version matrix in pre-upgrade checklist

# Force pod restart
kubectl rollout restart deployment/cnpg-controller-manager -n cnpg-system
```

### Cluster Won't Reconcile After Upgrade

```bash
# Check cluster conditions
kubectl describe cluster aeterna-db -n aeterna | grep -A5 "Conditions:"

# If stuck, check pod events
kubectl describe pod aeterna-db-1 -n aeterna | grep -A10 "Events:"

# Last resort: delete cluster and restore from backup (pre-upgrade)
kubectl delete cluster aeterna-db -n aeterna
# Then restore as shown in rollback section
```

## Reference: CloudNativePG Values Section

From `values.yaml`:

```yaml
postgresql:
  cloudnativepg:
    enabled: true
    # Helm chart repo and version
    repository: https://cloudnative-pg.github.io/charts
    version: "0.23.x"
    
    # Operator configuration
    operator:
      image:
        repository: ghcr.io/cloudnative-pg/cloudnative-pg
        tag: "1.23.1"
      replicas: 2  # HA for operator itself
    
    # Cluster configuration
    cluster:
      instances: 3  # Number of PostgreSQL replicas
      postgresql:
        version: 16
        parameters:
          shared_preload_libraries: "pgvector"
      storage:
        size: 100Gi
        storageClass: fast-ssd
    
    # Backup configuration
    backup:
      enabled: true
      schedule: "0 2 * * *"  # 2 AM daily
      retention: 30  # Keep 30 days
```

## Summary

1. **Always backup** before any upgrade
2. **Test in staging** to catch issues early
3. **Monitor closely** during operator restart
4. **Verify extensions** work after upgrade (pgvector, PostGIS, etc.)
5. **Plan PostgreSQL major version upgrades** separately (pg_upgrade is automatic but slow)
6. **Have rollback procedure ready** in case of issues

For additional help, consult the [official CNPG upgrade guide](https://cloudnative-pg.io/documentation/current/upgrade/).
