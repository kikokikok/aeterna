# High Availability Deployment Guide

## Overview

This guide describes the configuration and deployment of the Aeterna High Availability (HA) infrastructure. To achieve 99.9% availability, Aeterna uses a multi-node storage architecture with automated failover for all critical data stores.

## Architecture

The HA setup consists of the following components:

- **PostgreSQL**: Managed by **Patroni** for automated failover and streaming replication.
- **Qdrant**: Native **Cluster Mode** with multiple nodes and replication factor of 2.
- **Redis**: **Sentinel** mode for monitoring, notification, and automatic failover.
- **Load Balancing**: Multi-AZ load balancer distributing traffic across service replicas.

## Prerequisites

- Kubernetes cluster with at least 3 nodes across multiple Availability Zones (AZs).
- `kubectl` and `helm` installed and configured.
- Access to the `infrastructure/ha/` directory in the Aeterna repository.

## Component Configuration

### 1. PostgreSQL with Patroni

Patroni manages the PostgreSQL cluster using `etcd` or `Consul` for leader election.

**File**: `infrastructure/ha/patroni/patroni.yml`

The configuration ensures:
- Automated primary election.
- Synchronous or asynchronous replication.
- Health check endpoints for load balancers.

**Deployment Command**:
```bash
helm install patroni-postgresql ./charts/patroni -f infrastructure/ha/patroni/patroni.yml
```

### 2. Qdrant Cluster

Qdrant cluster mode provides horizontal scaling and redundancy for vector embeddings.

**File**: `infrastructure/ha/qdrant/qdrant-cluster.yaml`

**Key Settings**:
- `replication_factor: 2`
- `shard_number: 3`
- `consensus_enabled: true`

**Deployment Command**:
```bash
kubectl apply -f infrastructure/ha/qdrant/qdrant-cluster.yaml
```

### 3. Redis Sentinel

Redis Sentinel provides high availability for the caching layer.

**File**: `infrastructure/ha/redis/redis-sentinel.yaml`

**Configuration**:
- 3 Redis instances (1 Master, 2 Replicas).
- 3 Sentinel instances for quorum-based failover.

**Deployment Command**:
```bash
kubectl apply -f infrastructure/ha/redis/redis-sentinel.yaml
```

## Deployment Order

To ensure a smooth startup, deploy components in the following order:

1. **Infrastructure Foundation**: etcd/Consul (if used by Patroni).
2. **Storage Layer**:
   - PostgreSQL (Patroni)
   - Redis (Sentinel)
   - Qdrant (Cluster)
3. **Aeterna Services**:
   - Memory Service
   - Knowledge Service
   - Governance Service
   - Sync Bridge

## Health Check Commands

Verify the health of the HA components using these commands:

### PostgreSQL (Patroni)
```bash
# List cluster members and status
kubectl exec -it patroni-postgresql-0 -- patronictl list
```

### Qdrant Cluster
```bash
# Check cluster status via API
curl http://qdrant-cluster:6333/cluster
```

### Redis Sentinel
```bash
# Query sentinel for master status
kubectl exec -it redis-sentinel-0 -- redis-cli -p 26379 sentinel master mymaster
```

## Common Issues & Resolution

| Issue | Symptom | Resolution |
|-------|---------|------------|
| Patroni split-brain | Multiple masters reported | Check etcd/Consul connectivity; restart minority nodes. |
| Qdrant read-only | Shards in "Red" state | Verify node connectivity; check disk space on vector nodes. |
| Redis failover delay | Sentinels cannot reach quorum | Ensure at least 2 Sentinels are running and can communicate. |
| High replication lag | Stale data on reads | Check network throughput between AZs; optimize write heavy workloads. |

## Health Check Endpoint Summary

| Service | Port | Endpoint |
|---------|------|----------|
| Patroni (Leader) | 8008 | `/primary` |
| Patroni (Replica) | 8008 | `/replica` |
| Qdrant | 6333 | `/healthz` |
| Aeterna Services | 9090 | `/health` |
