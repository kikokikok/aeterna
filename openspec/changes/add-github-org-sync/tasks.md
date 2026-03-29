## 1. Configuration & Types

- [x] 1.1 Add `GitHubConfig` struct to `idp-sync/src/config.rs` with fields: `org_name`, `app_id`, `installation_id`, `private_key_pem`, `team_filter`, `sync_repos_as_projects`
- [x] 1.2 Add `GitHub(GitHubConfig)` variant to `IdpProvider` enum in `idp-sync/src/config.rs`
- [x] 1.3 Add `GitHubTeam` and `GitHubNestedTeam` variants to `GroupType` enum in `idp-sync/src/okta.rs`
- [x] 1.4 Update `IdpSyncConfig::default()` and `get_provider_name()` in sync.rs to handle the new GitHub variant

## 2. GitHub App Authentication

- [x] 2.1 Create `idp-sync/src/github.rs` with `GitHubClient` struct holding `octocrab` client, credentials, and `CachedToken`
- [x] 2.2 Implement `mint_installation_token()` using JWT signing with `jsonwebtoken` crate (mirror `knowledge::git_provider` pattern)
- [x] 2.3 Implement `ensure_valid_token()` that checks cache expiry and refreshes within 5-minute window
- [x] 2.4 Implement `GitHubClient::new(config: GitHubConfig)` constructor that mints initial token and validates PEM key

## 3. IdpClient Trait Implementation

- [x] 3.1 Implement `list_users()` → `GET /orgs/{org}/members` with pagination, mapping GitHub members to `IdpUser` with `idp_provider: "github"` and org role detection
- [x] 3.2 Implement `list_groups()` → `GET /orgs/{org}/teams` with pagination, mapping to `IdpGroup` with parent info encoded in description field as `"parent:<slug>"`
- [x] 3.3 Implement `get_group_members(team_slug)` → `GET /orgs/{org}/teams/{slug}/members` with pagination, including team role (maintainer/member) mapping
- [x] 3.4 Implement `get_user(user_id)` → `GET /users/{login}` mapping to `IdpUser`
- [x] 3.5 Register `github` module in `idp-sync/src/lib.rs` exports

## 4. Hierarchy Mapper

- [x] 4.1 Create `GitHubHierarchyMapper` in `idp-sync/src/github.rs` that takes a list of `IdpGroup` and builds the Company → Org → Team tree
- [x] 4.2 Implement `create_hierarchy()` that creates Company unit from org name, sorts teams by parent relationship, creates Org units for top-level teams and Team units for nested teams
- [x] 4.3 Implement `store_group_to_team_mappings()` to persist team-slug → unit-id mappings in `idp_group_mappings` table
- [x] 4.4 Implement idempotent upsert logic: match existing units by GitHub team slug, update name/parent if changed, skip if identical

## 5. Admin Sync API Endpoint

- [x] 5.1 Add `POST /api/v1/admin/sync/github` route to `cli/src/server/router.rs`
- [x] 5.2 Implement `handle_github_sync` handler: validate admin role, construct `GitHubClient` from `AppState` config, run two-phase sync
- [x] 5.3 Add sync-in-progress guard (AtomicBool or Mutex) to prevent concurrent syncs, return 409 if already running
- [x] 5.4 Return `SyncReport` JSON response with created/updated/deactivated counts

## 6. Webhook Extension

- [x] 6.1 Add `organization` event handler to `cli/src/server/webhooks.rs` for `member_added` and `member_removed` actions
- [x] 6.2 Add `team` event handler for `created`, `deleted`, and `edited` (parent change) actions
- [x] 6.3 Add `membership` event handler for `added` and `removed` actions (team membership changes)
- [x] 6.4 Implement role recalculation on membership webhook events (recompute highest role across all memberships)

## 7. CLI Command

- [x] 7.1 Add `sync github` subcommand to `cli/src/commands/admin.rs` that calls `POST /api/v1/admin/sync/github` via the server API
- [x] 7.2 Add `--dry-run` flag support that passes through to the sync endpoint
- [x] 7.3 Display `SyncReport` output in a formatted table (matching existing CLI output patterns)

## 8. Integration Tests

- [x] 8.1 Add unit tests for `GitHubClient` token minting and refresh logic using mocked HTTP responses
- [x] 8.2 Add unit tests for `GitHubHierarchyMapper` with various team nesting scenarios (flat, 2-level, 3-level)
- [x] 8.3 Add unit tests for role mapping (org owner → Admin, team maintainer → TechLead, member → Developer)
- [x] 8.4 Add integration test for full sync flow using testcontainers (PostgreSQL) with mocked GitHub API
- [x] 8.5 Add integration test for webhook event processing (organization, team, membership events)

## 9. Deployment Configuration

- [x] 9.1 Add `aeterna.github.orgSync` section to Helm values schema (`enabled`, `orgName`, `appId`, `installationId`, `pemSecretName`)
- [x] 9.2 Update Helm deployment template to inject GitHub App env vars and mount PEM secret when orgSync is enabled
- [x] 9.3 Update `environments/ci-dev-04/values.yaml` in `aeterna-kyriba-deployment` with `kyriba-eng` org sync config

## 10. E2E Testing & Deployment

- [ ] 10.1 Update GitHub App `aeterna-knowledge-bot` permissions: add Organization Members (Read), Organization Administration (Read)
- [ ] 10.2 Subscribe GitHub App to `organization`, `team`, and `membership` webhook events
- [x] 10.3 Add E2E Newman test for `POST /api/v1/admin/sync/github` endpoint (trigger sync, verify SyncReport response)
- [ ] 10.4 Build, deploy, and verify full sync against live `kyriba-eng` org on ci-dev-04
