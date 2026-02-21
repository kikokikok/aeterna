# Hybrid Mode: Architecture and Data Flow

This document describes **hybrid mode** — a distributed architecture combining **local edge caching** with a **remote central server**. Hybrid mode enables multi-region deployments, offline operation, and graduated sync with central authority.

## Architecture Overview

Hybrid mode runs Aeterna in two tiers:

```
┌─────────────────────────────────────────────────────────────┐
│                   CENTRAL SERVER (Region 1)                  │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ PostgreSQL (episodic memory - write authority)       │   │
│  │ Qdrant (vector DB - complete knowledge graph)        │   │
│  │ OPAL (Cedar policies - policy authority)             │   │
│  │ Sync Service (bi-directional replication)            │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
            ↑                ↑                ↑
  Async sync │ Periodic sync │ Policy updates│
            ↓                ↓                ↓
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ EDGE 1 (Region) │ │ EDGE 2 (Region) │ │ EDGE N (Region) │
├─────────────────┤ ├─────────────────┤ ├─────────────────┤
│ Dragonfly Cache │ │ Dragonfly Cache │ │ Dragonfly Cache │
│ Qdrant (subset) │ │ Qdrant (subset) │ │ Qdrant (subset) │
│ Cedar Agent     │ │ Cedar Agent     │ │ Cedar Agent     │
│ (offline capable)│ │ (offline capable)│ │ (offline capable)│
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

### Key Properties

1. **Central Authority**: Central server is the source of truth for all data
2. **Edge Autonomy**: Edges operate independently, cache locally, serve requests offline
3. **Eventual Consistency**: Edge caches eventually consistent with central server
4. **Graduated Sync**: Data syncs at three memory hierarchy levels (Working → Session → Episodic)

## Memory Hierarchy and Sync

Aeterna manages data across three memory layers, each with different sync behavior:

### 1. Working Memory (Milliseconds)
- **Storage**: Dragonfly cache (local edge only)
- **Scope**: Current request context, temporary state
- **Sync**: No sync (ephemeral)
- **Consistency**: Immediate (local only)
- **TTL**: 1-5 seconds

```yaml
# Working memory configuration
cache:
  backend: dragonfly
  layers:
    working:
      ttl: 5s
      maxSize: 1Gi
      evictionPolicy: lru
```

### 2. Session Memory (Minutes to Hours)
- **Storage**: Local Dragonfly + Central PostgreSQL
- **Scope**: User session state, conversation history, recent decisions
- **Sync**: Batched every 30-60 seconds
- **Consistency**: Eventual (async batch sync)
- **TTL**: 24 hours (configurable)

```yaml
# Session memory configuration
sync:
  sessionMemory:
    enabled: true
    batchSize: 100  # Sessions per batch
    interval: 60s   # Sync every 60 seconds
    compression: gzip
    maxRetries: 3
    retryBackoff: exponential

# Central server receives:
# {
#   "sessionId": "sess-123",
#   "userId": "user-456",
#   "messages": [...],  # Conversation history
#   "state": {...},     # Session state
#   "timestamp": 1708024800,
#   "checksum": "sha256..."
# }
```

### 3. Episodic Memory (Long-term)
- **Storage**: Central PostgreSQL only
- **Scope**: Historical decisions, knowledge graph, long-term patterns
- **Sync**: On-demand + scheduled snapshots
- **Consistency**: Strong (synchronous writes or eventual)
- **TTL**: Indefinite (with archival)

```yaml
# Episodic memory configuration
sync:
  episodicMemory:
    enabled: true
    mode: "eventual"  # or "strong" for critical systems
    snapshotInterval: 86400s  # Daily snapshots
    vectorSync:
      enabled: true
      batchSize: 1000
      interval: 3600s  # Hourly
```

## Data Flow Diagram

### Session State Sync (Every 60 seconds)

```
Edge Agent writes to local Dragonfly:
  cache.set("session:sess-123", {...})
                           │
                           ↓
  Every 60 seconds, batch collector reads:
  sessions = dragonfly.scan("session:*", batchSize=100)
                           │
                           ↓
  Sync service sends to central server (gzipped):
  POST /api/sync/sessions
  {
    "agent_id": "edge-agent-1",
    "region": "us-west-2",
    "sessions": [
      {
        "id": "sess-123",
        "last_activity": 1708024800,
        "memory": {...},
        "state": {...}
      },
      ...
    ],
    "signature": "hmac-sha256(...)"
  }
                           │
                           ↓
  Central server verifies signature, upserts to PostgreSQL:
  INSERT INTO session_memory (session_id, agent_id, data, synced_at)
  VALUES (...)
  ON CONFLICT (session_id) DO UPDATE SET data = EXCLUDED.data
                           │
                           ↓
  Acknowledgement sent back to edge:
  {
    "status": "accepted",
    "processed": 87,
    "failed": 0,
    "next_sync": 1708024900
  }
```

### Vector Embedding Sync (Hourly)

```
Edge executes query:
  vector_search(embedding=[...], limit=10)
                           │
                           ↓
  Searches local Qdrant (subset of vectors):
  → Finds 8 results locally
  → Confidence score < threshold (partial results)
                           │
                           ↓
  Triggers request to central server:
  POST /api/search/vectors
  {
    "embedding": [...],
    "limit": 5,  # Additional results needed
    "filter": {"region": "exclude:us-west-2"}  # Don't duplicate local results
  }
                           │
                           ↓
  Central Qdrant searches complete knowledge graph:
  → Finds 3 additional results with higher confidence
  → Returns top 3 to edge
                           │
                           ↓
  Edge caches new vectors for future queries:
  qdrant.upsert(...)  # Async, doesn't block response
  Schedules sync back to central for offset tracking
```

## Sync Configuration

### Core Sync Settings

```yaml
sync:
  # Central server endpoint
  enabled: true
  centralServer:
    url: "https://central.aeterna.example.com"
    port: 443
    timeout: 30s
    maxRetries: 3
  
  # Authentication with central
  auth:
    method: "serviceAccount"  # or "apiKey", "oauth2"
    existingSecret: "aeterna-sync-credentials"  # k8s Secret name
    # Secret contents:
    # apiKey: "sk-..."
    # clientId: "client-..."
    # clientSecret: "..."
  
  # Batch configuration
  batch:
    maxSize: 100  # Max items per batch
    maxBytes: 10485760  # 10MB max payload
    flushInterval: 60s  # Max time before forcing flush
  
  # Memory layer sync
  memoryLayers:
    working:
      syncEnabled: false  # Never sync working memory
    session:
      syncEnabled: true
      interval: 60s
      batchSize: 100
      compression: gzip
      onError: "queue"  # Queue failed items for retry
    episodic:
      syncEnabled: true
      interval: 3600s  # Hourly
      batchSize: 1000
      compression: gzip
      mode: "eventual"  # Don't block on central writes
  
  # Network settings
  network:
    compression: gzip  # Reduce bandwidth
    encryption: tls12  # Minimum TLS 1.2
    certificatePinning: true
    pinnedCertificates:
      - "sha256/AAAAAABBBBBBCCCCCCDDDDDDEEEEEEFFFFFFFFGGGGGGHHHHHHII="
  
  # Offline operation
  offline:
    mode: "queue"  # Queue writes when central unreachable
    maxQueueSize: 10000  # Max queued items
    queueTTL: 3600s  # Discard queue items after 1 hour
    fallbackCache: true  # Return cached results during outage
```

### Example Sync Secret

```bash
# Create secret for sync authentication
kubectl create secret generic aeterna-sync-credentials \
  --from-literal=apiKey="sk-edge-sync-key-12345" \
  --from-literal=clientId="edge-sync-client" \
  --namespace aeterna

# Or for OAuth2
kubectl create secret generic aeterna-sync-credentials \
  --from-literal=clientId="edge-sync-client" \
  --from-literal=clientSecret="client-secret-xyz" \
  --from-literal=tokenUrl="https://auth.aeterna.example.com/token" \
  --namespace aeterna
```

### Monitoring Sync Health

```bash
# Check sync service status
kubectl get deployment aeterna-sync -n aeterna
kubectl logs -n aeterna deployment/aeterna-sync --tail=50

# Monitor sync metrics (if Prometheus enabled)
kubectl exec -it aeterna-sync-0 -n aeterna -- curl http://localhost:8080/metrics | grep sync

# Check queue backlog
kubectl exec -it aeterna-sync-0 -n aeterna -- redis-cli LLEN sync:queue:session
kubectl exec -it aeterna-sync-0 -n aeterna -- redis-cli LLEN sync:queue:episodic

# Verify last successful sync
kubectl get configmap aeterna-sync-status -n aeterna -o jsonpath='{.data.lastSync}'
# Output: 2025-02-21T10:30:45Z
```

## Offline Operation

Hybrid mode enables **offline operation** — the Cedar Agent continues to function when central server is unreachable.

### Offline Behavior

```
Online Mode:
  request → [Edge Cache] → [Central] → response
  
Offline Mode (central unavailable):
  request → [Edge Cache] → [Queued decision] → response (cached/heuristic)
           (No network call, serve immediately)

Reconnect:
  [Queued decisions] → replay to central → [Central reconciles]
```

### Offline Configuration

```yaml
aeterna:
  offline:
    enabled: true
    fallbackMode: "cache"  # Return cached results
    decisionQueueing: true  # Queue decisions for later replay
    maxQueueSize: 5000
    conflictResolution: "timestamp"  # Use timestamps if conflicts arise

# Detect offline condition
centralServer:
  healthCheck:
    interval: 30s
    timeout: 5s
    failureThreshold: 3  # Mark offline after 3 failed checks
```

### Conflict Resolution During Replay

When edge decisions are replayed to central server after connectivity restoration:

```bash
# Central server receives queued decisions
POST /api/sync/replay-decisions
{
  "agent_id": "edge-1",
  "decisions": [
    {
      "id": "dec-123",
      "timestamp": 1708024500,  # When edge decided
      "action": "approve",
      "resource": "resource-456",
      "metadata": {...}
    }
  ]
}

# Central checks for conflicts
SELECT * FROM decisions 
WHERE resource_id = 'resource-456' 
  AND timestamp > 1708024500

# If conflicts exist, use conflict resolution policy:
# - "timestamp": Keep decision with latest timestamp (default)
# - "central": Discard edge decision, keep central
# - "manual": Flag for manual review
```

## Central Server Requirements

For hybrid mode to work, the central server must:

1. **PostgreSQL 16+** with pgvector for semantic search
2. **Qdrant** or **Weaviate** for complete vector knowledge graph
3. **OPAL** Cedar authorization engine
4. **Sync API** endpoints:
   - `POST /api/sync/sessions` — accept session state updates
   - `POST /api/sync/vectors` — accept vector embeddings
   - `GET /api/search/vectors` — return additional search results
   - `POST /api/sync/replay-decisions` — replay offline decisions
5. **API Rate Limiting**: 100 req/sec per edge agent
6. **TLS 1.2+** with certificate pinning for transport security

### Central Server Helm Values

```yaml
# Central server values.yaml
deploymentMode: central

postgresql:
  cloudnativepg:
    enabled: true
    cluster:
      instances: 5  # HA for central
      postgresql:
        version: 16
        parameters:
          shared_preload_libraries: "pgvector"
      storage:
        size: 500Gi  # Large for global data

vectorDatabase:
  qdrant:
    enabled: true
    replicas: 5
    storage:
      size: 300Gi

sync:
  enabled: true
  listenPort: 8081
  maxEdges: 1000  # Max edge agents
  rateLimit: "100/sec"  # Per agent
  
opal:
  enabled: true
  replicas: 3

# TLS for sync API
tls:
  enabled: true
  certManager:
    enabled: true
    issuer: letsencrypt-prod
```

## Authentication Methods

### 1. API Key (Simple)

```yaml
sync:
  auth:
    method: apiKey
    existingSecret: aeterna-sync-credentials
```

```bash
# Create secret
kubectl create secret generic aeterna-sync-credentials \
  --from-literal=apiKey="sk-edge-sync-abc123def456" \
  -n aeterna
```

Request format:
```
Authorization: Bearer sk-edge-sync-abc123def456
```

### 2. OAuth2 (Recommended)

```yaml
sync:
  auth:
    method: oauth2
    existingSecret: aeterna-sync-credentials
    # Secret contains: clientId, clientSecret, tokenUrl
```

Central server issues JWT tokens to edges:
```
Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...
```

### 3. Service Account (Kubernetes)

```yaml
sync:
  auth:
    method: serviceAccount
    serviceAccount:
      name: aeterna-edge-sync
```

```yaml
# RBAC for edge sync
apiVersion: v1
kind: ServiceAccount
metadata:
  name: aeterna-edge-sync
  namespace: aeterna

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: aeterna-edge-sync
rules:
- apiGroups: [""]
  resources: ["secrets"]
  verbs: ["get"]
```

## Bandwidth Considerations

Hybrid mode trades **latency for bandwidth** — each central query adds ~100-500ms of network delay. Optimize:

### 1. Batch Requests

Instead of:
```
for each decision:
  POST /api/sync/session  # N requests
```

Do:
```
batch = [decisions...]
POST /api/sync/sessions  # 1 request
```

### 2. Compression

All payloads gzipped (default):
```
gzip compression: ~80% reduction for JSON
Session state: 100KB → 20KB
```

### 3. Smart Caching

Cache frequently accessed vectors locally:

```yaml
qdrant:
  cache:
    enabled: true
    # Cache top 10K frequently accessed vectors
    size: 10000
    ttl: 86400s  # 1 day
    policy: "frequency"  # Based on access frequency
```

## Failover Behavior

### When Central Server is Unreachable

1. **Edge detects failure** (health check timeout after 3 failures, ~90 seconds)
2. **Offline mode activated**:
   - Serve requests from cache
   - Queue decisions locally
   - No external API calls
3. **Monitor automatically retries** central connection every 30 seconds
4. **On reconnect**:
   - Send queued sync batches
   - Verify no conflicts (timestamp-based resolution)
   - Resume normal sync

### When Edge Goes Offline

Central server considers edge offline after:
- No sync batches for 5 minutes (configurable)
- Edge agent explicitly goes offline
- Network partition detected

Central continues accepting queries, clients may get stale cache results.

## Summary

**Hybrid mode enables:**
- Multi-region deployments with local caching
- Offline-capable agents (queue decisions locally)
- Graduated consistency (working→session→episodic)
- Reduced latency for common operations (cached locally)
- Centralized authority (single source of truth)

**Tradeoffs:**
- More operational complexity (two-tier architecture)
- Eventual consistency (session/episodic layer)
- Network dependency for full functionality
- Additional bandwidth for sync traffic

For detailed configuration, see `values-hybrid.yaml`. For remote mode (no local caching), see **remote-mode.md**.
