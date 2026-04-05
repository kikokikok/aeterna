## Why

The current repository still has release-stopping authorization and tenant-boundary weaknesses: some tenant-scoped APIs trust caller-supplied headers when plugin auth is disabled, core memory paths use allow-all authorization, the propagated tenant context is weaker than the governance specs require, and role/model drift makes consistent enforcement impossible. These issues weaken tenant isolation and make the effective authorization boundary different from what the specs and operators expect.

## What Changes

- Remove header-only tenant identity trust from tenant-scoped HTTP APIs and require validated tenant/user context on privileged routes.
- Replace allow-all authorization wiring in memory and related control-plane paths with the configured real authorization backend or explicit fail-closed behavior.
- Reconcile the canonical role catalog and `TenantContext` model with the governance specs, including propagated roles and hierarchy path.
- Implement real role lookup, assignment, and revocation behavior in the active authorization adapter rather than no-op or empty-return paths.
- Add missing governance safety checks such as self-approval prevention and consistent role-catalog validation across API, CLI, and policy layers.

## Capabilities

### New Capabilities
- `authorization-boundary-hardening`: hardening requirements for request identity trust boundaries, propagated tenant context, and runtime authorization consistency.

### Modified Capabilities
- `multi-tenant-governance`: reconcile `TenantContext`, role catalog, authorization semantics, and approval safety checks with the actual implementation.
- `user-auth`: require validated request identity on tenant-scoped APIs and remove unsafe plain-header fallback behavior.
- `governance`: strengthen approval and authorization semantics for governance mutations.

## Impact

- Affected code: `cli/src/server/{bootstrap,plugin_auth,memory_api,knowledge_api}.rs`, `knowledge/src/api.rs`, `mk_core/src/types.rs`, `adapters/src/auth/{cedar,permit}.rs`, authorization middleware, governance storage, and related tests.
- Affected APIs: tenant-scoped memory/knowledge/admin routes, auth middleware behavior, role mutation/query paths, and governance approval paths.
- Affected systems: tenant isolation, authorization policy enforcement, plugin-authenticated workflows, and governance approval integrity.
