## Why

The `aeterna` CLI has a fully implemented `auth` module (`login`, `logout`, `status`) but it is not wired into the CLI command enum, so users cannot authenticate. The existing `login` implementation requires manually copying a GitHub PAT, which is poor UX. The OpenCode plugin already supports GitHub OAuth App device-code authentication — the CLI should use the same flow so users can authenticate by visiting a URL and entering a code in their browser, with automatic token refresh when sessions expire.

## What Changes

- Wire the existing `auth` subcommand (`login`, `logout`, `status`) into the CLI's `Commands` enum so `aeterna auth login/logout/status` works
- Add GitHub Device Flow (OAuth 2.0 Device Authorization Grant) as the default interactive login method — CLI requests a device code from GitHub, displays a URL + user code, polls for completion, then exchanges the GitHub access token with the Aeterna server's bootstrap endpoint
- Keep PAT-based login as a fallback when `--github-token` is explicitly provided
- Add automatic token refresh — when a stored credential is expired and a refresh token is available, silently refresh before any API call. If refresh fails, prompt re-login
- Use the existing GitHub App (client_id derived from appId [REDACTED_APP_ID]) for the device flow

## Capabilities

### New Capabilities
- `cli-device-flow-auth`: CLI authentication lifecycle including GitHub Device Flow login, credential persistence, automatic token refresh, and session management

### Modified Capabilities
- `opencode-plugin-auth`: Extend device-code auth requirements to cover CLI clients in addition to the OpenCode plugin, sharing the same GitHub App and server-side bootstrap endpoint

## Impact

- Affected CLI code: `cli/src/commands/mod.rs` (wire auth), `cli/src/commands/auth.rs` (add device flow), `cli/src/client.rs` (add device flow HTTP calls, auto-refresh middleware)
- Affected config: `cli/src/credentials.rs` (already supports credential storage), `cli/src/profile.rs` (already supports profile management)
- Reuses: existing `POST /api/v1/auth/plugin/bootstrap` server endpoint — no server changes needed
- External dependency: GitHub Device Authorization endpoint (`https://github.com/login/device/code`) — needs the GitHub App's client_id
- [REDACTED_TENANT] deployment: client_id must be configured (either hardcoded for the known GitHub App or configurable via env/config)
