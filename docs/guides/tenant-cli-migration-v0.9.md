# Tenant CLI migration (v0.9): `apply` replaces per-field mutation commands

**Status:** Breaking change. Lands with B2 `harden-tenant-provisioning` §7.6.

## TL;DR

All per-field **mutation** subcommands under `aeterna tenant` have been removed. The single `aeterna tenant apply -f <manifest.json>` entry point now performs the equivalent write through the transactional `/provision` server endpoint.

**Read-only** commands (`list`, `show`, `repo-binding show`, `config inspect`, `connection list`) are unchanged. The GitOps loop (`validate`, `render`, `diff`, `apply`, `watch`) is unchanged. User-context commands (`use`, `switch`, `current`) are unchanged. Lifecycle (`deactivate`) is unchanged.

## Removed subcommands

The following subcommands **no longer exist**. `clap` will reject them with an "unrecognized subcommand" error:

| Removed | Replaced by | Manifest field |
|---------|-------------|----------------|
| `tenant create` | `tenant apply -f manifest.json` | `tenant` (`slug`, `name`) |
| `tenant update` | `tenant apply -f manifest.json` | `tenant` (`slug`, `name`) |
| `tenant domain-map` | `tenant apply -f manifest.json` | `tenant.domainMappings[]` |
| `tenant repo-binding set` | `tenant apply -f manifest.json` | `repository` *(note: top-level field is `repository`, not `repoBinding`)* |
| `tenant repo-binding validate` | `tenant validate -f manifest.json` | n/a |
| `tenant config upsert` | `tenant apply -f manifest.json` | `config.fields{}` |
| `tenant config validate` | `tenant validate -f manifest.json` | n/a |
| `tenant secret set` | `tenant apply -f manifest.json` | `secrets[]` |
| `tenant secret delete` | ⚠ no `--prune` shipped yet — use the server's `DELETE /admin/tenants/{slug}/secrets/{name}` directly, or remove the entry from the manifest and wait for `apply --prune` (tracked under B2 §7.1 `--prune`) |

## Preserved (no manifest equivalent)

| Kept | Why |
|------|-----|
| `tenant connection grant` | Git-provider-connection visibility lives in a separate junction table (`git_provider_connections_tenants`); `/provision` does not touch it. A future manifest revision may add a `connections[]` block (B2 §2.10 idea). |
| `tenant connection revoke` | Same as above. |
| `tenant deactivate` | Soft-delete lifecycle with tombstone semantics; not cleanly a manifest state. |

## Why

Per-field mutation subcommands fanned out to N independent server endpoints. Each had its own idempotency story, its own authorisation check, its own audit row, and — critically — its own race window with the others. Creating a tenant with a repo binding and a config was three round trips, three transactions, three chances to end up in a half-provisioned state after a pod restart.

The `/provision` endpoint (§2.3 of `harden-tenant-provisioning`) writes the full tenant state in one transaction from a single authoritative manifest. `tenant apply` wraps that endpoint. One command, one transaction, one audit row per apply, one generation bump. The admin UI uses the same endpoint with the same manifest shape, so CLI and UI are always byte-equivalent.

## Quick migration

### Before (v0.8)

```bash
aeterna tenant create --slug acme --name "Acme Corp"
aeterna tenant repo-binding set acme \
  --kind github --github-owner acme --github-repo knowledge \
  --branch main \
  --credential-kind githubApp --credential-ref acme-gh-app
aeterna tenant config upsert --tenant acme --file tenant-config.json
aeterna tenant secret set --tenant acme repo.token --value "$GH_TOKEN"
aeterna tenant connection grant acme --connection github-default   # still exists, see below
```

### After (v0.9)

```bash
cat > acme.manifest.json <<'MANIFEST'
{
  "apiVersion": "aeterna/v1",
  "kind": "Tenant",
  "tenant": { "slug": "acme", "name": "Acme Corp" },
  "repository": {
    "kind": "github",
    "github": { "owner": "acme", "repo": "knowledge" },
    "branch": "main",
    "credentialKind": "githubApp",
    "credentialRef": "acme-gh-app"
  },
  "config": {
    "fields": { "...": "inline the k/v pairs from tenant-config.json here" },
    "secretReferences": { }
  },
  "secrets": [
    { "logicalName": "repo.token", "ownership": "tenant", "secretValue": "$GH_TOKEN_LITERAL" }
  ]
}
MANIFEST

aeterna tenant validate -f acme.manifest.json   # dry-run preview
aeterna tenant apply    -f acme.manifest.json   # real apply, interactive confirmation

# Connection visibility is NOT a manifest field — still a standalone call:
aeterna tenant connection grant acme --connection github-default
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
| `tenant connection list` / `grant` / `revoke` | unchanged — Git-connection visibility has no manifest equivalent in v1 |
| `tenant validate` / `render` / `diff` / `apply` / `watch` | unchanged (GitOps pipeline) |

## Server endpoints

The underlying per-resource server endpoints (`PUT /admin/tenants/{slug}/repo-binding`, etc.) are **not** removed in this release — they still back the UI's per-section save flows and the legacy v0.8 CLI if operators pin to it. They may be removed in a future release once `/provision` covers all write paths; track §2.10 in `harden-tenant-provisioning`.

## Rollback

If this break is disruptive, pin to `aeterna-cli@v0.8.x` — the server remains backward-compatible with v0.8 CLI calls for at least one minor version. File an issue if you need the old subcommands preserved; they can be re-added as thin wrappers that assemble a minimal manifest and call `/provision`, which was the original §7.6 design before the no-backwards-compat decision was taken.
