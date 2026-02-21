# Design: Helm Chart + CLI Setup Wizard

## Context

Aeterna needs to be deployable on vanilla Kubernetes clusters with flexible infrastructure options:
- **Self-contained**: Deploy all dependencies with the chart
- **External references**: Connect to existing Redis, PostgreSQL, Qdrant instances
- **Hybrid**: Mix of bundled and external services
- **Local development**: Docker Compose with same configuration model

Target: 300 developers using the system in various configurations.

**Constraint**: Bitnami charts are now closed-source and require payment. All dependencies must use genuinely open-source alternatives.

## Goals / Non-Goals

### Goals
- Single Helm chart deployable to vanilla Kubernetes
- CLI setup wizard for interactive configuration
- Optional dependency subcharts (CloudNativePG, Dragonfly/Valkey, Qdrant)
- External service connection support
- Production-ready defaults (security, observability, scaling)
- GitOps-friendly (ArgoCD, Flux compatible)
- OpenCode integration configuration
- Multi-target output (Helm, Docker Compose, OpenCode)

### Non-Goals
- OpenShift-specific configurations (vanilla K8s only)
- Redis Sentinel/Cluster mode (standalone for self-deploy)
- Multi-region deployment (single cluster scope)
- Custom operators for Aeterna (use existing operators)

## Decisions

### 1. CLI Framework

**Selected**: `dialoguer` + `console` crates

**Rationale**:
- `dialoguer` provides rich interactive prompts (Select, MultiSelect, Input, Confirm)
- `console` provides terminal styling and progress indicators
- Both are well-maintained and have minimal dependencies
- Native Rust, no Python/Node.js runtime required

**Alternatives Considered**:
- `inquire`: Similar features but less mature
- `clap` with interactive mode: Too limited for wizard flows
- Python `questionary`: Would require Python runtime

### 2. Chart Structure

```
charts/aeterna/
├── Chart.yaml
├── values.yaml
├── values.schema.json           # JSON Schema for IDE validation
├── templates/
│   ├── _helpers.tpl
│   ├── aeterna/
│   │   ├── deployment.yaml      # Core Aeterna services
│   │   ├── service.yaml
│   │   ├── configmap.yaml
│   │   ├── secret.yaml
│   │   ├── hpa.yaml
│   │   ├── pdb.yaml
│   │   ├── ingress.yaml
│   │   ├── serviceaccount.yaml
│   │   ├── rbac.yaml
│   │   ├── networkpolicy.yaml
│   │   ├── servicemonitor.yaml
│   │   └── job-migration.yaml   # Database migrations
│   ├── opal/
│   │   ├── opal-server.yaml
│   │   ├── cedar-agent.yaml
│   │   └── opal-fetcher.yaml
│   ├── validation/
│   │   └── _validation.tpl      # Pre-flight checks
│   └── NOTES.txt
└── charts/                      # Subcharts (helm dependency)
    ├── cloudnative-pg/
    ├── dragonfly/
    ├── valkey/
    ├── qdrant/
    ├── weaviate/
    └── percona-mongodb/
```

### 3. Dependency Management

Using community-maintained and official charts only:

| Component | Chart Repository | Version | License |
|-----------|------------------|---------|---------|
| PostgreSQL | `https://cloudnative-pg.github.io/charts` | 0.23.x | Apache-2.0 |
| Redis (Dragonfly) | `oci://ghcr.io/dragonflydb/dragonfly` | 1.x | Apache-2.0 |
| Redis (Valkey) | `https://valkey.io/valkey-helm/` | 1.x | BSD-3 |
| Qdrant | `https://qdrant.github.io/qdrant-helm` | 0.10.x | Apache-2.0 |
| Weaviate | `https://weaviate.github.io/weaviate-helm/` | 17.x | BSD-3 |
| MongoDB | `https://percona.github.io/percona-helm-charts/` | 1.21.x | Apache-2.0 |
| OPAL | (embedded, not subchart) | 0.7.5 | Apache-2.0 |

### 4. Configuration Strategy

```yaml
# values.yaml structure
global:
  storageClass: ""
  imageRegistry: ""
  imagePullSecrets: []

aeterna:
  image:
    repository: ghcr.io/kikokikok/aeterna
    tag: latest
    pullPolicy: IfNotPresent
  replicas: 2
  resources:
    requests:
      memory: "512Mi"
      cpu: "250m"
    limits:
      memory: "2Gi"
      cpu: "1"
  
  # Feature flags
  features:
    cca: true                    # CCA agents (Context Architect, etc.)
    radkit: false                # A2A protocol
    rlm: true                    # Recursive Language Model

# Vector Backend Selection (mutually exclusive for primary)
vectorBackend:
  type: qdrant                   # qdrant | pgvector | pinecone | weaviate | mongodb | vertex-ai | databricks
  
  qdrant:
    enabled: true                # false = use external
    external:
      host: ""
      port: 6333
      apiKey: ""
      existingSecret: ""
      existingSecretKey: "api-key"
  
  pgvector:
    enabled: false               # Uses PostgreSQL with pgvector extension
  
  pinecone:
    enabled: false
    apiKey: ""
    environment: ""
    indexName: ""
    existingSecret: ""
  
  weaviate:
    enabled: false
    external:
      host: ""
      apiKey: ""
  
  mongodb:
    enabled: false
    uri: ""
    existingSecret: ""
  
  vertexAi:
    enabled: false
    projectId: ""
    region: ""
    indexEndpoint: ""
    serviceAccountJson: ""
    existingSecret: ""
  
  databricks:
    enabled: false
    workspaceUrl: ""
    token: ""
    catalog: ""
    existingSecret: ""

# Cache Selection
cache:
  type: dragonfly                # dragonfly | valkey | external
  
  dragonfly:
    enabled: true
    replicas: 1
    resources:
      requests:
        memory: "256Mi"
        cpu: "100m"
  
  valkey:
    enabled: false
    replicas: 1
  
  external:
    host: ""
    port: 6379
    password: ""
    existingSecret: ""
    existingSecretKey: "redis-password"

# PostgreSQL
postgresql:
  enabled: true                  # false = use external
  
  # CloudNativePG configuration
  instances: 3
  storage:
    size: 10Gi
    storageClass: ""
  
  # pgvector extension
  pgvector:
    enabled: true
  
  # Backup configuration
  backup:
    enabled: false
    schedule: "0 2 * * *"
    destination:
      type: s3                   # s3 | gcs | azure
      bucket: ""
      path: ""
  
  external:
    host: ""
    port: 5432
    database: "aeterna"
    username: ""
    password: ""
    existingSecret: ""
    existingSecretKey: "password"

# OPAL Authorization Stack
opal:
  enabled: true                  # false = single-tenant mode
  
  server:
    replicas: 3
    resources:
      requests:
        cpu: 100m
        memory: 256Mi
    pdb:
      enabled: true
      minAvailable: 2
  
  cedarAgent:
    enabled: true
    # DaemonSet runs on all nodes
    tolerations:
      - key: node-role.kubernetes.io/control-plane
        operator: Exists
        effect: NoSchedule
  
  fetcher:
    replicas: 2
    resources:
      requests:
        cpu: 50m
        memory: 64Mi

# LLM Provider
llm:
  provider: openai               # openai | anthropic | ollama | none
  
  openai:
    apiKey: ""
    existingSecret: ""
    existingSecretKey: "api-key"
    embeddingModel: "text-embedding-3-small"
    chatModel: "gpt-4o"
  
  anthropic:
    apiKey: ""
    existingSecret: ""
    model: "claude-3-haiku-20240307"
  
  ollama:
    host: ""
    model: "llama3.2"

# Ingress
ingress:
  enabled: false
  className: ""
  annotations: {}
  hosts:
    - host: aeterna.local
      paths:
        - path: /
          pathType: Prefix
  tls: []

# Observability
observability:
  serviceMonitor:
    enabled: false
    interval: 30s
  
  tracing:
    enabled: false
    endpoint: ""

# Security
security:
  networkPolicy:
    enabled: false
  
  podSecurityContext:
    runAsNonRoot: true
    runAsUser: 1000
    fsGroup: 1000
  
  containerSecurityContext:
    allowPrivilegeEscalation: false
    readOnlyRootFilesystem: true
    capabilities:
      drop:
        - ALL

# Secret Management
secrets:
  provider: helm                 # helm | sops | external-secrets
  
  externalSecrets:
    enabled: false
    secretStoreRef:
      name: ""
      kind: ClusterSecretStore
```

### 5. CLI Wizard Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        aeterna setup                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q1: Deployment Target                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > Local development (Docker Compose)                     │   │
│  │   Kubernetes (Helm chart)                                │   │
│  │   OpenCode configuration only                            │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   [Docker Compose]      [Kubernetes]         [OpenCode]
        │                     │                     │
        ▼                     ▼                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q2: Vector Database Backend                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > Qdrant (default, self-hosted)                          │   │
│  │   pgvector (PostgreSQL extension)                        │   │
│  │   Pinecone (managed cloud)                               │   │
│  │   Weaviate (hybrid search)                               │   │
│  │   MongoDB Atlas (managed)                                │   │
│  │   Vertex AI (Google Cloud)                               │   │
│  │   Databricks (Unity Catalog)                             │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q3: Redis-Compatible Cache                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > Dragonfly (recommended, 5x faster, Apache-2.0)         │   │
│  │   Valkey (official Redis fork, BSD-3)                    │   │
│  │   External Redis (bring your own)                        │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q4: PostgreSQL Deployment                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > CloudNativePG (production operator, Apache-2.0)        │   │
│  │   External PostgreSQL (bring your own)                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│  (If external, prompt for connection details)                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q5: OPAL Authorization Stack                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > Yes (recommended for multi-tenant)                     │   │
│  │   No (single-tenant mode)                                │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q6: LLM Provider                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > OpenAI (text-embedding-3-small, gpt-4o)                │   │
│  │   Anthropic (claude-3-haiku)                             │   │
│  │   Ollama (local, no API key)                             │   │
│  │   Skip (configure later)                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│  (If OpenAI/Anthropic, prompt for API key)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q7: OpenCode Integration                                        │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ > Yes (configure MCP tools)                              │   │
│  │   No                                                     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Q8: Advanced Options (optional, collapsed by default)          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ [ ] Enable Ingress                                       │   │
│  │ [ ] Enable ServiceMonitor (Prometheus)                   │   │
│  │ [ ] Enable Network Policies                              │   │
│  │ [ ] Enable HPA (Horizontal Pod Autoscaler)               │   │
│  │ [ ] Enable PodDisruptionBudget                           │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Generation                                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Generating configuration files...                        │   │
│  │                                                          │   │
│  │ ✓ values.yaml                                            │   │
│  │ ✓ docker-compose.yaml                                    │   │
│  │ ✓ .aeterna/config.toml                                   │   │
│  │ ✓ ~/.config/opencode/mcp.json                            │   │
│  │                                                          │   │
│  │ All files generated successfully!                        │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 6. Output File Generators

| Target | Output Files | Purpose |
|--------|--------------|---------|
| Kubernetes | `values.yaml` | Helm chart configuration |
| Docker | `docker-compose.yaml` | Local development |
| Runtime | `.aeterna/config.toml` | Aeterna server configuration |
| OpenCode | `~/.config/opencode/mcp.json` | MCP tool integration |

### 7. Alternatives Considered

1. **Kustomize overlays**: Rejected - less flexible for optional dependencies
2. **Operator pattern**: Rejected - overkill for current scope, higher maintenance
3. **Separate charts per component**: Rejected - harder to manage as single unit
4. **Web-based configurator**: Rejected - CLI is more accessible and scriptable
5. **YAML template engine (ytt, jsonnet)**: Rejected - Helm is more widely adopted

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Subchart version drift | Pin subchart versions in Chart.yaml, document upgrade path |
| Complex values schema | Provide JSON Schema for IDE validation, extensive examples |
| Resource contention | Clear resource requests/limits, HPA configuration |
| CLI maintenance burden | Keep wizard simple, use declarative configuration |
| Docker Compose drift from Helm | Generate both from same source of truth |

## Migration Plan

1. **Phase 1**: CLI setup wizard with Docker Compose output
2. **Phase 2**: Helm chart with core Aeterna services
3. **Phase 3**: Subchart integration (CloudNativePG, Dragonfly, Qdrant)
4. **Phase 4**: Merge OPAL chart as subchart
5. **Phase 5**: Production hardening (PDBs, network policies, HPA)
6. **Phase 6**: OpenCode integration and documentation

## Open Questions

- [x] Container registry: ghcr.io (decided)
- [ ] Base image: distroless, alpine, or debian-slim?
- [ ] Multi-arch builds (amd64 + arm64)?
- [ ] Chart versioning strategy (SemVer, CalVer)?
