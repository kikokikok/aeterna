# Change: Split CLI into three binaries

## Why

The `aeterna` binary currently bundles everything: CLI commands, HTTP server, and direct-to-DB migration logic. This creates problems:

1. **CLI users pull server dependencies** (axum, tower, hyper, storage, memory, knowledge crates) they never use.
2. **`admin migrate` connects directly to PostgreSQL via sqlx** — CLI users shouldn't need DB credentials, and it leaks a dangerous surface into a client-side tool.
3. **No lightweight CLI binary** for end-user distribution — the full binary is bloated with server code.

## What Changes

Split the single `aeterna` crate into three binary targets:

### 1. `aeterna` (full — CLI + server)
- Everything that exists today. No user-facing change.
- Used in Docker images and server deployments.

### 2. `aeterna-cli` (lean — CLI only)
- All CLI commands EXCEPT `serve`.
- No `server/` module, no axum/tower/hyper deps.
- `admin migrate` calls the server API instead of connecting to the DB directly.
- Target binary for end-user installation (brew, GitHub releases, curl).

### 3. `aeterna-migrate` (dedicated migration binary)
- Tiny binary: embedded SQL + sqlx + config loading + sha2.
- Used by Helm Jobs and ops for direct-to-DB migrations.
- Replaces the current `admin migrate up/down/status` direct-DB path.
- The Helm migration Job switches from `aeterna admin migrate up` to `aeterna-migrate up`.

### Dependency changes

| Dependency | `aeterna` | `aeterna-cli` | `aeterna-migrate` |
|---|---|---|---|
| axum, tower, hyper | Yes | **No** | No |
| storage, memory, knowledge, sync, tools, agent-a2a, adapters, idp-sync | Yes | **No** | No |
| sqlx | Yes (via migrate) | **No** | Yes |
| mk_core, context, config, errors | Yes | Yes | Yes (config only) |
| clap, serde, reqwest, etc. | Yes | Yes | Minimal |

### `admin migrate` behavior change

| Subcommand | Current (direct DB) | After (`aeterna-cli`) | After (`aeterna-migrate`) |
|---|---|---|---|
| `status` | sqlx query | `GET /api/v1/admin/migrate/status` | sqlx query |
| `up` | sqlx apply | Points to `aeterna-migrate` | sqlx apply |
| `down` | Already unsupported | Points to `aeterna-migrate` | Same |

### CI changes
- `cli-release.yml` builds `aeterna-cli` for all 4 targets (x86_64/aarch64 linux/macos).
- Existing workflow continues building `aeterna` (full).
- Add `aeterna-migrate` to Docker image (already there via full binary, but now explicit).

## Impact

- Affected specs: `server-runtime`, `deployment`
- Affected code:
  - `cli/Cargo.toml` — cargo features to gate server deps, add `[[bin]]` entries
  - `cli/src/main.rs` — conditional `serve` command via feature flag
  - `cli/src/lib.rs` — conditional `server` module
  - `cli/src/commands/mod.rs` — conditional `Serve` variant
  - `cli/src/commands/admin.rs` — replace sqlx with API calls behind feature flag
  - New crate: `migrate/` with `aeterna-migrate` binary
  - `.github/workflows/cli-release.yml` — build `aeterna-cli`
  - Helm chart migration Job — use `aeterna-migrate` instead of `aeterna admin migrate up`
