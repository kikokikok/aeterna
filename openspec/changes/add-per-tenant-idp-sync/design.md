## Design: Per-Tenant IdP Sync

### Problem

`admin_sync.rs::build_github_config()` reads `AETERNA_GITHUB_ORG_NAME`, `AETERNA_GITHUB_APP_ID`,
`AETERNA_GITHUB_INSTALLATION_ID`, `AETERNA_GITHUB_APP_PEM` from the process environment and targets
a single tenant derived from `AETERNA_TENANT_ID`. A single Aeterna deployment serving multiple tenants
has no way to run independent GitHub org syncs per tenant.

### Solution

Move GitHub App config from env vars into the `TenantConfigDocument` / tenant secrets system that already
exists (`KubernetesTenantConfigProvider`). Env vars become a **fallback** for single-tenant deployments
or backward compatibility only.

### Config Key Namespace

Non-secret fields stored in `TenantConfigDocument.fields` (ownership: `Platform`):

| Key | Type | Description |
|---|---|---|
| `github.org_name` | string | GitHub organization name to sync |
| `github.app_id` | string | GitHub App ID (numeric, stored as string) |
| `github.installation_id` | string | GitHub App installation ID |
| `github.team_filter` | string (optional) | Regex filter for teams |
| `github.sync_repos_as_projects` | bool | Map repos as Aeterna projects |

Secret field stored via `TenantSecretEntry` (logical name):

| Logical Name | Description |
|---|---|
| `github.app_pem` | GitHub App private key PEM content |

### Resolution Order

```
1. Tenant config fields (github.org_name, github.app_id, github.installation_id)
   + Tenant secret (github.app_pem)
        ↓ if any field missing
2. Platform env vars (AETERNA_GITHUB_ORG_NAME, AETERNA_GITHUB_APP_ID, ...)
        ↓ if env vars also missing
3. Error: "No GitHub config for tenant {id}"
```

### API Design

#### Multi-tenant fan-out (PlatformAdmin only)
```
POST /admin/sync/github
```
- Iterates all tenants with `github.org_name` present in tenant config
- Falls back to env-var config for tenants without per-tenant config
- Returns `{ results: [ { tenant_id, report | error }, ... ] }`

#### Per-tenant sync (PlatformAdmin or TenantAdmin)
```
POST /admin/tenants/{tenant}/sync/github
```
- Resolves GitHub config from that tenant's config/secrets
- Falls back to env vars if no per-tenant config
- Returns the same `SyncReport` as the current handler

### Concurrency

The current `AtomicBool SYNC_IN_PROGRESS` guard applies per-process globally. With per-tenant sync,
we need per-tenant locking:

```rust
// DashMap<TenantId, ()> — entry present = sync in progress
static SYNC_LOCKS: LazyLock<DashMap<String, ()>> = LazyLock::new(DashMap::new);
```

The global lock is kept for the fan-out route to prevent overlapping fan-out runs.

### Helm CronJob

The CronJob calls `POST /admin/sync/github` via `curl` rather than embedding credentials as env vars.
The job uses a service account JWT (injected as `AETERNA_SERVICE_TOKEN` from a projected service account
token) to authenticate. This removes the need to mount GitHub App PEM into the CronJob pod — credentials
stay in tenant config Kubernetes Secrets.

```yaml
command: ["sh", "-c"]
args:
  - |
    curl -sf -X POST \
      -H "Authorization: Bearer ${AETERNA_SERVICE_TOKEN}" \
      http://{{ include "aeterna.fullname" . }}:{{ .Values.aeterna.service.port }}/admin/sync/github
```

### Backward Compatibility

- All existing env vars (`AETERNA_GITHUB_ORG_NAME` etc.) continue to work
- The CLI `aeterna admin sync github` command continues to work via env vars
- No breaking changes to the existing API surface — `POST /admin/sync/github` still works

### Testing Strategy

- Unit: `build_github_config_for_tenant()` resolves fields in priority order; missing fields fall back
- Unit: per-tenant locking prevents concurrent sync for the same tenant
- Integration: `POST /admin/tenants/{tenant}/sync/github` returns 409 when sync in progress
- Integration: fan-out returns partial success when one tenant has no config
