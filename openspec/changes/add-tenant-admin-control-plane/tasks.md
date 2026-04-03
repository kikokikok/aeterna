## 1. Authority model and tenant records

- [x] 1.1 Add a canonical tenant record model and real tenant lifecycle API/storage flows for create, list, show, update, deactivate, and context selection.
- [x] 1.2 Add explicit `PlatformAdmin` authority and reconcile the canonical role catalog between Rust types, Cedar policies, and CLI validation.
- [x] 1.3 Implement verified tenant-resolution rules with admin-managed domain mappings and fail-closed ambiguity handling.
- [x] 1.4 Reconcile runtime tenant-context extraction and propagation with the full roles and hierarchy-path model already required by the governance specs.
- [x] 1.5 Add explicit platform-admin tenant-target selection and audit semantics for tenant-scoped operations.

## 2. Server-backed admin APIs

- [x] 2.1 Add server endpoints for tenant lifecycle operations and tenant-scoped hierarchy administration.
- [x] 2.2 Add server endpoints for org/team/project membership and role administration across company, org, team, and project scopes.
- [x] 2.3 Add server endpoints for permission inspection, including role-to-permission matrix queries and effective-permission evaluation for a principal at a scope.
- [x] 2.4 Emit durable audit events for tenant, repository-binding, and role-admin mutations.
- [x] 2.5 Add source-ownership metadata for sync-managed versus admin-managed tenant and hierarchy records.

## 3. Tenant knowledge repository bindings

- [x] 3.1 Add a canonical tenant repository binding model covering repository kind, local path or remote URL, branch policy, and credential references.
- [x] 3.2 Route tenant knowledge reads and writes through the configured tenant repository binding and fail closed when a required binding is missing or invalid.
- [x] 3.3 Add API operations to show, set, and validate tenant repository bindings.
- [x] 3.4 Integrate tenant bootstrap and IdP/admin sync flows so manual admin setup and sync-managed tenants can coexist without leaking bindings across tenants.
- [x] 3.5 Ensure tenant repository bindings store secret references rather than raw secret material and validate them through supported secret sources.

## 4. CLI control-plane completion

- [x] 4.1 Add supported `aeterna tenant ...` commands for tenant lifecycle management, context selection, and tenant repository administration.
- [x] 4.2 Replace stubbed `org`, `team`, `user roles`, and `govern roles` command paths with real backend-backed execution.
- [x] 4.3 Add CLI commands for permission inspection and role-to-permission matrix viewing.
- [x] 4.4 Ensure admin CLI outputs return persisted results, authorization failures, or validation failures honestly instead of previews on live paths.
- [x] 4.5 Add CLI UX for explicit platform-admin tenant targeting so cross-tenant operators do not rely on hidden ambient context.

## 5. Policy alignment, docs, and verification

- [x] 5.1 Update Cedar policy bundles, adapters, and fixtures for the reconciled role catalog and platform-admin boundary rules.
- [x] 5.2 Document platform-admin, tenant-admin, tenant bootstrap, tenant repository configuration, and scoped role-management workflows end to end.
- [x] 5.3 Add integration and CLI end-to-end tests for tenant lifecycle, repository binding management, scoped role mutations, and permission inspection.
- [x] 5.4 Add regression tests for cross-tenant denial, ambiguous domain mapping, missing tenant bindings, and policy/role-catalog drift.
- [x] 5.5 Add coexistence tests ensuring GitHub/IdP sync does not overwrite admin-managed repository bindings or verified tenant mappings.
- [x] 5.6 Add Newman/Postman scenarios for every supported end-to-end tenant-admin and platform-admin API workflow covered by this change.
