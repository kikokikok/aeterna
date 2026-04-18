## Why

The admin UI authenticates today by reusing the `plugin-access` token kind minted by the OpenCode plugin bootstrap flow (`POST /api/v1/auth/plugin/bootstrap`). This was a pragmatic shortcut when the admin UI shipped: the same JWT issuer, same validation path, same refresh rotation. It has three practical problems:

1. **Audience confusion**: the `aud` claim says `aeterna-plugin`, but the token is being accepted by admin UI flows that were never intended for plugin consumption. Any audit of the token surface reports two different consumers sharing one audience.
2. **Scope over-grant**: plugin-access tokens are designed for OpenCode plugin API calls — typically narrower than the admin UI's full feature set. Today the check is "is the user a PlatformAdmin or TenantAdmin?" which is coarse; a properly audienced admin token would let us narrow plugin tokens without collateral damage to the UI.
3. **Revocation asymmetry**: revoking all plugin tokens for a user (because of a compromised plugin) currently also logs them out of the admin UI, because they share the token kind. Users have no way to "log out of my CLI plugins but stay logged in to the browser".

This change introduces a distinct `admin-ui` token kind alongside `plugin-access`, with separate issuance, validation, and revocation paths. Refresh tokens for each kind are fully independent.

## What Changes

- Introduce a new token kind `admin-ui` (in addition to existing `plugin-access`). Both are JWTs signed by the same key but with different `aud` claims (`aeterna-admin-ui` vs `aeterna-plugin`) and potentially different TTLs (admin-ui access tokens shorter, matching browser session norms).
- Add `POST /api/v1/auth/admin-ui/bootstrap` that exchanges a GitHub OAuth code for an `admin-ui` token pair (access + refresh). The OAuth redirect URI and state management are admin-ui specific.
- Add `POST /api/v1/auth/admin-ui/refresh` that rotates the admin-ui refresh token (same single-use rotation policy as plugin-access).
- Add `POST /api/v1/auth/admin-ui/revoke` that revokes all admin-ui tokens for the calling user, without touching plugin tokens.
- Admin UI migrates from `/auth/plugin/bootstrap` to `/auth/admin-ui/bootstrap`; the plugin-access path remains for the OpenCode plugin unchanged.
- Server validates `aud` claim against the endpoint family: admin-ui endpoints reject plugin-access tokens, and vice versa, with `401 wrong_audience` error code.
- Migration: existing admin-ui sessions remain valid (plugin-access tokens keep working on admin-ui endpoints during a grace period gated by `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS=true`, default `true` for 1 release cycle, then flipped to `false`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `opencode-plugin-auth`: scope narrowed to only `plugin-access` tokens; admin UI sessions no longer consume this endpoint family; `aud` claim enforcement added.
- `user-auth`: adds a new admin-ui token family (`bootstrap`, `refresh`, `revoke` endpoints) with its own audience, TTLs, and revocation scope; defines `aud`-based endpoint routing.

## Impact

- **Affected code**: `cli/src/server/plugin_auth.rs` (split into `plugin_auth.rs` + new `admin_ui_auth.rs`), `cli/src/server/jwt.rs` (validate `aud` against expected audience per endpoint), `cli/src/server/router.rs` (new `/auth/admin-ui/*` routes), `admin-ui/src/auth/token-manager.ts` + `admin-ui/src/auth/LoginPage.tsx` (switch bootstrap endpoint).
- **Affected APIs**: new `POST /api/v1/auth/admin-ui/bootstrap|refresh|revoke`. Error code `wrong_audience` added to the error catalog.
- **Affected storage**: `oauth_refresh_tokens` table grows a `token_kind TEXT NOT NULL DEFAULT 'plugin-access'` column; migration backfills existing rows.
- **Dependencies**: coordinates with `add-admin-web-ui` (which introduces the admin UI login flow). Can land independently but ideally after the admin UI is in production so real-world traffic informs TTL tuning.
- **Rollout**: legacy acceptance flag (`ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS`) defaulting on, flipped off one release later; clients migrate during the grace window.
