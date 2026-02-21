# Local Mode Deployment Guide

This document explains **local mode** — the default Aeterna deployment pattern where all components run within a single Kubernetes cluster. Local mode is ideal for development, single-cluster production, and air-gapped environments.

## What is Local Mode?

In local mode, Aeterna deploys all required components **in-cluster**:
- **PostgreSQL** (via CloudNativePG) for episodic memory
- **Dragonfly** or **Valkey** for session cache (Redis-compatible)
- **Qdrant** vector database for semantic search
- **OPAL** authorization engine (Cedar-based policies)
- **Agentic Runtime** (Cedar Agent, Planner, Orchestrator)

All components run in the same Kubernetes cluster. There is no external central server or cloud dependency.

### Configuration

Enable local mode in `values.yaml`:

```yaml
deploymentMode: local

# All components enabled by default
postgresql:
  cloudnativepg:
    enabled: true
    cluster:
      instances: 3  # HA setup

cache:
  enabled: true
  backend: dragonfly  # or valkey

vectorDatabase:
  qdrant:
    enabled: true
    replicas: 3

opal:
  enabled: true
  authBackend: cedar
```

## Resource Requirements

### Minimum Cluster Specifications

Local mode requires sufficient resources for HA (high availability) clusters:

| Component | CPU | Memory | Storage | Notes |
|-----------|-----|--------|---------|-------|
| PostgreSQL (3 nodes) | 500m each | 2Gi each | 50Gi-200Gi | pgvector extension |
| Dragonfly (3 nodes) | 250m each | 512Mi each | 10Gi | Redis-compatible cache |
| Qdrant (3 nodes) | 300m each | 512Mi each | 20Gi-100Gi | Vector embeddings |
| OPAL (2 nodes) | 200m each | 256Mi each | 1Gi | Cedar policies |
| Aeterna (3 pods) | 200m each | 512Mi each | — | Application pods |

**Total Minimum**: 8GB RAM, 3 nodes, 150GB storage (highly dependent on data volume)

### Recommended Configuration for Production

```yaml
# PostgreSQL (3 replicas for HA)
postgresql:
  cloudnativepg:
    cluster:
      instances: 3
      resources:
        requests:
          cpu: 1
          memory: 4Gi
        limits:
          cpu: 2
          memory: 8Gi
      storage:
        size: 200Gi
        storageClass: fast-ssd

# Dragonfly (high-throughput cache)
cache:
  dragonfly:
    replicas: 3
    resources:
      requests:
        cpu: 500m
        memory: 1Gi
      limits:
        cpu: 1
        memory: 2Gi
    persistence:
      size: 30Gi

# Qdrant (vector database)
vectorDatabase:
  qdrant:
    replicas: 3
    resources:
      requests:
        cpu: 500m
        memory: 1Gi
      limits:
        cpu: 2
        memory: 4Gi
    storage:
      size: 50Gi

# Agentic runtime
aeterna:
  replicas: 3
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 1
      memory: 2Gi
```

## Limitations of Local Mode

### 1. Single-Cluster Bound

Local mode runs entirely within **one Kubernetes cluster**. You cannot:
- Distribute agents across multiple geographic regions
- Run separate Aeterna instances without data synchronization challenges
- Achieve true multi-region failover (only cluster-level HA)

**Implication**: If the cluster becomes unavailable, Aeterna is unavailable. Use cluster-level backups and disaster recovery tools (e.g., Velero) to protect against cluster loss.

### 2. No Cross-Region Deployment

Local mode has **no mechanism for replicating data across regions**. For multi-region requirements, use **hybrid mode** with a central server and regional cache layers.

### 3. Storage Bound to Cluster

All persistent volumes are **local to the cluster**. You cannot:
- Migrate data easily to another cluster
- Share data between clusters without external tooling
- Detach storage from the cluster independently

**Mitigation**: Use external storage (e.g., S3, EBS snapshots) for backups:

```yaml
postgresql:
  cloudnativepg:
    backup:
      enabled: true
      destinationPath: "s3://my-bucket/aeterna-backups"
      s3Credentials: aeterna-s3-secret
      schedule: "0 2 * * *"  # Daily at 2 AM
      retention: 30  # Keep 30 days
```

### 4. Scaling Constraints

Local mode scaling is limited by **cluster capacity**:
- Vertical scaling: Limited by node size
- Horizontal scaling: Limited by cluster node count
- Sharding: Not supported; all data in single PostgreSQL instance

**Practical limits**:
- Up to ~50,000 concurrent agents per cluster (CPU-bound)
- Up to ~2TB episodic memory per cluster (storage-bound)
- Up to ~10M vector embeddings (Qdrant limits)

## When to Use Local Mode

### ✅ Good Fit

- **Development & Testing**: Single-node clusters (minikube, kind) for local development
- **Single-Region Production**: Organizations with data residency in one region
- **Air-Gapped Environments**: No external connectivity requirements
- **Proof of Concept**: Rapid deployment without operational complexity
- **Compliance**: All data stays within your cluster (HIPAA, SOC 2)

### ❌ Poor Fit

- **Multi-Region Deployments**: Use hybrid mode instead
- **Global Scale**: Use remote mode with central server architecture
- **High Availability Across Clusters**: Use hybrid mode with regional cache layers
- **Data Sharing Across Orgs**: Use remote mode with API access control

## Configuration Reference

### Core Values for Local Mode

```yaml
# Deployment mode
deploymentMode: local

# Container registry
image:
  repository: ghcr.io/kikokikok/aeterna
  tag: "v1.0.0"
  pullPolicy: IfNotPresent

# Service configuration
service:
  type: ClusterIP  # or LoadBalancer for external access
  port: 8080

# Ingress (optional)
ingress:
  enabled: false  # Set to true for HTTP routing
  className: nginx
  hosts:
    - host: aeterna.local
      paths:
        - path: /
          pathType: Prefix

# PostgreSQL (required in local mode)
postgresql:
  cloudnativepg:
    enabled: true
    cluster:
      instances: 3
      postgresql:
        version: 16
        parameters:
          shared_preload_libraries: "pgvector"
      storage:
        size: 100Gi
        storageClass: standard

# Cache layer
cache:
  enabled: true
  backend: dragonfly  # Alternative: valkey
  dragonfly:
    enabled: true
    replicas: 3
    persistence:
      enabled: true
      size: 20Gi

# Vector database
vectorDatabase:
  qdrant:
    enabled: true
    replicas: 3
    storage:
      size: 50Gi

# OPAL (authorization)
opal:
  enabled: true
  authBackend: cedar
  policyStore: postgresql  # Policies in PostgreSQL

# Network policies
networkPolicy:
  enabled: true
  policyTypes:
    - Ingress
    - Egress

# Pod security
podSecurityPolicy:
  enforce: restricted
  readOnlyRootFilesystem: true
```

### Storage Configuration

Local mode uses **PersistentVolumeClaims (PVCs)** for all stateful components:

```yaml
# Define StorageClass for fast SSDs
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: fast-ssd
provisioner: ebs.csi.aws.com  # AWS example
parameters:
  iops: "3000"
  throughput: "125"
  type: gp3
allowVolumeExpansion: true

---
# PVC for PostgreSQL
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: aeterna-db-data
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-ssd
  resources:
    requests:
      storage: 200Gi

---
# PVC for Qdrant
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: aeterna-qdrant-data
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-ssd
  resources:
    requests:
      storage: 50Gi
```

### Expansion and Patching

**Expand storage** (if StorageClass supports `allowVolumeExpansion`):

```bash
kubectl patch pvc aeterna-db-data -p '{"spec":{"resources":{"requests":{"storage":"300Gi"}}}}'
# PostgreSQL will automatically use expanded capacity (no downtime)
```

**Patch for updates**:

```bash
helm upgrade aeterna ./charts/aeterna \
  -n aeterna \
  -f values-local.yaml \
  --wait
```

## Comparison: Local vs Hybrid vs Remote

| Feature | Local | Hybrid | Remote |
|---------|-------|--------|--------|
| **Location** | Single cluster | Local + central | Central only |
| **Data residency** | All local | Cached locally | All remote |
| **Setup complexity** | Low | Medium | Medium |
| **High availability** | Cluster-level | Cluster + central | Central |
| **Offline operation** | Full access | Degraded | None |
| **Multi-region** | No | Yes | Yes |
| **Compliance** | Local data only | Hybrid sync | Trust model |
| **Cost** | Compute-heavy | Balanced | Minimal edge |
| **Latency** | Sub-1ms | 1-50ms | 50-500ms |

## Disaster Recovery

### Backup Strategy

For local mode, implement **3-2-1 rule**:
- **3 copies** of data (PostgreSQL replicas, Qdrant replication, S3 backup)
- **2 different media** (cluster storage + S3)
- **1 off-site** copy (S3 in different region)

```yaml
postgresql:
  cloudnativepg:
    backup:
      enabled: true
      schedule: "0 2 * * *"
      s3:
        destinationPath: "s3://backup-bucket/aeterna"
        endpoint: "https://s3.us-west-2.amazonaws.com"
        existingSecret: s3-credentials
      retention: 30

vectorDatabase:
  qdrant:
    backup:
      enabled: true
      schedule: "0 3 * * *"
      destination: s3://backup-bucket/qdrant-snapshots
```

### Recovery Procedure

```bash
# 1. Restore from S3 backup
aws s3 cp s3://backup-bucket/aeterna/latest.tar.gz . --region us-west-2

# 2. Create new PostgreSQL cluster from backup
kubectl apply -f - <<EOF
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: aeterna-db-restored
spec:
  instances: 3
  bootstrap:
    recovery:
      source: backup-s3
      recoveryTarget:
        timeline: latest
EOF

# 3. Verify data integrity
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -d aeterna -c "SELECT COUNT(*) FROM knowledge_graph;"

# 4. Restore Qdrant snapshots
curl -X PUT "http://qdrant:6333/snapshots/recover" \
  -H "Content-Type: application/json" \
  -d '{"snapshot_path":"s3://backup-bucket/qdrant-snapshots/latest"}'
```

## Monitoring in Local Mode

Essential dashboards for local mode:

```bash
# PostgreSQL replication lag
kubectl exec -it aeterna-db-1 -n aeterna -- psql -U postgres -c \
  "SELECT EXTRACT(EPOCH FROM now() - pg_last_xact_replay_timestamp()) as replication_lag_seconds;"

# Cache hit ratio (Dragonfly)
kubectl exec -it aeterna-cache-0 -n aeterna -- redis-cli INFO stats | grep hit_ratio

# Qdrant collection status
curl http://qdrant:6333/collections?details=true | jq '.result[] | {name, points_count}'

# Resource utilization
kubectl top nodes
kubectl top pods -n aeterna
```

## Summary

**Local mode is ideal for**:
- Organizations deploying to a single Kubernetes cluster
- Environments requiring data residency in one location
- Development and testing
- Air-gapped deployments

**Key constraints**:
- Single cluster only (no cross-region)
- Storage bound to cluster
- Horizontal scaling limited by cluster size
- Requires external backups for disaster recovery

**Next steps**: If multi-region is needed, see **hybrid-mode.md**. If minimal edge deployment is needed, see **remote-mode.md**.
