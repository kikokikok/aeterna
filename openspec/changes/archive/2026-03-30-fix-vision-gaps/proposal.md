# Change: Fix Vision Gaps — Complete Aeterna Vision Alignment

## Why

Aeterna has ~167,000 lines of Rust across 18 workspace crates covering 23 OpenSpec specifications with ~270+ formal requirements. However, a comprehensive gap analysis reveals that while **~60% of the code exists**, only **~40% is deployed and actually working end-to-end**. The most critical gap is the GitHub Org Sync → OPAL/Cedar authorization pipeline: sync code compiles and tests pass in isolation, but the deployed stack fails at runtime because the `tenants` table doesn't exist, schema initialization is never called, and OPAL fetcher views are missing. Beyond this immediate blocker, multiple major vision components (code search, hybrid deployment, OpenCode integration) are scaffolded but non-functional.

This change documents every gap, fixes the critical sync blockers, and creates a structured implementation plan for the remaining vision.

## What Changes

### Tier 0 — BROKEN (deployed but non-functional)
- **BREAKING**: Fix 3 BLOCKER bugs preventing GitHub Org Sync from running
- Add `tenants` table DDL to `initialize_github_sync_schema()`
- Call `initialize_github_sync_schema()` at start of `run_github_sync()` and in `admin_sync.rs` before `resolve_tenant_id()`
- Create 6 PostgreSQL views for OPAL fetcher (`v_hierarchy`, `v_user_permissions`, `v_agent_permissions`, `v_code_search_repositories`, `v_code_search_requests`, `v_code_search_identities`)
- Bridge sync results to `governance_roles`/`user_roles` tables
- Wire `CedarAuthorizer` to load entities from OPAL instead of `Entities::empty()`
- Add `slug` column to `organizational_units`
- Create `agents` table for `v_agent_permissions` view
- Add PG NOTIFY triggers for real-time OPAL updates

### Tier 1 — HIGH VALUE (enables core use cases)
- Code Search: Build indexing pipeline (tree-sitter parsing + embedding generation → vector storage)
- Code Search: Wire MCP tools (`code_search`, `code_trace_callers`, `code_trace_callees`, `code_graph`, `code_index_status`) to actual search backend
- Hybrid Deployment: Define local vs. cloud component split, add Aeterna binary to `docker-compose.yml`
- OpenCode Plugin: Wire all MCP tools to the NPM plugin
- Close remaining OpenSpec changes (7 tasks across 4 active changes)

### Tier 2 — IMPORTANT (full vision)
- Memory System: End-to-end test of full 7-layer promotion chain with real backends
- Observability: Add SLO monitoring and distributed tracing across all crates
- Cloud Deployment: OpenTofu modules for GCP/AWS
- Fix `PolicyRule` struct field mismatch in `memory/llm_google_e2e_test`

### Tier 3 — FUTURE (polish & scale)
- Cross-repo call graph resolution
- PII redaction wired into memory pipeline
- HPA + PDB for HA
- Per-tenant metrics in observability

## Impact

### Affected specs (13 of 23)
- `multi-tenant-governance` — Fix ReBAC disconnection, create views, bridge sync→governance
- `codesearch` — Build indexing pipeline
- `codesearch-integration` — Wire MCP tools to search backend
- `deployment` — Hybrid mode, local docker-compose
- `observability` — SLO monitoring, distributed tracing
- `opencode-integration` — Wire MCP tools in NPM plugin
- `memory-system` — E2E validation, fix test compilation
- `storage` — New tables (tenants, agents), views, schema fixes
- `context-architect` — Integration testing
- `hindsight-learning` — Integration testing
- `meta-agent` — Integration testing
- `note-taking-agent` — Integration testing
- `extension-system` — E2E testing

### Affected code
- `idp-sync/src/github.rs` — Schema init fixes
- `cli/src/server/admin_sync.rs` — Call order fix
- `opal-fetcher/src/` — View queries
- `adapters/src/auth/cedar.rs` — Entity loading
- `storage/src/postgres.rs` — New DDL (tenants, agents, views, triggers)
- `tools/src/codesearch/` — Indexing pipeline
- `tools/src/central_index/` — Central index service
- `packages/opencode-plugin/` — MCP tool wiring
- `charts/aeterna/` — Docker Compose, Helm updates
- `docker-compose.yml` — Add Aeterna service
- `memory/tests/` — Fix PolicyRule fields

### Vision scorecard impact
| Domain | Before | After (Tier 0+1) |
|---|---|---|
| GitHub Org Sync | 🔴 30% | 🟢 90% |
| Multi-Tenant Governance | 🟠 50% | 🟢 85% |
| Code Search | 🔴 15% | 🟡 60% |
| Deployment (Hybrid) | 🔴 35% | 🟡 65% |
| OpenCode Integration | 🔴 30% | 🟡 60% |
