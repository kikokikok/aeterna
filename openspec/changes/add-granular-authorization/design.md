## Context

Aeterna's authorization layer exists in code but is effectively disabled in the current deployment. The Helm chart has no knob to set `AETERNA_PLUGIN_AUTH_ENABLED`, `default_tenant_claim()` returns `"default"`, MCP routes are completely unprotected, and `POST /api/v1/admin/sync/github` has no auth guard. The existing Cedar policy set covers 32 actions across 6 roles but the route-to-action mapping is incomplete (90+ REST routes, 49 MCP tools, only ~20 handlers have guards). The `Role` Rust enum has 6 variants (`Developer`, `TechLead`, `Architect`, `Admin`, `Agent`, `PlatformAdmin`) but the `role_permission_matrix()` already includes a `viewer` key that doesn't correspond to any enum variant. The markdown RBAC matrix in `docs/security/rbac-matrix.md` is outdated (5 roles, generic actions, doesn't match Cedar).

**Stakeholders**: Platform operators (need auth enabled via Helm), tenant admins (need scoped role grants and tenant provisioning), developers (need granular access), AI agents (need delegated permissions).

**Constraints**:
- Auth uses GitHub device-code login — GitHub identities are the authentication source
- Cedar + OPAL is the authorization backend (already deployed, not to be replaced)
- Must not break existing disabled-auth dev flows (backwards compatible default)
- Zero leakage of internal info to public repo

## Goals / Non-Goals

**Goals:**
- Map every REST endpoint and MCP tool to a specific Cedar action — no unprotected routes when auth is enabled
- Expand Cedar actions from 32 to cover tenant management, session management, sync, MCP invocation, config management, and git provider connections
- Add `Viewer` and `TenantAdmin` to the Rust `Role` enum and update the full permission matrix
- Design resource-scoped role grants using Cedar's entity hierarchy (Instance → Tenant → Org → Team → Project → Session)
- Create a single-file YAML tenant provisioning manifest and API/CLI command
- Wire `AETERNA_PLUGIN_AUTH_ENABLED` through Helm chart
- Fix `default_tenant_claim()` to derive from authenticated GitHub identity
- Add auth to MCP routes and `admin/sync/github`
- Fix `governance_role_assign` tool name collision
- Regenerate `docs/security/rbac-matrix.md`

**Non-Goals:**
- Replace Cedar/OPAL with a different authorization backend
- Implement fine-grained field-level authorization (e.g., which memory fields a user can read)
- Build a UI for role/permission management (API/CLI only for now)
- Implement Okta as an auth source (GitHub device-code is the auth source; Okta references in specs are historical)
- Change the existing Cedar entity hierarchy structure (Company > Org > Team > Project)

## Decisions

### Decision 1: Expand Cedar Actions to ~65 Total

**What**: Add ~33 new Cedar actions to cover all gaps. Group them by domain:

| Domain | New Actions | Rationale |
|--------|-------------|-----------|
| Tenant Mgmt | `ListTenants`, `CreateTenant`, `ViewTenant`, `UpdateTenant`, `DeactivateTenant` | Tenant lifecycle currently has no Cedar actions |
| Tenant Config | `ViewTenantConfig`, `UpdateTenantConfig`, `ManageTenantSecrets` | Config/secrets admin surface |
| Repository Binding | `ViewRepositoryBinding`, `UpdateRepositoryBinding` | Git repo binding is a sensitive operation |
| Git Provider | `ManageGitProviderConnections`, `ViewGitProviderConnections` | Platform-owned shared connections |
| Session | `CreateSession`, `ViewSession`, `EndSession` | Session lifecycle |
| Sync | `TriggerSync`, `ViewSyncStatus`, `ResolveConflict` | Sync operations |
| MCP | `InvokeMcpTool` | Global MCP tool invocation gate (plus per-tool routing to existing domain actions) |
| Memory Extended | `OptimizeMemory`, `ReasonMemory`, `CloseMemory`, `FeedbackMemory`, `ListMemory`, `SearchMemory` | MCP tools that don't map to existing 5 memory actions |
| Knowledge Extended | `ListKnowledge`, `SearchKnowledge`, `BatchKnowledge` | MCP/API tools not covered |
| Graph | `QueryGraph`, `ModifyGraph` | DuckDB graph operations |
| CCA | `InvokeCCA` | Context Architect, Note-Taking, Hindsight, Meta-Agent |
| User Mgmt | `ViewUser`, `RegisterUser`, `UpdateUser`, `DeactivateUser` | User lifecycle |
| Admin | `AdminSyncGitHub` | Currently unprotected sync endpoint |

**Alternatives considered**:
- Single `InvokeMcpTool` action for all tools → Too coarse, no way to grant memory-only access
- One Cedar action per MCP tool (49 actions) → Too granular, unmanageable. Instead, MCP tools map to domain actions (e.g., `memory_add` → `CreateMemory`, `knowledge_query` → `SearchKnowledge`)

**Schema changes**: Add new action declarations to `aeterna.cedarschema`, add new entity type `Tenant` for tenant-scoped resources, add `Session` entity type.

### Decision 2: Add Viewer and TenantAdmin Roles

**What**: Expand the `Role` enum from 6 to 8 variants:

```rust
pub enum Role {
    Viewer,         // precedence 0 — read-only access
    Agent,          // precedence 1 — delegated from user
    Developer,      // precedence 2 — standard dev access
    TechLead,       // precedence 3 — team lead
    Architect,      // precedence 4 — design authority
    Admin,          // precedence 5 — full tenant admin (legacy compat)
    TenantAdmin,    // precedence 6 — explicit tenant admin
    PlatformAdmin,  // precedence 7 — cross-tenant super admin
}
```

`TenantAdmin` and `Admin` initially have identical permissions (backward compatibility). `Admin` is retained as an alias. `TenantAdmin` is the explicit name used in new code and docs.

**Viewer permissions**: Read-only — `ViewMemory`, `ViewKnowledge`, `ViewPolicy`, `ViewGovernanceRequest`, `ViewOrganization`, `ViewSession`, `ViewSyncStatus`, `SimulatePolicy`, `ViewTenantConfig` (own tenant only).

### Decision 3: Resource-Scoped Role Grants via Cedar Entity Hierarchy

**What**: Leverage Cedar's existing `in` hierarchy for resource-scoped grants. A role grant is modeled as a Cedar entity relationship:

```
User::"alice" in Role::"Admin@Tenant::acme"
User::"alice" in Role::"TechLead@Team::api-team"
```

The scope is encoded in the Role entity ID: `<role>@<resource_type>::<resource_id>`. Cedar policies use `principal in resource.members` patterns that naturally inherit down the hierarchy.

**API surface**:
```
POST   /api/v1/admin/roles/grant    { user_id, role, resource_type, resource_id }
DELETE /api/v1/admin/roles/revoke   { user_id, role, resource_type, resource_id }
GET    /api/v1/admin/roles/grants?user_id=...&resource_type=...
```

This extends the existing `assign_unit_role` / `remove_unit_role` endpoints with explicit resource scoping.

**Hierarchy inheritance**: Admin on an Organization inherits to all Teams and Projects within it. This is native Cedar behavior via the `in` operator.

**Alternatives considered**:
- Separate role-grant table in Postgres + custom evaluation → Duplicates Cedar's built-in hierarchy features, adds consistency risk
- OpenFGA (as mentioned in some specs) → We've already committed to Cedar + OPAL; switching would be a major migration

### Decision 4: Global Auth Middleware as Axum Tower Layer

**What**: Replace per-handler auth checks with a global authentication tower layer on the router.

```rust
// router.rs — conceptual
let app = Router::new()
    .merge(health_routes())          // excluded from auth
    .merge(auth_routes())            // excluded from auth (bootstrap)
    .nest("/api/v1", protected_routes()
        .layer(AuthenticationLayer::new(auth_config)))
    .nest("/mcp", mcp_routes()
        .layer(AuthenticationLayer::new(auth_config)));
```

The layer:
1. Checks `AppState.plugin_auth_state.config.enabled` — if false, passes through (backward compat)
2. If enabled, validates `Authorization: Bearer <token>` → extracts `TenantContext` from JWT claims
3. Injects `TenantContext` as a request extension
4. Routes without valid token get 401

Individual handlers still check Cedar authorization (action-level), but authentication is guaranteed by the layer.

**Routes excluded from auth**: `/health`, `/live`, `/ready`, `/api/v1/auth/plugin/bootstrap`, `/api/v1/auth/plugin/refresh`, `/api/v1/auth/plugin/logout`.

**Alternatives considered**:
- Keep per-handler auth → Already proven unreliable (MCP, admin_sync missed). Global layer is standard practice.

### Decision 5: MCP Tool Permission Mapping

**What**: The MCP tool dispatcher (`tools/src/server.rs`) currently has a `check_permission("call_tool", &name)` call. Expand this to map each tool to its domain Cedar action:

```rust
fn tool_to_cedar_action(tool_name: &str) -> &str {
    match tool_name {
        "memory_add" => "CreateMemory",
        "memory_search" => "SearchMemory",
        "memory_delete" => "DeleteMemory",
        "knowledge_query" => "SearchKnowledge",
        "knowledge_get" => "ViewKnowledge",
        "aeterna_knowledge_propose" => "ProposeKnowledge",
        "governance_role_assign" => "AssignRoles",
        "context_assemble" | "note_capture" | "hindsight_query" | "meta_loop_status" => "InvokeCCA",
        "graph_query" | "graph_neighbors" | "graph_path" | "graph_traverse" | "graph_find_path" => "QueryGraph",
        "graph_link" | "graph_unlink" => "ModifyGraph",
        "sync_now" => "TriggerSync",
        // ... complete mapping for all 49 tools
        _ => "InvokeMcpTool", // fallback: generic permission
    }
}
```

Also fix the `governance_role_assign` name collision — rename one to `governance_role_grant` or similar.

### Decision 6: Single-File Tenant Provisioning Manifest

**What**: A YAML file that declaratively describes a complete tenant, processed by a single API call or CLI command.

```yaml
# tenant-manifest.yaml
apiVersion: aeterna.io/v1
kind: TenantManifest

tenant:
  slug: acme-corp
  name: "Acme Corporation"

config:
  fields:
    governance_mode: "standard"
    approval_required: "true"
    embedding_model: "text-embedding-3-small"

secrets:
  - name: openai-api-key
    value: "sk-..." # or valueFrom: { envVar: "OPENAI_API_KEY" }

repository:
  kind: github
  remote_url: "https://github.com/acme/knowledge-repo"
  branch: main
  git_provider_connection_id: "shared-github-app-01"

hierarchy:
  organizations:
    - slug: platform-eng
      name: "Platform Engineering"
      teams:
        - slug: api-team
          name: "API Team"
          projects:
            - slug: payments-service
              name: "Payments Service"
              git_remote: "https://github.com/acme/payments"

roles:
  - user_id: "alice"
    role: TenantAdmin
    scope: tenant  # applies to the whole tenant
  - user_id: "bob"
    role: TechLead
    scope: "team/platform-eng/api-team"
  - user_id: "carol"
    role: Developer
    scope: "project/platform-eng/api-team/payments-service"
```

**Processing order**:
1. Create tenant: `POST /api/v1/admin/tenants`
2. Apply config: `PUT /api/v1/admin/tenants/{t}/config`
3. Apply secrets: `PUT /api/v1/admin/tenants/{t}/secrets/{name}` for each
4. Set repository binding: `PUT /api/v1/admin/tenants/{t}/repository-binding`
5. Create hierarchy: Create orgs → teams → projects
6. Assign roles: Grant each role on the specified scope

**API**: `POST /api/v1/admin/tenants/provision` (body: the manifest as JSON, or multipart with YAML file)
**CLI**: `aeterna admin tenant provision --file manifest.yaml`

**Alternatives considered**:
- Helm values overlay → Already exists for basic ConfigMap seeding but can't handle hierarchy, roles, or secrets
- Multiple CLI commands → Error-prone, no atomicity. Single manifest is the user's explicit request.

### Decision 7: Helm Chart Auth Values Block

**What**: Add to `values.yaml`:

```yaml
pluginAuth:
  enabled: false  # backwards compatible default
  jwtSecret: ""   # or reference a k8s secret
  github:
    clientId: ""
    clientSecret: ""
    # ... other GitHub OAuth App fields
```

Wire into `configmap.yaml` as `AETERNA_PLUGIN_AUTH_ENABLED`, `AETERNA_PLUGIN_AUTH_JWT_SECRET`, etc. Wire into `deployment.yaml` env section.

### Decision 8: Fix default_tenant_claim()

**What**: In `plugin_auth.rs:559`, replace the hardcoded `"default"` with tenant resolution from the GitHub user's identity. The flow:
1. GitHub device-code login → get GitHub user info (login, id, email, orgs)
2. Look up the user in Aeterna's user store → find their tenant mapping
3. If no mapping found → fail closed (no default tenant)

This aligns with the `fix-multi-tenant-fail-closed` spec requirement.

## Risks / Trade-offs

- **Breaking MCP clients**: MCP routes getting auth when enabled will break existing unauthenticated MCP clients → Mitigation: auth is off by default, clients must opt in. Document migration path.
- **Cedar action explosion**: Going from 32 to ~65 actions increases policy surface → Mitigation: role templates group them; operators rarely write custom Cedar. The matrix is auto-generated and tested.
- **Single-file provisioning atomicity**: If step 4 of 6 fails, tenant is partially provisioned → Mitigation: implement idempotent operations and a `status` field on the manifest result showing which steps succeeded. Cleanup on full failure.
- **Role enum expansion**: Adding `Viewer` and `TenantAdmin` requires migration of persisted role strings → Mitigation: `Admin` stays as-is. `TenantAdmin` is a new value. `Viewer` is a new value. No existing data changes meaning.
- **Global middleware perf**: Extra layer on every request → Mitigation: when disabled, it's a single bool check (negligible). When enabled, JWT validation is <1ms.

## Migration Plan

1. **Phase A (Helm + Config)**: Add `pluginAuth` values block, wire env vars. Deploy with `pluginAuth.enabled: false` — no behavior change.
2. **Phase B (Code)**: Add global auth middleware, expand Cedar schema/policies, fix MCP auth, fix admin_sync auth, fix default_tenant_claim. All behind the `enabled` flag.
3. **Phase C (Roles + Permissions)**: Add `Viewer`/`TenantAdmin` to enum, expand `ALL_ACTIONS`, update `role_permission_matrix()`, regenerate RBAC matrix doc, update Cedar .cedar files.
4. **Phase D (Resource-Scoped Grants)**: Add role grant/revoke/list APIs, Cedar entity patterns for scoped roles.
5. **Phase E (Tenant Provisioning)**: Add manifest schema, provisioning API endpoint, CLI command.
6. **Phase F (Enable)**: Deploy with `pluginAuth.enabled: true`, run e2e suite with auth tokens.

**Rollback**: Set `pluginAuth.enabled: false` in Helm values — reverts to no-auth behavior instantly.

## Open Questions

1. **Agent role in scoped grants**: Can an Agent have a scoped role, or is Agent always derived from its delegating user's scope? Current design: Agent inherits from delegator, no independent scoped grants.
2. **TenantAdmin vs Admin cleanup**: Should we deprecate `Admin` in favor of `TenantAdmin` in a future change, or keep both permanently as aliases?
3. **MCP auth token source**: MCP SSE clients currently send no auth. Should they use the same Bearer token as REST, or a separate MCP-specific auth mechanism? Current design: same Bearer token.
4. **Tenant manifest secrets**: Should the provisioning API accept raw secret values in the manifest body, or only references to pre-existing Kubernetes secrets? Current design: accept raw values (encrypted in transit via TLS).
