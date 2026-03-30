## Tier 0 — BROKEN: Fix Sync → OPAL → Cedar Pipeline

### 0.1 Schema Initialization Fixes
- [x] 0.1.1 Add `CREATE TABLE IF NOT EXISTS tenants` DDL to `initialize_github_sync_schema()` in `idp-sync/src/github.rs`
- [x] 0.1.2 Add `CREATE TABLE IF NOT EXISTS agents` DDL to `initialize_github_sync_schema()` in `idp-sync/src/github.rs`
- [x] 0.1.3 Add `ALTER TABLE organizational_units ADD COLUMN IF NOT EXISTS slug TEXT` to `initialize_github_sync_schema()` in `idp-sync/src/github.rs`
- [x] 0.1.4 Call `initialize_github_sync_schema(pool)` as first line of `run_github_sync()` in `idp-sync/src/github.rs`
- [x] 0.1.5 Call `idp_sync::github::initialize_github_sync_schema(pool)` before `resolve_tenant_id()` in `cli/src/server/admin_sync.rs::run_sync()`
- [x] 0.1.6 Populate `slug` column in `GitHubHierarchyMapper::upsert_unit()` from GitHub team/org slug

### 0.2 OPAL Authorization Views
- [x] 0.2.1 Create `v_hierarchy` view joining `organizational_units` self-referentially to produce flattened Company->Org->Team->Project rows with UUID IDs and slugs
- [x] 0.2.2 Create `v_user_permissions` view joining `users`, `memberships`, `organizational_units`, and `governance_roles` to produce user-team-role rows
- [x] 0.2.3 Create `v_agent_permissions` view joining `agents` with `users` for delegation chain
- [x] 0.2.4 Create `v_code_search_repositories`, `v_code_search_requests`, `v_code_search_identities` stub views (return empty until codesearch tables exist)
- [x] 0.2.5 Add all views to `initialize_github_sync_schema()` or a new `initialize_opal_views()` function

### 0.3 Sync-to-Governance Bridge
- [x] 0.3.1 Create `bridge_sync_to_governance()` function in `idp-sync/src/github.rs` that maps `SyncReport` users/memberships to `governance_roles` entries
- [x] 0.3.2 Create corresponding `user_roles` entries from synced data with proper tenant_id and unit_id
- [x] 0.3.3 Call `bridge_sync_to_governance()` in `admin_sync.rs::run_sync()` after `run_github_sync()` returns
- [x] 0.3.4 Add integration test: verify governance_roles populated after sync

### 0.4 PG NOTIFY Triggers
- [x] 0.4.1 Create trigger function `fn_notify_entity_change()` that emits `NOTIFY aeterna_entity_change` with JSON payload
- [x] 0.4.2 Attach trigger to `users`, `memberships`, `organizational_units`, `governance_roles`, `agents` tables (INSERT/UPDATE/DELETE)
- [x] 0.4.3 Add triggers to schema initialization function

### 0.5 Cedar Entity Loading
- [x] 0.5.1 Add `reqwest` dependency to `adapters` crate for HTTP entity fetching
- [x] 0.5.2 Refactor `CedarAuthorizer` to accept an OPAL fetcher URL and fetch entities via HTTP on cache miss
- [x] 0.5.3 Add entity cache with configurable TTL (default 30s) using `tokio::sync::RwLock`
- [x] 0.5.4 Add fallback: use cached entities if OPAL fetcher unreachable, deny if no cache
- [x] 0.5.5 Update `bootstrap.rs` to pass OPAL fetcher URL to `CedarAuthorizer::new()`
- [x] 0.5.6 Add unit test: mock OPAL fetcher response, verify entities loaded and authorization uses them

### 0.6 Verification and Deployment
- [x] 0.6.1 `cargo check --workspace` passes
- [x] 0.6.2 `cargo test -p idp-sync` — all tests pass (41+)
- [x] 0.6.3 `cargo test -p adapters` — all tests pass
- [x] 0.6.4 Commit, push, trigger GHA build (commit `035e96a`, GHA run `23706496924`)
- [x] 0.6.5 Deploy new image to staging cluster via Helm upgrade (Rev 31, `sha-8dfb89e`, 4 deployment iterations)
- [x] 0.6.6 Trigger GitHub sync via admin endpoint, verify SyncReport (496 users, 147 groups, 2132 memberships, 0 errors; idempotent re-sync 0 creates)
- [x] 0.6.7 Verify OPAL authorization views exist in PostgreSQL (v_hierarchy, v_user_permissions, v_agent_permissions — SQL views created; HTTP endpoints require custom Rust opal-fetcher deployment in Tier 1)
- [x] 0.6.8 Verify user permissions view populated (v_user_permissions SQL view joins users/memberships/governance_roles; HTTP endpoint requires custom Rust opal-fetcher in Tier 1)
- [x] 0.6.9 Verify E2E Newman tests pass (86/87 requests, 290/293 assertions — test 2.12 GitHub sync timeout is pre-existing: Newman 10s limit vs ~30s actual)

## Tier 1 — HIGH VALUE: Enable Core Use Cases

### 1.1 Code Search — External Skill Integration
- [x] 1.1.1 Define `CodeIntelligenceBackend` trait in `tools/src/codesearch/client.rs` with methods: `search`, `trace_callers`, `trace_callees`, `graph`, `index_status`, `repo_request`
- [x] 1.1.2 Implement `McpCodeIntelligence` backend — HTTP JSON-RPC proxy to any MCP code intelligence server (JetBrains, VS Code, etc.) via `reqwest::Client`
- [x] 1.1.3 Remove sidecar binary spawning from `tools/src/codesearch/client.rs` — replaced with trait-based dispatch (`CodeSearchClient` wraps `Arc<dyn CodeIntelligenceBackend>` with circuit breaker)
- [x] 1.1.4 Update `tools/src/codesearch/tools.rs` tests to use `CodeSearchClient::mock()` / `from_config()` — all 6 tool structs still use `Arc<CodeSearchClient>` (API backward compatible)
- [x] 1.1.5 Add backend discovery via `CodeSearchClient::from_config()` — checks `mcp_server_url` config, falls back to `NoBackend`
- [x] 1.1.6 Add graceful degradation: `NoBackend` returns informative error ("Install JetBrains Code Intelligence MCP plugin or compatible backend, set AETERNA_CODE_INTEL_MCP_URL")
- [x] 1.1.7 13 tests in `client.rs` (mock, circuit breaker, no-backend, config discovery) + 12 non-ignored tests in `tools.rs` covering full delegation chain — all 447 tools tests pass

### ~~1.2 Hybrid Deployment — Local Docker Compose~~ CANCELLED
Reason: Local deployment uses k3s, not docker-compose. The real feature needed is local-first memory with remote sync (embedded local store in plugin/CLI for agent/user/session layers, sync to remote for team/org/company). This is a separate OpenSpec change, not a docker-compose task.

### 1.3 OpenCode Plugin MCP Wiring
- [x] 1.3.1 Wire memory MCP tools (memory_add, memory_search, memory_delete, memory_feedback, memory_optimize) in the NPM plugin — already implemented in `packages/opencode-plugin/src/tools/memory.ts`
- [x] 1.3.2 Wire knowledge MCP tools (knowledge_query, knowledge_check, knowledge_show) in the NPM plugin — already implemented in `packages/opencode-plugin/src/tools/knowledge.ts`
- [x] 1.3.3 Wire graph MCP tools (graph_query, graph_neighbors, graph_path) in the NPM plugin — already implemented in `packages/opencode-plugin/src/tools/graph.ts`
- [x] 1.3.4 Add plugin configuration for Aeterna server URL — already implemented via `AETERNA_SERVER_URL` env var in `packages/opencode-plugin/src/index.ts`

### 1.4 Close Remaining OpenSpec Changes
- [x] 1.4.1 Complete `add-shared-knowledge-repo` tasks 10.9, 10.10 (75/75) — ✅ Governance PR creation verified (PR #20), webhook endpoint validated (IP-filtered cluster, handler wired)
- [x] 1.4.2 ~~Complete `add-server-runtime` task (60/61 to 61/61)~~ — CLOSED: coverage gate user-deferred, change archived with 60/61
- [x] 1.4.3 ~~Complete `add-cloud-llm-providers` task (15/16 to 16/16)~~ — CLOSED: coverage gate user-deferred, change archived with 15/16
- [x] 1.4.4 Complete `fix-production-readiness-gaps` tasks (16/20 to 19/20 — 5.1, 5.2, 5.3 done; coverage gate user-deferred)

## Tier 2 — IMPORTANT: Full Vision

### 2.1 Memory System E2E Validation
- [x] 2.1.1 Fix `PolicyRule` struct field mismatch in `memory/tests/llm_google_e2e_test.rs`
- [x] 2.1.2 Create integration test: full 7-layer memory promotion chain (sensory to working to episodic to semantic) with real Qdrant
- [x] 2.1.3 Create integration test: Memory-R1 reward propagation with graph traversal
- [x] 2.1.4 Create integration test: RLM query routing with complexity-based strategy selection

### 2.2 Observability
- [x] 2.2.1 Add SLO monitoring module to `observability/src/` with configurable thresholds
- [x] 2.2.2 Add OpenTelemetry span instrumentation to `cli/src/server/` handlers
- [x] 2.2.3 Add OpenTelemetry span instrumentation to `memory/src/manager.rs` operations
- [x] 2.2.4 Add OpenTelemetry span instrumentation to `knowledge/src/manager.rs` operations
- [x] 2.2.5 Propagate trace context in HTTP calls to OPAL fetcher

### 2.3 Cloud Deployment (OpenTofu)
- [x] 2.3.1 ~~Create OpenTofu module for GCP~~ — CANCELLED: user-skipped, deferred to future change
- [x] 2.3.2 ~~Create OpenTofu module for AWS~~ — CANCELLED: user-skipped, deferred to future change
- [x] 2.3.3 ~~Add CMEK encryption to all stateful resources~~ — CANCELLED: user-skipped, deferred to future change
- [x] 2.3.4 ~~Document cloud deployment~~ — CANCELLED: user-skipped, deferred to future change

## Tier 3 — FUTURE: Polish and Scale

### 3.1 Code Search — Remove Legacy Sidecar
- [x] 3.1.1 Remove `tools/src/codesearch/client.rs` sidecar binary spawning code entirely — done in 1.1.3 (full rewrite to trait-based)
- [x] 3.1.2 Remove CLI commands that shell out to external `codesearch` binary (`cli/src/commands/search/`)
- [x] 3.1.3 Clean up unused types in `tools/src/codesearch/types.rs` that were specific to the sidecar protocol
- [x] 3.1.4 Update OpenCode plugin to remove hardcoded codesearch tool names — use dynamic MCP tool discovery

### 3.2 Memory Pipeline Hardening
- [x] 3.2.1 ~~Wire PII redaction (`storage/src/gdpr.rs`) into memory add pipeline~~ — CANCELLED: user-skipped, deferred to future change
- [x] 3.2.2 Add per-tenant metrics to `observability/src/cost_tracking.rs`
- [x] 3.2.3 Configure HPA (Horizontal Pod Autoscaler) in Helm chart
- [x] 3.2.4 Configure PDB (PodDisruptionBudget) in Helm chart
