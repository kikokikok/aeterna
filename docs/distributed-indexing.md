# Distributed Indexing Architecture

## Overview

Code Search repository indexing is designed to scale horizontally across multiple pods/containers while maintaining **data locality** - ensuring that operations on a repository always route to the pod where the clone resides.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Ingress                        │
│                  (X-Repo-Affinity Header)                   │
└─────────────────────────┬───────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
   ┌──────────────┐┌──────────────┐┌──────────────┐
   │ Indexer Pod  ││ Indexer Pod  ││ Indexer Pod  │
   │  (Shard 0)   ││  (Shard 1)   ││  (Shard 2)   │
   │ /data/repos  ││ /data/repos  ││ /data/repos  │
   └──────┬───────┘└──────┬───────┘└──────┬───────┘
          │               │               │
          └───────────────┼───────────────┘
                          │
               ┌──────────▼──────────┐
               │  S3 / Object Store  │
               │  (Cold Storage)     │
               └──────────┬──────────┘
                          │
               ┌──────────▼──────────┐
               │      PostgreSQL     │
               │  (Shard Registry)   │
               └─────────────────────┘
```

## Key Components

### 1. ShardRouter (Consistent Hashing)

The `ShardRouter` assigns repositories to indexer pods using **consistent hashing** with virtual nodes for even distribution.

```rust
let router = ShardRouter::new(pool, Some("shard-0".to_string()));

// Register this pod on startup
router.register_shard("shard-0", "codesearch-indexer-0", "10.0.0.5", 100).await?;

// Assign a new repository to a shard
let shard_id = router.assign_shard(repo_id).await?;

// Check if this pod should handle the request
if router.is_local(repo_id).await? {
    // Process locally
} else {
    // Forward to correct pod
}
```

### 2. Pod Registration & Heartbeat

Each indexer pod registers itself on startup and sends periodic heartbeats:

```yaml
# Kubernetes Deployment
env:
  - name: SHARD_ID
    valueFrom:
      fieldRef:
        fieldPath: metadata.name  # e.g., codesearch-indexer-0
  - name: POD_IP
    valueFrom:
      fieldRef:
        fieldPath: status.podIP
```

```rust
// In pod startup
router.register_shard(&shard_id, &pod_name, &pod_ip, 100).await?;

// Heartbeat loop (every 10 seconds)
loop {
    router.heartbeat(&shard_id).await?;
    tokio::time::sleep(Duration::from_secs(10)).await;
}
```

### 3. Cold Storage Backup

Inactive repositories are backed up to S3 as Git bundles:

```rust
let cold_storage = ColdStorageManager::new("codesearch-repo-bundles".to_string()).await;

// Backup before pod shutdown
let uri = cold_storage.backup_repo(&tenant_id, repo_id, &local_path).await?;

// Restore on new pod
cold_storage.restore_repo(&uri, &local_path).await?;
```

### 4. Graceful Shutdown

When a pod receives SIGTERM:

```rust
pub async fn prepare_for_shutdown(&self, shard_id: &str) -> Result<i32, CodeSearchError> {
    // 1. Mark shard as draining (no new assignments)
    router.drain_shard(shard_id).await?;

    // 2. Backup all repos to cold storage
    for repo_id in get_repos_for_shard(shard_id) {
        backup_to_cold_storage(repo_id).await?;
    }

    // 3. Rebalance repos to other shards
    router.rebalance_from_shard(shard_id).await?;
}
```

## Kubernetes Configuration

### StatefulSet for Predictable Pod Names

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: codesearch-indexer
spec:
  serviceName: codesearch-indexer
  replicas: 3
  selector:
    matchLabels:
      app: codesearch-indexer
  template:
    metadata:
      labels:
        app: codesearch-indexer
    spec:
      terminationGracePeriodSeconds: 300  # 5 minutes for backup
      containers:
        - name: indexer
          image: codesearch-indexer:latest
          env:
            - name: SHARD_ID
              valueFrom:
                fieldRef:
                  fieldPath: metadata.name
          lifecycle:
            preStop:
              exec:
                command: ["/bin/sh", "-c", "curl -X POST localhost:8080/prepare-shutdown"]
          volumeMounts:
            - name: repo-data
              mountPath: /data/repos
  volumeClaimTemplates:
    - metadata:
        name: repo-data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 100Gi
```

### Headless Service for Pod Discovery

```yaml
apiVersion: v1
kind: Service
metadata:
  name: codesearch-indexer
spec:
  clusterIP: None  # Headless
  selector:
    app: codesearch-indexer
  ports:
    - port: 8080
```

### Ingress with Affinity Routing

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: codesearch
  annotations:
    nginx.ingress.kubernetes.io/upstream-hash-by: "$http_x_repo_id"
spec:
  rules:
    - host: codesearch.example.com
      http:
        paths:
          - path: /api/repos
            pathType: Prefix
            backend:
              service:
                name: codesearch-indexer
                port:
                  number: 8080
```

## Request Flow

### New Repository Request

1. Request arrives at any pod
2. `ShardRouter::assign_shard()` determines target shard
3. If not local, return redirect to correct pod
4. Clone repository to local disk
5. Trigger indexing

### Existing Repository Operation

1. Client includes `X-Repo-ID` header
2. Ingress routes to correct pod via hash
3. Pod validates it owns the repo
4. Performs operation locally

### Scale-Down Event

1. HPA triggers scale-down
2. Kubernetes sends SIGTERM to victim pod
3. Pod calls `prepare_for_shutdown()`
4. All repos backed up to S3
5. Repos reassigned to remaining pods
6. Pod terminates

## Monitoring

### Key Metrics

| Metric | Description |
|--------|-------------|
| `codesearch_shard_load` | Current repos per shard |
| `codesearch_shard_capacity` | Max repos per shard |
| `codesearch_heartbeat_lag` | Time since last heartbeat |
| `codesearch_cold_storage_ops` | Backup/restore operations |
| `codesearch_rebalance_count` | Repos rebalanced |

### Alerts

```yaml
- alert: ShardUnhealthy
  expr: time() - codesearch_last_heartbeat > 30
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Indexer shard {{ $labels.shard_id }} missed heartbeat"
```

## FAQ

**Q: What happens if a pod dies suddenly?**

A: Other pods will detect the missing heartbeat (>30s). The rebalancing job will:
1. Mark the dead shard as offline
2. Reassign repos to healthy shards
3. New pods will restore from cold storage

**Q: How do we handle very large repositories?**

A: Use `git clone --filter=blob:none` for partial clones, or increase pod storage via PVC.

**Q: Can we use NFS instead of per-pod storage?**

A: Yes, but performance will be lower. Set `accessModes: ["ReadWriteMany"]` on the PVC.
