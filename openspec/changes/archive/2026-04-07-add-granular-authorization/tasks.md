## 1. Helm Chart Auth Configuration (Phase A)

- [x] 1.1 Add `pluginAuth` values block to `charts/aeterna/values.yaml` (enabled: false default, jwtSecret, github.clientId, github.clientSecret)
- [x] 1.2 Add `pluginAuth` JSON schema to `charts/aeterna/values.schema.json` with required-field validation when enabled
- [x] 1.3 Wire `AETERNA_PLUGIN_AUTH_ENABLED` and all `AETERNA_PLUGIN_AUTH_*` env vars into `charts/aeterna/templates/aeterna/configmap.yaml`
- [x] 1.4 Inject auth env vars from configmap into `charts/aeterna/templates/aeterna/deployment.yaml` container spec
- [x] 1.5 Add `helm template` test verifying: auth disabled by default, auth enabled injects all env vars, missing credentials fail validation

## 2. Role Enum Expansion (Phase C — prerequisite for later phases)

- [x] 2.1 Add `Viewer` (precedence 0) and `TenantAdmin` (precedence 6) variants to `Role` enum in `mk_core/src/types.rs`
- [x] 2.2 Update all `Role` serialization/deserialization, Display, FromStr, and precedence logic to include the two new variants
- [x] 2.3 Ensure `TenantAdmin` and `Admin` have identical permissions initially (backward compatibility)
- [x] 2.4 Fix any compile errors across the codebase caused by the new enum variants (exhaustive match arms)

## 3. Cedar Schema Expansion (Phase C)

- [x] 3.1 Add new action declarations to `policies/cedar/aeterna.cedarschema`: tenant management (`ListTenants`, `CreateTenant`, `ViewTenant`, `UpdateTenant`, `DeactivateTenant`), tenant config (`ViewTenantConfig`, `UpdateTenantConfig`, `ManageTenantSecrets`), repository binding (`ViewRepositoryBinding`, `UpdateRepositoryBinding`), git provider (`ManageGitProviderConnections`, `ViewGitProviderConnections`)
- [x] 3.2 Add session actions to Cedar schema: `CreateSession`, `ViewSession`, `EndSession`
- [x] 3.3 Add sync actions to Cedar schema: `TriggerSync`, `ViewSyncStatus`, `ResolveConflict`
- [x] 3.4 Add extended memory actions to Cedar schema: `SearchMemory`, `ListMemory`, `OptimizeMemory`, `ReasonMemory`, `CloseMemory`, `FeedbackMemory`
- [x] 3.5 Add extended knowledge actions to Cedar schema: `ListKnowledge`, `SearchKnowledge`, `BatchKnowledge`
- [x] 3.6 Add graph and CCA actions to Cedar schema: `QueryGraph`, `ModifyGraph`, `InvokeCCA`
- [x] 3.7 Add MCP invocation action to Cedar schema: `InvokeMcpTool`
- [x] 3.8 Add user management actions to Cedar schema: `ViewUser`, `RegisterUser`, `UpdateUser`, `DeactivateUser`
- [x] 3.9 Add admin actions to Cedar schema: `AdminSyncGitHub`
- [x] 3.10 Add `Tenant` and `Session` entity types to Cedar schema with appropriate attribute definitions

## 4. Cedar Policy Files (Phase C)

- [x] 4.1 Add permit rules in `policies/cedar/rbac.cedar` for Viewer role (read-only: View*, SimulatePolicy, ViewSyncStatus)
- [x] 4.2 Add permit rules in `policies/cedar/rbac.cedar` for TenantAdmin role (all tenant-scoped actions, excluding cross-tenant)
- [x] 4.3 Add permit rules in `policies/cedar/rbac.cedar` for all 8 roles covering the ~33 new actions (tenant mgmt, session, sync, graph, CCA, user mgmt, admin)
- [x] 4.4 Add forbid rules in `policies/cedar/forbid.cedar` for cross-tenant access and privilege escalation on new action domains
- [x] 4.5 Validate Cedar policy files parse correctly: `cedar validate` or equivalent tooling

## 5. Rust Permission Matrix Expansion (Phase C)

- [x] 5.1 Expand `ALL_ACTIONS` constant in `cli/src/server/tenant_api.rs` to include all ~65 Cedar actions
- [x] 5.2 Update `role_permission_matrix()` in `cli/src/server/tenant_api.rs` to include Viewer and TenantAdmin role entries with correct action sets
- [x] 5.3 Update all existing role entries (Developer, TechLead, Architect, Admin, Agent, PlatformAdmin) with new action grants
- [x] 5.4 Update RBAC matrix tests in `adapters/tests/rbac_matrix_test.rs` to verify all 8 roles against the expanded action set (positive and negative cases)
- [x] 5.5 Ensure `role_permission_matrix()` output matches `policies/cedar/rbac.cedar` permit rules — test fails on mismatch

## 6. Global Authentication Middleware (Phase B)

- [x] 6.1 Create an Axum tower `AuthenticationLayer` that validates `Authorization: Bearer <token>` and injects `TenantContext` as request extension
- [x] 6.2 Apply the layer to `/api/v1/*` and `/mcp/*` route groups in `cli/src/server/router.rs`
- [x] 6.3 Exclude health routes (`/health`, `/live`, `/ready`) and auth bootstrap routes (`/api/v1/auth/plugin/*`) from the layer
- [x] 6.4 Implement pass-through behavior when `pluginAuth.enabled: false` (backward compat — legacy header-based identity)
- [x] 6.5 Return 401 Unauthorized for missing/invalid/expired tokens on protected routes (without revealing route existence)
- [x] 6.6 Remove redundant per-handler auth checks that are now covered by the global layer (preserve Cedar action-level checks)

## 7. Security Gap Fixes (Phase B)

- [x] 7.1 Add PlatformAdmin auth guard to `POST /api/v1/admin/sync/github` in `cli/src/server/admin_sync.rs`
- [x] 7.2 Add auth enforcement to MCP routes (`/mcp/sse`, `/mcp/message`) in `cli/src/server/mcp_transport.rs` — validate bearer token and derive tenant context
- [x] 7.3 Fix `default_tenant_claim()` in `cli/src/server/plugin_auth.rs:559` — derive tenant from GitHub identity mapping instead of returning `"default"`
- [x] 7.4 Implement fail-closed behavior: reject authentication if no tenant mapping exists for the GitHub user (no fallback to default tenant)
- [x] 7.5 Validate MCP `tenantContext` in JSON payload against authenticated identity in `tools/src/server.rs` — reject mismatches

## 8. MCP Tool Authorization (Phase C)

- [x] 8.1 Create `tool_to_cedar_action()` mapping function in `tools/src/server.rs` covering all 49 MCP tools to their domain Cedar actions
- [x] 8.2 Replace the generic `check_permission("call_tool", &name)` in tool dispatcher with `check_permission(tool_to_cedar_action(&name), &name)`
- [x] 8.3 Add `InvokeMcpTool` fallback for unmapped tools with warning log
- [x] 8.4 Fix `governance_role_assign` name collision — rename one tool to `governance_role_grant` in `tools/src/governance.rs`
- [x] 8.5 Add duplicate tool name detection at MCP server registration in `tools/src/server.rs` — fail startup on collision
- [x] 8.6 Add CCA tenant context propagation to `tools/src/cca.rs` (currently missing)

## 9. Resource-Scoped Role Grants (Phase D)

- [x] 9.1 Design and implement Cedar entity relationship patterns for scoped roles: `User::"alice" in Role::"Admin@Tenant::acme"` encoding
- [x] 9.2 Add `POST /api/v1/admin/roles/grant` endpoint accepting `{ user_id, role, resource_type, resource_id }` — creates Cedar entity relationship
- [x] 9.3 Add `DELETE /api/v1/admin/roles/revoke` endpoint — removes Cedar entity relationship
- [x] 9.4 Add `GET /api/v1/admin/roles/grants` endpoint with query filters (`user_id`, `resource_type`, `resource_id`) — lists active grants
- [x] 9.5 Add scope validation: PlatformAdmin only at instance scope, TenantAdmin only at tenant+, Developer/TechLead/etc. at any scope
- [x] 9.6 Add `AssignRoles` Cedar authorization check on all grant/revoke endpoints
- [x] 9.7 Add integration tests for hierarchy inheritance (org grant inherits to teams/projects, team grant does not propagate upward)

## 10. Tenant Provisioning Manifest (Phase E)

- [x] 10.1 Define `TenantManifest` Rust struct with serde for the YAML schema (`apiVersion: aeterna.io/v1`, `kind: TenantManifest`, tenant, config, secrets, repository, hierarchy, roles sections)
- [x] 10.2 Implement manifest validation: required fields, kebab-case slug, valid role names, valid scope paths, git_provider_connection_id existence check
- [x] 10.3 Add `POST /api/v1/admin/tenants/provision` endpoint — PlatformAdmin only, processes manifest in order (tenant → config → secrets → repo binding → hierarchy → roles)
- [x] 10.4 Implement idempotent behavior: re-submitting manifest for existing tenant slug updates rather than duplicates
- [x] 10.5 Return step-by-step provisioning status in response (which steps succeeded, which failed)
- [x] 10.6 Add `aeterna admin tenant provision --file manifest.yaml` CLI command
- [x] 10.7 Add integration tests for full manifest provisioning and idempotent re-application

## 11. RBAC Matrix Documentation (Phase C)

- [x] 11.1 Create a script or Rust binary that generates `docs/security/rbac-matrix.md` from `role_permission_matrix()` output
- [x] 11.2 Include all 8 roles, all ~65 actions grouped by domain, and MCP tool-to-action mapping table
- [x] 11.3 Run the generator and commit the updated `docs/security/rbac-matrix.md`
- [x] 11.4 Add a CI check or test that the committed doc matches the current matrix (detects drift)

## 12. Deployment Validation (Phase F)

- [ ] 12.1 Deploy with `pluginAuth.enabled: false` — verify no behavior change from current deployment
- [ ] 12.2 Deploy with `pluginAuth.enabled: true` and valid GitHub OAuth credentials
- [ ] 12.3 Run e2e suite with auth tokens — verify protected endpoints return 200 with valid token and 401/403 without
- [ ] 12.4 Verify MCP routes require auth when enabled (SSE connection + tool invocation)
- [ ] 12.5 Verify tenant provisioning API works end-to-end with a test manifest
- [ ] 12.6 Verify scoped role grants work: grant TechLead on a team, verify access to team resources and denial for sibling teams
