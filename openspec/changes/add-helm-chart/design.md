# Design: Helm Chart for Kubernetes Deployment

## Context

Aeterna needs to be deployable on vanilla Kubernetes clusters with flexible infrastructure options:
- **Self-contained**: Deploy all dependencies with the chart
- **External references**: Connect to existing Redis, PostgreSQL, Qdrant instances
- **Hybrid**: Mix of bundled and external services

Target: 300 developers using the system in various configurations.

## Goals / Non-Goals

### Goals
- Single Helm chart deployable to vanilla Kubernetes
- Optional dependency subcharts (Redis standalone, PostgreSQL, Qdrant)
- External service connection support
- Production-ready defaults (security, observability, scaling)
- GitOps-friendly (ArgoCD, Flux compatible)

### Non-Goals
- OpenShift-specific configurations (vanilla K8s only)
- Redis Sentinel/Cluster mode (standalone for self-deploy)
- Multi-region deployment (single cluster scope)

## Decisions

### Chart Structure

```
charts/aeterna/
├── Chart.yaml
├── values.yaml
├── values-local.yaml        # Dev defaults
├── values-production.yaml   # Prod defaults
├── templates/
│   ├── _helpers.tpl
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── hpa.yaml
│   ├── ingress.yaml
│   ├── serviceaccount.yaml
│   ├── networkpolicy.yaml
│   └── servicemonitor.yaml
└── charts/                  # Optional subcharts
    ├── dragonfly/           # Redis-compatible (Apache 2.0)
    ├── cloudnative-pg/      # PostgreSQL operator (CNCF)
    └── qdrant/              # Official Qdrant chart
```

### Dependency Management

Using community-maintained and official charts (avoiding Bitnami due to licensing changes):
- `oci://registry-1.docker.io/bitnamicharts/redis` → **Replaced with**: `oci://ghcr.io/dragonflydb/dragonfly` or upstream Redis via `redis/redis-stack` 
- Alternative: **Keydb** (drop-in Redis replacement, fully open source)
- `cloudnative-pg/cloudnative-pg` - CloudNativePG operator (CNCF project, Apache 2.0)
- `qdrant/qdrant` - Official Qdrant Helm chart (Apache 2.0)

**Selected Stack**:
- **Redis alternative**: Dragonfly (drop-in compatible, Apache 2.0) or KeyDB (BSD-3)
- **PostgreSQL**: CloudNativePG (CNCF, production-grade operator)
- **Qdrant**: Official chart (Apache 2.0)

### Configuration Strategy

```yaml
# values.yaml structure
global:
  storageClass: ""
  imageRegistry: ""

aeterna:
  image:
    repository: ghcr.io/kikokikok/aeterna
    tag: latest
  replicas: 2
  resources:
    requests:
      memory: "512Mi"
      cpu: "250m"
    limits:
      memory: "2Gi"
      cpu: "1"

redis:
  enabled: true              # false = use external
  external:
    host: ""
    port: 6379
    password: ""

postgresql:
  enabled: true
  external:
    host: ""
    port: 5432
    database: "aeterna"
    username: ""
    password: ""

qdrant:
  enabled: true
  external:
    host: ""
    port: 6333
    apiKey: ""
```

### Alternatives Considered

1. **Kustomize overlays**: Rejected - less flexible for optional dependencies
2. **Operator pattern**: Rejected - overkill for current scope, higher maintenance
3. **Separate charts per component**: Rejected - harder to manage as single unit

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Subchart version drift | Pin subchart versions, document upgrade path |
| Complex values schema | Provide values-*.yaml examples, JSON Schema for IDE |
| Resource contention | Clear resource requests/limits, HPA configuration |

## Migration Plan

1. Phase 1: Core chart with external-only dependencies
2. Phase 2: Add optional bundled dependencies
3. Phase 3: Production hardening (PDBs, network policies)

## Open Questions

- [ ] Container registry: ghcr.io or separate registry?
- [ ] Base image: distroless, alpine, or debian-slim?
- [ ] Multi-arch builds (amd64 + arm64)?
