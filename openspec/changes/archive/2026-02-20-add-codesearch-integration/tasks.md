# Tasks: Code Search Integration

## 1. Research & Design

### 1.1 Code Search Analysis
- [x] 1.1.1 Clone Code Search repo and analyze MCP server implementation
- [x] 1.1.2 Document Code Search's VectorStore interface compatibility with Aeterna backends
- [x] 1.1.3 Test Code Search with Qdrant backend (same as Aeterna)
- [x] 1.1.4 Test Code Search with PostgreSQL/pgvector backend
- [x] 1.1.5 Document call graph analysis capabilities and languages supported

### 1.2 Integration Architecture
- [x] 1.2.1 Design MCP proxy layer for unified tool exposure
- [x] 1.2.2 Design shared backend configuration (collection prefixes, schemas)
- [x] 1.2.3 Design workspace-to-tenant mapping strategy
- [x] 1.2.4 Design file watching coordination (avoid duplicate indexing)

---

## 2. Helm Chart Integration

### 2.1 Code Search Sidecar
- [x] 2.1.1 Add Code Search container to Aeterna deployment as sidecar
- [x] 2.1.2 Configure shared volume for index files (GOB backend)
- [x] 2.1.3 Add Code Search ConfigMap for `.codesearch/config.yaml`
- [x] 2.1.4 Configure Code Search to use Aeterna's Qdrant instance
- [x] 2.1.5 Add init container for `codesearch init` on mounted project paths

### 2.2 Values Configuration
- [x] 2.2.1 Add `codesearch.enabled` toggle (default: false)
- [x] 2.2.2 Add `codesearch.image` configuration
- [x] 2.2.3 Add `codesearch.embedder` configuration (ollama, openai)
- [x] 2.2.4 Add `codesearch.store` configuration (shared with Aeterna or separate)
- [x] 2.2.5 Add `codesearch.resources` (requests/limits)
- [x] 2.2.6 Add `codesearch.projects` list for auto-initialization

### 2.3 Networking
- [x] 2.3.1 Add Code Search MCP port to Service (stdio or HTTP)
- [x] 2.3.2 Configure inter-container communication for MCP proxy
- [x] 2.3.3 Add NetworkPolicy rules for Code Search

---

## 3. MCP Proxy Layer

### 3.1 Tool Proxy Implementation
- [x] 3.1.1 Create `tools/src/codesearch/mod.rs` module
- [x] 3.1.2 Implement `code_search` tool (proxy to `codesearch_search`)
- [x] 3.1.3 Implement `code_trace_callers` tool
- [x] 3.1.4 Implement `code_trace_callees` tool
- [x] 3.1.5 Implement `code_graph` tool
- [x] 3.1.6 Implement `code_index_status` tool

### 3.2 MCP Client
- [x] 3.2.1 Add MCP client library to Cargo.toml
- [x] 3.2.2 Implement stdio transport to Code Search sidecar
- [x] 3.2.3 Add connection pooling and retry logic
- [x] 3.2.4 Add circuit breaker for Code Search failures

### 3.3 Response Transformation
- [x] 3.3.1 Map Code Search responses to Aeterna's unified format
- [x] 3.3.2 Add tenant context to code search results
- [x] 3.3.3 Integrate call graph data with DuckDB graph layer

---

## 4. CLI Integration

### 4.1 Subcommands
- [x] 4.1.1 Add `aeterna codesearch` subcommand group
- [x] 4.1.2 Implement `aeterna codesearch init <path>` command
- [x] 4.1.3 Implement `aeterna codesearch search <query>` command
- [x] 4.1.4 Implement `aeterna codesearch trace callers <symbol>` command
- [x] 4.1.5 Implement `aeterna codesearch trace callees <symbol>` command
- [x] 4.1.6 Implement `aeterna codesearch status` command

### 4.2 Setup Wizard Integration
- [x] 4.2.1 Add Code Search option to `aeterna setup` wizard
- [x] 4.2.2 Add embedder selection (Ollama local vs OpenAI cloud)
- [x] 4.2.3 Add project path configuration
- [x] 4.2.4 Generate `.codesearch/config.yaml` alongside other configs

---

## 5. Shared Backend Configuration

### 5.1 Qdrant Shared Collections
- [x] 5.1.1 Design collection naming: `aeterna_memories_*` vs `codesearch_code_*`
- [x] 5.1.2 Document embedding dimension requirements (must match)
- [x] 5.1.3 Test concurrent access from both services
- [x] 5.1.4 Add collection cleanup for removed projects

### 5.2 PostgreSQL/pgvector Shared Schema
- [x] 5.2.1 Design table structure: `aeterna.memories` vs `codesearch.chunks`
- [x] 5.2.2 Add migration for Code Search schema (if not auto-created)
- [x] 5.2.3 Configure connection pooling for shared database
- [x] 5.2.4 Add health checks for shared database

### 5.3 DuckDB Unified Knowledge Graph
- [x] 5.3.1 Design unified graph schema (nodes: knowledge, memory, code_file, code_symbol, code_chunk)
- [x] 5.3.2 Design edge types (implements, references, violates, derived_from, calls, related_to)
- [x] 5.3.3 Create DuckDB migrations for graph_nodes and graph_edges tables
- [x] 5.3.4 Implement node sync: Aeterna knowledge → graph_nodes
- [x] 5.3.5 Implement node sync: Aeterna memory → graph_nodes
- [x] 5.3.6 Implement node sync: Code Search chunks → graph_nodes
- [x] 5.3.7 Implement edge sync: Code Search call graph → graph_edges (calls)
- [x] 5.3.8 Add graph visualization export (DOT, JSON)

### 5.4 Automatic Link Detection
- [x] 5.4.1 Implement semantic similarity linking (memory ↔ code, threshold 0.85)
- [x] 5.4.2 Implement policy violation detection on code index
- [x] 5.4.3 Implement ADR implementation tracking (pattern matching)
- [x] 5.4.4 Add background job for periodic link refresh
- [x] 5.4.5 Add link confidence scoring and decay

### 5.5 Graph MCP Tools
- [x] 5.5.1 Implement `graph_link` tool (explicit edge creation)
- [x] 5.5.2 Implement `graph_unlink` tool (edge removal)
- [x] 5.5.3 Implement `graph_traverse` tool (multi-hop traversal)
- [x] 5.5.4 Implement `graph_find_path` tool (shortest path)
- [x] 5.5.5 Implement `graph_violations` tool (policy violation query)
- [x] 5.5.6 Implement `graph_implementations` tool (ADR → code query)
- [x] 5.5.7 Implement `graph_context` tool (gather all context for file)
- [x] 5.5.8 Implement `graph_related` tool (cross-search by similarity)

---

## 6. OpenCode Plugin Integration

### 6.1 Tool Registration
- [x] 6.1.1 Add Code Search tools to OpenCode plugin manifest
- [x] 6.1.2 Update MCP tool definitions in plugin
- [x] 6.1.3 Add tool documentation and examples

### 6.2 Context Enhancement
- [x] 6.2.1 Auto-inject relevant code context from Code Search search
- [x] 6.2.2 Add call graph context for refactoring tasks
- [x] 6.2.3 Link code chunks to related memories

---

## 7. Testing

### 7.1 Unit Tests
- [x] 7.1.1 Test MCP proxy tool implementations
- [x] 7.1.2 Test response transformation
- [x] 7.1.3 Test CLI commands

### 7.2 Integration Tests
- [ ] 7.2.1 Test sidecar deployment in kind cluster
- [ ] 7.2.2 Test shared Qdrant backend
- [ ] 7.2.3 Test shared PostgreSQL backend
- [ ] 7.2.4 Test MCP communication between containers

### 7.3 E2E Tests
- [ ] 7.3.1 Test full flow: index code → search → trace → memory link
- [ ] 7.3.2 Test with real codebase (Aeterna itself)
- [ ] 7.3.3 Test OpenCode plugin with Code Search tools

---

## 8. Central Index Service (Org-Wide)

### 8.1 GitHub Actions Workflow Template
- [x] 8.1.1 Create reusable workflow template for per-repo indexing
- [x] 8.1.2 Add fork protection (`if: github.repository_owner == 'org'`)
- [x] 8.1.3 Add concurrency groups to prevent duplicate runs
- [x] 8.1.4 Configure OpenAI/Ollama embedder based on secrets
- [x] 8.1.5 Add Qdrant Cloud or self-hosted endpoint configuration
- [x] 8.1.6 Add index notification webhook to Aeterna Central
- [x] 8.1.7 Add graph refresh trigger after indexing

### 8.2 Aeterna Central API
- [x] 8.2.1 Implement `POST /api/v1/index/updated` endpoint
- [x] 8.2.2 Implement `POST /api/v1/graph/refresh` endpoint
- [x] 8.2.3 Implement `GET /api/v1/index/status` endpoint
- [x] 8.2.4 Implement `POST /api/v1/search/cross-repo` endpoint
- [x] 8.2.5 Add authentication via API key or OAuth2
- [x] 8.2.6 Add rate limiting for index update notifications
- [x] 8.2.7 Add webhook signature verification

### 8.3 Code Search Workspace Management
- [x] 8.3.1 Design workspace naming convention: `org-{tenant_id}`
- [x] 8.3.2 Implement workspace auto-creation on first project index
- [x] 8.3.3 Implement project auto-registration to workspace
- [x] 8.3.4 Configure workspace-level store (shared Qdrant)
- [x] 8.3.5 Configure workspace-level embedder (consistent model)
- [x] 8.3.6 Add workspace status monitoring

### 8.4 Cross-Repository Search
- [x] 8.4.1 Implement `code_search` with workspace parameter
- [x] 8.4.2 Implement project inclusion/exclusion filters
- [x] 8.4.3 Implement `code_trace_callers` across workspace
- [x] 8.4.4 Implement `code_trace_callees` across workspace
- [x] 8.4.5 Add result grouping by project
- [x] 8.4.6 Add result deduplication for shared code

### 8.5 Incremental Indexing
- [x] 8.5.1 Implement commit-based incremental indexing (`--since <sha>`)
- [x] 8.5.2 Track last indexed commit per project
- [x] 8.5.3 Handle force pushes and rebases
- [x] 8.5.4 Implement chunk invalidation for modified files
- [x] 8.5.5 Add index health checks and repair

---

## 9. Documentation

### 8.1 User Documentation
- [x] 8.1.1 Add Code Search integration guide to docs
- [x] 8.1.2 Document shared backend configuration
- [x] 8.1.3 Add troubleshooting section
- [x] 8.1.4 Add architecture diagram

### 8.2 Chart Documentation
- [x] 8.2.1 Update charts/aeterna/README.md with Code Search section
- [x] 8.2.2 Add example values for Code Search-enabled deployment
- [x] 8.2.3 Document resource requirements

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 9 | Research & Design |
| 2 | 14 | Helm Chart Integration |
| 3 | 10 | MCP Proxy Layer |
| 4 | 10 | CLI Integration |
| 5 | 24 | Shared Backend + Unified Graph |
| 6 | 6 | OpenCode Plugin Integration |
| 7 | 10 | Testing |
| 8 | 27 | Central Index Service (Org-Wide) |
| 9 | 7 | Documentation |
| **Total** | **117** | |

**Estimated effort**: 6-8 weeks

---

## Dependencies

| Dependency | Version | License | Notes |
|------------|---------|---------|-------|
| Code Search | v0.26.0+ | MIT | Go binary, runs as sidecar |
| MCP SDK | latest | MIT | For client communication |

---

## Open Questions

1. **Embedding model alignment**: Should Aeterna and Code Search use the same embedding model for cross-search compatibility?
2. **Index freshness**: How to coordinate file watching between Aeterna's sync and Code Search's watcher?
3. **Multi-tenant isolation**: How to isolate Code Search indexes per tenant in shared backend?
4. **Local vs Cloud embeddings**: Default to Ollama (privacy) or OpenAI (speed)?
