## Context

Aeterna is deployed on a staging cluster (Rev 27, `sha-3311bed`) with the full OPAL stack running (cedar-agent, opal-fetcher, 2x opal-server). The GitHub Org Sync was implemented (40/40 tasks marked done, `idp-sync` crate compiles, 41 tests pass) but **fails at runtime** because:

1. The `tenants` table doesn't exist — `resolve_tenant_id()` crashes
2. `run_github_sync()` never calls `initialize_github_sync_schema()`
3. `admin_sync.rs` calls `resolve_tenant_id()` before schema initialization

Beyond these 3 blockers, the full Sync → OPAL → Cedar → Authorization pipeline has 7 additional gaps that prevent synced data from being visible to the authorization system.

The broader vision has 23 OpenSpec specs with ~270+ requirements. Code search, hybrid deployment, and OpenCode integration are scaffolded but non-functional.

**Stakeholders**: Platform Engineering, Security, DevEx teams
**Constraints**: Must not break existing 87/87 E2E tests. Must use existing Axum HTTP architecture (not gRPC). Must use testcontainers for integration tests.

## Goals / Non-Goals

### Goals
- Fix all 10 Sync → OPAL pipeline gaps so GitHub Org → Authorization works end-to-end
- Create PostgreSQL views that the OPAL fetcher already queries (`v_hierarchy`, `v_user_permissions`, `v_agent_permissions`)
- Bridge `idp-sync` output to `governance_roles`/`user_roles` tables
- Wire `CedarAuthorizer` to load entities from OPAL (not `Entities::empty()`)
- Document and plan all remaining vision gaps for structured implementation

### Non-Goals
- Rewriting the existing `idp-sync` crate architecture
- Migrating from PostgreSQL to another database
- Implementing OpenTofu cloud modules (Tier 2, tracked but not in this change)
- Full code search indexing pipeline (Tier 1, tracked but large scope)
- Changing the OPAL fetcher's entity model (it's already correct)

## Decisions

### Decision 1: Add `tenants` table to `initialize_github_sync_schema()`
**What**: Add `CREATE TABLE IF NOT EXISTS tenants (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), name TEXT NOT NULL UNIQUE, created_at TIMESTAMPTZ DEFAULT NOW())` to `initialize_github_sync_schema()`.
**Why**: The `admin_sync.rs` `resolve_tenant_id_from_pool()` queries `SELECT id FROM tenants WHERE name = $1` — this table must exist before the query runs.
**Alternatives considered**: Creating the table in a separate migration — rejected because all other sync-related tables use `CREATE TABLE IF NOT EXISTS` in the schema init function, staying consistent is better.

### Decision 2: Call schema init before any DB queries
**What**: In `admin_sync.rs::run_sync()`, call `initialize_github_sync_schema(pool)` as the very first line, before `build_github_config()` and `resolve_tenant_id()`. Also add it as the first line of `run_github_sync()` for standalone usage.
**Why**: Both paths need the schema to exist. The admin endpoint is the primary entry point. The standalone function is used by CronJob. Both must be safe to call independently.
**Alternatives considered**: Calling only in one place — rejected because the CronJob path bypasses `admin_sync.rs`.

### Decision 3: Add `slug` and fix UUID handling in `organizational_units`
**What**: Add `slug TEXT` column. The OPAL fetcher expects `company_slug`, `org_slug`, `team_slug`, `project_slug` from the `v_hierarchy` view. The current `organizational_units.id` is TEXT but `governance_roles.*_id` is UUID. The views will CAST where needed.
**Why**: The opal-fetcher entity model is already deployed and correct. We adapt the underlying data to fit its expectations.
**Alternatives considered**: Changing `organizational_units.id` to UUID — rejected because existing sync code writes TEXT IDs like `company-<org-name>`. Instead, views will generate deterministic UUIDs from TEXT IDs using `uuid_generate_v5()`.

### Decision 4: Create `agents` table for `v_agent_permissions` view
**What**: `CREATE TABLE IF NOT EXISTS agents (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), name TEXT NOT NULL, agent_type TEXT NOT NULL DEFAULT 'coding-assistant', delegated_by_user_id UUID REFERENCES users(id), delegated_by_agent_id UUID REFERENCES agents(id), delegation_depth INT NOT NULL DEFAULT 0, capabilities JSONB DEFAULT '[]', allowed_company_ids UUID[], allowed_org_ids UUID[], allowed_team_ids UUID[], allowed_project_ids UUID[], status TEXT NOT NULL DEFAULT 'active', created_at TIMESTAMPTZ DEFAULT NOW(), updated_at TIMESTAMPTZ DEFAULT NOW())`.
**Why**: The OPAL fetcher's `v_agent_permissions` view reads from an agents table. Without it, agent authorization data doesn't exist.

### Decision 5: Bridge sync → governance with post-sync hook
**What**: After `run_github_sync()` returns a `SyncReport`, iterate over synced users and their memberships to populate `governance_roles` and `user_roles`. Use `ON CONFLICT DO UPDATE` for idempotency.
**Why**: The `idp-sync` crate writes to `users` and `memberships` but the governance/authorization system reads from `governance_roles` and `user_roles`. Without this bridge, synced users have no permissions.

### Decision 6: PG NOTIFY triggers for real-time OPAL updates
**What**: Create triggers on `users`, `memberships`, `organizational_units`, `governance_roles`, and `agents` that emit `NOTIFY aeterna_entity_change` on INSERT/UPDATE/DELETE. The OPAL fetcher's `listener.rs` already has `LISTEN aeterna_entity_change`.
**Why**: Without triggers, OPAL only gets data on fetcher restart or manual refresh.

### Decision 7: CedarAuthorizer entity loading via HTTP to OPAL fetcher
**What**: Instead of embedding entity queries directly, `CedarAuthorizer` will fetch entities from the OPAL fetcher HTTP endpoint (`/v1/hierarchy`, `/v1/users`, `/v1/agents`) and cache them with a configurable TTL (default 30s).
**Why**: The OPAL fetcher already does all the heavy lifting of transforming DB rows into Cedar entities. The authorizer just needs to consume them. This maintains separation of concerns and avoids duplicating the entity transformation logic.
**Alternatives considered**: Direct DB queries in the authorizer — rejected because entity transformation is complex and already implemented in opal-fetcher.

## Risks / Trade-offs

- **Risk**: `uuid_generate_v5()` for deterministic UUID generation requires `pgcrypto` extension — **Mitigation**: Already creating `pgcrypto` in `initialize_github_sync_schema()`.
- **Risk**: View performance with JOINs across TEXT→UUID cast — **Mitigation**: Views are queried infrequently (on OPAL fetch cycle, ~30s TTL). Not a hot path.
- **Risk**: Post-sync governance bridge could fail independently of sync — **Mitigation**: Wrap in its own transaction, log errors but don't fail the sync report.
- **Trade-off**: Using HTTP for entity fetching adds latency vs. direct DB — **Accepted**: Simplicity wins. The fetcher is a sidecar, latency is <5ms.

## Migration Plan

1. Apply schema changes (all `IF NOT EXISTS` / `IF NOT EXISTS` — safe to rerun)
2. Create views (all `CREATE OR REPLACE VIEW` — safe to rerun)
3. Create triggers (all `CREATE OR REPLACE FUNCTION` + `DROP TRIGGER IF EXISTS` — safe to rerun)
4. Deploy new image with code fixes
5. Trigger sync, verify SyncReport
6. Verify OPAL fetcher serves entities via curl
7. Verify Cedar authorization evaluates correctly

**Rollback**: All changes are additive (`IF NOT EXISTS`, `OR REPLACE`). Rolling back to the previous image simply ignores the new tables/views. No data loss.

## Open Questions

- Should the governance bridge assign a default role to all synced users, or only to users who have explicit team membership? **Decision**: Only team members get roles — org-level members get a baseline `viewer` role.
- Should PG NOTIFY include the changed entity type in the payload? **Decision**: Yes, include `{"type": "user|membership|org_unit|role|agent", "id": "..."}` so the fetcher can do targeted refresh.

## Decision 8: Code Search = External Skill, Not Built-In Subsystem

**What**: Code search (symbol navigation, call graph, semantic search) is NOT an Aeterna core subsystem. It is a developer navigation tool that belongs in the IDE/editor layer. Aeterna integrates code intelligence via pluggable MCP backends as an external skill.

**Architecture**:
- **Remove**: Built-in tree-sitter parsing, embedding generation, and vector storage for code indexing from Aeterna core
- **Remove**: External `codesearch` sidecar binary spawning pattern (`tools/src/codesearch/client.rs`)
- **Keep**: `storage/src/repo_manager.rs` for repository approval/management workflow (separate concern)
- **Keep**: OPAL code search views as stubs (will populate when repo management is used)
- **Add**: `CodeIntelligenceBackend` trait with pluggable MCP backend support
- **Add**: Dynamic tool discovery — agent checks for available MCP code intelligence servers at startup
- **Primary backend**: JetBrains Code Intelligence MCP plugin (https://plugins.jetbrains.com/plugin/29509-code-intelligence-mcp)
- **Extensible to**: VS Code extensions, Neovim LSP bridges, or any MCP-compatible code intelligence server

**Why**:
- Code search is about navigating code in the developer's editor, not about Aeterna's knowledge/memory domain
- JetBrains' PSI engine provides best-in-class AST understanding — no need to reimplement with tree-sitter
- Avoids building and maintaining a complex indexing pipeline (tree-sitter + embeddings + vector store)
- Works with the IDE the developer is already using — no separate tooling to set up
- MCP protocol makes it pluggable — future IDE plugins just work

**Trade-offs**:
- Code intelligence only available when IDE is running (acceptable — code search is an IDE activity)
- Tied to MCP-compatible backends (acceptable — MCP is the standard Aeterna uses)
- No server-side cross-repo search (deferred — can be added later via shared index if needed)

**Embedding configuration** (for future semantic search layer if needed):
- Provider: Configurable, default Ollama (local `nomic-embed-text` or similar)
- No fallback — if embedding provider unavailable, semantic search is disabled
- Vector store: In-process Rust library (e.g., `usearch`) — no external dependency
