# Code Search Integration Specification

## ADDED Requirements

### Requirement: Semantic Code Search
The system SHALL provide semantic code search via Code Search integration, enabling natural language queries to find relevant code chunks.

#### Scenario: Search code by meaning
- **GIVEN** a project with indexed code
- **WHEN** agent calls `code_search` with query "user authentication flow"
- **THEN** system returns code chunks semantically matching the query with file paths, line numbers, and relevance scores

#### Scenario: Search with limit
- **GIVEN** a project with indexed code
- **WHEN** agent calls `code_search` with query "error handling" and limit 5
- **THEN** system returns at most 5 code chunks sorted by relevance

#### Scenario: Search respects tenant isolation
- **GIVEN** multiple tenants with indexed codebases
- **WHEN** agent in tenant A searches for code
- **THEN** results only include code from tenant A's projects

---

### Requirement: Call Graph Analysis
The system SHALL provide call graph analysis via Code Search integration, enabling agents to trace function callers and callees.

#### Scenario: Trace callers
- **GIVEN** a codebase with function `HandleLogin`
- **WHEN** agent calls `code_trace_callers` with symbol "HandleLogin"
- **THEN** system returns all functions that call `HandleLogin` with file locations

#### Scenario: Trace callees
- **GIVEN** a codebase with function `ProcessPayment`
- **WHEN** agent calls `code_trace_callees` with symbol "ProcessPayment"
- **THEN** system returns all functions called by `ProcessPayment` with file locations

#### Scenario: Build call graph
- **GIVEN** a codebase with function `AuthMiddleware`
- **WHEN** agent calls `code_graph` with symbol "AuthMiddleware" and depth 2
- **THEN** system returns a graph of callers and callees up to 2 levels deep

---

### Requirement: Real-time Index Updates
The system SHALL maintain up-to-date code indexes by watching file changes.

#### Scenario: Index updates on file change
- **GIVEN** Code Search is watching a project directory
- **WHEN** a source file is modified
- **THEN** the index is updated within 5 seconds

#### Scenario: Index updates on file creation
- **GIVEN** Code Search is watching a project directory
- **WHEN** a new source file is created
- **THEN** the file is indexed within 5 seconds

#### Scenario: Index updates on file deletion
- **GIVEN** Code Search is watching a project directory
- **WHEN** a source file is deleted
- **THEN** the file is removed from the index within 5 seconds

---

### Requirement: MCP Tool Proxy
The system SHALL expose Code Search tools through Aeterna's MCP interface as `code_*` tools.

#### Scenario: Tool discovery
- **GIVEN** Code Search sidecar is enabled
- **WHEN** agent requests available tools via MCP
- **THEN** response includes `code_search`, `code_trace_callers`, `code_trace_callees`, `code_graph`, `code_index_status`

#### Scenario: Proxy forwards requests
- **GIVEN** Code Search sidecar is running
- **WHEN** agent calls `code_search` tool
- **THEN** Aeterna forwards request to Code Search's `codesearch_search` tool and returns transformed response

#### Scenario: Proxy handles Code Search failure
- **GIVEN** Code Search sidecar is unavailable
- **WHEN** agent calls `code_search` tool
- **THEN** Aeterna returns graceful error with status code and message

---

### Requirement: Shared Vector Backend
The system SHALL support configuring Code Search to use Aeterna's vector backend (Qdrant or PostgreSQL/pgvector).

#### Scenario: Shared Qdrant backend
- **GIVEN** Aeterna deployed with Qdrant
- **WHEN** Code Search is configured with `store.backend: qdrant`
- **THEN** Code Search creates collections prefixed with `codesearch_` in the same Qdrant instance

#### Scenario: Shared PostgreSQL backend
- **GIVEN** Aeterna deployed with PostgreSQL/pgvector
- **WHEN** Code Search is configured with `store.backend: postgres`
- **THEN** Code Search creates tables in `codesearch` schema in the same PostgreSQL database

#### Scenario: Collection isolation by tenant
- **GIVEN** multi-tenant deployment with Code Search enabled
- **WHEN** indexing projects for tenant "acme-corp"
- **THEN** Code Search uses collection `codesearch_acme-corp_{project}` for isolation

---

### Requirement: Helm Chart Sidecar Deployment
The system SHALL support deploying Code Search as a sidecar container in the Aeterna pod.

#### Scenario: Enable Code Search sidecar
- **GIVEN** Helm values with `codesearch.enabled: true`
- **WHEN** deploying the chart
- **THEN** Aeterna pod includes Code Search container alongside main container

#### Scenario: Disable Code Search sidecar
- **GIVEN** Helm values with `codesearch.enabled: false` (default)
- **WHEN** deploying the chart
- **THEN** Aeterna pod does not include Code Search container

#### Scenario: Configure Code Search resources
- **GIVEN** Helm values with `codesearch.resources.limits.memory: 1Gi`
- **WHEN** deploying the chart
- **THEN** Code Search container has memory limit of 1Gi

---

### Requirement: CLI Integration
The system SHALL provide CLI commands for interacting with Code Search functionality.

#### Scenario: Initialize project for indexing
- **GIVEN** a project directory at `/path/to/project`
- **WHEN** user runs `aeterna codesearch init /path/to/project`
- **THEN** Code Search configuration is created and initial index is built

#### Scenario: Search from CLI
- **GIVEN** an indexed project
- **WHEN** user runs `aeterna codesearch search "authentication logic"`
- **THEN** CLI displays matching code chunks with file paths and snippets

#### Scenario: Trace from CLI
- **GIVEN** an indexed project
- **WHEN** user runs `aeterna codesearch trace callers LoginHandler`
- **THEN** CLI displays functions that call LoginHandler

---

### Requirement: Embedder Configuration
The system SHALL support configuring embedding providers for Code Search (Ollama or OpenAI).

#### Scenario: Use Ollama embeddings (default)
- **GIVEN** Helm values with `codesearch.embedder.provider: ollama`
- **WHEN** Code Search indexes code
- **THEN** embeddings are generated using local Ollama with model `nomic-embed-text`

#### Scenario: Use OpenAI embeddings
- **GIVEN** Helm values with `codesearch.embedder.provider: openai` and API key configured
- **WHEN** Code Search indexes code
- **THEN** embeddings are generated using OpenAI's `text-embedding-3-small` model

#### Scenario: Embedding model alignment
- **GIVEN** both Aeterna and Code Search configured with same embedding model
- **WHEN** comparing embedding dimensions
- **THEN** both produce embeddings of identical dimensions (768 for nomic-embed-text)

---

### Requirement: Unified Knowledge Graph
The system SHALL maintain a unified graph in DuckDB linking knowledge, memories, and code artifacts.

#### Scenario: Knowledge node creation
- **GIVEN** Aeterna stores a new ADR "ADR-042: Use PostgreSQL"
- **WHEN** the knowledge is persisted
- **THEN** a graph node of type `knowledge` is created in DuckDB with the ADR's ID and content

#### Scenario: Memory node creation
- **GIVEN** Aeterna stores a new memory "Auth retry needs exponential backoff"
- **WHEN** the memory is persisted
- **THEN** a graph node of type `memory` is created in DuckDB with the memory's ID and embedding

#### Scenario: Code node creation
- **GIVEN** Code Search indexes a new file `src/auth/login.rs`
- **WHEN** the file is indexed
- **THEN** graph nodes of type `code_file` and `code_symbol` are created for the file and its functions

---

### Requirement: Graph Edge Creation
The system SHALL support creating edges (relationships) between any graph nodes.

#### Scenario: Explicit link by agent
- **GIVEN** a memory with ID "mem-123" and a code file "src/auth/login.rs"
- **WHEN** agent calls `graph_link` with from="mem-123", to="code:src/auth/login.rs", type="derived_from"
- **THEN** an edge of type `derived_from` is created between the memory and code file

#### Scenario: Automatic semantic link
- **GIVEN** a new memory is added with content about "authentication flow"
- **WHEN** the system finds code chunks with embedding similarity > 0.85
- **THEN** edges of type `related_to` are automatically created with similarity score as weight

#### Scenario: Policy violation link
- **GIVEN** a policy "No panic!() in production code" exists
- **WHEN** Code Search indexes code containing `panic!()`
- **THEN** an edge of type `violates` is created from the code chunk to the policy

---

### Requirement: Graph Traversal Queries
The system SHALL support traversing the unified graph to discover relationships.

#### Scenario: Find code implementing ADR
- **GIVEN** ADR-042 exists with implementing code linked via `implements` edges
- **WHEN** agent calls `graph_traverse` from "ADR-042" following "implements" edges
- **THEN** system returns all code symbols/files that implement the ADR

#### Scenario: Find memories related to code
- **GIVEN** memories linked to code via `derived_from` or `related_to` edges
- **WHEN** agent calls `graph_traverse` from "code:src/auth/login.rs" following inbound edges
- **THEN** system returns all memories related to that code file

#### Scenario: Multi-hop traversal
- **GIVEN** ADR → code → memory chain exists
- **WHEN** agent calls `graph_traverse` from ADR with depth=2
- **THEN** system returns both the implementing code AND memories derived from that code

#### Scenario: Find policy violations
- **GIVEN** code files with `violates` edges to policies
- **WHEN** agent calls `graph_violations` for project "payments-service"
- **THEN** system returns all code chunks violating policies with violation details

---

### Requirement: Cross-Search
The system SHALL support searching across knowledge, memories, and code using semantic similarity.

#### Scenario: Find code related to memory
- **GIVEN** a memory about "database connection pooling"
- **WHEN** agent calls `graph_related` with memory ID and target_type="code"
- **THEN** system returns code chunks semantically similar to the memory content

#### Scenario: Find knowledge related to code
- **GIVEN** code implementing retry logic
- **WHEN** agent calls `graph_related` with code chunk ID and target_type="knowledge"
- **THEN** system returns ADRs and policies related to retry/resilience patterns

#### Scenario: Context gathering for file
- **GIVEN** agent is about to modify `src/payment/handler.rs`
- **WHEN** agent calls `graph_context` for that file
- **THEN** system returns: related ADRs, applicable policies, relevant memories, and call graph neighbors

---

### Requirement: Central Index Service
The system SHALL support organization-wide code indexing with updates triggered on PR merge to main branches.

#### Scenario: Index update on PR merge
- **GIVEN** a GitHub Actions workflow configured for repository `payments-api`
- **WHEN** a PR is merged to the `main` branch
- **THEN** Code Search indexes the repository and notifies Aeterna Central via webhook

#### Scenario: Cross-repository search
- **GIVEN** an organization with 5 indexed repositories
- **WHEN** agent calls `code_search` with workspace "org-acme-corp" and query "authentication flow"
- **THEN** system returns matching code chunks from ALL 5 repositories with project names

#### Scenario: Project-filtered search
- **GIVEN** an organization with indexed repositories including `payments-api` and `auth-service`
- **WHEN** agent calls `code_search` with workspace "org-acme-corp", query "JWT token", and include_projects=["auth-service"]
- **THEN** system returns results ONLY from `auth-service` repository

#### Scenario: Index status tracking
- **GIVEN** multiple repositories being indexed via GitHub Actions
- **WHEN** admin calls `GET /api/v1/index/status` for org "acme-corp"
- **THEN** system returns status of each repository including last_indexed commit and timestamp

---

### Requirement: GitHub Actions Integration
The system SHALL provide a reusable GitHub Actions workflow template for per-repository indexing.

#### Scenario: Workflow template installation
- **GIVEN** a repository in the organization
- **WHEN** admin adds the Aeterna indexing workflow from template
- **THEN** workflow is configured with org secrets for Qdrant, embedder, and Aeterna API

#### Scenario: Fork protection
- **GIVEN** a forked repository running the indexing workflow
- **WHEN** the fork pushes to main
- **THEN** the workflow skips indexing (repository owner check fails)

#### Scenario: Concurrent push handling
- **GIVEN** multiple PRs merged in quick succession
- **WHEN** multiple workflow runs are triggered
- **THEN** only one indexing job runs at a time (concurrency group cancels previous)

---

### Requirement: Incremental Indexing
The system SHALL support incremental indexing to minimize processing time on large repositories.

#### Scenario: Index only changed files
- **GIVEN** a repository with last indexed commit `abc123`
- **WHEN** new commit `def456` changes 3 files
- **THEN** Code Search indexes only the 3 changed files, not the entire repository

#### Scenario: Handle force push
- **GIVEN** a repository with indexed history
- **WHEN** a force push rewrites history
- **THEN** system detects divergence and triggers full re-index

#### Scenario: Track indexing state
- **GIVEN** a repository indexed at commit `abc123`
- **WHEN** querying index status
- **THEN** response includes `last_indexed_commit: "abc123"` and `indexed_at` timestamp

---

### Requirement: Cross-Repository Call Graph
The system SHALL support tracing function calls across repository boundaries within an organization.

#### Scenario: Trace callers across repos
- **GIVEN** function `AuthMiddleware` in `auth-service` is called by `payments-api` and `billing-service`
- **WHEN** agent calls `code_trace_callers` with symbol "AuthMiddleware" and workspace "org-acme-corp"
- **THEN** system returns callers from BOTH `payments-api` and `billing-service` repositories

#### Scenario: Dependency impact analysis
- **GIVEN** a shared library `common-utils` used by 10 services
- **WHEN** agent calls `code_trace_callers` for a function in `common-utils`
- **THEN** system returns all callers across all 10 dependent services with file locations

#### Scenario: Migration tracking
- **GIVEN** legacy monolith with handlers being migrated to microservices
- **WHEN** agent queries for unmigrated code
- **THEN** system identifies legacy code without corresponding `migrated_to` graph edges
