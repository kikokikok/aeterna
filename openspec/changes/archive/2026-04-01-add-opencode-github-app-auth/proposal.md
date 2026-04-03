## Why

The OpenCode plugin currently authenticates to Aeterna with a static `AETERNA_TOKEN`, while the server only checks for bearer-token presence on key routes and does not derive end-user identity from plugin requests. This prevents a secure user-authenticated plugin flow and does not satisfy the desired GitHub OAuth App device-code authentication model for enterprise OpenCode usage.

## What Changes

- Add a GitHub OAuth App-backed device-code authentication flow for the OpenCode plugin so users can sign in interactively and obtain Aeterna-issued session credentials without manually managing static API keys.
- Add server-side auth endpoints and token validation for plugin-originated requests, including user identity extraction and tenant/user mapping from validated claims.
- Update the OpenCode plugin to use an auth flow compatible with OpenCode's plugin model instead of relying on `AETERNA_TOKEN` as the primary interactive credential.
- Preserve a separate supported machine-auth path for automation and service-to-service traffic.
- Improve OpenCode plugin credential handling, refresh behavior, and installation/configuration guidance to reflect the supported auth flow.

## Capabilities

### New Capabilities
- `opencode-plugin-auth`: Authentication lifecycle for the OpenCode plugin, including user sign-in, token acquisition, refresh, and authenticated request behavior.

### Modified Capabilities
- `opencode-integration`: Replace static-token assumptions for interactive plugin usage with a GitHub OAuth App-backed device-code auth flow and update credential security expectations.
- `user-auth`: Extend supported non-browser/client authentication behavior to cover authenticated plugin access while preserving separation from Okta-backed interactive browser auth.
- `server-runtime`: Add plugin auth route handling and validated identity-aware request processing for server APIs used by the OpenCode plugin.

## Impact

- Affected plugin code: `packages/opencode-plugin/src/index.ts`, `packages/opencode-plugin/src/client.ts`, `packages/opencode-plugin/src/types.ts`, and related auth/config integration points.
- Affected server code: `cli/src/server/router.rs`, `cli/src/server/bootstrap.rs`, `cli/src/server/sync.rs`, and session/auth-adjacent handlers.
- Reuse/reference areas: existing auth/config patterns in `config/src/config.rs` and existing GitHub integration boundaries in `knowledge/src/git_provider.rs` that must remain separate from plugin user auth.
- Affected systems: OpenCode plugin runtime, Aeterna HTTP auth/session boundaries, GitHub OAuth App credentials/config, and downstream tenant/user identity propagation.
