# Sizing Guide for Aeterna Deployments

This guide helps you select appropriate resources and infrastructure for Aeterna deployments based on memory volume, user concurrency, and AI agent complexity.

## Sizing Tiers Overview

Aeterna defines three sizing tiers based on total memory objects stored:

| Tier | Memory Count | Use Case | Deployment Target |
|------|-------------|----------|-------------------|
| **Small** | < 1,000 | Dev, testing, small teams | 1-2 nodes, minimal resources |
| **Medium** | 1,000 - 100,000 | Production SMB, mid-size teams | 3-5 nodes, balanced resources |
| **Large** | > 100,000 | Enterprise, multi-tenant, complex agents | 5+ nodes, optimized resources |

Each tier includes pre-configured `values-small.yaml`, `values-medium.yaml`, and `values-large.yaml` sizing files.

## Small Tier Configuration

**Target**: Development, testing, proof-of-concept, <1K memories.

### Minimum Infrastructure

- **Kubernetes Cluster**: 1-2 nodes (single-node acceptable for dev)
- **Node Specs**: 2 CPU cores, 8GB RAM per node
- **Total Cluster**: 2-4 CPU cores, 16GB RAM

### Component Resource Requests and Limits

```yaml
# values-small.yaml
aeterna:
  replicaCount: 1  # Single replica acceptable
  resources:
    requests:
      cpu: 200m
      memory: 512Mi
    limits:
      cpu: 1000m
      memory: 1Gi

postgresql:
  cloudnativepg:
    instances: 1  # Single instance with WAL backup only
    resources:
      requests:
        cpu: 250m
        memory: 512Mi
      limits:
        cpu: 1000m
        memory: 2Gi
    storage:
      size: 10Gi

qdrant:
  replicaCount: 1
  resources:
    requests:
      cpu: 250m
      memory: 512Mi
    limits:
      cpu: 1000m
      memory: 1Gi
  persistence:
    size: 20Gi

dragonfly:
  resources:
    requests:
      cpu: 100m
      memory: 256Mi
    limits:
      cpu: 500m
      memory: 512Mi

opal:
  opalServer:
    replicaCount: 1
    resources:
      requests:
        cpu: 100m
        memory: 256Mi
      limits:
        cpu: 500m
        memory: 512Mi
```

### Storage Sizing

- **PostgreSQL**: 1KB per memory object minimum → 10GB for 1K memories + WAL overhead
- **Qdrant**: 2KB per vector minimum → 20GB for 1K memories
- **Dragonfly Cache**: 256MB to 1GB
- **Total**: ~30-50GB persistent storage

### Network Bandwidth

- **Baseline**: 1-10 Mbps for small deployments
- **Peak bursts**: Agent inference requests (varies by embedding model)

### Use Cases

- Laptop/local machine testing
- Single-engineer development
- Feature demos
- Integration testing

---

## Medium Tier Configuration

**Target**: Production SMB environments, 1K-100K memories, 10-50 concurrent users.

### Minimum Infrastructure

- **Kubernetes Cluster**: 3-5 nodes
- **Availability Zones**: 2 AZs minimum (HA-capable)
- **Node Specs**: 4 CPU cores, 16GB RAM per node minimum
- **Total Cluster**: 12-20 CPU cores, 48-80GB RAM
- **HA Ready**: Yes (multi-replica, cross-AZ distribution)

### Component Resource Requests and Limits

```yaml
# values-medium.yaml
aeterna:
  replicaCount: 3  # HA minimum
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 2000m
      memory: 2Gi

postgresql:
  cloudnativepg:
    instances: 3  # Primary + 2 standbys
    resources:
      requests:
        cpu: 500m
        memory: 1Gi
      limits:
        cpu: 2000m
        memory: 3Gi
    storage:
      size: 100Gi  # ~1KB per memory + 50% headroom

qdrant:
  replicaCount: 3
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 2000m
      memory: 2Gi
  persistence:
    size: 200Gi  # ~2KB per vector

dragonfly:
  replicaCount: 2  # Primary + replica
  resources:
    requests:
      cpu: 250m
      memory: 512Mi
    limits:
      cpu: 1000m
      memory: 2Gi

opal:
  opalServer:
    replicaCount: 2
    resources:
      requests:
        cpu: 250m
        memory: 512Mi
      limits:
        cpu: 1000m
        memory: 1Gi
```

### Storage Sizing

- **PostgreSQL**: ~100KB per memory minimum → 100GB for 100K memories
- **Qdrant**: ~2KB per vector → 200GB for 100M vectors
- **Dragonfly Cache**: 2-5GB
- **Backup Storage (S3/GCS)**: 1.5x PostgreSQL + Qdrant snapshot size = ~450GB
- **Total**: 300-500GB persistent + 450GB offsite backup

### Network Bandwidth

- **Baseline**: 10-50 Mbps sustained
- **Peaks**: 100+ Mbps during bulk memory ingestion or batch inference
- **Cross-AZ Traffic**: ~20% of total bandwidth (multi-AZ penalty)

### Autoscaling Configuration

```yaml
autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 8
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

### Cost Estimates (AWS Example)

- **EC2 Nodes**: ~$400-600/month (3 m5.xlarge instances)
- **EBS Volumes**: ~$50/month (500GB provisioned)
- **Data Transfer**: ~$20-50/month
- **RDS/Managed DB** (optional): +$100-200/month
- **Total**: ~$600-1000/month

---

## Large Tier Configuration

**Target**: Enterprise deployments, >100K memories, 100+ concurrent users, multi-tenant.

### Minimum Infrastructure

- **Kubernetes Cluster**: 5+ nodes (typically 7-10 for production)
- **Availability Zones**: 3 AZs minimum
- **Node Specs**: 8+ CPU cores, 32GB RAM per node
- **Total Cluster**: 40+ CPU cores, 160GB+ RAM
- **Dedicated Node Pools**: Optional (database, cache, compute-intensive)

### Component Resource Requests and Limits

```yaml
# values-large.yaml
aeterna:
  replicaCount: 5  # Higher for throughput + fault tolerance
  resources:
    requests:
      cpu: 1000m
      memory: 2Gi
    limits:
      cpu: 4000m
      memory: 4Gi
  # Use dedicated node pool: nodeSelector: { workload: compute }

postgresql:
  cloudnativepg:
    instances: 5  # 1 primary + 4 standbys for read scaling
    resources:
      requests:
        cpu: 1000m
        memory: 4Gi
      limits:
        cpu: 4000m
        memory: 8Gi
    storage:
      size: 500Gi  # 500GB for 500K memories + headroom
    # Use high-performance storage: io2/gp3 with iops: 5000

qdrant:
  replicaCount: 5
  resources:
    requests:
      cpu: 2000m
      memory: 4Gi
    limits:
      cpu: 4000m
      memory: 8Gi
  persistence:
    size: 1000Gi  # 1TB for large vector collections
    # Use fast storage: gp3 or NVMe local volumes

dragonfly:
  replicaCount: 3  # 1 primary + 2 replicas for write distribution
  resources:
    requests:
      cpu: 500m
      memory: 2Gi
    limits:
      cpu: 2000m
      memory: 4Gi

opal:
  opalServer:
    replicaCount: 3
    resources:
      requests:
        cpu: 500m
        memory: 512Mi
      limits:
        cpu: 1000m
        memory: 1Gi
```

### Storage Sizing

- **PostgreSQL**: ~1KB per memory minimum → 500GB for 500K memories
- **Qdrant**: ~2KB per vector → 1TB for large vector space
- **Dragonfly Cache**: 5-10GB (high cardinality lookups)
- **Backup Storage**: 2x main data (tiered: S3 Standard → Glacier)
- **Total Persistent**: 1.5-2TB, **Offsite Backup**: 2TB+

### Network Bandwidth

- **Baseline**: 100-500 Mbps sustained
- **Peaks**: 1+ Gbps during bulk operations
- **Cross-AZ Replication**: 200+ Mbps for data sync
- **Recommendation**: Direct peering or private links between AZs

### Advanced Autoscaling

```yaml
autoscaling:
  enabled: true
  minReplicas: 5
  maxReplicas: 20
  
  # Metrics-driven scaling
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 65
    
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 75
    
    # Custom metrics: queries per second, embedding latency
    - type: Pods
      pods:
        metric:
          name: queries_per_second
        target:
          type: AverageValue
          averageValue: "1000"
```

### Cost Estimates (AWS Example)

- **EC2 Nodes**: ~$2,000-3,000/month (7 m5.2xlarge instances)
- **EBS Volumes**: ~$200/month (2TB provisioned, io2)
- **S3 Backup**: ~$50/month (2TB)
- **Data Transfer**: ~$200-400/month (inter-AZ)
- **RDS/Managed DB** (optional): +$500-1000/month
- **Total**: ~$3,000-5,000/month

---

## Monitoring Metrics by Tier

### Key Metrics to Watch

| Metric | Small Alert | Medium Alert | Large Alert |
|--------|------------|--------------|-------------|
| **CPU Utilization** | > 80% | > 70% | > 65% |
| **Memory Utilization** | > 85% | > 80% | > 75% |
| **Disk I/O (IOPS)** | > 500 | > 2000 | > 5000 |
| **Storage Used** | > 80% capacity | > 75% capacity | > 70% capacity |
| **Query Latency (p99)** | > 500ms | > 200ms | > 100ms |
| **Replication Lag** | > 100ms | > 50ms | > 10ms |

### Metrics Queries (Prometheus)

```promql
# CPU utilization by component
sum(rate(container_cpu_usage_seconds_total[5m])) by (pod_label_app) / on() group_left sum(kube_pod_container_resource_limits{resource="cpu"})

# Memory pressure
kube_pod_container_memory_usage_bytes / kube_pod_container_resource_limits{resource="memory"} * 100

# Disk IOPS (CloudWatch/custom exporter)
rate(container_fs_io_time_seconds_total[1m])

# PostgreSQL replication lag
max(cnpg_replication_lag_bytes) by (cluster)

# Qdrant query latency
histogram_quantile(0.99, rate(qdrant_query_duration_seconds_bucket[5m]))
```

---

## When to Scale Up

### Scale Up Triggers

1. **Small → Medium**:
   - > 1,000 memory objects
   - CPU consistently > 75%
   - Memory > 80% utilization
   - Need for HA/multi-replica

2. **Medium → Large**:
   - > 100,000 memory objects
   - 100+ concurrent users
   - Query latency p99 > 200ms
   - Storage > 70% full
   - Node CPU/memory capacity exhausted

### Scaling Process

```bash
# 1. Update values file
helm get values aeterna -n aeterna > values-old.yaml
cp values-large.yaml values.yaml

# 2. Dry-run to preview changes
helm upgrade aeterna ./charts/aeterna -n aeterna -f values.yaml --dry-run

# 3. Add nodes to cluster (if needed)
kubectl scale node-pool production --num-nodes 8

# 4. Perform upgrade
helm upgrade aeterna ./charts/aeterna -n aeterna -f values.yaml --wait

# 5. Verify all components running
kubectl get pods -n aeterna
kubectl top nodes
kubectl top pods -n aeterna
```

---

## Pre-Sized Values Files

All three sizing tiers are provided with optimized defaults:

```bash
# Deploy small
helm install aeterna ./charts/aeterna \
  -n aeterna --create-namespace \
  -f charts/aeterna/values-small.yaml

# Deploy medium
helm install aeterna ./charts/aeterna \
  -n aeterna --create-namespace \
  -f charts/aeterna/values-medium.yaml

# Deploy large (with optional overrides)
helm install aeterna ./charts/aeterna \
  -n aeterna --create-namespace \
  -f charts/aeterna/values-large.yaml \
  --set postgresql.cloudnativepg.instances=6
```

---

## Right-Sizing Checklist

- [ ] Estimated memory count falls within tier bounds
- [ ] Node count matches or exceeds minimum recommendation
- [ ] Total cluster CPU >= tier requirement (considering system daemons)
- [ ] Storage size >= 1.5x estimated data volume
- [ ] Network bandwidth provisioned for baseline + 50% headroom
- [ ] HA requirements understood (multi-AZ, replica counts)
- [ ] Backup storage allocated offsite
- [ ] Autoscaling limits set (minReplicas, maxReplicas)
- [ ] Monitoring configured with tier-appropriate thresholds
- [ ] Load testing validates sizing assumptions
