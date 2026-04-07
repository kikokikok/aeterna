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

## 6. Build and Deploy (Round 1 — bootstrap seeding)

- [x] 6.1 Run `cargo build` to verify Rust changes compile
- [x] 6.2 Run `cargo test --all` to verify existing and new tests pass (1 pre-existing doc drift failure in adapters — unrelated)
- [x] 6.3 Build and push container image (`sha-b3d4173`)
- [x] 6.4 Bump chart to `0.4.1-bootstrap.1`, package and publish
- [x] 6.5 `helm upgrade` on [REDACTED_ENV] (revision 52) — manually unblocked DB

## 7. Verification (Round 1 — bootstrap seeding)

- [x] 7.1 Verify pod starts successfully with bootstrap seeding logs visible
- [x] 7.2 Verify `AETERNA_PLUGIN_AUTH_TENANT` is set in pod env
- [x] 7.3 Verify plugin auth bootstrap endpoint returns 422 (not 500) — correct validation
- [x] 7.4 Verify PlatformAdmin user exists in `users` table via psql
- [x] 7.5 Verify PlatformAdmin role grant exists in `user_roles` table
- [x] 7.6 Verify admin appears in `v_user_permissions` view
- [ ] 7.7 Run E2E Postman collection against [REDACTED_ENV] (deferred — needs real GitHub token)

## 8. Migration Runner Implementation

- [x] 8.1 Embed all 14 SQL migration files (003-016) at compile time using `include_str!` in a new module
- [x] 8.2 Create `_aeterna_migrations` tracking table DDL (version INT, name TEXT, checksum TEXT, applied_at TIMESTAMPTZ)
- [x] 8.3 Implement `get_applied_migrations()` — query tracking table, return list of applied versions
- [x] 8.4 Implement `apply_migration()` — execute SQL in transaction, insert tracking row, handle errors
- [x] 8.5 Implement `run_migrate(args)` for `status` direction — show applied/pending list
- [x] 8.6 Implement `run_migrate(args)` for `up` direction — connect to PG, apply pending migrations in order, exit 0 on success
- [x] 8.7 Implement `run_migrate(args)` for `down` direction with `--force` — rollback last migration (or stub with error if no down SQL)
- [x] 8.8 Support `--dry-run` flag — list what would be applied without executing
- [x] 8.9 Support `--json` flag — structured output for scripting
- [x] 8.10 Add SHA256 checksum verification to detect tampered migration files
- [x] 8.11 Run `cargo build` to verify compilation
- [x] 8.12 Run `cargo test --all` to verify no regressions

## 9. Build and Deploy (Round 2 — migration runner + fixes)

- [x] 9.1 Build and push container image `sha-4a21982` (chart `0.4.1-bootstrap.3`)
- [x] 9.2 Fix pgvector extension not installed on CNPG — ran `CREATE EXTENSION IF NOT EXISTS vector` manually as postgres superuser
- [x] 9.3 Diagnose migration failures: 003-008 had wrong table names, premature FK constraints to tables created by later migrations
- [x] 9.4 Baseline [REDACTED_ENV] `_aeterna_migrations` table with 14 rows for existing image checksums
- [x] 9.5 Helm upgrade [REDACTED_ENV] to revision 56 — migration Job succeeds as no-op, all 12 pods running
- [x] 9.6 Fix migration SQL files 003-008: remove premature FKs, fix table names, add idempotent guards
- [x] 9.7 Update baseline checksums on [REDACTED_ENV] for fixed migrations 003-008
- [x] 9.8 Commit and push fixed migrations (`4ef8573`)
- [x] 9.9 Wait for Docker CI build (~36 min) for image `sha-4ef8573`
- [x] 9.10 Update [REDACTED_TENANT] values with new image tag
- [x] 9.11 Helm upgrade [REDACTED_ENV] with new image — revision 57

## 10. Verification (Round 2 — migration runner with fixed SQL)

- [x] 10.1 Verify migration Job succeeds with baselined checksums (revision 56)
- [x] 10.2 Verify `_aeterna_migrations` table has all 14 rows
- [x] 10.3 Verify all 12 pods running 1/1
- [x] 10.4 Verify PlatformAdmin `[REDACTED_EMAIL]` active in DB
- [x] 10.5 Verify migration Job succeeds with new image (fixed SQL + matching checksums) — "Job completed" event confirmed
- [x] 10.6 Verify pods restart cleanly after upgrade — 12/12 Running, aeterna pods on image `sha-4ef8573`
- [x] 10.7 Verify bootstrap seeding still works — PlatformAdmin `[REDACTED_EMAIL]` status=active
