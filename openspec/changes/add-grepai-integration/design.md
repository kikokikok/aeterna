# Design: GrepAI Integration

## Context

Aeterna provides memory and knowledge management for AI agents. GrepAI provides semantic code search and call graph analysis. Combining them gives agents complete context: organizational knowledge + codebase understanding.

**Constraints:**
- GrepAI is written in Go; Aeterna is Rust
- Both support MCP protocol
- Both support Qdrant and PostgreSQL/pgvector backends
- Must not break standalone operation of either tool

**Stakeholders:**
- AI coding agents (primary users)
- Platform engineers (deployment)
- Developers (CLI users)

## Goals / Non-Goals

### Goals
- Unified MCP interface exposing both Aeterna and GrepAI tools
- Shared vector backend to reduce infrastructure
- Tenant-aware code search (isolate by project/team)
- Seamless Helm deployment with GrepAI sidecar

### Non-Goals
- Rewriting GrepAI in Rust
- Replacing GrepAI's indexing logic
- Breaking GrepAI's standalone CLI operation
- Real-time bidirectional sync of embeddings

## Architecture Decision

### Option 1: MCP Sidecar (SELECTED)

```
┌─────────────────────────────────────────────────────────┐
│                    Kubernetes Pod                        │
│                                                          │
│  ┌──────────────────┐    ┌──────────────────┐          │
│  │  Aeterna Server  │    │  GrepAI Sidecar  │          │
│  │                  │    │                  │          │
│  │  MCP Server      │◄──►│  MCP Server      │          │
│  │  (Rust)          │    │  (Go)            │          │
│  │                  │    │                  │          │
│  │  Tools:          │    │  Tools:          │          │
│  │  - memory_*      │    │  - grepai_search │          │
│  │  - knowledge_*   │    │  - grepai_trace_*│          │
│  │  - graph_*       │    │                  │          │
│  │  - code_* (proxy)│───►│                  │          │
│  └────────┬─────────┘    └────────┬─────────┘          │
│           │                       │                     │
│           └───────────┬───────────┘                     │
│                       │                                  │
│              ┌────────▼────────┐                        │
│              │  Shared Volume  │                        │
│              │  /data/projects │                        │
│              └─────────────────┘                        │
└─────────────────────────────────────────────────────────┘
                        │
           ┌────────────┴────────────┐
           │                         │
    ┌──────▼──────┐          ┌──────▼──────┐
    │   Qdrant    │          │ PostgreSQL  │
    │             │          │  +pgvector  │
    │ Collections:│          │             │
    │ - aeterna_* │          │ Schemas:    │
    │ - grepai_*  │          │ - aeterna   │
    └─────────────┘          │ - grepai    │
                             └─────────────┘
```

**Pros:**
- Clean separation of concerns
- Each component independently upgradeable
- Shared infrastructure (Qdrant/PostgreSQL)
- MCP-native communication

**Cons:**
- Extra container per pod
- Inter-process communication overhead
- Two processes to monitor

### Option 2: HTTP API Gateway (Rejected)

Run GrepAI as separate Deployment, proxy via HTTP.

**Rejected because:**
- MCP is stdio-based, adding HTTP adds complexity
- Latency for every tool call
- More network hops in Kubernetes

### Option 3: Embedded Library (Rejected)

Call GrepAI Go code from Rust via FFI.

**Rejected because:**
- FFI complexity (cgo + Rust)
- Build complexity (two toolchains)
- Tight coupling makes upgrades hard

## Decisions

### D1: Sidecar Pattern
**Decision:** Deploy GrepAI as sidecar container in Aeterna pod.
**Rationale:** Shares pod lifecycle, network namespace, and volumes. MCP over stdio is reliable and fast.

### D2: Shared Qdrant Collections
**Decision:** Use separate collections with prefixes: `aeterna_memories_{tenant}`, `grepai_code_{tenant}`.
**Rationale:** Isolation by tenant, same Qdrant instance reduces cost.

### D3: MCP Proxy Tools
**Decision:** Aeterna exposes `code_*` tools that proxy to GrepAI's `grepai_*` tools.
**Rationale:** Single MCP endpoint for AI agents. Aeterna handles auth/tenant context.

### D4: Embedding Model Alignment
**Decision:** Configure both to use same embedding model (default: `nomic-embed-text` via Ollama).
**Rationale:** Enables future cross-search (find memories related to code), consistent dimensions.

### D5: File Watching Coordination
**Decision:** GrepAI watches project directories; Aeterna watches knowledge repo. No overlap.
**Rationale:** Clear responsibility boundaries. GrepAI indexes `.rs`, `.ts`, etc. Aeterna indexes knowledge markdown.

## Data Flow

### Code Search Flow

```
Agent                Aeterna               GrepAI              Qdrant
  │                    │                     │                   │
  │─code_search────────►                     │                   │
  │                    │                     │                   │
  │                    │─grepai_search───────►                   │
  │                    │   (MCP stdio)       │                   │
  │                    │                     │─vector_search─────►
  │                    │                     │                   │
  │                    │                     │◄──results─────────│
  │                    │                     │                   │
  │                    │◄──results───────────│                   │
  │                    │                     │                   │
  │                    │  (add tenant ctx)   │                   │
  │◄──results──────────│                     │                   │
```

### Memory + Code Linked Query (Future)

```
Agent                Aeterna               GrepAI              DuckDB
  │                    │                     │                   │
  │─"find code related │                     │                   │
  │  to auth decision" │                     │                   │
  │────────────────────►                     │                   │
  │                    │                     │                   │
  │                    │─memory_search───────────────────────────►
  │                    │   "auth decision"   │                   │
  │                    │◄──memory_id─────────────────────────────│
  │                    │                     │                   │
  │                    │─graph_neighbors─────────────────────────►
  │                    │   (find linked code)│                   │
  │                    │◄──code_file_refs────────────────────────│
  │                    │                     │                   │
  │                    │─grepai_search───────►                   │
  │                    │   (file context)    │                   │
  │                    │◄──code_chunks───────│                   │
  │                    │                     │                   │
  │◄──combined_results─│                     │                   │
```

## Helm Values Schema

```yaml
grepai:
  enabled: false
  
  image:
    repository: ghcr.io/yoanbernabeu/grepai
    tag: "v0.26.0"
    pullPolicy: IfNotPresent
  
  embedder:
    provider: ollama  # ollama | openai
    model: nomic-embed-text
    # For OpenAI:
    # provider: openai
    # model: text-embedding-3-small
    # apiKey: ""  # or existingSecret
  
  store:
    backend: qdrant  # qdrant | postgres | gob
    # Uses Aeterna's Qdrant/PostgreSQL by default
    collectionPrefix: grepai
  
  projects:
    - path: /data/projects/api
      name: api-service
    - path: /data/projects/web
      name: web-app
  
  resources:
    requests:
      cpu: 100m
      memory: 256Mi
    limits:
      cpu: 500m
      memory: 512Mi
  
  watch:
    enabled: true
    debounceMs: 500
```

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| GrepAI version breaks MCP contract | High | Pin version, test in CI, monitor releases |
| Embedding dimension mismatch | High | Validate at startup, fail fast |
| Index corruption on crash | Medium | GrepAI handles gracefully, add liveness probe |
| Sidecar resource contention | Medium | Separate resource limits, priority classes |
| Ollama not available | Medium | Fallback to OpenAI with warning |

## Knowledge-Memory-Code Graph (Deep Integration)

The killer feature: **linking organizational knowledge and memories to actual code** via DuckDB graph layer.

### Unified Graph Schema

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         UNIFIED KNOWLEDGE GRAPH                              │
│                                                                              │
│   ┌─────────────┐         ┌─────────────┐         ┌─────────────┐          │
│   │  KNOWLEDGE  │         │   MEMORY    │         │    CODE     │          │
│   │             │         │             │         │  (GrepAI)   │          │
│   │  • ADRs     │────────►│  • Learnings│────────►│  • Functions│          │
│   │  • Policies │         │  • Decisions│         │  • Classes  │          │
│   │  • Patterns │◄────────│  • Context  │◄────────│  • Files    │          │
│   │  • Runbooks │         │  • Feedback │         │  • Symbols  │          │
│   └──────┬──────┘         └──────┬──────┘         └──────┬──────┘          │
│          │                       │                       │                  │
│          └───────────────────────┼───────────────────────┘                  │
│                                  │                                          │
│                        ┌─────────▼─────────┐                               │
│                        │   DuckDB Graph    │                               │
│                        │                   │                               │
│                        │  Relationships:   │                               │
│                        │  • implements     │                               │
│                        │  • references     │                               │
│                        │  • violates       │                               │
│                        │  • supersedes     │                               │
│                        │  • derived_from   │                               │
│                        └───────────────────┘                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Graph Node Types

| Node Type | Source | Example |
|-----------|--------|---------|
| `knowledge` | Aeterna Knowledge | ADR-042: Use PostgreSQL |
| `memory` | Aeterna Memory | "Auth service needs retry logic" |
| `code_file` | GrepAI | `src/auth/login.rs` |
| `code_symbol` | GrepAI | `fn handle_login()` |
| `code_chunk` | GrepAI | Lines 23-45 of login.rs |

### Graph Edge Types (Relationships)

| Edge Type | From → To | Meaning |
|-----------|-----------|---------|
| `implements` | code_symbol → knowledge | Code implements an ADR decision |
| `references` | memory → code_file | Memory mentions this file |
| `violates` | code_chunk → knowledge | Code violates a policy |
| `derived_from` | memory → code_chunk | Learning derived from this code |
| `calls` | code_symbol → code_symbol | Function call (from GrepAI) |
| `supersedes` | knowledge → knowledge | ADR replaces older ADR |
| `related_to` | any → any | Semantic similarity link |

### DuckDB Schema

```sql
-- Node tables
CREATE TABLE graph_nodes (
    id VARCHAR PRIMARY KEY,
    node_type VARCHAR NOT NULL,  -- 'knowledge', 'memory', 'code_file', 'code_symbol', 'code_chunk'
    tenant_id VARCHAR NOT NULL,
    source VARCHAR NOT NULL,     -- 'aeterna' or 'grepai'
    external_id VARCHAR,         -- ID in source system
    content TEXT,
    metadata JSON,
    embedding FLOAT[768],        -- For similarity queries
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Edge tables
CREATE TABLE graph_edges (
    id VARCHAR PRIMARY KEY,
    from_node_id VARCHAR REFERENCES graph_nodes(id),
    to_node_id VARCHAR REFERENCES graph_nodes(id),
    edge_type VARCHAR NOT NULL,  -- 'implements', 'references', 'violates', etc.
    weight FLOAT DEFAULT 1.0,    -- Confidence/strength
    metadata JSON,
    created_at TIMESTAMP DEFAULT NOW(),
    created_by VARCHAR           -- 'auto', 'agent', 'user'
);

-- Indexes for traversal
CREATE INDEX idx_edges_from ON graph_edges(from_node_id);
CREATE INDEX idx_edges_to ON graph_edges(to_node_id);
CREATE INDEX idx_edges_type ON graph_edges(edge_type);
CREATE INDEX idx_nodes_type ON graph_nodes(node_type, tenant_id);
```

### Link Creation Mechanisms

#### 1. Explicit Agent Links
Agent explicitly creates link during task:

```rust
// Agent learns something about code
memory_add("Auth retry logic needs exponential backoff", layer: Project)
  .link_to_code("src/auth/client.rs", lines: 45-67, relationship: "derived_from")
```

#### 2. Automatic Semantic Links
System detects similarity between memory/knowledge and code:

```rust
// When memory is added, find related code
let memory_embedding = embed("Auth retry logic needs exponential backoff");
let similar_code = grepai_search(embedding: memory_embedding, threshold: 0.85);
for chunk in similar_code {
    graph.create_edge(memory.id, chunk.id, "related_to", weight: chunk.score);
}
```

#### 3. Policy Violation Detection
When code is indexed, check against policies:

```rust
// GrepAI indexes new code chunk
let chunk = CodeChunk { file: "api/handler.rs", content: "panic!()" };

// Check against knowledge policies
let policies = knowledge_query(type: Policy, tags: ["error-handling"]);
for policy in policies {
    if policy.rule.matches(&chunk.content) {
        graph.create_edge(chunk.id, policy.id, "violates", metadata: { line: 42 });
    }
}
```

#### 4. ADR Implementation Tracking
Link code that implements architectural decisions:

```rust
// ADR says "Use Result<T, Error> for error handling"
let adr = knowledge_get("ADR-015-error-handling");

// Find code implementing this pattern
let implementations = grepai_search("Result<.*, Error>");
for impl in implementations {
    graph.create_edge(impl.id, adr.id, "implements");
}
```

### Query Examples

#### "What code implements ADR-042?"
```sql
SELECT n.* FROM graph_nodes n
JOIN graph_edges e ON n.id = e.from_node_id
WHERE e.to_node_id = 'knowledge:ADR-042'
  AND e.edge_type = 'implements'
  AND n.node_type IN ('code_symbol', 'code_file');
```

#### "What memories are related to this function?"
```sql
SELECT n.* FROM graph_nodes n
JOIN graph_edges e ON n.id = e.from_node_id
WHERE e.to_node_id = 'code_symbol:handle_login'
  AND n.node_type = 'memory';
```

#### "What policies does this file violate?"
```sql
SELECT k.*, e.metadata->>'line' as violation_line
FROM graph_nodes k
JOIN graph_edges e ON k.id = e.to_node_id
WHERE e.from_node_id LIKE 'code_chunk:src/auth/%'
  AND e.edge_type = 'violates'
  AND k.node_type = 'knowledge';
```

#### "Traverse: ADR → implementing code → related memories"
```sql
WITH RECURSIVE traversal AS (
    -- Start from ADR
    SELECT id, node_type, 0 as depth
    FROM graph_nodes WHERE id = 'knowledge:ADR-042'
    
    UNION ALL
    
    -- Follow edges
    SELECT n.id, n.node_type, t.depth + 1
    FROM traversal t
    JOIN graph_edges e ON t.id = e.to_node_id OR t.id = e.from_node_id
    JOIN graph_nodes n ON n.id = CASE 
        WHEN e.from_node_id = t.id THEN e.to_node_id 
        ELSE e.from_node_id 
    END
    WHERE t.depth < 3
)
SELECT DISTINCT * FROM traversal;
```

### New MCP Tools for Graph

| Tool | Description |
|------|-------------|
| `graph_link` | Create edge between any two nodes |
| `graph_unlink` | Remove edge between nodes |
| `graph_traverse` | Multi-hop traversal from node |
| `graph_find_path` | Find shortest path between nodes |
| `graph_subgraph` | Extract subgraph around node |
| `graph_violations` | Find all policy violations in code |
| `graph_implementations` | Find code implementing knowledge |

### Sync Strategy

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Aeterna   │     │   GrepAI    │     │   DuckDB    │
│   Events    │     │   Events    │     │   Graph     │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       │ memory_added      │                   │
       │──────────────────────────────────────►│ create node
       │                   │                   │
       │                   │ chunk_indexed     │
       │                   │──────────────────►│ create node
       │                   │                   │
       │                   │                   │ auto-link
       │                   │                   │ (semantic)
       │                   │                   │
       │ knowledge_updated │                   │
       │──────────────────────────────────────►│ update node
       │                   │                   │ re-check links
```

### Example Use Case: Strangler Fig Migration

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    STRANGLER FIG KNOWLEDGE GRAPH                         │
│                                                                          │
│   ADR-001: Strangler Fig Strategy                                       │
│        │                                                                 │
│        ├──implements──► src/facade/payment_facade.rs                    │
│        │                     │                                           │
│        │                     ├──calls──► legacy/payment_service.go      │
│        │                     └──calls──► new/payment_handler.rs         │
│        │                                                                 │
│   Memory: "Legacy API has 20-char ID limit"                             │
│        │                                                                 │
│        ├──derived_from──► legacy/models/payment.go:15-20                │
│        └──related_to──► src/facade/id_transformer.rs                    │
│                                                                          │
│   Policy: "No new code in legacy/"                                      │
│        │                                                                 │
│        └──violates──► legacy/hotfix_2024.go (ALERT!)                    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Agent query**: "What do I need to know before modifying payment_facade.rs?"

**Graph traversal returns**:
1. ADR-001 (architectural context)
2. Memory about ID limit (gotcha)
3. Related legacy code (dependencies)
4. Policy violations nearby (warnings)

## Migration Plan

### Phase 1: Optional Sidecar (v0.2.0)
- Add GrepAI sidecar (disabled by default)
- Implement MCP proxy tools
- Test with shared Qdrant

### Phase 2: CLI Integration (v0.3.0)
- Add `aeterna grepai` commands
- Setup wizard integration
- Documentation

### Phase 3: Deep Integration (v0.4.0)
- DuckDB graph schema for unified nodes/edges
- Sync events from Aeterna and GrepAI
- Automatic semantic linking
- `graph_*` MCP tools for traversal

### Phase 4: Intelligence Layer (v0.5.0)
- Policy violation detection on code index
- ADR implementation tracking
- Memory-to-code derivation
- Cross-search: "find code related to this memory"

## Central Index Service (Org-Wide Repository Indexing)

The enterprise killer feature: **Central GrepAI index across all organization repositories**, updated automatically on PR merge.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ORGANIZATION-WIDE CODE INDEX                              │
│                                                                              │
│   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐            │
│   │  payments-api   │  │   auth-service  │  │   web-frontend  │            │
│   │    (repo)       │  │     (repo)      │  │     (repo)      │            │
│   └────────┬────────┘  └────────┬────────┘  └────────┬────────┘            │
│            │                    │                    │                      │
│            │    PR Merge to main triggers GitHub Action                     │
│            │                    │                    │                      │
│            ▼                    ▼                    ▼                      │
│   ┌─────────────────────────────────────────────────────────────────┐      │
│   │                   GitHub Actions Workflow                        │      │
│   │                                                                  │      │
│   │   1. grepai init --workspace org-{org_id}                       │      │
│   │   2. grepai workspace add org-{org_id} .                        │      │
│   │   3. grepai watch --workspace org-{org_id} --once               │      │
│   │   4. Notify Aeterna Central: POST /api/v1/index/updated         │      │
│   └─────────────────────────────────────────────────────────────────┘      │
│                                    │                                        │
│                                    ▼                                        │
│   ┌─────────────────────────────────────────────────────────────────┐      │
│   │                    AETERNA CENTRAL SERVICE                       │      │
│   │                                                                  │      │
│   │   ┌─────────────────┐    ┌─────────────────┐                   │      │
│   │   │  GrepAI Server  │    │  Aeterna Server │                   │      │
│   │   │                 │    │                 │                   │      │
│   │   │  Workspaces:    │    │  Tenants:       │                   │      │
│   │   │  - org-acme     │    │  - acme-corp    │                   │      │
│   │   │  - org-bigco    │    │  - bigco-inc    │                   │      │
│   │   └────────┬────────┘    └────────┬────────┘                   │      │
│   │            │                      │                             │      │
│   │            └──────────┬───────────┘                             │      │
│   │                       │                                         │      │
│   │              ┌────────▼────────┐                               │      │
│   │              │   Shared Qdrant │                               │      │
│   │              │                 │                               │      │
│   │              │  Collections:   │                               │      │
│   │              │  - grepai_acme_payments-api                     │      │
│   │              │  - grepai_acme_auth-service                     │      │
│   │              │  - grepai_acme_web-frontend                     │      │
│   │              │  - grepai_bigco_backend                         │      │
│   │              │  - aeterna_acme_memories                        │      │
│   │              │  - aeterna_bigco_memories                       │      │
│   │              └─────────────────┘                               │      │
│   └─────────────────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### GrepAI Workspace for Org-Wide Indexing

GrepAI natively supports **workspaces** for multi-project indexing:

```yaml
# ~/.grepai/workspace.yaml (on central server)
version: 1
workspaces:
  org-acme-corp:
    name: org-acme-corp
    store:
      backend: qdrant
      qdrant:
        endpoint: "qdrant.aeterna-central.svc"
        port: 6334
        collection: "grepai_acme-corp"
    embedder:
      provider: ollama
      model: nomic-embed-text
      endpoint: http://ollama.aeterna-central.svc:11434
    projects:
      - name: payments-api
        path: /data/repos/acme-corp/payments-api
      - name: auth-service
        path: /data/repos/acme-corp/auth-service
      - name: web-frontend
        path: /data/repos/acme-corp/web-frontend
```

**Key Feature**: File paths are prefixed with `workspace/project/`:
```
Original: /data/repos/acme-corp/payments-api/src/handler.rs
Stored:   org-acme-corp/payments-api/src/handler.rs
```

This enables **cross-project search with project filtering**:
```bash
# Search all repos in org
grepai search --workspace org-acme-corp "authentication flow"

# Search specific repos only
grepai search --workspace org-acme-corp --project payments-api --project auth-service "JWT token"
```

### GitHub Actions Workflow (Per Repository)

Each repository in the org has this workflow:

```yaml
# .github/workflows/index-to-aeterna.yml
name: Update Aeterna Code Index

on:
  push:
    branches: [main, master]
  workflow_dispatch:

concurrency:
  group: aeterna-index-${{ github.repository }}
  cancel-in-progress: true

env:
  AETERNA_CENTRAL_URL: ${{ secrets.AETERNA_CENTRAL_URL }}
  ORG_ID: ${{ github.repository_owner }}
  PROJECT_NAME: ${{ github.event.repository.name }}

jobs:
  index:
    if: github.repository_owner == 'your-org'  # Fork protection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install GrepAI
        run: |
          curl -sSL https://raw.githubusercontent.com/yoanbernabeu/grepai/main/install.sh | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      - name: Configure GrepAI
        run: |
          mkdir -p ~/.grepai
          cat > ~/.grepai/config.yaml << EOF
          embedder:
            provider: openai
            model: text-embedding-3-small
            api_key: ${{ secrets.OPENAI_API_KEY }}
          store:
            backend: qdrant
            qdrant:
              endpoint: ${{ secrets.QDRANT_ENDPOINT }}
              port: 443
              use_tls: true
              api_key: ${{ secrets.QDRANT_API_KEY }}
              collection: grepai_${{ env.ORG_ID }}_${{ env.PROJECT_NAME }}
          EOF

      - name: Index Repository
        run: |
          grepai init
          grepai watch --once  # Index once and exit

      - name: Notify Aeterna Central
        run: |
          curl -X POST "${{ env.AETERNA_CENTRAL_URL }}/api/v1/index/updated" \
            -H "Authorization: Bearer ${{ secrets.AETERNA_API_KEY }}" \
            -H "Content-Type: application/json" \
            -d '{
              "org_id": "${{ env.ORG_ID }}",
              "project_id": "${{ env.PROJECT_NAME }}",
              "commit_sha": "${{ github.sha }}",
              "indexed_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
            }'

      - name: Update Graph Links
        if: success()
        run: |
          # Trigger Aeterna to refresh graph links for this repo
          curl -X POST "${{ env.AETERNA_CENTRAL_URL }}/api/v1/graph/refresh" \
            -H "Authorization: Bearer ${{ secrets.AETERNA_API_KEY }}" \
            -H "Content-Type: application/json" \
            -d '{
              "org_id": "${{ env.ORG_ID }}",
              "project_id": "${{ env.PROJECT_NAME }}",
              "operations": ["semantic_links", "policy_violations"]
            }'
```

### Cross-Repository Queries

**Agent querying from `payments-api` can now search across all org repos:**

```
Agent: "Find how other services authenticate with our API"

MCP Request:
{
  "tool": "code_search",
  "arguments": {
    "query": "payments-api authentication client",
    "workspace": "org-acme-corp",
    "exclude_project": "payments-api"  // Don't search self
  }
}

Response:
[
  {
    "file": "org-acme-corp/billing-service/src/clients/payments.rs",
    "lines": "45-67",
    "content": "impl PaymentsClient { fn authenticate(&self) { ... } }",
    "score": 0.94
  },
  {
    "file": "org-acme-corp/mobile-backend/src/api/payments_gateway.ts",
    "lines": "23-41",
    "content": "const paymentsAuth = await getServiceToken('payments-api');",
    "score": 0.89
  }
]
```

**Graph-Enhanced Query:**

```
Agent: "What services depend on our authentication changes?"

1. code_trace_callers("AuthMiddleware", workspace: "org-acme-corp")
   → Finds all callers across ALL org repos

2. graph_traverse(from: "ADR-015-auth-flow", edge_type: "implements")
   → Finds all code implementing the auth ADR

3. Combined response shows:
   - 5 services call AuthMiddleware
   - 3 services implement ADR-015
   - 2 policy violations detected in mobile-backend
```

### Aeterna Central API Endpoints

New endpoints for central index management:

```rust
// POST /api/v1/index/updated
// Called by GitHub Actions after indexing
struct IndexUpdatedRequest {
    org_id: String,
    project_id: String,
    commit_sha: String,
    indexed_at: DateTime<Utc>,
}

// POST /api/v1/graph/refresh
// Triggers graph link refresh for a project
struct GraphRefreshRequest {
    org_id: String,
    project_id: String,
    operations: Vec<String>,  // ["semantic_links", "policy_violations", "adr_implementations"]
}

// GET /api/v1/index/status
// Returns indexing status across all org repos
struct IndexStatusResponse {
    org_id: String,
    projects: Vec<ProjectIndexStatus>,
    last_updated: DateTime<Utc>,
    total_chunks: u64,
}

// POST /api/v1/search/cross-repo
// Search across multiple repos with org context
struct CrossRepoSearchRequest {
    org_id: String,
    query: String,
    include_projects: Option<Vec<String>>,
    exclude_projects: Option<Vec<String>>,
    limit: u32,
}
```

### Deployment Models

#### Model A: Centralized (Recommended for Large Orgs)

```
┌─────────────────────────────────────────────┐
│           Aeterna Central Cluster            │
│                                              │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│   │ Aeterna  │  │  GrepAI  │  │  Qdrant  │ │
│   │  Server  │  │  Server  │  │ Cluster  │ │
│   └──────────┘  └──────────┘  └──────────┘ │
│                                              │
│   All repos indexed to central Qdrant       │
│   Single source of truth                     │
└─────────────────────────────────────────────┘
         ▲
         │ HTTPS/gRPC
         │
┌────────┴────────┐
│  GitHub Actions  │
│  (per-repo)      │
└──────────────────┘
```

**Pros**: Single index, cross-repo search, centralized management
**Cons**: Network latency for indexing, central point of failure

#### Model B: Federated (For Multi-Region)

```
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│  US-East      │     │  EU-West      │     │  APAC         │
│  Aeterna      │◄───►│  Aeterna      │◄───►│  Aeterna      │
│  + GrepAI     │     │  + GrepAI     │     │  + GrepAI     │
│  + Qdrant     │     │  + Qdrant     │     │  + Qdrant     │
└───────────────┘     └───────────────┘     └───────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
   US-East repos        EU repos            APAC repos
```

**Pros**: Low latency, regional compliance, fault isolation
**Cons**: Complex federation, eventual consistency

#### Model C: Hybrid (Per-Team Sidecars + Central Index)

```
┌─────────────────────────────────────────────────────────────┐
│                    Aeterna Central                           │
│                                                              │
│   Aggregated index from all team sidecars                   │
│   Cross-team search capability                              │
└────────────────────────┬────────────────────────────────────┘
                         │
        ┌────────────────┼────────────────┐
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Team A Pod  │  │  Team B Pod  │  │  Team C Pod  │
│              │  │              │  │              │
│  Aeterna     │  │  Aeterna     │  │  Aeterna     │
│  + GrepAI    │  │  + GrepAI    │  │  + GrepAI    │
│  (sidecar)   │  │  (sidecar)   │  │  (sidecar)   │
└──────────────┘  └──────────────┘  └──────────────┘
        │                │                │
        ▼                ▼                ▼
   Team A repos     Team B repos     Team C repos
```

**Pros**: Team autonomy, fast local search, central aggregation
**Cons**: Sync complexity, storage duplication

### Strangler Fig Use Case: Cross-Repo Migration Tracking

```
Organization: Acme Corp (migrating monolith to microservices)

Repositories:
- legacy-monolith (being strangled)
- payments-service (new)
- auth-service (new)
- api-gateway (facade)

Agent Query: "What legacy code still needs migration?"

1. Search legacy-monolith for unmigrated handlers
   grepai search --workspace org-acme --project legacy-monolith "func Handle"

2. Cross-reference with new services
   grepai search --workspace org-acme --exclude-project legacy-monolith "PaymentHandler"

3. Graph query: Find code NOT yet implementing new patterns
   SELECT code.* FROM graph_nodes code
   WHERE code.node_type = 'code_symbol'
     AND code.metadata->>'project' = 'legacy-monolith'
     AND NOT EXISTS (
       SELECT 1 FROM graph_edges e
       WHERE e.from_node_id = code.id
         AND e.edge_type = 'migrated_to'
     )

Result: 47 handlers in legacy need migration
        12 have partial implementations in new services
        35 have no migration started
```

## Open Questions

1. **Q: Should we vendor GrepAI or use upstream releases?**
   A: Use upstream. Pin version. Fork only if critical patches needed.

2. **Q: How to handle GrepAI downtime?**
   A: Circuit breaker in proxy. Return graceful error. Memory/knowledge tools still work.

3. **Q: Multi-tenant index isolation?**
   A: Collection prefix per tenant: `grepai_{company}_{team}_{project}`.

4. **Q: Central vs Distributed indexing?**
   A: Start with centralized (Model A) for simplicity. Add federation (Model B) when multi-region is needed.

5. **Q: How to handle large repos (>1M LOC)?**
   A: Incremental indexing via `grepai watch --since <commit>`. Only re-index changed files.

6. **Q: Rate limiting for GitHub Actions?**
   A: Use concurrency groups. Debounce multiple pushes. Batch index updates.
