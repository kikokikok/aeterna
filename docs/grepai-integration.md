# GrepAI Integration Guide

## Overview

GrepAI integration brings semantic code search and call graph analysis capabilities to Aeterna, enabling AI agents to access both organizational knowledge (Aeterna) and codebase understanding (GrepAI) through a unified MCP (Model Context Protocol) interface.

## Architecture

### Sidecar Pattern

```
┌─────────────────────────────────────────────────────────────┐
│ Pod: aeterna                                                 │
│                                                              │
│  ┌──────────────────┐         ┌─────────────────────────┐  │
│  │  Aeterna         │         │  GrepAI Sidecar         │  │
│  │  Container       │────────▶│  (MCP Server)           │  │
│  │                  │ :9090   │                         │  │
│  │  - Memory API    │         │  - code_search          │  │
│  │  - Knowledge API │         │  - code_trace_callers   │  │
│  │  - Sync Bridge   │         │  - code_trace_callees   │  │
│  │  - Tool Interface│         │  - code_graph           │  │
│  │                  │         │  - code_index_status    │  │
│  └──────────────────┘         └─────────────────────────┘  │
│           │                              │                  │
│           ▼                              ▼                  │
│  ┌────────────────────────────────────────────────────┐    │
│  │  Shared Backends                                    │    │
│  │  - Qdrant (vector storage)                         │    │
│  │  - PostgreSQL (relational storage)                 │    │
│  │  - Redis/Dragonfly (caching)                       │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Components

1. **MCP Proxy Tools** (`tools/src/grepai/`):
   - Implements MCP client for communication with GrepAI
   - Provides 5 code intelligence tools
   - Circuit breaker for resilience
   - Tenant context support

2. **CLI Commands** (`cli/src/commands/grepai/`):
   - `init` - Initialize GrepAI for a project
   - `search` - Semantic code search
   - `trace` - Call graph analysis (callers, callees, graph)
   - `status` - Index status monitoring

3. **Helm Chart Sidecar** (`charts/aeterna/`):
   - Sidecar container in Aeterna pod
   - Init container for project setup
   - Shared backend configuration
   - Health monitoring

## Installation

### Prerequisites

- Kubernetes cluster (1.27+)
- Helm 3.10+
- Storage class for persistent volumes
- Vector database (Qdrant recommended)
- PostgreSQL database

### Helm Installation

#### 1. Basic Installation (Default: Disabled)

```yaml
# values.yaml
grepai:
  enabled: true
  
  embedder:
    type: ollama
    model: nomic-embed-text
  
  store:
    type: qdrant
```

Install the chart:

```bash
helm install aeterna ./charts/aeterna \
  --namespace aeterna \
  --create-namespace \
  --values values.yaml
```

#### 2. With Project Initialization

```yaml
# values.yaml
grepai:
  enabled: true
  
  # Projects to initialize on startup
  projects:
    - path: /workspace/aeterna
      name: aeterna
    - path: /workspace/my-project
      name: my-project
  
  embedder:
    type: ollama
    model: nomic-embed-text
  
  store:
    type: qdrant
    qdrant:
      collectionPrefix: grepai_
```

#### 3. With OpenAI Embeddings

```yaml
# values.yaml
grepai:
  enabled: true
  
  embedder:
    type: openai
    model: text-embedding-3-small
  
  store:
    type: qdrant

llm:
  provider: openai
  openai:
    apiKey: "sk-..."
    # Or use existing secret:
    # existingSecret: my-openai-secret
```

#### 4. With PostgreSQL Storage

```yaml
# values.yaml
grepai:
  enabled: true
  
  embedder:
    type: ollama
    model: nomic-embed-text
  
  store:
    type: postgres
    postgres:
      schema: grepai

postgresql:
  bundled: true
  cluster:
    instances: 3
    storage:
      size: 50Gi
```

### Configuration Reference

#### GrepAI Values

```yaml
grepai:
  # Enable GrepAI sidecar
  enabled: false
  
  # MCP server URL (default: localhost for sidecar)
  mcpServerUrl: "http://localhost:9090"
  
  # GrepAI image
  image:
    repository: ghcr.io/grepai/grepai
    tag: latest
    pullPolicy: IfNotPresent
  
  # Resource limits
  resources:
    limits:
      cpu: 500m
      memory: 512Mi
    requests:
      cpu: 50m
      memory: 128Mi
  
  # Embedder configuration
  embedder:
    type: ollama  # ollama | openai
    model: nomic-embed-text
    endpoint: ""  # Optional, defaults to llm.ollama.host
  
  # Storage backend
  store:
    type: qdrant  # qdrant | postgres | gob
    
    qdrant:
      collectionPrefix: grepai_
    
    postgres:
      schema: grepai
  
  # Projects to initialize
  projects: []
    # - path: /workspace/project1
    #   name: project1
  
  # Init container settings
  initContainer:
    enabled: true
    timeout: 300
```

## Usage

### CLI Commands

#### Initialize a Project

```bash
# Basic initialization
aeterna grepai init /path/to/project

# With custom embedder
aeterna grepai init /path/to/project \
  --embedder openai \
  --store qdrant

# Force re-initialization
aeterna grepai init /path/to/project --force
```

#### Semantic Code Search

```bash
# Natural language search
aeterna grepai search "authentication middleware"

# With filters
aeterna grepai search "database queries" \
  --limit 10 \
  --threshold 0.7 \
  --pattern "*.go" \
  --language go

# JSON output
aeterna grepai search "error handling" --json

# Files only (no snippets)
aeterna grepai search "config loader" --files-only
```

#### Call Graph Analysis

```bash
# Find all callers of a function
aeterna grepai trace callers HandleRequest

# Recursive tracing
aeterna grepai trace callers AuthMiddleware \
  --recursive \
  --max-depth 3 \
  --file src/auth/middleware.go

# Find all callees
aeterna grepai trace callees ProcessOrder \
  --recursive \
  --max-depth 2

# Build full call graph
aeterna grepai trace graph UserService \
  --depth 2 \
  --include-callers \
  --include-callees \
  --format dot

# Generate visualization
aeterna grepai trace graph AuthService \
  --depth 2 \
  --format dot | dot -Tpng > auth-graph.png

# Mermaid format for docs
aeterna grepai trace graph OrderService \
  --depth 1 \
  --format mermaid
```

#### Index Status

```bash
# Check all projects
aeterna grepai status

# Specific project
aeterna grepai status --project aeterna

# Watch mode (real-time updates)
aeterna grepai status --watch

# JSON output
aeterna grepai status --json
```

### MCP Tools (Programmatic)

#### Tool: code_search

Search codebase using natural language:

```json
{
  "tool": "code_search",
  "arguments": {
    "query": "authentication middleware",
    "limit": 10,
    "threshold": 0.7,
    "tenant_id": "org-123",
    "file_pattern": "*.go",
    "language": "go"
  }
}
```

Response:
```json
{
  "results": [
    {
      "file": "src/auth/middleware.go",
      "line": 42,
      "score": 0.92,
      "snippet": "func AuthMiddleware(next http.Handler) http.Handler {",
      "context": "..."
    }
  ],
  "total": 5,
  "query_embedding_cached": true
}
```

#### Tool: code_trace_callers

Find all functions that call a symbol:

```json
{
  "tool": "code_trace_callers",
  "arguments": {
    "symbol": "HandleLogin",
    "file_hint": "src/handlers/auth.go",
    "recursive": true,
    "max_depth": 3,
    "tenant_id": "org-123"
  }
}
```

Response:
```json
{
  "callers": [
    {
      "symbol": "AuthRouter",
      "file": "src/routes/auth.go",
      "line": 15,
      "depth": 1
    },
    {
      "symbol": "main",
      "file": "cmd/server/main.go",
      "line": 42,
      "depth": 2
    }
  ],
  "symbol": "HandleLogin",
  "total_callers": 2,
  "max_depth_reached": false
}
```

#### Tool: code_trace_callees

Find all functions called by a symbol:

```json
{
  "tool": "code_trace_callees",
  "arguments": {
    "symbol": "ProcessOrder",
    "recursive": true,
    "max_depth": 2,
    "tenant_id": "org-123"
  }
}
```

#### Tool: code_graph

Build a call dependency graph:

```json
{
  "tool": "code_graph",
  "arguments": {
    "symbol": "UserService",
    "depth": 2,
    "include_callers": true,
    "include_callees": true,
    "tenant_id": "org-123"
  }
}
```

Response:
```json
{
  "graph": {
    "nodes": [
      {"id": "UserService", "type": "root"},
      {"id": "CreateUser", "type": "callee"},
      {"id": "ValidateUser", "type": "callee"},
      {"id": "UserHandler", "type": "caller"}
    ],
    "edges": [
      {"from": "UserService", "to": "CreateUser"},
      {"from": "UserService", "to": "ValidateUser"},
      {"from": "UserHandler", "to": "UserService"}
    ]
  },
  "symbol": "UserService",
  "depth": 2
}
```

#### Tool: code_index_status

Get indexing status:

```json
{
  "tool": "code_index_status",
  "arguments": {
    "project": "aeterna",
    "tenant_id": "org-123"
  }
}
```

Response:
```json
{
  "projects": [
    {
      "name": "aeterna",
      "path": "/workspace/aeterna",
      "state": "indexed",
      "files_indexed": 342,
      "total_chunks": 5281,
      "last_indexed": "2026-02-01T19:00:00Z"
    }
  ]
}
```

## Use Cases

### 1. AI Agent Code Understanding

Enable AI agents to understand codebase structure:

```python
# Agent query: "Find authentication implementation"
result = mcp_client.call_tool("code_search", {
    "query": "user authentication flow",
    "limit": 5,
    "tenant_id": session.tenant_id
})

# Agent analyzes results and traces dependencies
for match in result["results"]:
    callers = mcp_client.call_tool("code_trace_callers", {
        "symbol": extract_symbol(match["snippet"]),
        "file_hint": match["file"],
        "recursive": True,
        "max_depth": 2,
        "tenant_id": session.tenant_id
    })
```

### 2. Code Review Assistance

Help reviewers understand impact:

```bash
# PR introduces changes to PaymentService
aeterna grepai trace callers PaymentService \
  --recursive --max-depth 5 > callers.txt

# Check all affected components
aeterna grepai trace graph PaymentService \
  --depth 3 \
  --include-callers \
  --format dot | dot -Tpng > impact.png
```

### 3. Onboarding New Developers

Visualize system architecture:

```bash
# Generate architecture diagram
aeterna grepai trace graph ApplicationCore \
  --depth 2 \
  --include-callers \
  --include-callees \
  --format mermaid > architecture.md
```

### 4. Bug Investigation

Trace execution flow:

```bash
# Find where error is thrown
aeterna grepai search "InsufficientFundsError"

# Trace all code paths leading to error
aeterna grepai trace callers ThrowInsufficientFundsError \
  --recursive --max-depth 10
```

### 5. Refactoring Planning

Understand dependencies before refactoring:

```bash
# Check all usages of deprecated function
aeterna grepai trace callers OldFunction --recursive

# Build dependency graph
aeterna grepai trace graph OldFunction \
  --depth 5 --include-callers --format json > dependencies.json
```

## Backend Configurations

### Qdrant (Recommended)

Best for production use with semantic search:

```yaml
grepai:
  store:
    type: qdrant
    qdrant:
      collectionPrefix: grepai_

vectorBackend:
  type: qdrant
  qdrant:
    bundled: true
```

**Pros**:
- Fast semantic search
- Efficient similarity scoring
- Scales well with large codebases
- Native multi-tenancy support

**Cons**:
- Requires more memory
- Separate service to manage

### PostgreSQL

Good for unified storage with Aeterna data:

```yaml
grepai:
  store:
    type: postgres
    postgres:
      schema: grepai

postgresql:
  bundled: true
```

**Pros**:
- Single database for all data
- Strong consistency
- Familiar query language
- ACID transactions

**Cons**:
- Slower semantic search
- Requires pgvector extension
- Higher query latency

### GOB (File-based)

Suitable for development/testing:

```yaml
grepai:
  store:
    type: gob
```

**Pros**:
- No external dependencies
- Fast for small projects
- Simple setup

**Cons**:
- Not production-ready
- No multi-tenancy
- Limited scalability
- No persistence across restarts

## Embedder Options

### Ollama (Default)

Local, open-source embeddings:

```yaml
grepai:
  embedder:
    type: ollama
    model: nomic-embed-text

llm:
  ollama:
    host: http://ollama:11434
```

**Recommended Models**:
- `nomic-embed-text` - Best all-around (768 dims)
- `mxbai-embed-large` - Highest quality (1024 dims)
- `all-minilm` - Fastest (384 dims)

**Pros**:
- No API costs
- Data privacy (local)
- Consistent embeddings
- Offline capable

**Cons**:
- Slower than cloud APIs
- Requires GPU for speed
- Limited model selection

### OpenAI

Cloud-based, high-quality embeddings:

```yaml
grepai:
  embedder:
    type: openai
    model: text-embedding-3-small

llm:
  provider: openai
  openai:
    apiKey: sk-...
```

**Recommended Models**:
- `text-embedding-3-small` - Best value (1536 dims, $0.02/1M tokens)
- `text-embedding-3-large` - Highest quality (3072 dims, $0.13/1M tokens)
- `text-embedding-ada-002` - Legacy (1536 dims)

**Pros**:
- Highest quality
- Very fast
- No infrastructure
- Latest models

**Cons**:
- API costs
- Data leaves cluster
- Rate limits
- Internet dependency

## Troubleshooting

### GrepAI Sidecar Not Starting

**Symptoms**: Pod stuck in `Init:0/1` or `CrashLoopBackOff`

**Solutions**:

1. Check init container logs:
```bash
kubectl logs -n aeterna <pod-name> -c grepai-init
```

2. Check sidecar logs:
```bash
kubectl logs -n aeterna <pod-name> -c grepai
```

3. Verify embedder configuration:
```bash
# For Ollama
kubectl get svc -n aeterna ollama

# For OpenAI
kubectl get secret -n aeterna aeterna-openai
```

4. Check storage backend:
```bash
# For Qdrant
kubectl get svc -n aeterna aeterna-qdrant

# For PostgreSQL
kubectl get cluster -n aeterna aeterna-postgresql
```

### Project Initialization Fails

**Symptoms**: Init container succeeds but no projects indexed

**Solutions**:

1. Check if projects exist in values:
```bash
helm get values aeterna -n aeterna | grep -A 10 "projects:"
```

2. Manually initialize:
```bash
kubectl exec -n aeterna <pod-name> -c grepai -- \
  grepai init /path/to/project \
  --embedder ollama \
  --store qdrant
```

3. Check init script execution:
```bash
kubectl logs -n aeterna <pod-name> -c grepai-init | grep "Initializing project"
```

### MCP Connection Errors

**Symptoms**: `code_search` returns connection refused

**Solutions**:

1. Verify MCP server is running:
```bash
kubectl exec -n aeterna <pod-name> -c grepai -- \
  curl http://localhost:9090/health
```

2. Check port forwarding:
```bash
kubectl port-forward -n aeterna <pod-name> 9090:9090
curl http://localhost:9090/health
```

3. Verify MCP server URL in Aeterna config:
```bash
kubectl exec -n aeterna <pod-name> -c aeterna -- \
  env | grep GREPAI
```

### Slow Semantic Search

**Symptoms**: `code_search` takes >5 seconds

**Solutions**:

1. Check embedding cache:
```bash
# Enable Redis/Dragonfly cache
cache:
  type: dragonfly
  dragonfly:
    enabled: true
```

2. Increase GrepAI resources:
```yaml
grepai:
  resources:
    limits:
      cpu: 2000m
      memory: 2Gi
    requests:
      cpu: 500m
      memory: 1Gi
```

3. Use faster embedder:
```yaml
grepai:
  embedder:
    type: ollama
    model: all-minilm  # Faster, smaller model
```

4. Reduce index size:
```bash
# Exclude vendor/node_modules
grepai init /project --exclude vendor --exclude node_modules
```

### High Memory Usage

**Symptoms**: OOMKilled or memory limits exceeded

**Solutions**:

1. Increase memory limits:
```yaml
grepai:
  resources:
    limits:
      memory: 2Gi
```

2. Use PostgreSQL instead of Qdrant:
```yaml
grepai:
  store:
    type: postgres
```

3. Reduce batch size for indexing:
```bash
# Set in ConfigMap
GREPAI_INDEX_BATCH_SIZE: "50"
```

## Performance Tuning

### Indexing Performance

```yaml
# Fast indexing
grepai:
  resources:
    limits:
      cpu: 4000m
      memory: 4Gi
  embedder:
    type: openai  # Fastest
    model: text-embedding-3-small
```

### Query Performance

```yaml
# Fast queries
grepai:
  store:
    type: qdrant  # Best for semantic search
cache:
  type: dragonfly  # Enable embedding cache
  dragonfly:
    enabled: true
```

### Resource Optimization

```yaml
# Balanced configuration
grepai:
  resources:
    limits:
      cpu: 1000m
      memory: 1Gi
    requests:
      cpu: 200m
      memory: 512Mi
  embedder:
    type: ollama
    model: nomic-embed-text  # Good quality/speed ratio
```

## Security

### Network Policies

```yaml
networkPolicy:
  enabled: true

grepai:
  enabled: true
  # Sidecar only accessible from Aeterna container
```

### Secret Management

```yaml
# Use existing secrets
llm:
  openai:
    existingSecret: my-openai-secret

vectorBackend:
  qdrant:
    external:
      existingSecret: my-qdrant-secret

postgresql:
  external:
    existingSecret: my-postgres-secret
```

### Pod Security Context

```yaml
grepai:
  # Already configured in deployment
  securityContext:
    allowPrivilegeEscalation: false
    readOnlyRootFilesystem: true
    runAsNonRoot: true
    runAsUser: 1000
    capabilities:
      drop:
        - ALL
```

## Monitoring

### Metrics

GrepAI exposes Prometheus metrics on `/metrics`:

```yaml
# ServiceMonitor
observability:
  serviceMonitor:
    enabled: true
```

**Key Metrics**:
- `grepai_search_duration_seconds` - Search latency
- `grepai_index_chunks_total` - Total indexed chunks
- `grepai_mcp_requests_total` - MCP request count
- `grepai_embedding_cache_hit_ratio` - Cache effectiveness

### Logs

```bash
# View GrepAI logs
kubectl logs -n aeterna <pod-name> -c grepai -f

# View init logs
kubectl logs -n aeterna <pod-name> -c grepai-init

# Search for errors
kubectl logs -n aeterna <pod-name> -c grepai | grep ERROR
```

### Health Checks

```bash
# Check health endpoint
kubectl exec -n aeterna <pod-name> -c grepai -- \
  curl http://localhost:9090/health

# Check index status
kubectl exec -n aeterna <pod-name> -c aeterna -- \
  aeterna grepai status --json
```

## Best Practices

### 1. Start with Ollama

```yaml
grepai:
  embedder:
    type: ollama
    model: nomic-embed-text
```

- No API costs
- Test without rate limits
- Migrate to OpenAI later if needed

### 2. Use Qdrant for Production

```yaml
grepai:
  store:
    type: qdrant
```

- Best semantic search performance
- Scales well
- Multi-tenancy support

### 3. Enable Init Container

```yaml
grepai:
  initContainer:
    enabled: true
  projects:
    - path: /workspace/project
      name: project
```

- Automatic setup on deployment
- No manual initialization
- Consistent across environments

### 4. Configure Resource Limits

```yaml
grepai:
  resources:
    limits:
      cpu: 1000m
      memory: 1Gi
    requests:
      cpu: 200m
      memory: 512Mi
```

- Prevents resource starvation
- Enables autoscaling
- Predictable costs

### 5. Use Semantic Cache

```yaml
cache:
  type: dragonfly
  dragonfly:
    enabled: true
```

- 60-80% cost reduction
- Faster responses
- Reduced embedder load

### 6. Monitor Index Status

```bash
# Add to monitoring dashboard
aeterna grepai status --json | \
  jq '.projects[] | {name, files_indexed, state}'
```

- Track indexing progress
- Detect stale indexes
- Plan re-indexing

## Migration Guide

### From Standalone GrepAI

If you're using standalone GrepAI:

1. **Export existing indexes**:
```bash
grepai export --output backup.json
```

2. **Enable GrepAI in Helm**:
```yaml
grepai:
  enabled: true
```

3. **Import indexes** (optional):
```bash
kubectl exec -n aeterna <pod-name> -c grepai -- \
  grepai import --input /data/backup.json
```

4. **Update client configuration**:
```python
# Before
mcp_client = MCPClient("http://grepai:9090")

# After (sidecar)
mcp_client = MCPClient("http://localhost:9090")
```

### Upgrading GrepAI Version

```yaml
grepai:
  image:
    tag: v2.0.0  # Update version
```

```bash
helm upgrade aeterna ./charts/aeterna \
  --namespace aeterna \
  --reuse-values \
  --set grepai.image.tag=v2.0.0
```

## Support

### Documentation
- GrepAI: https://github.com/grepai/grepai
- Aeterna: https://github.com/kikokikok/aeterna
- MCP Protocol: https://modelcontextprotocol.io

### Community
- GitHub Issues: https://github.com/kikokikok/aeterna/issues
- Discussions: https://github.com/kikokikok/aeterna/discussions

### Commercial Support
Contact: support@aeterna.ai
