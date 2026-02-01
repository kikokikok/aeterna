# Tasks: GrepAI Integration

## 1. Research & Design

### 1.1 GrepAI Analysis
- [ ] 1.1.1 Clone GrepAI repo and analyze MCP server implementation
- [ ] 1.1.2 Document GrepAI's VectorStore interface compatibility with Aeterna backends
- [ ] 1.1.3 Test GrepAI with Qdrant backend (same as Aeterna)
- [ ] 1.1.4 Test GrepAI with PostgreSQL/pgvector backend
- [ ] 1.1.5 Document call graph analysis capabilities and languages supported

### 1.2 Integration Architecture
- [ ] 1.2.1 Design MCP proxy layer for unified tool exposure
- [ ] 1.2.2 Design shared backend configuration (collection prefixes, schemas)
- [ ] 1.2.3 Design workspace-to-tenant mapping strategy
- [ ] 1.2.4 Design file watching coordination (avoid duplicate indexing)

---

## 2. Helm Chart Integration

### 2.1 GrepAI Sidecar
- [ ] 2.1.1 Add GrepAI container to Aeterna deployment as sidecar
- [ ] 2.1.2 Configure shared volume for index files (GOB backend)
- [ ] 2.1.3 Add GrepAI ConfigMap for `.grepai/config.yaml`
- [ ] 2.1.4 Configure GrepAI to use Aeterna's Qdrant instance
- [ ] 2.1.5 Add init container for `grepai init` on mounted project paths

### 2.2 Values Configuration
- [ ] 2.2.1 Add `grepai.enabled` toggle (default: false)
- [ ] 2.2.2 Add `grepai.image` configuration
- [ ] 2.2.3 Add `grepai.embedder` configuration (ollama, openai)
- [ ] 2.2.4 Add `grepai.store` configuration (shared with Aeterna or separate)
- [ ] 2.2.5 Add `grepai.resources` (requests/limits)
- [ ] 2.2.6 Add `grepai.projects` list for auto-initialization

### 2.3 Networking
- [ ] 2.3.1 Add GrepAI MCP port to Service (stdio or HTTP)
- [ ] 2.3.2 Configure inter-container communication for MCP proxy
- [ ] 2.3.3 Add NetworkPolicy rules for GrepAI

---

## 3. MCP Proxy Layer

### 3.1 Tool Proxy Implementation
- [ ] 3.1.1 Create `tools/src/grepai/mod.rs` module
- [ ] 3.1.2 Implement `code_search` tool (proxy to `grepai_search`)
- [ ] 3.1.3 Implement `code_trace_callers` tool
- [ ] 3.1.4 Implement `code_trace_callees` tool
- [ ] 3.1.5 Implement `code_graph` tool
- [ ] 3.1.6 Implement `code_index_status` tool

### 3.2 MCP Client
- [ ] 3.2.1 Add MCP client library to Cargo.toml
- [ ] 3.2.2 Implement stdio transport to GrepAI sidecar
- [ ] 3.2.3 Add connection pooling and retry logic
- [ ] 3.2.4 Add circuit breaker for GrepAI failures

### 3.3 Response Transformation
- [ ] 3.3.1 Map GrepAI responses to Aeterna's unified format
- [ ] 3.3.2 Add tenant context to code search results
- [ ] 3.3.3 Integrate call graph data with DuckDB graph layer

---

## 4. CLI Integration

### 4.1 Subcommands
- [ ] 4.1.1 Add `aeterna grepai` subcommand group
- [ ] 4.1.2 Implement `aeterna grepai init <path>` command
- [ ] 4.1.3 Implement `aeterna grepai search <query>` command
- [ ] 4.1.4 Implement `aeterna grepai trace callers <symbol>` command
- [ ] 4.1.5 Implement `aeterna grepai trace callees <symbol>` command
- [ ] 4.1.6 Implement `aeterna grepai status` command

### 4.2 Setup Wizard Integration
- [ ] 4.2.1 Add GrepAI option to `aeterna setup` wizard
- [ ] 4.2.2 Add embedder selection (Ollama local vs OpenAI cloud)
- [ ] 4.2.3 Add project path configuration
- [ ] 4.2.4 Generate `.grepai/config.yaml` alongside other configs

---

## 5. Shared Backend Configuration

### 5.1 Qdrant Shared Collections
- [ ] 5.1.1 Design collection naming: `aeterna_memories_*` vs `grepai_code_*`
- [ ] 5.1.2 Document embedding dimension requirements (must match)
- [ ] 5.1.3 Test concurrent access from both services
- [ ] 5.1.4 Add collection cleanup for removed projects

### 5.2 PostgreSQL/pgvector Shared Schema
- [ ] 5.2.1 Design table structure: `aeterna.memories` vs `grepai.chunks`
- [ ] 5.2.2 Add migration for GrepAI schema (if not auto-created)
- [ ] 5.2.3 Configure connection pooling for shared database
- [ ] 5.2.4 Add health checks for shared database

### 5.3 DuckDB Unified Knowledge Graph
- [ ] 5.3.1 Design unified graph schema (nodes: knowledge, memory, code_file, code_symbol, code_chunk)
- [ ] 5.3.2 Design edge types (implements, references, violates, derived_from, calls, related_to)
- [ ] 5.3.3 Create DuckDB migrations for graph_nodes and graph_edges tables
- [ ] 5.3.4 Implement node sync: Aeterna knowledge → graph_nodes
- [ ] 5.3.5 Implement node sync: Aeterna memory → graph_nodes
- [ ] 5.3.6 Implement node sync: GrepAI chunks → graph_nodes
- [ ] 5.3.7 Implement edge sync: GrepAI call graph → graph_edges (calls)
- [ ] 5.3.8 Add graph visualization export (DOT, JSON)

### 5.4 Automatic Link Detection
- [ ] 5.4.1 Implement semantic similarity linking (memory ↔ code, threshold 0.85)
- [ ] 5.4.2 Implement policy violation detection on code index
- [ ] 5.4.3 Implement ADR implementation tracking (pattern matching)
- [ ] 5.4.4 Add background job for periodic link refresh
- [ ] 5.4.5 Add link confidence scoring and decay

### 5.5 Graph MCP Tools
- [ ] 5.5.1 Implement `graph_link` tool (explicit edge creation)
- [ ] 5.5.2 Implement `graph_unlink` tool (edge removal)
- [ ] 5.5.3 Implement `graph_traverse` tool (multi-hop traversal)
- [ ] 5.5.4 Implement `graph_find_path` tool (shortest path)
- [ ] 5.5.5 Implement `graph_violations` tool (policy violation query)
- [ ] 5.5.6 Implement `graph_implementations` tool (ADR → code query)
- [ ] 5.5.7 Implement `graph_context` tool (gather all context for file)
- [ ] 5.5.8 Implement `graph_related` tool (cross-search by similarity)

---

## 6. OpenCode Plugin Integration

### 6.1 Tool Registration
- [ ] 6.1.1 Add GrepAI tools to OpenCode plugin manifest
- [ ] 6.1.2 Update MCP tool definitions in plugin
- [ ] 6.1.3 Add tool documentation and examples

### 6.2 Context Enhancement
- [ ] 6.2.1 Auto-inject relevant code context from GrepAI search
- [ ] 6.2.2 Add call graph context for refactoring tasks
- [ ] 6.2.3 Link code chunks to related memories

---

## 7. Testing

### 7.1 Unit Tests
- [ ] 7.1.1 Test MCP proxy tool implementations
- [ ] 7.1.2 Test response transformation
- [ ] 7.1.3 Test CLI commands

### 7.2 Integration Tests
- [ ] 7.2.1 Test sidecar deployment in kind cluster
- [ ] 7.2.2 Test shared Qdrant backend
- [ ] 7.2.3 Test shared PostgreSQL backend
- [ ] 7.2.4 Test MCP communication between containers

### 7.3 E2E Tests
- [ ] 7.3.1 Test full flow: index code → search → trace → memory link
- [ ] 7.3.2 Test with real codebase (Aeterna itself)
- [ ] 7.3.3 Test OpenCode plugin with GrepAI tools

---

## 8. Central Index Service (Org-Wide)

### 8.1 GitHub Actions Workflow Template
- [ ] 8.1.1 Create reusable workflow template for per-repo indexing
- [ ] 8.1.2 Add fork protection (`if: github.repository_owner == 'org'`)
- [ ] 8.1.3 Add concurrency groups to prevent duplicate runs
- [ ] 8.1.4 Configure OpenAI/Ollama embedder based on secrets
- [ ] 8.1.5 Add Qdrant Cloud or self-hosted endpoint configuration
- [ ] 8.1.6 Add index notification webhook to Aeterna Central
- [ ] 8.1.7 Add graph refresh trigger after indexing

### 8.2 Aeterna Central API
- [ ] 8.2.1 Implement `POST /api/v1/index/updated` endpoint
- [ ] 8.2.2 Implement `POST /api/v1/graph/refresh` endpoint
- [ ] 8.2.3 Implement `GET /api/v1/index/status` endpoint
- [ ] 8.2.4 Implement `POST /api/v1/search/cross-repo` endpoint
- [ ] 8.2.5 Add authentication via API key or OAuth2
- [ ] 8.2.6 Add rate limiting for index update notifications
- [ ] 8.2.7 Add webhook signature verification

### 8.3 GrepAI Workspace Management
- [ ] 8.3.1 Design workspace naming convention: `org-{tenant_id}`
- [ ] 8.3.2 Implement workspace auto-creation on first project index
- [ ] 8.3.3 Implement project auto-registration to workspace
- [ ] 8.3.4 Configure workspace-level store (shared Qdrant)
- [ ] 8.3.5 Configure workspace-level embedder (consistent model)
- [ ] 8.3.6 Add workspace status monitoring

### 8.4 Cross-Repository Search
- [ ] 8.4.1 Implement `code_search` with workspace parameter
- [ ] 8.4.2 Implement project inclusion/exclusion filters
- [ ] 8.4.3 Implement `code_trace_callers` across workspace
- [ ] 8.4.4 Implement `code_trace_callees` across workspace
- [ ] 8.4.5 Add result grouping by project
- [ ] 8.4.6 Add result deduplication for shared code

### 8.5 Incremental Indexing
- [ ] 8.5.1 Implement commit-based incremental indexing (`--since <sha>`)
- [ ] 8.5.2 Track last indexed commit per project
- [ ] 8.5.3 Handle force pushes and rebases
- [ ] 8.5.4 Implement chunk invalidation for modified files
- [ ] 8.5.5 Add index health checks and repair

---

## 9. Documentation

### 8.1 User Documentation
- [ ] 8.1.1 Add GrepAI integration guide to docs
- [ ] 8.1.2 Document shared backend configuration
- [ ] 8.1.3 Add troubleshooting section
- [ ] 8.1.4 Add architecture diagram

### 8.2 Chart Documentation
- [ ] 8.2.1 Update charts/aeterna/README.md with GrepAI section
- [ ] 8.2.2 Add example values for GrepAI-enabled deployment
- [ ] 8.2.3 Document resource requirements

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
| GrepAI | v0.26.0+ | MIT | Go binary, runs as sidecar |
| MCP SDK | latest | MIT | For client communication |

---

## Open Questions

1. **Embedding model alignment**: Should Aeterna and GrepAI use the same embedding model for cross-search compatibility?
2. **Index freshness**: How to coordinate file watching between Aeterna's sync and GrepAI's watcher?
3. **Multi-tenant isolation**: How to isolate GrepAI indexes per tenant in shared backend?
4. **Local vs Cloud embeddings**: Default to Ollama (privacy) or OpenAI (speed)?
