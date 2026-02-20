# Code Search Integration Guide

## Overview

Code Search integration brings semantic code search and call graph analysis capabilities to Aeterna, enabling AI agents to access both organizational knowledge (Aeterna) and codebase understanding (Code Search) through a unified MCP (Model Context Protocol) interface.

## Architecture

### Sidecar Pattern

```
┌─────────────────────────────────────────────────────────────┐
│ Pod: aeterna                                                 │
│                                                              │
│  ┌──────────────────┐         ┌─────────────────────────┐  │
│  │  Aeterna         │         │  Code Search Sidecar         │  │
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

1. **MCP Proxy Tools** (`tools/src/codesearch/`):
   - Implements MCP client for communication with Code Search
   - Provides 5 code intelligence tools
   - Circuit breaker for resilience
   - Tenant context support

2. **CLI Commands** (`cli/src/commands/codesearch/`):
   - `init` - Initialize Code Search for a project
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
codesearch:
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
codesearch:
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
      collectionPrefix: codesearch_
```

#### 3. With OpenAI Embeddings

```yaml
# values.yaml
codesearch:
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
codesearch:
  enabled: true
  
  embedder:
    type: ollama
    model: nomic-embed-text
  
  store:
    type: postgres
    postgres:
      schema: codesearch

postgresql:
  bundled: true
  cluster:
    instances: 3
    storage:
      size: 50Gi
```

### Configuration Reference

#### Code Search Values

```yaml
codesearch:
  # Enable Code Search sidecar
  enabled: false
  
  # MCP server URL (default: localhost for sidecar)
  mcpServerUrl: "http://localhost:9090"
  
  # Code Search image
  image:
    repository: ghcr.io/codesearch/codesearch
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
      collectionPrefix: codesearch_
    
    postgres:
      schema: codesearch
  
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
aeterna codesearch init /path/to/project

# With custom embedder
aeterna codesearch init /path/to/project \
  --embedder openai \
  --store qdrant

# Force re-initialization
aeterna codesearch init /path/to/project --force
```

#### Semantic Code Search

```bash
# Natural language search
aeterna codesearch search "authentication middleware"

# With filters
aeterna codesearch search "database queries" \
  --limit 10 \
  --threshold 0.7 \
  --pattern "*.go" \
  --language go

# JSON output
aeterna codesearch search "error handling" --json

# Files only (no snippets)
aeterna codesearch search "config loader" --files-only
```

#### Call Graph Analysis

```bash
# Find all callers of a function
aeterna codesearch trace callers HandleRequest

# Recursive tracing
aeterna codesearch trace callers AuthMiddleware \
  --recursive \
  --max-depth 3 \
  --file src/auth/middleware.go

# Find all callees
aeterna codesearch trace callees ProcessOrder \
  --recursive \
  --max-depth 2

# Build full call graph
aeterna codesearch trace graph UserService \
  --depth 2 \
  --include-callers \
  --include-callees \
  --format dot

# Generate visualization
aeterna codesearch trace graph AuthService \
  --depth 2 \
  --format dot | dot -Tpng > auth-graph.png

# Mermaid format for docs
aeterna codesearch trace graph OrderService \
  --depth 1 \
  --format mermaid
```

#### Index Status

```bash
# Check all projects
aeterna codesearch status

# Specific project
aeterna codesearch status --project aeterna

# Watch mode (real-time updates)
aeterna codesearch status --watch

# JSON output
aeterna codesearch status --json
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
aeterna codesearch trace callers PaymentService \
  --recursive --max-depth 5 > callers.txt

# Check all affected components
aeterna codesearch trace graph PaymentService \
  --depth 3 \
  --include-callers \
  --format dot | dot -Tpng > impact.png
```

### 3. Onboarding New Developers

Visualize system architecture:

```bash
# Generate architecture diagram
aeterna codesearch trace graph ApplicationCore \
  --depth 2 \
  --include-callers \
  --include-callees \
  --format mermaid > architecture.md
```

### 4. Bug Investigation

Trace execution flow:

```bash
# Find where error is thrown
aeterna codesearch search "InsufficientFundsError"

# Trace all code paths leading to error
aeterna codesearch trace callers ThrowInsufficientFundsError \
  --recursive --max-depth 10
```

### 5. Refactoring Planning

Understand dependencies before refactoring:

```bash
# Check all usages of deprecated function
aeterna codesearch trace callers OldFunction --recursive

# Build dependency graph
aeterna codesearch trace graph OldFunction \
  --depth 5 --include-callers --format json > dependencies.json
```

## Backend Configurations

### Qdrant (Recommended)

Best for production use with semantic search:

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
codesearch:
  store:
    type: postgres
    postgres:
      schema: codesearch

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
codesearch:
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
codesearch:
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
codesearch:
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

### Code Search Sidecar Not Starting

**Symptoms**: Pod stuck in `Init:0/1` or `CrashLoopBackOff`

**Solutions**:

1. Check init container logs:
```bash
kubectl logs -n aeterna <pod-name> -c codesearch-init
```

2. Check sidecar logs:
```bash
kubectl logs -n aeterna <pod-name> -c codesearch
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
kubectl exec -n aeterna <pod-name> -c codesearch -- \
  codesearch init /path/to/project \
  --embedder ollama \
  --store qdrant
```

3. Check init script execution:
```bash
kubectl logs -n aeterna <pod-name> -c codesearch-init | grep "Initializing project"
```

### MCP Connection Errors

**Symptoms**: `code_search` returns connection refused

**Solutions**:

1. Verify MCP server is running:
```bash
kubectl exec -n aeterna <pod-name> -c codesearch -- \
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
  env | grep CODESEARCH
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

2. Increase Code Search resources:
```yaml
codesearch:
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
codesearch:
  embedder:
    type: ollama
    model: all-minilm  # Faster, smaller model
```

4. Reduce index size:
```bash
# Exclude vendor/node_modules
codesearch init /project --exclude vendor --exclude node_modules
```

### High Memory Usage

**Symptoms**: OOMKilled or memory limits exceeded

**Solutions**:

1. Increase memory limits:
```yaml
codesearch:
  resources:
    limits:
      memory: 2Gi
```

2. Use PostgreSQL instead of Qdrant:
```yaml
codesearch:
  store:
    type: postgres
```

3. Reduce batch size for indexing:
```bash
# Set in ConfigMap
CODESEARCH_INDEX_BATCH_SIZE: "50"
```

## Performance Tuning

### Indexing Performance

```yaml
# Fast indexing
codesearch:
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
codesearch:
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
codesearch:
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

codesearch:
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
codesearch:
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

Code Search exposes Prometheus metrics on `/metrics`:

```yaml
# ServiceMonitor
observability:
  serviceMonitor:
    enabled: true
```

**Key Metrics**:
- `codesearch_search_duration_seconds` - Search latency
- `codesearch_index_chunks_total` - Total indexed chunks
- `codesearch_mcp_requests_total` - MCP request count
- `codesearch_embedding_cache_hit_ratio` - Cache effectiveness

### Logs

```bash
# View Code Search logs
kubectl logs -n aeterna <pod-name> -c codesearch -f

# View init logs
kubectl logs -n aeterna <pod-name> -c codesearch-init

# Search for errors
kubectl logs -n aeterna <pod-name> -c codesearch | grep ERROR
```

### Health Checks

```bash
# Check health endpoint
kubectl exec -n aeterna <pod-name> -c codesearch -- \
  curl http://localhost:9090/health

# Check index status
kubectl exec -n aeterna <pod-name> -c aeterna -- \
  aeterna codesearch status --json
```

## Best Practices

### 1. Start with Ollama

```yaml
codesearch:
  embedder:
    type: ollama
    model: nomic-embed-text
```

- No API costs
- Test without rate limits
- Migrate to OpenAI later if needed

### 2. Use Qdrant for Production

```yaml
codesearch:
  store:
    type: qdrant
```

- Best semantic search performance
- Scales well
- Multi-tenancy support

### 3. Enable Init Container

```yaml
codesearch:
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
codesearch:
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
aeterna codesearch status --json | \
  jq '.projects[] | {name, files_indexed, state}'
```

- Track indexing progress
- Detect stale indexes
- Plan re-indexing

## Migration Guide

### From Standalone Code Search

If you're using standalone Code Search:

1. **Export existing indexes**:
```bash
codesearch export --output backup.json
```

2. **Enable Code Search in Helm**:
```yaml
codesearch:
  enabled: true
```

3. **Import indexes** (optional):
```bash
kubectl exec -n aeterna <pod-name> -c codesearch -- \
  codesearch import --input /data/backup.json
```

4. **Update client configuration**:
```python
# Before
mcp_client = MCPClient("http://codesearch:9090")

# After (sidecar)
mcp_client = MCPClient("http://localhost:9090")
```

### Upgrading Code Search Version

```yaml
codesearch:
  image:
    tag: v2.0.0  # Update version
```

```bash
helm upgrade aeterna ./charts/aeterna \
  --namespace aeterna \
  --reuse-values \
  --set codesearch.image.tag=v2.0.0
```

## Central Index Service

The Central Index Service provides a centralised API for managing code search indexes across multiple repositories and tenants. It accepts webhook notifications from CI/CD pipelines, coordinates graph refreshes, reports index status, and enables cross-repository semantic search.

### API Endpoints

#### POST /api/v1/index/updated

Notify the central service that a repository has been re-indexed.

**Headers:**
- `Authorization: Bearer <AETERNA_API_KEY>` (required)
- `X-Hub-Signature-256: sha256=<hex>` (optional, verified when `AETERNA_WEBHOOK_SECRET` is set)

**Request body:**

```json
{
  "repository": "acme/api-server",
  "tenant_id": "acme",
  "commit_sha": "abc123def456",
  "branch": "main",
  "project": "acme/api-server"
}
```

**Response:**

```json
{
  "success": true,
  "queued": true,
  "job_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### POST /api/v1/graph/refresh

Trigger a call-graph refresh for a tenant's project.

**Request body:**

```json
{
  "tenant_id": "acme",
  "project": "acme/api-server"
}
```

**Response:**

```json
{
  "status": "accepted",
  "tenant_id": "acme",
  "project": "acme/api-server"
}
```

#### GET /api/v1/index/status

Query current indexing status. Accepts optional `tenant_id` and `project` query parameters.

**Response:**

```json
{
  "status": "ok",
  "filters": { "tenant_id": "acme", "project": null },
  "projects": []
}
```

#### POST /api/v1/search/cross-repo

Semantic search across all repositories within a tenant workspace.

**Request body:**

```json
{
  "query": "authentication middleware",
  "tenant_id": "acme",
  "projects": ["api-server", "auth-lib"],
  "limit": 10
}
```

**Response:**

```json
{
  "results": [],
  "total": 0
}
```

### Authentication & Rate Limiting

- All endpoints require `Authorization: Bearer <token>` matching the `AETERNA_API_KEY` env var.
- Webhook endpoints optionally verify `X-Hub-Signature-256` against `AETERNA_WEBHOOK_SECRET`.
- Rate limiting: 100 requests per minute per API key (sliding window).

## GitHub Actions Setup

A reusable workflow is provided at `.github/workflows/codesearch-index.yml` to automatically index repositories on push to `main`/`master`.

### Basic Usage

Add the following to your repository:

```yaml
# .github/workflows/index.yml
name: Index Code Search
on:
  push:
    branches: [main]

jobs:
  index:
    uses: kikokikok/aeterna/.github/workflows/codesearch-index.yml@main
    secrets:
      AETERNA_API_KEY: ${{ secrets.AETERNA_API_KEY }}
      QDRANT_URL: ${{ secrets.QDRANT_URL }}
      OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

### Required Secrets

| Secret | Required | Description |
|--------|----------|-------------|
| `AETERNA_API_KEY` | Yes | API key for Central Index Service |
| `QDRANT_URL` | Yes | Qdrant instance URL (e.g. `http://qdrant:6333`) |
| `OPENAI_API_KEY` | No | If set, uses OpenAI embeddings; otherwise falls back to Ollama |

### Required Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `AETERNA_CENTRAL_URL` | No | Central Index Service URL. When set, the workflow notifies Aeterna after indexing. |

### Custom Project Name

```yaml
jobs:
  index:
    uses: kikokikok/aeterna/.github/workflows/codesearch-index.yml@main
    with:
      project_name: my-custom-project
    secrets:
      AETERNA_API_KEY: ${{ secrets.AETERNA_API_KEY }}
      QDRANT_URL: ${{ secrets.QDRANT_URL }}
```

### What the Workflow Does

1. Checks out the repository with full history
2. Downloads the `codesearch` CLI binary
3. Auto-detects embedder type (OpenAI if key present, else Ollama)
4. Initialises and indexes the repository into the tenant's workspace
5. Notifies the Central Index Service (if `AETERNA_CENTRAL_URL` is configured)
6. Triggers a graph refresh for the project

## Multi-Tenant Workspace Management

Each tenant gets an isolated workspace with deterministic naming:

| Concept | Convention | Example |
|---------|------------|---------|
| Workspace name | `org-<lowercase-tenant>` | `org-acme-corp` |
| Collection prefix | `codesearch_<normalised>_` | `codesearch_acme_corp_` |
| Project isolation | Per-project within workspace | `codesearch_acme_corp_api_server` |

Workspaces are created on first use via the `WorkspaceManager`. Each workspace tracks:
- Registered projects
- Store type (qdrant, postgres, etc.)
- Embedder type (openai, ollama)
- Creation and last-active timestamps

## Troubleshooting (Central Index)

### Binary Not Found

**Symptom:** `codesearch: command not found` in CI

**Solutions:**
1. Verify the download URL in the workflow matches the release architecture
2. Check that the binary was moved to a directory in `$PATH`
3. Test locally: `curl -sSfL <url> -o codesearch && chmod +x codesearch && ./codesearch --version`

### Embedding Dimension Mismatch

**Symptom:** `dimension mismatch` error during indexing or search

**Cause:** The collection was created with one embedding model but you're now using a different one (e.g. switched from `nomic-embed-text` at 768 dims to `text-embedding-3-small` at 1536 dims).

**Solutions:**
1. Delete and recreate the Qdrant collection: `curl -X DELETE http://qdrant:6333/collections/<name>`
2. Re-index with `codesearch index . --force`
3. Ensure all repositories in a workspace use the same embedder model

### Qdrant Connection Issues

**Symptom:** `connection refused` or `timeout` when indexing

**Solutions:**
1. Verify `QDRANT_URL` is reachable from the CI runner
2. For private clusters, use a self-hosted runner or tunnel
3. Check firewall/network policy allows the runner IP
4. Test connectivity: `curl -s $QDRANT_URL/collections`

### Rate Limit Exceeded

**Symptom:** `429 Too Many Requests` from the Central Index Service

**Solutions:**
1. The default limit is 100 requests/minute per API key
2. Add concurrency controls to your workflow to avoid parallel runs
3. For bulk re-indexing, space requests with `sleep` between repositories

### Webhook Signature Verification Failed

**Symptom:** `Invalid webhook signature` error

**Solutions:**
1. Ensure `AETERNA_WEBHOOK_SECRET` matches on both the sender and receiver
2. Verify the signature header format is `sha256=<hex>`
3. Check that the request body was not modified in transit (e.g. by a proxy)

## Support

### Documentation
- Code Search: https://github.com/codesearch/codesearch
- Aeterna: https://github.com/kikokikok/aeterna
- MCP Protocol: https://modelcontextprotocol.io

### Community
- GitHub Issues: https://github.com/kikokikok/aeterna/issues
- Discussions: https://github.com/kikokikok/aeterna/discussions

### Commercial Support
Contact: support@aeterna.ai
