## 1. Server auth foundation

- [x] 1.1 Add dedicated plugin-auth configuration types and loading for GitHub-backed plugin authentication settings
- [x] 1.2 Add server-side plugin auth state separate from existing knowledge-repo GitHub credentials and existing service auth state
- [x] 1.3 Add `/api/v1/auth/*` route scaffolding to the Axum server router for plugin auth bootstrap, refresh, and session termination

## 2. Plugin token issuance and validation

- [x] 2.1 Implement server-side plugin auth bootstrap that validates the upstream GitHub OAuth App device-code identity result and issues Aeterna plugin session credentials
- [x] 2.2 Implement server-side refresh handling for Aeterna-issued plugin credentials
- [x] 2.3 Implement plugin bearer-token validation middleware or request extraction for plugin-facing API routes
- [x] 2.4 Replace default plugin request identity fallback with tenant/user context derived from validated token claims

## 3. OpenCode plugin authentication flow

- [x] 3.1 Add plugin auth configuration/types needed for interactive sign-in and token lifecycle management
- [x] 3.2 Refactor the plugin client to attach bearer tokens dynamically instead of storing a fixed token at construction time
- [x] 3.3 Implement plugin-side sign-in/bootstrap flow using the supported OpenCode auth mechanism
- [x] 3.4 Implement plugin-side token refresh and re-authentication handling for expired or revoked sessions

## 4. Compatibility and migration behavior

- [x] 4.1 Preserve static token support for non-interactive/service use while preferring interactive auth for normal plugin sign-in
- [x] 4.2 Ensure browser-based Okta auth behavior remains unchanged for protected Aeterna product endpoints
- [x] 4.3 Ensure plugin-auth changes do not alter knowledge-repository GitHub App behavior or existing service-to-service auth flows

## 5. Verification and documentation

- [x] 5.1 Add automated tests for server auth bootstrap, token refresh, token validation, and invalid-token rejection paths
- [x] 5.2 Add automated tests for plugin sign-in, session reuse, refresh, and explicit re-auth requirements after refresh failure
- [x] 5.3 Update plugin and server documentation to describe the supported auth flow, configuration, and migration from static interactive tokens
- [x] 5.4 Verify the end-to-end OpenCode plugin flow against a real server path with authenticated user identity resolution
