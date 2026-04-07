## Context

The Aeterna CLI has an existing `auth` module with `login`, `logout`, and `status` commands that are fully implemented but not wired into the CLI's command enum. The current `login` uses PAT-based exchange — user must manually create and paste a GitHub PAT.

The server-side `POST /api/v1/auth/plugin/bootstrap` endpoint already accepts a GitHub access token and returns Aeterna-issued JWT credentials (access + refresh tokens). The OpenCode plugin already implements GitHub Device Flow auth that exchanges through this same endpoint.

The CLI needs to:
1. Be wired into the command enum (trivial)
2. Use GitHub Device Flow for a zero-friction interactive login
3. Auto-refresh tokens transparently

### Constraints
- No server-side changes needed — reuse existing bootstrap endpoint
- Must use the existing GitHub App (client_id derived from appId)
- No secrets in the public repo — client_id for the GitHub App can be embedded (it's public per OAuth spec)
- Credential storage already exists at `~/.config/aeterna/credentials.toml`

## Goals / Non-Goals

**Goals:**
- GitHub Device Flow login as default interactive auth method
- Automatic token refresh on expired credentials
- Clean UX with spinners and clear instructions

**Non-Goals:**
- OS keychain integration (tracked separately, future work)
- Server-side changes to the bootstrap endpoint
- Support for providers other than GitHub

## Decisions

### Decision: Client-side Device Flow, server-side bootstrap exchange

The Device Flow happens entirely between the CLI and GitHub. Once the user authorizes and the CLI obtains a GitHub access token, it exchanges that token with the Aeterna server's existing `POST /api/v1/auth/plugin/bootstrap` endpoint for Aeterna credentials.

**Why:** Zero server-side changes. The bootstrap endpoint already handles GitHub token → Aeterna JWT exchange. Device Flow is a standard OAuth 2.0 grant that doesn't require the server to know about it.

### Decision: GitHub App client_id configuration

The GitHub App client_id will be configurable via:
1. CLI profile config (`~/.config/aeterna/config.toml` — `github_client_id` field)
2. Environment variable `AETERNA_GITHUB_CLIENT_ID`
3. Server URL config endpoint `GET /api/v1/auth/config` (future, not in this change)

For [REDACTED_TENANT] deployment, the client_id is derived from the GitHub App (appId [REDACTED_APP_ID]). The client_id for a GitHub App can be found in the App settings — it is NOT the same as the appId.

**Why:** client_id is public per OAuth spec (no secret). Making it configurable supports multiple GitHub Apps across environments.

### Decision: Auto-refresh as middleware in the AeternaClient

When making authenticated API calls, the `AeternaClient` checks if the stored credential is expired. If so, it calls `POST /api/v1/auth/plugin/refresh` with the refresh token before proceeding. If refresh fails (revoked, expired refresh token), it returns an error with a "re-login" hint.

**Why:** Transparent UX — users don't need to know about token lifecycles. Same pattern the OpenCode plugin uses.

### Decision: Polling with exponential backoff

During device flow, the CLI polls `POST https://github.com/login/oauth/access_token` at the `interval` returned by GitHub (typically 5s), with exponential backoff on `slow_down` responses. The CLI shows a spinner with the verification URL and user code.

**Why:** GitHub's device flow spec requires respecting the polling interval. Exponential backoff prevents rate limiting.

## Risks / Trade-offs

- **Risk:** GitHub App might not have device flow enabled → Mitigation: verify in GitHub App settings (must enable "Device authorization flow" under Optional features)
- **Risk:** User might not have browser access → Mitigation: PAT fallback via `--github-token` flag preserved
- **Trade-off:** Embedding client_id in config rather than auto-discovering from server → Simpler, no server change needed

## Open Questions

- What is the actual client_id for the GitHub App (appId [REDACTED_APP_ID])? Need to check GitHub App settings page.
