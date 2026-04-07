## Why

Aeterna has 90+ REST endpoints and 49 MCP tools, but authorization is effectively disabled in production: the Helm chart has no knob to enable `AETERNA_PLUGIN_AUTH_ENABLED`, the tenant claim is hardcoded to `"default"`, MCP routes carry zero auth, and `POST /api/v1/admin/sync/github` is completely unprotected. The existing Cedar policy set covers 32 actions across 6 roles but does not map to actual route enforcement — there is no global auth middleware, and each handler implements (or omits) its own guard. We need a complete, enforceable authorization layer that maps every functionality to a specific permission, groups permissions into assignable roles, supports resource-scoped grants across the full hierarchy, and enables single-file tenant provisioning.

## What Changes

- Expand the Cedar action set from 32 to ~60+ actions covering every REST endpoint and MCP tool category
- Add a `Viewer` role variant to the Rust `Role` enum and align all roles (7 total: Agent, Developer, TechLead, Architect, Admin, TenantAdmin, PlatformAdmin) with a comprehensive permission matrix
- Introduce resource-scoped role grants: assign a role to a user on a specific resource (instance-wide, tenant, organization, team, project, session)
- Design and implement a single-file YAML tenant provisioning manifest that orchestrates tenant creation, config, secrets, hierarchy bootstrap, repository binding, and initial role assignments
- Add global auth middleware as an Axum tower layer — replace per-handler auth checks with a unified request authentication pipeline
- **BREAKING**: MCP routes (`/mcp/sse`, `/mcp/message`) will require authentication when auth is enabled
- **BREAKING**: `POST /api/v1/admin/sync/github` will require PlatformAdmin authentication
- Wire `AETERNA_PLUGIN_AUTH_ENABLED` through the Helm chart `values.yaml` and deployment template
- Fix `default_tenant_claim()` to resolve tenant from GitHub identity rather than hardcoding `"default"`
- Fix `governance_role_assign` tool name collision (two tools register the same name)
- Regenerate `docs/security/rbac-matrix.md` to match the actual Cedar policies and code

## Capabilities

### New Capabilities
- `granular-authorization`: Complete permission-to-route/tool mapping, expanded Cedar actions, role permission matrix, and global auth middleware enforcement
- `resource-scoped-roles`: Resource-scoped role grant model — assign roles on specific hierarchy nodes (instance, tenant, org, team, project, session) with Cedar-backed evaluation and inheritance
- `tenant-provisioning`: Single-file YAML manifest schema and API/CLI command for bootstrapping a complete tenant (identity, config, secrets, hierarchy, repository binding, role assignments) in one operation

### Modified Capabilities
- `multi-tenant-governance`: Extend RBAC with Viewer role, TenantAdmin distinction, expanded action set, and resource-scoped role assignment APIs
- `server-runtime`: Add global authentication tower layer, fix MCP route auth gap, fix admin sync auth gap
- `opencode-plugin-auth`: Fix `default_tenant_claim()` to derive tenant from authenticated GitHub identity
- `governance`: Add Cedar policies for new actions, update policy conflict detection to cover expanded action set
- `user-auth`: Extend authentication contract to support GitHub device-code as primary interactive identity source (replacing Okta references)
- `deployment`: Add `pluginAuth` values block to Helm chart for enabling/configuring auth in Kubernetes deployments
- `tool-interface`: Add per-tool permission requirements to MCP tool dispatch, fix `governance_role_assign` name collision

## Impact

- Affected specs: `multi-tenant-governance`, `server-runtime`, `opencode-plugin-auth`, `governance`, `user-auth`, `deployment`, `tool-interface` (7 modified), plus 3 new capabilities
- Affected Cedar files: `rbac.cedar`, `forbid.cedar`, `agent-delegation.cedar`, `aeterna.cedarschema` — all need expansion
- Affected Rust code: `mk_core/src/types.rs` (Role enum), `cli/src/server/tenant_api.rs` (ALL_ACTIONS, role_permission_matrix), `cli/src/server/router.rs` (global middleware), `cli/src/server/plugin_auth.rs` (default_tenant_claim), `cli/src/server/mcp_transport.rs` (add auth), `cli/src/server/admin_sync.rs` (add auth), `tools/src/server.rs` (per-tool permissions, fix collision), `adapters/src/auth/cedar.rs` (scoped authorization), `adapters/tests/rbac_matrix_test.rs` (expanded matrix tests)
- Affected Helm: `charts/aeterna/values.yaml`, `values.schema.json`, `templates/aeterna/configmap.yaml`, `templates/aeterna/deployment.yaml`
- Affected docs: `docs/security/rbac-matrix.md` — full regeneration
- New API endpoints: `POST /api/v1/admin/tenants/provision` (single-file provisioning), scoped role grant/revoke endpoints
- New CLI command: `aeterna admin tenant provision --file manifest.yaml`
