## Why

The current runtime still contains production code paths that assign the `default` tenant or `system` user when authenticated tenant context is missing, malformed, or disabled. This violates the existing multi-tenant fail-closed requirements and creates a real tenant-isolation risk across plugin auth, HTTP APIs, MCP transport, webhooks, and storage enforcement.

## What Changes

- Remove implicit default tenant/user fallback behavior from request-handling boundaries and require explicit tenant resolution from authenticated or trusted identity sources.
- Bind MCP `tenantContext` usage to authenticated caller identity so callers cannot self-assert arbitrary tenant scope in JSON-RPC payloads.
- Tighten runtime auth defaults so production-capable deployments do not silently rely on allow-all behavior or spoofable context boundaries.
- Reconcile and harden storage-level tenant protections, including RLS coverage and activation in hot paths.
- Add negative-path and boundary tests for missing tenant, forged tenant, auth-disabled, and spoofed header scenarios.

## Capabilities

### New Capabilities

### Modified Capabilities
- `multi-tenant-governance`: Enforce fail-closed tenant propagation, remove default fallbacks, and strengthen tenant-context handling across all tenant-aware surfaces.
- `user-auth`: Require authenticated interactive flows to resolve tenant context from trusted identity or authenticated claims instead of assigning a default tenant.
- `server-runtime`: Require mounted APIs, transports, and webhook handlers to reject missing or unauthenticated tenant context in production-capable modes.
- `governance`: Tighten auth defaults and tenant-aware request validation for governance and agent-facing runtime surfaces.
- `storage`: Require effective database-level tenant isolation controls, including correct RLS coverage and activation for tenant-scoped tables.

## Impact

- Affected code: `cli/src/server/plugin_auth.rs`, `knowledge_api.rs`, `sync.rs`, `webhooks.rs`, `mcp_transport.rs`, `bootstrap.rs`, `knowledge/src/api.rs`, `tools/src/server.rs`, `storage/src/*`, context resolution paths, and related tests.
- Affected APIs: plugin bootstrap/refresh, knowledge/sync endpoints, webhooks, MCP transport, and any route deriving `TenantContext`.
- Affected systems: tenant isolation guarantees, deployment safety, authorization behavior, auditability, and compliance posture.
