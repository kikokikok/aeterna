# Tenant CLI migration (v0.9): `apply` replaces per-field mutation commands

**Status:** Breaking change. Lands with B2 `harden-tenant-provisioning` §7.6.

## TL;DR

All per-field **mutation** subcommands under `aeterna tenant` have been removed. The single `aeterna tenant apply -f <manifest.json>` entry point now performs the equivalent write through the transactional `/provision` server endpoint.

**Read-only** commands (`list`, `show`, `repo-binding show`, `config inspect`, `connection list`) are unchanged. The GitOps loop (`validate`, `render`, `diff`, `apply`, `watch`) is unchanged. User-context commands (`use`, `switch`, `current`) are unchanged. Lifecycle (`deactivate`) is unchanged.

## Removed subcommands

The following subcommands **no longer exist**. `clap` will reject them with an "unrecognized subcommand" error:

| Removed | Replaced by |
|---------|-------------|
| `tenant create` | `tenant apply -f manifest.json` |
| `tenant update` | `tenant apply -f manifest.json` |
| `tenant domain-map` | `tenant apply -f manifest.json` (`domainMappings` field) |
| `tenant repo-binding set` | `tenant apply -f manifest.json` (`repoBinding` field) |
| `tenant repo-binding validate` | `tenant validate -f manifest.json` |
| `tenant config upsert` | `tenant apply -f manifest.json` (`tenantConfig` field) |
| `tenant config validate` | `tenant validate -f manifest.json` |
| `tenant secret set` | `tenant apply -f manifest.json` (`secrets[]` field) |
| `tenant secret delete` | `tenant apply -f manifest.json` (omit entry; set `--prune` once available) |
| `tenant connection grant` | `tenant apply -f manifest.json` (`connections[]` field) |
| `tenant connection revoke` | `tenant apply -f manifest.json` (omit entry; set `--prune` once available) |

## Why

Per-field mutation subcommands fanned out to N independent server endpoints. Each had its own idempotency story, its own authorisation check, its own audit row, and — critically — its own race window with the others. Creating a tenant with a repo binding and a config was three round trips, three transactions, three chances to end up in a half-provisioned state after a pod restart.

The `/provision` endpoint (§2.3 of `harden-tenant-provisioning`) writes the full tenant state in one transaction from a single authoritative manifest. `tenant apply` wraps that endpoint. One command, one transaction, one audit row per apply, one generation bump. The admin UI uses the same endpoint with the same manifest shape, so CLI and UI are always byte-equivalent.

## Quick migration

### Before (v0.8)

```bash
aeterna tenant create --slug acme --name "Acme Corp"
aeterna tenant repo-binding set acme --kind github --github-owner acme --github-repo knowledge --branch main --credential-kind githubApp --credential-ref acme-gh-app
aeterna tenant config upsert --tenant acme --file tenant-config.json
aeterna tenant secret set --tenant acme repo.token --from-env GH_TOKEN
aeterna tenant connection grant acme --connection github-default
```

### After (v0.9)

```bash
cat > acme.manifest.json <<'MANIFEST'
{
  "apiVersion": "aeterna/v1",
  "kind": "Tenant",
  "tenant": { "slug": "acme", "name": "Acme Corp" },
  "repoBinding": {
    "kind": "github",
    "github": { "owner": "acme", "repo": "knowledge" },
    "branch": "main",
    "credentialKind": "githubApp",
    "credentialRef": "acme-gh-app"
  },
  "tenantConfig": { "...": "contents of tenant-config.json inlined here" },
  "secrets": [
    { "logicalName": "repo.token", "secretRef": "env:GH_TOKEN" }
  ],
  "connections": [ { "connectionId": "github-default" } ]
}
MANIFEST

aeterna tenant validate -f acme.manifest.json   # dry-run preview
aeterna tenant apply    -f acme.manifest.json   # real apply, interactive confirmation
```

**Tip:** Bootstrap the manifest by rendering the current server state:

```bash
aeterna tenant render --slug acme > acme.manifest.json
# edit acme.manifest.json
aeterna tenant diff --slug acme -f acme.manifest.json   # preview the delta
aeterna tenant apply -f acme.manifest.json --watch
```

## What did NOT change

| Surface | Status |
|---------|--------|
| `tenant list` | unchanged |
| `tenant show` | unchanged |
| `tenant deactivate` | unchanged (lifecycle; not a manifest field) |
| `tenant use` / `switch` / `current` | unchanged (user-context) |
| `tenant repo-binding show` | unchanged (read) |
| `tenant config inspect` | unchanged (read) |
| `tenant connection list` | unchanged (read) |
| `tenant validate` / `render` / `diff` / `apply` / `watch` | unchanged (GitOps pipeline) |

## Server endpoints

The underlying per-resource server endpoints (`PUT /admin/tenants/{slug}/repo-binding`, etc.) are **not** removed in this release — they still back the UI's per-section save flows and the legacy v0.8 CLI if operators pin to it. They may be removed in a future release once `/provision` covers all write paths; track §2.10 in `harden-tenant-provisioning`.

## Rollback

If this break is disruptive, pin to `aeterna-cli@v0.8.x` — the server remains backward-compatible with v0.8 CLI calls for at least one minor version. File an issue if you need the old subcommands preserved; they can be re-added as thin wrappers that assemble a minimal manifest and call `/provision`, which was the original §7.6 design before the no-backwards-compat decision was taken.
