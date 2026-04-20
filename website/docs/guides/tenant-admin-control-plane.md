# Tenant Admin Control Plane

This guide explains the current tenancy-management architecture exposed by the Aeterna control plane.

## Mental model

Start with the ownership boundary:

- **Platform-owned**: tenant lifecycle, shared Git provider connections, platform-owned tenant config fields, platform-owned tenant secret entries
- **Tenant-owned**: tenant-scoped config fields, tenant-scoped secret entries, canonical repository binding choices inside the tenant boundary
- **Deployment-specific**: environment overlays and secret material that belong in a **private deployment repository**, not in the public Aeterna repo

## Tenant config provider

Tenant configuration is enforced through a `TenantConfigProvider` abstraction.

Current implementation:

- ConfigMap: `aeterna-tenant-<tenant-id>`
- Secret: `aeterna-tenant-<tenant-id>-secret`

Rules:

- raw secret values are write-only
- API and CLI responses expose only logical secret references
- `TenantAdmin` cannot mutate platform-owned config or secrets

## Shared Git provider connections

GitHub App connectivity is modeled as a platform-owned shared connection.

- the connection stores secret references such as PEM/webhook refs
- tenants reference a connection by ID only
- a tenant can use a connection only after explicit grant

## Core CLI flows

```bash
# authenticate
aeterna profile login

# create tenant
aeterna tenant create --slug acme --name "Acme Corp"

# repository binding
aeterna tenant repo-binding set acme \
  --kind github \
  --remote-url https://github.com/acme/knowledge.git \
  --branch main \
  --credential-kind githubApp \
  --github-owner acme \
  --github-repo knowledge

# tenant config
aeterna tenant config inspect --tenant acme
aeterna tenant config upsert --tenant acme --file tenant-config.json
aeterna tenant secret set --tenant acme repo.token --value '...'

# shared connection visibility
aeterna tenant connection list acme
aeterna tenant connection grant acme --connection <id>
```

## Cross-tenant targeting

`--target-tenant <tenant-id>` is reserved for `PlatformAdmin` cross-tenant operations. `TenantAdmin` remains bound to its own tenant.

## More detail

For the repo version of this guide with the longer operator walkthrough, see:

- [`docs/guides/tenant-admin-control-plane.md`](https://github.com/kikokikok/aeterna/blob/master/docs/guides/tenant-admin-control-plane.md) on GitHub
