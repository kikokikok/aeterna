## 1. Schema

- [ ] 1.1 Migration `024_admin_ui_token_kind.sql`: add `token_kind TEXT NOT NULL DEFAULT 'plugin-access'` to `oauth_refresh_tokens` with a `CHECK (token_kind IN ('plugin-access', 'admin-ui'))` constraint; add index `idx_oauth_refresh_tokens_user_kind(user_id, token_kind)`.
- [ ] 1.2 Backfill: existing rows retain `'plugin-access'` via default.
- [ ] 1.3 Add downgrade note in `storage/migrations/README.md`.

## 2. Server: admin-ui auth endpoints

- [ ] 2.1 Extract common JWT helpers into `cli/src/server/jwt.rs` if not already: mint, verify, extract claims. Parameterize `aud`.
- [ ] 2.2 Create `cli/src/server/admin_ui_auth.rs` with `admin_ui_bootstrap`, `admin_ui_refresh`, `admin_ui_revoke` handlers. Each mirrors the plugin-auth handler but uses `aud: "aeterna-admin-ui"`.
- [ ] 2.3 Configure TTLs per kind: `plugin-access` keeps current (1 hour access / 30 day refresh), `admin-ui` gets shorter (30 min access / 14 day refresh).
- [ ] 2.4 Wire routes at `/api/v1/auth/admin-ui/bootstrap|refresh|revoke`.
- [ ] 2.5 Revoke endpoint deletes ONLY `token_kind='admin-ui'` rows for the caller.

## 3. Server: audience enforcement

- [ ] 3.1 In the request auth middleware, look up the endpoint's expected `aud` from a route-table (admin-ui endpoints expect `aeterna-admin-ui`, plugin endpoints expect `aeterna-plugin`, general API endpoints accept either).
- [ ] 3.2 Reject mismatched `aud` with `401 wrong_audience`.
- [ ] 3.3 Behind `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS=true` (default true for one release), allow `plugin-access` tokens on admin-ui endpoints with a `deprecated-token-kind` structured log event.

## 4. Admin UI migration

- [ ] 4.1 Update `admin-ui/src/auth/LoginPage.tsx` to call `POST /api/v1/auth/admin-ui/bootstrap` instead of `/auth/plugin/bootstrap`.
- [ ] 4.2 Update `admin-ui/src/auth/token-manager.ts` to hit `/auth/admin-ui/refresh` on rotation and `/auth/admin-ui/revoke` on logout.
- [ ] 4.3 Update `admin-ui/src/api/client.ts` to treat `401 wrong_audience` as "session expired" (force re-login rather than refresh).

## 5. Tests

- [ ] 5.1 Integration: `admin-ui/bootstrap` issues a token with `aud: aeterna-admin-ui`; `plugin/bootstrap` issues with `aud: aeterna-plugin`.
- [ ] 5.2 Integration: a `plugin-access` token sent to an admin-ui endpoint with legacy flag OFF → `401 wrong_audience`.
- [ ] 5.3 Integration: a `plugin-access` token sent to an admin-ui endpoint with legacy flag ON → accepted with a deprecation log line.
- [ ] 5.4 Integration: `admin-ui/revoke` deletes only admin-ui refresh tokens for the user; plugin tokens remain valid.
- [ ] 5.5 Integration: `admin-ui/refresh` with a plugin refresh token → `401 wrong_audience`.
- [ ] 5.6 Unit: TTL values applied correctly per kind.

## 6. Rollout

- [ ] 6.1 Release N: ship the new endpoints + audience enforcement with `ADMIN_UI_ACCEPT_LEGACY_PLUGIN_TOKENS=true` default. Admin UI switches to the new endpoints. Existing UI sessions keep working via the legacy flag.
- [ ] 6.2 Release N+1: flip the default to `false`; document that admins who hard-pinned the old flag must migrate.
- [ ] 6.3 Monitor the `deprecated-token-kind` log event to detect any unexpected external consumers.
