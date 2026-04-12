## Why

GitHub organization sync is currently hardcoded to a single global GitHub App configuration read from environment variables, resolving a single tenant via `AETERNA_TENANT_ID`. This makes it impossible for multiple tenants to each sync their own GitHub org (or different providers) independently. Each tenant must be able to configure and trigger their own IdP/GitHub sync using credentials stored in their tenant config.

## What Changes

- Each tenant stores GitHub App credentials (`github_app_id`, `github_installation_id`, `github_app_pem`, `github_org_name`, `github_team_filter`, `github_sync_repos_as_projects`) as typed tenant config keys
- `POST /admin/sync/github` now accepts an optional `tenant` path param; without it, syncs all tenants that have GitHub config present
- New per-tenant route: `POST /admin/tenants/{tenant}/sync/github` — triggers sync for that tenant only, resolving credentials from tenant config rather than env vars
- Global env-var fallback is preserved for backward compatibility when no per-tenant config is found
- The GitHub sync CronJob in Helm is updated to call the new per-tenant route (or the multi-tenant fan-out)
- `TenantConfigProvider` gains typed accessors for GitHub App config keys

## Capabilities

### New Capabilities
- `tenant-idp-sync`: Per-tenant IdP (GitHub) sync configuration and trigger API

### Modified Capabilities
- `github-org-sync`: The existing sync behavior changes: credentials now resolve from tenant config first, env vars as fallback; route supports per-tenant scoping

## Impact

- `cli/src/server/admin_sync.rs` — `build_github_config()` gains per-tenant resolution path
- `cli/src/server/tenant_api.rs` — new route `POST /admin/tenants/{tenant}/sync/github`
- `cli/src/server/router.rs` — wire new route
- `storage/src/tenant_config_provider.rs` — new typed config key constants for GitHub App fields
- `charts/aeterna/templates/aeterna/cronjob-github-sync.yaml` — updated to use per-tenant sync endpoint
- `openspec/specs/github-org-sync/spec.md` — new spec
- `openspec/specs/tenant-idp-sync/spec.md` — new spec
