# Aeterna Helm Chart

Universal Memory & Knowledge Framework for Enterprise AI Agent Systems

## Prerequisites

- Kubernetes 1.25+
- Helm 3.10+
- kubectl configured to access your cluster

## Quick Start

```bash
# Add the Aeterna Helm repository (when published)
helm repo add aeterna https://kikokikok.github.io/aeterna
helm repo update

# Install with default configuration (Local mode)
helm install aeterna aeterna/aeterna

# Or install from local chart
helm install aeterna ./charts/aeterna
```

## Deployment Modes

Aeterna supports three deployment modes:

| Mode | Description | Use Case |
|------|-------------|----------|
| **local** | All components deployed in-cluster | Development, single-cluster production |
| **hybrid** | Local cache + remote central server | Multi-cluster with local performance |
| **remote** | Thin client, all data on central server | Edge deployments, cost optimization |

### Local Mode (Default)

```yaml
deploymentMode: local
# All subcharts enabled (PostgreSQL, Qdrant, Dragonfly, OPAL)
```

### Hybrid Mode

```yaml
deploymentMode: hybrid
central:
  url: "https://aeterna-central.example.com"
  auth: apiKey
  existingSecret: "aeterna-central-credentials"
```

### Remote Mode

```yaml
deploymentMode: remote
central:
  url: "https://aeterna-central.example.com"
  auth: oauth2
# Local storage components disabled
```

## Vector Backend Selection

Choose from 7 supported vector backends:

| Backend | Best For | Bundled |
|---------|----------|---------|
| **qdrant** | Default, self-hosted | Yes |
| **pgvector** | Simplicity, existing PostgreSQL | No (uses PostgreSQL) |
| **pinecone** | Managed, serverless | No |
| **weaviate** | Hybrid search, GraphQL | Yes |
| **mongodb** | Existing MongoDB Atlas | Yes (Percona) |
| **vertexai** | GCP environments | No |
| **databricks** | Databricks lakehouse | No |

### Using Qdrant (Default)

```yaml
vectorBackend:
  type: qdrant
  qdrant:
    bundled: true  # Uses subchart

qdrant:
  enabled: true
  replicaCount: 3  # HA configuration
  persistence:
    size: 50Gi
```

### Using External Qdrant

```yaml
vectorBackend:
  type: qdrant
  qdrant:
    bundled: false
    external:
      host: "qdrant.example.com"
      port: 6333
      existingSecret: "qdrant-credentials"

qdrant:
  enabled: false
```

### Using Pinecone

```yaml
vectorBackend:
  type: pinecone
  pinecone:
    enabled: true
    environment: "us-west1-gcp"
    indexName: "aeterna-prod"
    existingSecret: "pinecone-credentials"

qdrant:
  enabled: false
```

### Using pgvector

```yaml
vectorBackend:
  type: pgvector
  pgvector:
    enabled: true
# Uses the same PostgreSQL instance with pgvector extension

qdrant:
  enabled: false
```

## Cache Configuration

```yaml
cache:
  type: dragonfly  # Options: dragonfly, valkey, external

  # Using bundled Dragonfly (default)
  dragonfly:
    enabled: true

  # Or using external Redis
  external:
    enabled: true
    host: "redis.example.com"
    port: 6379
    existingSecret: "redis-credentials"
```

## PostgreSQL Configuration

### Bundled CloudNativePG (Default)

```yaml
postgresql:
  bundled: true
  cluster:
    instances: 3  # HA configuration
    storage:
      size: 50Gi
      storageClass: "fast-ssd"
```

### External PostgreSQL

```yaml
postgresql:
  bundled: false
  external:
    host: "postgres.example.com"
    port: 5432
    database: "aeterna"
    existingSecret: "postgres-credentials"
    sslMode: "require"

cnpg:
  enabled: false
```

## LLM Provider Configuration

```yaml
llm:
  provider: openai  # Options: openai, anthropic, ollama, none

  openai:
    existingSecret: "openai-credentials"
    model: "gpt-4o"
    embeddingModel: "text-embedding-3-small"

  # Or Anthropic
  anthropic:
    existingSecret: "anthropic-credentials"
    model: "claude-sonnet-4-20250514"

  # Or local Ollama
  ollama:
    host: "http://ollama.default.svc:11434"
    model: "llama3.2"
    embeddingModel: "nomic-embed-text"
```

## OPAL Authorization Stack

```yaml
opal:
  enabled: true

  server:
    replicaCount: 2  # HA configuration
    resources:
      limits:
        cpu: 500m
        memory: 512Mi

  cedarAgent:
    enabled: true

  fetcher:
    enabled: true
```

## Observability

### Prometheus Metrics

```yaml
observability:
  serviceMonitor:
    enabled: true
    interval: 30s
    labels:
      release: prometheus
```

### Distributed Tracing

```yaml
observability:
  tracing:
    enabled: true
    endpoint: "http://jaeger-collector:4317"
    samplingRatio: 0.1
```

### Logging

```yaml
observability:
  logging:
    level: info
    format: json
```

## High Availability Configuration

For production deployments:

```yaml
aeterna:
  replicaCount: 3

  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10
    targetCPUUtilizationPercentage: 70

  pdb:
    enabled: true
    minAvailable: 2

  topologySpreadConstraints:
    - maxSkew: 1
      topologyKey: topology.kubernetes.io/zone
      whenUnsatisfiable: DoNotSchedule

postgresql:
  cluster:
    instances: 3

qdrant:
  replicaCount: 3

opal:
  server:
    replicaCount: 2
```

## Network Policies

```yaml
networkPolicy:
  enabled: true
```

When enabled, creates network policies that:
- Allow ingress only from specified sources
- Allow egress to required services (PostgreSQL, Qdrant, Redis)
- Deny all other traffic

## Ingress Configuration

```yaml
aeterna:
  ingress:
    enabled: true
    className: "nginx"
    annotations:
      cert-manager.io/cluster-issuer: "letsencrypt-prod"
    hosts:
      - host: aeterna.example.com
        paths:
          - path: /
            pathType: Prefix
    tls:
      - secretName: aeterna-tls
        hosts:
          - aeterna.example.com
```

## Secrets Management

### Using Existing Secrets

For production, use existing Kubernetes secrets:

```yaml
central:
  existingSecret: "aeterna-central-credentials"
  secretKey: "api-key"

vectorBackend:
  qdrant:
    external:
      existingSecret: "qdrant-credentials"

llm:
  openai:
    existingSecret: "openai-credentials"
```

### Creating Secrets

```bash
# Create secrets before installing the chart
kubectl create secret generic openai-credentials \
  --from-literal=api-key='sk-...'

kubectl create secret generic postgres-credentials \
  --from-literal=postgres-password='...'
```

## Resource Sizing Guide

| Size | Memories | Aeterna | PostgreSQL | Qdrant |
|------|----------|---------|------------|--------|
| **Small** | <1,000 | 256Mi/0.5CPU | 512Mi/0.5CPU | 512Mi/0.5CPU |
| **Medium** | <100,000 | 1Gi/1CPU | 2Gi/1CPU | 2Gi/1CPU |
| **Large** | >100,000 | 4Gi/2CPU | 8Gi/2CPU | 8Gi/4CPU |

## Upgrading

```bash
# Update chart repository
helm repo update

# Upgrade release
helm upgrade aeterna aeterna/aeterna -f values.yaml

# View release history
helm history aeterna

# Rollback if needed
helm rollback aeterna 1
```

## Uninstalling

```bash
helm uninstall aeterna

# Note: PVCs are not deleted by default
kubectl delete pvc -l app.kubernetes.io/instance=aeterna
```

## Troubleshooting

### Check pod status

```bash
kubectl get pods -l app.kubernetes.io/name=aeterna
kubectl describe pod <pod-name>
kubectl logs <pod-name>
```

### Check health endpoints

```bash
kubectl port-forward svc/aeterna 8080:8080
curl http://localhost:8080/health/ready
curl http://localhost:8080/health/live
```

### Common Issues

**PostgreSQL connection failed**
- Check PostgreSQL pod is running
- Verify credentials in secret
- Check network policies

**Vector backend connection failed**
- Verify backend service is reachable
- Check API key/credentials
- Verify collection/index exists

**OPAL policy sync failed**
- Check OPAL server logs
- Verify policy repository access
- Check fetcher connectivity

## Code Search Integration

Aeterna integrates with Code Search to provide semantic code search and call graph analysis capabilities. Code Search runs as a sidecar container and communicates with Aeterna via MCP (Model Context Protocol).

### Quick Start

Enable Code Search in your values:

```yaml
codesearch:
  enabled: true
  
  embedder:
    type: ollama
    model: nomic-embed-text
  
  store:
    type: qdrant
  
  projects:
    - path: /workspace/my-project
      name: my-project
```

### Features

- **Semantic Code Search**: Natural language queries to find code
- **Call Graph Analysis**: Trace function callers and callees
- **Dependency Graphs**: Visualize code dependencies
- **MCP Tools**: 5 code intelligence tools for AI agents

### Configuration Options

#### Embedder Types

**Ollama** (Default - Local, free):
```yaml
codesearch:
  embedder:
    type: ollama
    model: nomic-embed-text  # or mxbai-embed-large, all-minilm

llm:
  ollama:
    host: http://ollama:11434
```

**OpenAI** (Cloud - High quality):
```yaml
codesearch:
  embedder:
    type: openai
    model: text-embedding-3-small

llm:
  provider: openai
  openai:
    apiKey: sk-...
    # Or use existing secret:
    # existingSecret: my-openai-secret
```

#### Storage Backends

**Qdrant** (Recommended):
```yaml
codesearch:
  store:
    type: qdrant
    qdrant:
      collectionPrefix: codesearch_

vectorBackend:
  type: qdrant
  qdrant:
    bundled: true
```

**PostgreSQL**:
```yaml
codesearch:
  store:
    type: postgres
    postgres:
      schema: codesearch

postgresql:
  bundled: true
```

### CLI Usage

```bash
# Initialize a project
kubectl exec -it <pod-name> -c aeterna -- \
  aeterna codesearch init /workspace/project

# Semantic search
kubectl exec -it <pod-name> -c aeterna -- \
  aeterna codesearch search "authentication middleware"

# Call graph analysis
kubectl exec -it <pod-name> -c aeterna -- \
  aeterna codesearch trace callers HandleLogin --recursive

# Build dependency graph
kubectl exec -it <pod-name> -c aeterna -- \
  aeterna codesearch trace graph UserService --depth 2 --format dot
```

### Monitoring

```bash
# Check Code Search health
kubectl exec -it <pod-name> -c codesearch -- \
  curl http://localhost:9090/health

# View logs
kubectl logs <pod-name> -c codesearch -f

# Check index status
kubectl exec -it <pod-name> -c aeterna -- \
  aeterna codesearch status
```

### Troubleshooting

**Sidecar not starting**:
```bash
kubectl logs <pod-name> -c codesearch-init
kubectl logs <pod-name> -c codesearch
```

**Project initialization fails**:
```bash
kubectl exec -it <pod-name> -c codesearch -- \
  codesearch init /workspace/project --force
```

**Slow search**:
- Increase Code Search resources in values.yaml
- Use faster embedder (all-minilm)
- Enable Redis/Dragonfly cache

For complete documentation, see [docs/codesearch-integration.md](../../docs/codesearch-integration.md)

## Values Reference

See [values.yaml](./values.yaml) for the complete list of configurable parameters.

Key parameters:

| Parameter | Description | Default |
|-----------|-------------|---------|
| `deploymentMode` | Deployment mode | `local` |
| `vectorBackend.type` | Vector backend type | `qdrant` |
| `cache.type` | Cache type | `dragonfly` |
| `postgresql.bundled` | Use bundled PostgreSQL | `true` |
| `opal.enabled` | Enable OPAL stack | `true` |
| `codesearch.enabled` | Enable Code Search sidecar | `false` |
| `codesearch.embedder.type` | Code Search embedder type | `ollama` |
| `codesearch.store.type` | Code Search storage backend | `qdrant` |
| `aeterna.replicaCount` | Aeterna replicas | `1` |
| `aeterna.autoscaling.enabled` | Enable HPA | `false` |
| `networkPolicy.enabled` | Enable network policies | `false` |

## License

Apache License 2.0 - See [LICENSE](../../LICENSE) for details.
