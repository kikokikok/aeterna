## Why

The current Aeterna control plane can authenticate and reach the server, but tenant administration, tenant-scoped knowledge repository configuration, and role/permission administration remain incomplete. The CLI still exposes mostly stubbed tenant/org/team/user/governance admin flows, there is no first-class tenant lifecycle surface, and operators cannot inspect or manage the effective role-to-permission model without reading Cedar policies directly.

## What Changes

- Add a supported tenant administration control plane with real API- and CLI-backed tenant lifecycle operations, tenant context selection, and scoped hierarchy administration.
- Add explicit administrative authority boundaries between cross-tenant platform administration and tenant-scoped administration, including verified tenant onboarding rules and fail-closed tenant resolution.
- Add supported tenant knowledge repository binding management so each tenant can configure, validate, and inspect its canonical knowledge repository from the control plane.
- Replace stubbed role-management CLI flows with real backend-backed role assignment, revocation, listing, and effective-permission inspection across company, org, team, and project scopes.
- Add a queryable role-to-permission matrix derived from the active authorization policy bundle so operators can inspect what each role can do without reverse-engineering policy files.
- Add explicit platform-admin tenant-context switching rules so cross-tenant operators can manage tenant lifecycle safely without implicit tenant-content access.
- Preserve coexistence between manual tenant administration and IdP-managed hierarchy sync, including source tracking and non-destructive handling of tenant repository bindings.
- Add Newman/Postman end-to-end coverage for all supported operator and tenant-admin API scenarios so the control plane is verifiable outside Rust-only test harnesses.
- Reconcile the role catalog and tenant context model with the implemented authorization stack so runtime context, policy evaluation, and admin UX describe the same authority model.

## Capabilities

### New Capabilities
- `tenant-admin-control-plane`: CLI and API workflows for tenant lifecycle management, scoped hierarchy administration, tenant repository administration, role assignment, and permission inspection.

### Modified Capabilities
- `multi-tenant-governance`: add explicit platform-admin versus tenant-admin boundaries, verified tenant resolution rules, and the expanded administrative role catalog.
- `knowledge-repository`: add canonical per-tenant repository bindings and fail-closed resolution for tenant knowledge operations.
- `github-org-sync`: preserve coexistence between IdP-managed hierarchy sync and manual tenant administration without overwriting tenant-admin-owned configuration.

## Impact

- Affected code: `cli/src/commands/{admin,org,team,user,govern,knowledge}.rs`, server admin routes, tenant/role persistence layers, authorization adapters, Cedar policy bundles, context extraction, and knowledge repository resolution.
- Affected APIs: tenant lifecycle endpoints, role administration endpoints, permission inspection endpoints, tenant repository binding endpoints, and existing hierarchy/member-management endpoints that are currently stubbed in the CLI.
- Affected systems: tenant onboarding, IdP/bootstrap integration, CLI operator workflows, policy inspection, audit trails, tenant knowledge storage configuration, and the Newman/Postman E2E regression suite under `e2e/`.
