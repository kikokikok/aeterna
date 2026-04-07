## Why

A fresh Aeterna Helm deployment is currently broken out of the box: the plugin auth bootstrap endpoint returns 500 because no tenant is configured (`AETERNA_PLUGIN_AUTH_TENANT` is unset) and no PlatformAdmin user exists in PostgreSQL. The operator must manually exec into the database to create the initial admin — a chicken-and-egg problem since the API itself requires authentication. We need a declarative, Helm-driven way to seed the initial PlatformAdmin and plugin auth tenant so that deployments are operational from the first boot.

## What Changes

- Add Helm values for declaring the initial PlatformAdmin email, auth provider, and default plugin auth tenant
- Add a Rust startup bootstrap routine that idempotently seeds the PlatformAdmin user and instance-scoped role grant into PostgreSQL on first boot (before the HTTP server starts accepting traffic)
- Wire `AETERNA_PLUGIN_AUTH_TENANT` into the Helm deployment template so tenant resolution works for the plugin bootstrap flow
- The seeded PlatformAdmin can then create tenants and assign TenantAdmins through the API (CLI, CRD, or web interface later)

## Capabilities

### New Capabilities
- `admin-bootstrap`: Declarative first-boot seeding of PlatformAdmin identity and plugin auth tenant from Helm values, ensuring a fresh deployment is operational without manual database intervention

### Modified Capabilities
- `deployment`: Add Helm values schema for `adminBootstrap` (email, provider, providerSubject, defaultTenantId) and wire `AETERNA_PLUGIN_AUTH_TENANT` env var into the deployment template
- `opencode-plugin-auth`: Document that `AETERNA_PLUGIN_AUTH_TENANT` (or `pluginAuth.defaultTenantId` in Helm) is required for the bootstrap endpoint to resolve a tenant

## Impact

- **Helm chart**: New `adminBootstrap` values section, new env vars in deployment template
- **Rust server**: New bootstrap seeding logic in `cli/src/server/bootstrap.rs` (runs at startup, idempotent)
- **PostgreSQL schema**: Uses existing `users` + `user_roles` tables — no new migration
- **Config crate**: New env var `AETERNA_ADMIN_BOOTSTRAP_EMAIL` (+ provider fields) read at startup
- **Deployment repo**: [REDACTED_TENANT] values updated with admin bootstrap config
