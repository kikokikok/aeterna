## 0. Instance Scope Sentinel
- [x] 0.1 Add `INSTANCE_SCOPE_TENANT_ID` constant (`"__root__"`) to `mk_core/src/types.rs`
- [x] 0.2 Update `bootstrap.rs` `seed_platform_admin` to use `__root__` instead of `"default"` for PlatformAdmin tenant_id and unit_id
- [x] 0.3 Add migration in bootstrap: `UPDATE user_roles SET tenant_id = '__root__' WHERE role = 'PlatformAdmin' AND tenant_id = 'default'`
- [x] 0.4 Update `organizational_units` bootstrap to create a `__root__` unit if needed (or decouple from unit creation)

## 1. Storage Layer
- [x] 1.1 Add `resolve_user_id_by_idp_subject(idp_subject: &str) -> Option<String>` to `PostgresBackend` — queries `users` table by `idp_subject` to return the user's UUID
- [x] 1.2 Add `get_user_roles_for_auth(user_id: &str, tenant_id: &str) -> Vec<RoleIdentifier>` to `PostgresBackend` — queries `user_roles` WHERE `user_id = $1 AND (tenant_id = $2 OR tenant_id = '__root__')`, returns deduplicated role list
- [x] 1.3 Write unit tests for both new methods

## 2. Auth Middleware (auth_middleware.rs)
- [x] 2.1 Add `PostgresBackend` (or a `PgPool`) to `PluginAuthState` so the middleware can access the DB
- [x] 2.2 Make `tenant_context_from_identity` async and add DB role lookup:
  - Resolve `identity.github_login` → `users.id` via `resolve_user_id_by_idp_subject`
  - Call `get_user_roles_for_auth(user_id, identity.tenant_id)` to get roles including `__root__` instance scope
  - Populate `ctx.roles` from the DB results
  - Ignore `X-User-Role` header when auth is enabled
- [x] 2.3 Update `AuthenticationService::call` to await the now-async `tenant_context_from_identity`
- [x] 2.4 Keep `dev_context_from_headers` unchanged (still trusts `X-User-Role` in dev mode)
- [x] 2.5 Write tests for DB-backed role resolution (mock or test DB)

## 3. Handler Context Helper (mod.rs)
- [x] 3.1 Make `authenticated_tenant_context` async and add the same DB role lookup logic
- [x] 3.2 Update all callers of `authenticated_tenant_context` to `.await`
- [x] 3.3 Update `tenant_scoped_context` (already async) to match

## 4. Handler Updates
- [x] 4.1 Update `require_platform_admin` in `tenant_api.rs` to be async and await `authenticated_tenant_context`
- [x] 4.2 Update all callers of `require_platform_admin` in `tenant_api.rs`
- [x] 4.3 Update callers in `user_api.rs`, `knowledge_api.rs`, and any other API modules

## 5. Server Startup
- [x] 5.1 Thread the `PgPool` or `PostgresBackend` into `PluginAuthState` during server construction

## 6. Verification
- [ ] 6.1 `cargo build` passes
- [ ] 6.2 `cargo test --all` passes (or pre-existing failures only)
- [ ] 6.3 Deploy and verify `aeterna tenant create` works for bootstrapped PlatformAdmin
