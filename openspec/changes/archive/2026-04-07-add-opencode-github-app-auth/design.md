## Context

The OpenCode plugin currently authenticates by reading a static `AETERNA_TOKEN` and sending it as a bearer token on every request. On the server side, key plugin-facing routes accept bearer tokens by presence and do not derive authenticated end-user identity from validated claims. This is insufficient for a secure enterprise plugin experience and does not align with the desired GitHub OAuth App device-code authentication model.

This change is cross-cutting across the OpenCode plugin, the Aeterna HTTP server, and identity propagation. It is also security-sensitive because it introduces interactive user sign-in, token issuance, token refresh, and claim-to-tenant/user mapping.

Existing code already contains a useful GitHub App reference in `knowledge/src/git_provider.rs`, where the server mints GitHub App installation tokens using RS256 JWT signing and exchanges them with GitHub. That logic is scoped to repository operations today and must remain separate from plugin user authentication, which uses a GitHub OAuth App device-code flow.

## Goals / Non-Goals

**Goals:**
- Provide a supported interactive authentication flow for the OpenCode plugin using an Aeterna-managed GitHub OAuth App device-code flow.
- Replace static-token assumptions for interactive plugin usage with short-lived Aeterna-issued credentials and refresh behavior.
- Ensure the server validates plugin bearer tokens and derives authenticated user identity for plugin-originated requests.
- Preserve separation between plugin interactive auth and existing service-to-service or automation authentication paths.
- Keep the design compatible with OpenCode's plugin runtime model and its auth-hook-driven UX.

**Non-Goals:**
- Replacing Okta as the system of record for browser-based Aeterna product authentication.
- Replacing the knowledge-repository GitHub App integration used for repository access tokens.
- Implementing GitLab or other identity providers in this change.
- Designing a generic provider-agnostic auth framework before the GitHub OAuth App device-code flow works end to end.

## Decisions

### 1. Use GitHub OAuth App device-code sign-in to bootstrap an Aeterna-issued session token, not raw GitHub access tokens for Aeterna APIs

The plugin will authenticate the user via a GitHub OAuth App device-code flow, but the credential used against Aeterna APIs will be an Aeterna-issued token with Aeterna-specific claims. The server remains the trust boundary and is responsible for validating upstream GitHub identity, normalizing claims, and issuing the plugin-facing token.

**Rationale:**
- Keeps Aeterna API authorization independent from GitHub token formats and scopes.
- Allows tenant/user mapping, expiry, refresh, and revocation rules under Aeterna control.
- Avoids binding all internal API behavior directly to GitHub access token semantics.

**Alternatives considered:**
- **Use raw GitHub access tokens for all Aeterna API calls**: rejected because it pushes GitHub token semantics into every Aeterna endpoint and complicates authorization and claim mapping.
- **Continue static `AETERNA_TOKEN` for plugin clients**: rejected because it does not identify the end user and is operationally weak for interactive use.

### 2. Add dedicated plugin auth endpoints under the Aeterna HTTP server

The server will expose a dedicated auth route group for plugin authentication and token refresh. Plugin-facing API routes will consume validated Aeterna-issued plugin tokens, while existing service-auth paths remain intact.

**Rationale:**
- Makes the plugin flow explicit and auditable.
- Prevents auth logic from being duplicated inside each feature route.
- Keeps plugin auth evolution isolated from Okta ingress auth and machine-only auth.

**Alternatives considered:**
- **Embed auth logic directly into existing session endpoints**: rejected because it mixes bootstrap auth with normal business APIs.
- **Handle all validation only in the plugin**: rejected because the server must remain the trust boundary.

### 3. Introduce a plugin-auth configuration block separate from knowledge-repo GitHub config

The server will use a dedicated configuration section for plugin authentication settings such as GitHub OAuth App client identifiers, device-code exchange settings, token issuer metadata, and signing material for Aeterna-issued tokens. This is separate from `KnowledgeRepoConfig` even if it reuses similar field shapes.

**Rationale:**
- Knowledge-repo GitHub access and end-user plugin authentication are different trust domains.
- Avoids accidental coupling between repository automation credentials and user login behavior.
- Keeps rollout and rotation procedures distinct.

**Alternatives considered:**
- **Reuse `KnowledgeRepoConfig` directly**: rejected because it conflates repository automation with user auth concerns.

### 4. The plugin must own token acquisition and refresh through an auth-aware client layer

The OpenCode plugin will add an auth-aware client path that obtains credentials during initialization, persists or reuses them via the supported OpenCode auth mechanism, and refreshes them before expiry. The HTTP client will attach the current Aeterna-issued bearer token dynamically instead of storing a fixed token at construction time.

**Rationale:**
- Matches the OpenCode plugin runtime model and avoids requiring users to export static secrets manually.
- Enables token rotation without restarting OpenCode.
- Supports future audit and logout behavior.

**Alternatives considered:**
- **Read refreshed token only from env vars**: rejected because it is brittle and not user-friendly.
- **Keep a constructor-fixed token**: rejected because it prevents seamless refresh.

### 5. Server-side request context must be derived from validated claims, not defaults

Plugin-facing routes will derive tenant and user context from validated token claims. The current default fallback (`tenant_id=default`, `user_id=system`) is not acceptable for authenticated plugin traffic.

**Rationale:**
- Restores a real user identity boundary.
- Enables downstream authorization and audit to reflect the actual user.
- Prevents cross-user ambiguity in sync/session operations.

**Alternatives considered:**
- **Continue default tenant/user for plugin routes**: rejected because it defeats authenticated per-user access.

## Risks / Trade-offs

- **[GitHub identity does not directly map to the desired enterprise user record]** → Mitigation: define explicit claim normalization and stable user resolution rules, with email as the idempotent user key where applicable.
- **[Plugin auth flow becomes tightly coupled to OpenCode auth-hook behavior]** → Mitigation: isolate auth logic behind a plugin-side abstraction so hook/API changes remain localized.
- **[Server now manages another token issuer lifecycle]** → Mitigation: use short-lived tokens, explicit issuer metadata, and separate signing/configuration from knowledge-repo GitHub credentials.
- **[GitHub integration terminology may obscure the difference between GitHub App installation tokens and GitHub OAuth App user sign-in]** → Mitigation: keep the design explicit that repository GitHub App access and plugin user authentication are separate responsibilities.
- **[Rollout may temporarily require coexistence of old and new auth paths]** → Mitigation: preserve static machine/service auth and treat plugin interactive auth as an additive path until migration is complete.

## Migration Plan

1. Add plugin-auth configuration and server auth route scaffolding.
2. Implement token issuance/refresh and validated plugin bearer-token middleware on plugin-facing APIs.
3. Update the OpenCode plugin to use the new auth flow while leaving static token support available for non-interactive/service use.
4. Roll out with both paths available; verify that authenticated plugin traffic resolves real user identity on the server.
5. Update plugin docs/examples to prefer the new flow.

**Rollback:**
- Disable the plugin-auth route/config path and continue using the prior static-token mechanism for plugin clients.
- Because service-auth remains separate, rollback does not require reverting Okta ingress auth or repository GitHub App behavior.

## Open Questions

- The initial OpenCode plugin UX SHALL use a GitHub OAuth App device-code-style flow.
- What exact claim set will Aeterna-issued plugin tokens carry for tenant, user, email, and role/group context?
- How should logout and token revocation be represented in the OpenCode plugin experience?
- Should plugin refresh tokens be persisted via the OpenCode auth store only, or also support external secure OS-backed storage when available?

## End-to-End Verification Path (task 5.4)

The following manual verification steps confirm the complete flow against a real server:

### Prerequisites
- Aeterna server running with plugin auth env vars set (see Server Configuration in README)
- GitHub OAuth App client configured for device flow

### Step-by-Step

1. **Start server** with `AETERNA_PLUGIN_AUTH_ENABLED=true`, `AETERNA_PLUGIN_AUTH_GITHUB_CLIENT_ID`, `AETERNA_PLUGIN_AUTH_GITHUB_CLIENT_SECRET`, `AETERNA_PLUGIN_AUTH_JWT_SECRET` set.

2. **Plugin starts** with `AETERNA_PLUGIN_AUTH_ENABLED=true`, `AETERNA_PLUGIN_AUTH_GITHUB_CLIENT_ID` set, **no** `AETERNA_TOKEN`.
   - Expected: plugin requests a device code and prints `verification_uri` + `user_code` to stderr.

3. **User opens the verification URL**, enters the `user_code`, and authorizes the Aeterna OAuth App.

4. **Plugin polls GitHub device flow automatically** until authorization completes.
   - Expected: plugin obtains a GitHub access token, calls `POST /api/v1/auth/plugin/bootstrap`, receives `access_token` + `refresh_token`, logs "signed in as <github-login>".

5. **Session starts** — `POST /api/v1/sessions` carries `Authorization: Bearer <access_token>`.
   - Expected: server validates JWT, sets `github_login` as `UserId` in `TenantContext`.

6. **Memory/knowledge calls** carry the bearer token.
   - Expected: `tenant_context_from_request` derives correct user identity.

7. **Access token expires** (wait for TTL or reduce `AETERNA_PLUGIN_AUTH_ACCESS_TOKEN_TTL_SECONDS=10`).
   - On next `session.start` event the session hook calls `refreshAuth()`.
   - Expected: `POST /api/v1/auth/plugin/refresh` returns new token pair; old refresh token is consumed.

8. **Logout** via `POST /api/v1/auth/plugin/logout` with refresh token.
   - Expected: refresh token is revoked; subsequent refresh attempts return 401.

9. **Static token fallback**: set `AETERNA_TOKEN=my-static-token`, unset `AETERNA_PLUGIN_AUTH_ENABLED`.
   - Expected: plugin uses static token, no OAuth flow triggered.

### Automated Integration Coverage

Server-side unit tests in `cli/src/server/plugin_auth.rs` cover:
- `mint_and_validate_roundtrip`
- `validate_rejects_wrong_secret`
- `validate_rejects_tampered_token`
- `validate_bearer_header_missing_returns_none`
- `validate_bearer_header_extracts_identity`
- `bootstrap_request_uses_github_access_token_contract`
- `bootstrap_request_rejects_legacy_code_shape`
- `tenant_context_uses_tenant_claim_from_bearer`
- `refresh_store_roundtrip`
- `refresh_store_revoke`
- `refresh_store_expired_returns_none`

Plugin-side unit tests in `src/auth.test.ts` (22 tests) cover:
- `bootstrapAuth`: success, error, body shape, no-redirect-uri
- `refreshAuth`: rotation, body, server-rejection (token cleared), no-token guard
- `logoutAuth`: revocation, no-op, network-failure resilience
- Token state accessors: `getAccessToken`, `hasRefreshToken`, `setAuthTokens`
- Static token precedence (task 4.1)
- `callWithReauth`: no-error passthrough, 401-then-refresh-retry, null-on-refresh-fail, non-auth rethrow, no-refresh-token rethrow
