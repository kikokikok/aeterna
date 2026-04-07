## 1. Config Crate: AdminBootstrapConfig

- [x] 1.1 Add `AdminBootstrapConfig` struct to `config/src/config.rs` with fields: `email: Option<String>`, `provider: String` (default "github"), `provider_subject: Option<String>`
- [x] 1.2 Add `admin_bootstrap: AdminBootstrapConfig` field to the root `Config` struct in `config/src/config.rs`
- [x] 1.3 Add `load_admin_bootstrap_from_env()` function in `config/src/loader.rs` that reads `AETERNA_ADMIN_BOOTSTRAP_EMAIL`, `AETERNA_ADMIN_BOOTSTRAP_PROVIDER`, `AETERNA_ADMIN_BOOTSTRAP_PROVIDER_SUBJECT`
- [x] 1.4 Wire `load_admin_bootstrap_from_env()` into the `load_from_env()` function in `config/src/loader.rs`
- [x] 1.5 Add unit tests for `AdminBootstrapConfig` default values and env loading

## 2. Bootstrap Seeding Logic

- [x] 2.1 Add `seed_platform_admin()` async function in `cli/src/server/bootstrap.rs` that takes `&PostgresBackend` and `&AdminBootstrapConfig`
- [x] 2.2 Implement idempotent upsert of `default` company in `organizational_units` table (type=company, ON CONFLICT DO NOTHING)
- [x] 2.3 Implement idempotent upsert of admin user in `users` table (email, idp_provider, idp_subject, ON CONFLICT (email) DO UPDATE for provider fields)
- [x] 2.4 Implement idempotent insert of membership record linking admin user to default company in `memberships` table (ON CONFLICT DO NOTHING)
- [x] 2.5 Implement idempotent insert of PlatformAdmin role in `user_roles` table (user_id=user_uuid, tenant_id=default, unit_id=company_unit_id, role=PlatformAdmin, ON CONFLICT DO NOTHING)
- [x] 2.6 Wrap all seeding SQL in a single transaction for atomicity
- [x] 2.7 Call `seed_platform_admin()` in `bootstrap()` after `postgres.initialize_schema().await?` (line 48), gated on `config.admin_bootstrap.email.is_some()`
- [x] 2.8 Add tracing logs: info on successful seed, warn on skip due to missing config, error on failure
- [ ] 2.9 Add unit tests for seed logic (mock DB or test with in-memory state)

## 3. Helm Chart: Deployment Template

- [x] 3.1 Add `pluginAuth.defaultTenantId` conditional env var block in `charts/aeterna/templates/aeterna/deployment.yaml` — inject `AETERNA_PLUGIN_AUTH_TENANT` when set
- [x] 3.2 Add `adminBootstrap` conditional env var block in the same deployment template — inject `AETERNA_ADMIN_BOOTSTRAP_EMAIL`, `AETERNA_ADMIN_BOOTSTRAP_PROVIDER`, `AETERNA_ADMIN_BOOTSTRAP_PROVIDER_SUBJECT` when `adminBootstrap.email` is non-empty
- [x] 3.3 Verify template renders correctly with `helm template` for both enabled and disabled bootstrap cases

## 4. Helm Chart: Values Schema

- [x] 4.1 Add `pluginAuth` section to `charts/aeterna/values.yaml` with `defaultTenantId: ""` default
- [x] 4.2 Add `adminBootstrap` section to `charts/aeterna/values.yaml` with defaults: `email: ""`, `provider: "github"`, `providerSubject: ""`
- [x] 4.3 Validate chart lints cleanly with `helm lint charts/aeterna`

## 5. [REDACTED_TENANT] Deployment Values (Private Repo)

- [x] 5.1 Add `adminBootstrap` values to `[REDACTED_PRIVATE_DEPLOYMENT_PATH]/environments/[REDACTED_ENV]/values.yaml` with `email: [REDACTED_EMAIL]`, `provider: github`, `providerSubject: <github-login>`
- [x] 5.2 Add `pluginAuth.defaultTenantId: "default"` to the same [REDACTED_TENANT] values file
- [x] 5.3 Verify no confidential values leak into the public Aeterna repo

## 6. Build and Deploy

- [x] 6.1 Run `cargo build` to verify Rust changes compile
- [x] 6.2 Run `cargo test --all` to verify existing and new tests pass (1 pre-existing doc drift failure in adapters — unrelated)
- [ ] 6.3 Build and push new container image
- [ ] 6.4 Bump chart version if needed, package and publish chart
- [ ] 6.5 `helm upgrade` on [REDACTED_ENV] with new chart and [REDACTED_TENANT] values

## 7. Verification

- [ ] 7.1 Verify pod starts successfully with bootstrap seeding logs visible
- [ ] 7.2 Verify `AETERNA_PLUGIN_AUTH_TENANT` is set in pod env
- [ ] 7.3 Verify plugin auth bootstrap endpoint returns 200 (not 500)
- [ ] 7.4 Verify PlatformAdmin user exists in `users` table via `kubectl exec` psql query
- [ ] 7.5 Verify PlatformAdmin role grant exists in `user_roles` table
- [ ] 7.6 Verify admin appears in `v_user_permissions` view
- [ ] 7.7 Run E2E Postman collection against [REDACTED_ENV]
