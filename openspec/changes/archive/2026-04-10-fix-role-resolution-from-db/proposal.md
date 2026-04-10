# Change: Server-side role resolution from database

## Why

The authentication middleware currently relies on the `X-User-Role` request header to determine the caller's role. This header is entirely client-asserted and not validated against the database. The bootstrap process seeds PlatformAdmin into the `user_roles` table, but no server-side code reads this table during API request authorization. As a result, bootstrapped PlatformAdmin users cannot perform privileged operations (e.g., tenant creation) because neither the CLI nor the plugin sends an `X-User-Role` header, and the server never looks up roles itself.

This is both a security concern (any authenticated client can claim any role by sending a header) and a functional blocker (PlatformAdmin operations always fail with 403).

Additionally, the bootstrap uses `tenant_id = 'default'` as a proxy for "instance scope" in the `user_roles` table, which is a design smell — `'default'` could be a real tenant slug, making the sentinel ambiguous.

## What Changes

- The auth middleware and `authenticated_tenant_context` helper SHALL resolve user roles from the `user_roles` database table after JWT validation, instead of trusting the `X-User-Role` header
- A user resolution step SHALL map the JWT `sub` (GitHub login) to the `users.id` UUID via `users.idp_subject`, then query `user_roles` for that user
- Instance-scoped roles (e.g., PlatformAdmin) SHALL use `tenant_id = '__root__'` as the explicit sentinel value, replacing the ambiguous `'default'`
- The role lookup SHALL query `user_roles` for both the JWT's resolved tenant_id AND `tenant_id = '__root__'`, so that instance-scoped roles are visible regardless of the caller's resolved tenant
- The bootstrap SHALL seed PlatformAdmin with `tenant_id = '__root__'` instead of `'default'`
- A migration SHALL update existing `user_roles` rows where `role = 'PlatformAdmin' AND tenant_id = 'default'` to use `tenant_id = '__root__'`
- The `X-User-Role` header SHALL be ignored when auth is enabled (production mode); it remains trusted only in dev mode (auth disabled) for backward compatibility
- The `authenticated_tenant_context` function SHALL become async to support the DB query

## Impact

- Affected specs: `opencode-plugin-auth`, `admin-bootstrap`
- Affected code:
  - `cli/src/server/auth_middleware.rs` — `tenant_context_from_identity` becomes async, adds DB role lookup
  - `cli/src/server/mod.rs` — `authenticated_tenant_context` becomes async, adds DB role lookup
  - `cli/src/server/bootstrap.rs` — change hardcoded `'default'` to `'__root__'` for PlatformAdmin tenant_id
  - `cli/src/server/tenant_api.rs` — all callers of `authenticated_tenant_context` updated for async
  - `cli/src/server/user_api.rs` — callers updated for async
  - `storage/src/postgres.rs` — new `resolve_user_id_by_idp_subject` and `get_all_user_roles` methods
  - `mk_core/src/types.rs` — add `INSTANCE_SCOPE_TENANT_ID` constant (`__root__`)
  - All handlers calling `require_platform_admin`, `require_tenant_admin_context`, `tenant_scoped_context` — updated for async
- **BREAKING**: `X-User-Role` header no longer honored when auth is enabled; roles come from DB only
- **MIGRATION**: Existing `user_roles` rows with `tenant_id = 'default'` and `role = 'PlatformAdmin'` migrated to `tenant_id = '__root__'`
