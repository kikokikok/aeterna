## Tasks: add-per-tenant-idp-sync

### T1 — Add GitHub config key constants to `memory::provider_registry::config_keys`
- Add `pub mod github_config_keys` inside `memory/src/provider_registry.rs`
- Keys: `ORG_NAME`, `APP_ID`, `INSTALLATION_ID`, `TEAM_FILTER`, `SYNC_REPOS_AS_PROJECTS`, `APP_PEM` (secret)
- Mirror the pattern of existing `config_keys` constants in that module
- **Done when**: constants compile and are pub-accessible from `memory::provider_registry::github_config_keys`

### T2 — Add `build_github_config_for_tenant()` in `admin_sync.rs`
- New async fn: `pub(crate) async fn build_github_config_for_tenant(tenant_id: &Uuid, state: &Arc<AppState>) -> anyhow::Result<GitHubConfig>`
- Resolution order: (1) tenant config fields + secret, (2) env var fallback via `build_github_config_from_env()`
- Uses `state.tenant_config_provider.get_config()` and `state.tenant_config_provider.get_secret_value()`
- **Done when**: unit tests cover: config-from-tenant, env-fallback, error-when-both-absent

### T3 — Per-tenant sync lock (replace global `AtomicBool`)
- Replace `static SYNC_IN_PROGRESS: AtomicBool` with `static SYNC_LOCKS: LazyLock<DashMap<String, ()>>`
- Keep a separate global `AtomicBool` or `Mutex` for the fan-out endpoint to prevent overlapping fan-outs
- **Done when**: existing test `sync_guard_prevents_concurrent_execution` still passes; new test covers per-tenant lock

### T4 — Add `POST /admin/tenants/{tenant}/sync/github` route
- New handler `sync_tenant_github` in `admin_sync.rs`
- Auth: PlatformAdmin OR TenantAdmin for that tenant (use existing `authenticated_tenant_context` + role check)
- Resolves tenant by name/UUID using existing `resolve_tenant_record()` pattern from `tenant_api.rs`
- Calls `build_github_config_for_tenant()` → `run_sync_for_tenant(tenant_id, github_config, state)`
- Returns `SyncReport` on success, 409 if locked, 400 if no config
- Wire into `admin_sync::router()` and `cli/src/server/router.rs`
- **Done when**: integration test covers 200 success, 409 concurrent, 403 wrong tenant

### T5 — Update `POST /admin/sync/github` (fan-out)
- Enumerate tenants with `github.org_name` present via `state.tenant_config_provider.list_configs()`
- For each: call `run_sync_for_tenant()` — collect results into `Vec<TenantSyncResult>`
- Fall back to env-var single-tenant sync if no tenants have per-tenant config (backward compat)
- Return `{ results: [...] }` with per-tenant status
- **Done when**: unit test covers fan-out result shape, partial failure

### T6 — Update Helm `cronjob-github-sync.yaml`
- Change command from `aeterna admin sync github` (which reads env vars) to an HTTP POST to the fan-out endpoint
- Use projected service account token for auth (or keep env-var path as fallback when `githubOrgSync.perTenantMode: false`)
- Add `githubOrgSync.perTenantMode` values flag (default: false for backward compat)
- **Done when**: `helm lint charts/aeterna` passes; both old and new mode render correctly

### T7 — Update `openspec/specs/github-org-sync/spec.md`
- Add requirements for credential resolution order
- Add scenario for per-tenant config taking precedence over env vars
- **Done when**: spec file updated and consistent with implementation
