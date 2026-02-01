# Phase 5: Horizontal Scaling Implementation Guide

## Overview

This document provides the complete implementation guide for Phase 5: Horizontal Scaling. The implementation decomposes the monolithic Aeterna service into independent microservices for better scalability and maintainability.

## Architecture

### Service Decomposition

```
┌──────────────────────────────────────────────────────────────┐
│                     API Gateway / Load Balancer               │
└──────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│ Memory Service│    │Knowledge Svc  │    │Governance Svc │
│   (Port 8001) │    │  (Port 8002)  │    │  (Port 8003)  │
├───────────────┤    ├───────────────┤    ├───────────────┤
│- Store memory │    │- Store KB     │    │- Policies     │
│- Search       │    │- Retrieve     │    │- Approval     │
│- Embeddings   │    │- Context arch │    │- Audit        │
│- Caching      │    │- Note taking  │    │- RLS          │
└───────────────┘    └───────────────┘    └───────────────┘
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              │
                    ┌─────────────────────┐
                    │  Shared Storage     │
                    ├─────────────────────┤
                    │  - PostgreSQL       │
                    │  - Qdrant           │
                    │  - Redis            │
                    │  - DuckDB           │
                    └─────────────────────┘
```

### Service Responsibilities

#### Memory Service (Port 8001)
- **Purpose**: Handle all memory-related operations
- **Endpoints**:
  - `POST /api/v1/memories` - Store new memory
  - `GET /api/v1/memories/:id` - Get memory by ID
  - `POST /api/v1/memories/search` - Vector search
  - `DELETE /api/v1/memories/:id` - Delete memory
  - `POST /api/v1/embeddings/generate` - Generate embeddings
- **Dependencies**: PostgreSQL, Qdrant, Redis (cache), OpenAI API

#### Knowledge Service (Port 8002)
- **Purpose**: Knowledge repository and context management
- **Endpoints**:
  - `POST /api/v1/knowledge` - Store knowledge item
  - `GET /api/v1/knowledge/:path` - Get knowledge by path
  - `POST /api/v1/knowledge/search` - Search knowledge
  - `POST /api/v1/context/assemble` - Assemble context
  - `POST /api/v1/notes` - Generate notes
- **Dependencies**: PostgreSQL, DuckDB, Redis

#### Governance Service (Port 8003)
- **Purpose**: Policy management and access control
- **Endpoints**:
  - `POST /api/v1/policies` - Create policy
  - `GET /api/v1/policies/:id` - Get policy
  - `POST /api/v1/approval/request` - Request approval
  - `POST /api/v1/approval/decide` - Make decision
  - `GET /api/v1/audit` - Audit logs
- **Dependencies**: PostgreSQL, Redis

#### Sync Service (Port 8004)
- **Purpose**: External system synchronization
- **Endpoints**:
  - `POST /api/v1/sync/start` - Start sync job
  - `GET /api/v1/sync/status/:id` - Get sync status
  - `POST /api/v1/sync/cancel/:id` - Cancel sync job
- **Dependencies**: PostgreSQL, Redis, External APIs

## Implementation Steps

### Step 1: Create Service Structure (Week 1)

```bash
# Create service directories
mkdir -p services/{memory,knowledge,governance,sync}

# Create Cargo.toml for each service
for service in memory knowledge governance sync; do
  cat > services/$service/Cargo.toml << EOF
[package]
name = "${service}-service"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "${service}-service"
path = "main.rs"

[dependencies]
mk_core.workspace = true
${service}.workspace = true
storage.workspace = true
config.workspace = true
tokio.workspace = true
axum = "0.7"
tower-http = { version = "0.6", features = ["trace", "cors"] }
tracing.workspace = true
serde.workspace = true
EOF
done
```

### Step 2: Implement Service Boilerplate (Week 1)

Each service should follow this template:

```rust
// services/memory/main.rs
use axum::{Router, routing::{get, post}};
use tower_http::{trace::TraceLayer, cors::CorsLayer};

#[derive(Clone)]
struct AppState {
    // Service-specific state
}

async fn health() -> &'static str {
    "OK"
}

async fn ready() -> &'static str {
    "READY"
}

fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        // Add service-specific routes
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or("8001".to_string());
    let state = AppState {};
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(&format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Step 3: Load Balancing Configuration (Week 2)

#### Kubernetes Service Configuration

```yaml
# deploy/k8s/memory-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: memory-service
  labels:
    app: memory-service
spec:
  type: ClusterIP
  ports:
    - port: 8001
      targetPort: 8001
      protocol: TCP
      name: http
  selector:
    app: memory-service
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: memory-service
spec:
  replicas: 3
  selector:
    matchLabels:
      app: memory-service
  template:
    metadata:
      labels:
        app: memory-service
    spec:
      containers:
      - name: memory-service
        image: aeterna/memory-service:latest
        ports:
        - containerPort: 8001
        env:
        - name: PORT
          value: "8001"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: aeterna-secrets
              key: database-url
        resources:
          requests:
            cpu: 500m
            memory: 1Gi
          limits:
            cpu: 2000m
            memory: 4Gi
        livenessProbe:
          httpGet:
            path: /health
            port: 8001
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8001
          initialDelaySeconds: 5
          periodSeconds: 5
```

#### Ingress Configuration

```yaml
# deploy/k8s/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: aeterna-ingress
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /$2
spec:
  ingressClassName: nginx
  rules:
  - host: api.aeterna.example.com
    http:
      paths:
      - path: /memory(/|$)(.*)
        pathType: Prefix
        backend:
          service:
            name: memory-service
            port:
              number: 8001
      - path: /knowledge(/|$)(.*)
        pathType: Prefix
        backend:
          service:
            name: knowledge-service
            port:
              number: 8002
      - path: /governance(/|$)(.*)
        pathType: Prefix
        backend:
          service:
            name: governance-service
            port:
              number: 8003
```

### Step 4: Horizontal Pod Autoscaling (Week 3)

```yaml
# deploy/k8s/hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: memory-service-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: memory-service
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "1000"
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 100
        periodSeconds: 60
      - type: Pods
        value: 2
        periodSeconds: 60
      selectPolicy: Max
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
      selectPolicy: Min
```

### Step 5: Tenant Sharding (Week 4-5)

#### Shard Router Implementation

```rust
// storage/src/tenant_router.rs
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub enum TenantSize {
    Small,   // < 10k memories
    Medium,  // 10k-100k memories
    Large,   // > 100k memories
}

#[derive(Debug, Clone)]
pub struct TenantShard {
    pub tenant_id: String,
    pub size: TenantSize,
    pub shard_id: String,
    pub collection_name: String,
}

pub struct TenantRouter {
    /// Map of tenant_id -> shard assignment
    assignments: Arc<RwLock<HashMap<String, TenantShard>>>,
}

impl TenantRouter {
    pub fn new() -> Self {
        Self {
            assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Assign a tenant to a shard based on size
    pub fn assign_shard(&self, tenant_id: &str, size: TenantSize) -> TenantShard {
        let shard_id = match size {
            TenantSize::Small | TenantSize::Medium => {
                // Shared shard for small/medium tenants
                "shared-shard-1".to_string()
            }
            TenantSize::Large => {
                // Dedicated shard for large tenants
                format!("dedicated-{}", tenant_id)
            }
        };
        
        let collection_name = match size {
            TenantSize::Small | TenantSize::Medium => {
                // Use shared collection with tenant filtering
                "memories-shared".to_string()
            }
            TenantSize::Large => {
                // Dedicated collection
                format!("memories-{}", tenant_id)
            }
        };
        
        let shard = TenantShard {
            tenant_id: tenant_id.to_string(),
            size,
            shard_id,
            collection_name,
        };
        
        self.assignments.write().insert(tenant_id.to_string(), shard.clone());
        shard
    }
    
    /// Get shard for a tenant
    pub fn get_shard(&self, tenant_id: &str) -> Option<TenantShard> {
        self.assignments.read().get(tenant_id).cloned()
    }
    
    /// Migrate tenant to a different shard
    pub async fn migrate_tenant(
        &self,
        tenant_id: &str,
        new_size: TenantSize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create new shard assignment
        let new_shard = self.assign_shard(tenant_id, new_size);
        
        // 2. Copy data to new shard (implementation depends on storage backend)
        // TODO: Implement data migration
        
        // 3. Update routing table
        self.assignments.write().insert(tenant_id.to_string(), new_shard);
        
        // 4. Delete old shard data
        // TODO: Implement cleanup
        
        Ok(())
    }
}
```

#### Shard Manager

```rust
// storage/src/shard_manager.rs
use std::collections::HashMap;

pub struct ShardManager {
    /// Available shards and their capacity
    shards: HashMap<String, ShardInfo>,
}

#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub shard_id: String,
    pub max_capacity: usize,
    pub current_tenants: usize,
    pub endpoint: String,
}

impl ShardManager {
    pub fn new() -> Self {
        let mut shards = HashMap::new();
        
        // Initialize shared shards
        shards.insert("shared-shard-1".to_string(), ShardInfo {
            shard_id: "shared-shard-1".to_string(),
            max_capacity: 100,
            current_tenants: 0,
            endpoint: "qdrant-shared-1:6333".to_string(),
        });
        
        Self { shards }
    }
    
    /// Find best shard for a new tenant
    pub fn find_best_shard(&self, size: TenantSize) -> Option<String> {
        match size {
            TenantSize::Large => None, // Will get dedicated shard
            _ => {
                // Find shard with most available capacity
                self.shards.values()
                    .filter(|s| s.current_tenants < s.max_capacity)
                    .max_by_key(|s| s.max_capacity - s.current_tenants)
                    .map(|s| s.shard_id.clone())
            }
        }
    }
    
    /// Create a dedicated shard for large tenant
    pub async fn create_dedicated_shard(
        &mut self,
        tenant_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let shard_id = format!("dedicated-{}", tenant_id);
        
        // TODO: Provision new Qdrant instance or collection
        
        self.shards.insert(shard_id.clone(), ShardInfo {
            shard_id: shard_id.clone(),
            max_capacity: 1,
            current_tenants: 1,
            endpoint: format!("qdrant-{}.svc.cluster.local:6333", shard_id),
        });
        
        Ok(shard_id)
    }
}
```

## Helm Chart Configuration

### Values.yaml

```yaml
# deploy/helm/aeterna/values.yaml
memoryService:
  enabled: true
  replicaCount: 3
  image:
    repository: aeterna/memory-service
    tag: latest
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 2000m
      memory: 4Gi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10
    targetCPUUtilization: 70
    targetMemoryUtilization: 80

knowledgeService:
  enabled: true
  replicaCount: 3
  image:
    repository: aeterna/knowledge-service
    tag: latest
  resources:
    requests:
      cpu: 500m
      memory: 1Gi
    limits:
      cpu: 2000m
      memory: 4Gi
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10

governanceService:
  enabled: true
  replicaCount: 2
  image:
    repository: aeterna/governance-service
    tag: latest
  resources:
    requests:
      cpu: 250m
      memory: 512Mi
    limits:
      cpu: 1000m
      memory: 2Gi

syncService:
  enabled: true
  replicaCount: 2
  image:
    repository: aeterna/sync-service
    tag: latest
  resources:
    requests:
      cpu: 250m
      memory: 512Mi
    limits:
      cpu: 1000m
      memory: 2Gi
```

## Testing Requirements

### Load Testing

```bash
# Install k6
brew install k6

# Run load test
k6 run - << EOF
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '2m', target: 100 },  // Ramp up
    { duration: '5m', target: 100 },  // Stay at 100
    { duration: '2m', target: 200 },  // Ramp to 200
    { duration: '5m', target: 200 },  // Stay at 200
    { duration: '2m', target: 0 },    // Ramp down
  ],
};

export default function () {
  let response = http.post('http://api.aeterna.local/memory/api/v1/memories', JSON.stringify({
    content: 'Load test memory',
    layer: 'user',
  }), {
    headers: { 'Content-Type': 'application/json' },
  });
  
  check(response, {
    'status is 201': (r) => r.status === 201,
    'response time < 500ms': (r) => r.timings.duration < 500,
  });
}
EOF
```

### Failover Testing

```bash
# Kill random pod and verify recovery
kubectl delete pod -n aeterna -l app=memory-service --field-selector=status.phase=Running --wait=false

# Monitor recovery
kubectl get pods -n aeterna -w
```

### Shard Migration Testing

```bash
# Migrate a tenant
./aeterna-cli tenant migrate \
  --tenant-id large-corp \
  --to-shard dedicated-1 \
  --verify

# Verify data integrity
./aeterna-cli tenant verify \
  --tenant-id large-corp \
  --check-all
```

## Monitoring and Observability

### Prometheus Metrics

Each service should expose:
- `http_requests_total{service, endpoint, status}`
- `http_request_duration_seconds{service, endpoint}`
- `database_connections_active{service}`
- `database_query_duration_seconds{service, query_type}`
- `cache_hits_total{service}`
- `cache_misses_total{service}`

### Grafana Dashboards

Create dashboards for:
1. **Service Health**: Request rate, error rate, latency (RED metrics)
2. **Resource Usage**: CPU, memory, network per service
3. **Database Performance**: Query duration, connection pool
4. **Cache Performance**: Hit rate, eviction rate
5. **Tenant Sharding**: Tenants per shard, shard utilization

## Deployment Strategy

### Blue-Green Deployment

```yaml
# deploy/k8s/deployment-blue.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: memory-service-blue
  labels:
    app: memory-service
    version: blue
# ... deployment spec
```

```bash
# Deploy green version
kubectl apply -f deployment-green.yaml

# Wait for health checks
kubectl wait --for=condition=available --timeout=300s deployment/memory-service-green

# Switch traffic
kubectl patch service memory-service -p '{"spec":{"selector":{"version":"green"}}}'

# Verify
# ... monitor metrics ...

# Rollback if needed
kubectl patch service memory-service -p '{"spec":{"selector":{"version":"blue"}}}'
```

## Migration Path

### From Monolith to Microservices

1. **Phase 1**: Deploy services alongside monolith
2. **Phase 2**: Route 10% traffic to new services
3. **Phase 3**: Gradually increase to 50%, 90%, 100%
4. **Phase 4**: Decommission monolith

### Feature Flags

```rust
// config/src/feature_flags.rs
pub struct FeatureFlags {
    pub use_memory_service: bool,
    pub use_knowledge_service: bool,
    pub use_governance_service: bool,
}

impl FeatureFlags {
    pub fn from_env() -> Self {
        Self {
            use_memory_service: std::env::var("USE_MEMORY_SERVICE")
                .unwrap_or("false".to_string()) == "true",
            use_knowledge_service: std::env::var("USE_KNOWLEDGE_SERVICE")
                .unwrap_or("false".to_string()) == "true",
            use_governance_service: std::env::var("USE_GOVERNANCE_SERVICE")
                .unwrap_or("false".to_string()) == "true",
        }
    }
}
```

## Expected Results

### Performance Improvements
- **Latency**: p50 < 50ms, p95 < 200ms, p99 < 500ms
- **Throughput**: 10,000 RPS per service instance
- **Scalability**: Linear scaling up to 100 instances

### Reliability Improvements
- **Availability**: 99.9% (3 nines)
- **MTTR**: < 5 minutes
- **Blast Radius**: Single service failures don't affect others

### Operational Improvements
- **Deployment**: Independent service deployments
- **Scaling**: Service-specific autoscaling
- **Cost**: 30-40% reduction through right-sizing

## Timeline

- **Week 1**: Service structure and boilerplate
- **Week 2**: Load balancing and ingress setup
- **Week 3**: HPA configuration and testing
- **Week 4**: Tenant sharding implementation
- **Week 5**: Migration tooling and rollout

## References

- [Kubernetes Deployment Best Practices](https://kubernetes.io/docs/concepts/workloads/controllers/deployment/)
- [HPA Documentation](https://kubernetes.io/docs/tasks/run-application/horizontal-pod-autoscale/)
- [Service Mesh Patterns](https://istio.io/latest/docs/concepts/traffic-management/)
