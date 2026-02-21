# High Availability Requirements for Aeterna

This guide details the infrastructure and configuration requirements for a highly available Aeterna deployment suitable for production environments handling enterprise-scale AI agent workloads.

## Infrastructure Baseline

### Kubernetes Cluster Requirements

**Minimum Cluster Configuration:**
- **Nodes**: 3 nodes minimum, distributed across 2+ availability zones
- **Node Specs**: 4 CPU cores, 16GB RAM minimum per node
- **Total Capacity**: 12+ CPU cores, 48GB+ RAM cluster-wide
- **Network**: Low-latency cross-AZ connectivity (\<10ms latency preferred)
- **Storage**: Replicated storage class (e.g., `pd-ssd` on GKE, `gp3` on EKS with replication)

### Zone Distribution

Deploy across at least 2 availability zones to survive single AZ failure:

```yaml
# Example: 3-node cluster across 2 AZs
nodes:
  - node-1: us-east-1a (1 core Aeterna, 1 core PostgreSQL, 1 core Qdrant)
  - node-2: us-east-1b (1 core Aeterna, 1 core PostgreSQL, 1 core Qdrant)
  - node-3: us-east-1a (1 core Aeterna, 1 core PostgreSQL, 1 core Qdrant)
```

## Component-Level High Availability

### Aeterna Deployment

**Configuration for HA:**

```yaml
# values.yaml for HA deployment
aeterna:
  replicaCount: 3
  
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        - labelSelector:
            matchExpressions:
              - key: app.kubernetes.io/name
                operator: In
                values:
                  - aeterna
          topologyKey: kubernetes.io/hostname
    
    podTopologySpread:
      - maxSkew: 1
        topologyKey: topology.kubernetes.io/zone
        whenUnsatisfiable: DoNotSchedule
        labelSelector:
          matchLabels:
            app.kubernetes.io/name: aeterna
  
  pdb:
    enabled: true
    minAvailable: 2  # Allow 1 pod disruption, keep 2 running

resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 2Gi
```

**Replica Count Strategy:**
- **Minimum**: 3 replicas (1 disruption allowed with minAvailable: 2)
- **Recommended**: 5 replicas for large deployments
- Scale with `kubectl autoscale deployment aeterna --min=3 --max=10 --cpu-percent=70`

### PostgreSQL High Availability

**CloudNativePG Configuration:**

```yaml
postgresql:
  cloudnativepg:
    enabled: true
    instances: 3  # 1 primary + 2 standbys
    
    primaryUpdateStrategy: unsupervised
    
    # Synchronous replication for data durability
    postgresql:
      synchronous_commit: "on"
      synchronous_standby_names: "ANY 2 (standby1, standby2)"
    
    # Automatic failover
    monitoring:
      enabled: true
    
    # Storage HA
    storage:
      size: 100Gi
      storageClass: gp3-replicated  # Must support replication
    
    # Backup + PITR
    backup:
      enabled: true
      schedule: "0 2 * * *"  # 2 AM UTC daily
      destination: s3
      s3Credentials:
        accessKeyId: <AWS_ACCESS_KEY>
        secretAccessKey: <AWS_SECRET_KEY>
        bucket: aeterna-backups
        region: us-east-1
```

**Failover Behavior:**
- Automatic promotion of standby to primary within 30 seconds
- Connection pooling via PgBouncer (included in CloudNativePG) routes traffic to new primary
- Monitoring detects unhealthy primary and triggers failover

### Qdrant Vector Database

**High Availability Setup:**

```yaml
qdrant:
  enabled: true
  replicaCount: 3
  
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        - labelSelector:
            matchExpressions:
              - key: app.kubernetes.io/name
                operator: In
                values:
                  - qdrant
          topologyKey: topology.kubernetes.io/zone
  
  pdb:
    enabled: true
    minAvailable: 2
  
  persistence:
    size: 200Gi
    storageClass: gp3-replicated
    replicas: 3  # 3-way replication within cluster
  
  config:
    cluster:
      enabled: true
      consensus:
        tick_period_ms: 100
```

**Cluster Consensus:**
Qdrant uses Raft consensus; 3 replicas tolerate 1 node failure.

### OPAL Authorization Server

**Replicated Setup:**

```yaml
opal:
  enabled: true
  opalServer:
    replicaCount: 2  # Minimum 2 for HA
    
    affinity:
      podAntiAffinity:
        preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app.kubernetes.io/name: opal-server
              topologyKey: kubernetes.io/hostname
  
  pdb:
    enabled: true
    minAvailable: 1
```

### Dragonfly Cache

**Replicated Cache:**

```yaml
dragonfly:
  enabled: true
  replicaCount: 2  # Primary + replica
  
  persistence:
    enabled: true
    size: 50Gi
    storageClass: gp3
  
  affinity:
    podAntiAffinity:
      preferredDuringSchedulingIgnoredDuringExecution:
        - weight: 100
          podAffinityTerm:
            labelSelector:
              matchLabels:
                app.kubernetes.io/name: dragonfly
            topologyKey: kubernetes.io/hostname
```

## Pod Disruption Budgets

**PDB Policies Across All Components:**

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: aeterna-pdb
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app.kubernetes.io/name: aeterna
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: qdrant-pdb
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app.kubernetes.io/name: qdrant
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: postgresql-pdb
spec:
  minAvailable: 1  # CloudNativePG manages this
  selector:
    matchLabels:
      cnpg.io/cluster: aeterna-postgres
```

## Topology Spread Constraints

**Zone-Aware Pod Distribution:**

```yaml
aeterna:
  affinity:
    podTopologySpread:
      - maxSkew: 1
        topologyKey: topology.kubernetes.io/zone
        whenUnsatisfiable: DoNotSchedule
        labelSelector:
          matchLabels:
            app.kubernetes.io/name: aeterna
      - maxSkew: 2
        topologyKey: kubernetes.io/hostname
        whenUnsatisfiable: ScheduleAnyway
        labelSelector:
          matchLabels:
            app.kubernetes.io/name: aeterna
```

**Effect**: Ensures 1 pod per zone minimum, prefers spread across nodes.

## Horizontal Pod Autoscaling

**HPA Configuration for Load Scaling:**

```yaml
autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
  # Scale up to handle memory load from large embeddings
```

## Storage High Availability

### Replicated Volume Configuration

```yaml
# For cloud providers:
# AWS EKS: Use GP3 with io1/io2 for critical workloads
# GKE: Use pd-ssd with regional persistent disks
# Azure AKS: Use Premium_LRS or StandardSSD_LRS with replication

postgresql:
  cloudnativepg:
    storage:
      storageClass: gp3-replicated
      # Ensure StorageClass has:
      # - parameters.replication: "true"
      # - parameters.replicas: 3
```

## Network Considerations

### Cross-AZ Traffic Policy

```yaml
# Minimize cross-AZ traffic where possible
aeterna:
  affinity:
    # Prefer same-zone for low latency
    podPreferredAffinity:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchLabels:
              app.kubernetes.io/name: postgresql
          topologyKey: topology.kubernetes.io/zone
```

### Network Policies

```yaml
# Restrict traffic between critical components
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: aeterna-network-policy
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: aeterna
  policyTypes:
    - Ingress
  ingress:
    - from:
        - podSelector:
            matchLabels:
              role: api-gateway
      ports:
        - protocol: TCP
          port: 8000
```

## Monitoring for HA Health

### Critical Alerts

```yaml
alertmanager:
  rules:
    - alert: AeternaPodsBelowMinReplicas
      expr: kube_deployment_status_replicas_available{deployment="aeterna"} < 2
      for: 5m
      
    - alert: PostgreSQLNotReplicating
      expr: cnpg_replication_lag_bytes > 1000000  # 1MB lag
      
    - alert: QdrantClusterDegraded
      expr: qdrant_cluster_peers_total < 3
      
    - alert: StorageIOPSThrottled
      expr: rate(kubelet_volume_stats_available_bytes[5m]) < 10000000
```

### Key Metrics to Monitor

- `kube_deployment_status_replicas_available`: Available replicas per component
- `cnpg_replication_lag_bytes`: PostgreSQL replication lag
- `qdrant_cluster_peers_total`: Active Qdrant peers
- `node_cpu_usage_percent`: Node CPU utilization
- `kubelet_volume_stats_capacity_bytes`: Storage capacity vs. usage

## Testing HA Failover

### Simulate Component Failure

```bash
# Test Aeterna failover
kubectl delete pod -n aeterna aeterna-0
# Verify new pod starts and traffic routes within 30s

# Test PostgreSQL failover
kubectl delete pod -n aeterna aeterna-postgres-1
# CloudNativePG promotes standby automatically

# Test zone failure (drain node in AZ)
kubectl drain node-1 --ignore-daemonsets --delete-emptydir-data
# All pods reschedule to remaining AZ
```

## Checklist for HA Deployment

- [ ] 3+ nodes across 2+ AZs provisioned
- [ ] Storage class supports replication
- [ ] All component replicaCounts ≥ minimum (Aeterna: 3, PostgreSQL: 3, Qdrant: 3)
- [ ] PDB policies configured for all components
- [ ] Anti-affinity rules deployed
- [ ] Topology spread constraints active
- [ ] HPA enabled and tested
- [ ] Backup/restore procedure validated
- [ ] Monitoring and alerting configured
- [ ] Load testing confirms HA under failure scenarios
