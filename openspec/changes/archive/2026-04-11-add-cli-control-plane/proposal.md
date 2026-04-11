## Why

The `aeterna` binary exposes a broad command surface, but most backend-facing workflows are still simulated, disconnected, or rely on ad hoc environment variables instead of a supported control-plane UX. We need a real CLI control plane now because the server, auth endpoints, and MCP/runtime layers already exist, and users need installable binaries, authenticated usage, target selection, and honest command behavior to operate Aeterna end to end.

## What Changes

- Add a dedicated CLI control-plane capability that defines authenticated CLI usage, secure credential storage, target server/environment selection, and real backend-backed command execution.
- Add supported `aeterna login`, `logout`, and `auth status` flows for interactive CLI users, with token refresh and secure local storage.
- Add canonical configuration and environment management for the CLI, including profile selection, server targeting, config validation, and consistent file locations.
- Replace simulated or `server_not_connected` command paths for memory, knowledge, governance, admin, sync, and identity workflows with real server-backed execution or explicit unsupported errors.
- Add supported CLI installation and packaging flows for macOS and Linux, including native release artifacts and package-manager-friendly installation paths.
- Clarify the post-legacy `code-search` CLI contract so the command either routes to a supported backend or fails explicitly with a supported migration path.
- Document whole CLI usage from the user perspective with end-to-end scenarios covering install, login, target selection, daily usage, and operator/admin flows.

## Capabilities

### New Capabilities
- `cli-control-plane`: Authenticated CLI control-plane behavior, command parity, server targeting, token lifecycle, user journeys, and packaging-facing UX.

### Modified Capabilities
- `user-auth`: Extend authentication requirements to cover CLI interactive login/logout/status flows, secure token handling, and CLI identity verification.
- `runtime-operations`: Require backend-facing CLI commands to execute real operations or fail explicitly, and define connectivity/offline behavior honestly.
- `configuration`: Define canonical CLI config files, profile precedence, `AETERNA_*` environment handling, and config management UX.
- `deployment`: Add supported native CLI binary distribution and installer requirements for macOS and Linux.
- `codesearch`: Update the CLI contract after legacy binary removal so users have a supported path instead of dead command shells.

## Impact

- Affected code: `cli/src/commands/*`, `cli/src/offline.rs`, `cli/src/ux_error.rs`, CLI config/context loading, auth bootstrap consumption, code-search commands, packaging/release automation, and end-to-end tests.
- Affected APIs: CLI-to-server HTTP usage, auth bootstrap/refresh consumption, command output/error contracts, profile selection, and code-search routing.
- Affected systems: local developer onboarding, operator workflows, release packaging, docs, and integration between CLI, server, and plugin auth surfaces.
