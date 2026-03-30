## Context

Aeterna's `idp-sync` crate provides a pluggable identity provider synchronization framework with two existing providers (Okta, Azure AD). The architecture is clean: an `IdpClient` async trait defines the contract (`list_users`, `list_groups`, `get_group_members`, `get_user`), and `IdpSyncService` orchestrates the full sync cycle (fetch → diff → create/update/deactivate).

The project already has a working GitHub App (`[REDACTED_APP_NAME]`, app ID [REDACTED_APP_ID], installation ID [REDACTED_INSTALLATION_ID]) with certificate-based authentication for the knowledge repository governance track. The `knowledge` crate's `GitHubProvider` implements JWT → installation token minting via `octocrab` + `jsonwebtoken`, with a `CachedToken` pattern that refreshes 5 minutes before expiry.

GitHub's organization model differs from flat IdP group models: teams can be nested (parent → child), and membership roles are explicit (`admin`, `member`, `maintainer`). The sync must map this tree structure into Aeterna's 4-level hierarchy: Company → Organization → Team → Project.

Stakeholders: Platform engineering (org owners), team leads (team membership accuracy), security (access audit trail).

## Goals / Non-Goals

**Goals:**
- Implement a `GitHubClient` that satisfies the existing `IdpClient` trait
- Map GitHub Org → Company, top-level teams → Organization, child teams → Team, members → Users
- Reuse the existing GitHub App authentication pattern from `knowledge::git_provider`
- Add `POST /api/v1/admin/sync/github` for on-demand sync triggering
- Extend the existing GitHub webhook handler to process `organization` and `team` events
- Support both full sync (periodic/on-demand) and incremental sync (webhook-driven)
- Pass through the existing `IdpSyncService` sync engine without modification

**Non-Goals:**
- Syncing GitHub repositories to Projects (future enhancement)
- Supporting multiple GitHub orgs per tenant simultaneously
- Replacing or modifying the existing Okta/Azure AD providers
- Implementing a standalone scheduler — reuse `SyncScheduler`
- Bidirectional sync (Aeterna → GitHub) — read-only from GitHub
- Modifying the `IdpClient` trait signature

## Decisions

### 1. Reuse `IdpClient` trait as-is, with augmented `IdpGroup` semantics

**Decision:** Implement `IdpClient` for `GitHubClient` without modifying the trait. GitHub teams map to `IdpGroup` with a naming convention to encode hierarchy depth:
- Top-level teams (no parent): `group_type = GroupType::OktaGroup` (reuse existing enum, means "primary group")
- Child teams: `group_type = GroupType::AppGroup` (reuse existing enum, means "sub-group")
- The `IdpGroup.id` stores the GitHub team slug (stable, URL-safe)
- The `IdpGroup.description` stores parent team slug (if nested) as `"parent:<parent-slug>"`

**Alternatives considered:**
- *Extend `IdpGroup` with a `parent_id` field* — Would require modifying the trait and all existing providers. Rejected: breaks backward compat.
- *Add a separate `list_team_hierarchy()` method to `IdpClient`* — Over-engineers for a single provider. Rejected: violates non-goal of trait modification.

**Rationale:** The `IdpSyncService.sync_groups_and_memberships()` already iterates groups and looks up team mappings via `get_group_to_team_mapping()`. The GitHub provider's `list_groups()` returns flattened teams with parent info encoded in description. A new `GitHubHierarchyMapper` (outside the trait) reads the parent info to create the correct Company → Org → Team hierarchy before the standard sync runs.

### 2. GitHub App authentication via shared token minting utility

**Decision:** Extract the JWT → installation token minting logic from `knowledge::git_provider::GitHubProvider::mint_installation_token()` into a shared `github-auth` utility module in the `idp-sync` crate. Both crates depend on `octocrab` + `jsonwebtoken` already.

**Pattern:**
```
GitHubClient {
    octocrab: Arc<Mutex<Octocrab>>,
    credentials: GitHubAppCredentials,
    token_cache: Arc<Mutex<Option<CachedToken>>>,
}
```

Token lifecycle:
1. On construction: mint installation token via JWT, build `Octocrab` with `personal_token()`
2. Before each API call: check `token_cache.expires_at` — if within 5 minutes, re-mint
3. On re-mint: rebuild `Octocrab` instance (same as `knowledge` crate pattern)

**Alternatives considered:**
- *Share the `knowledge::git_provider::GitHubProvider` directly* — Circular dependency (knowledge depends on mk_core, idp-sync is independent). Rejected.
- *Use octocrab's built-in App auth* — Produces opaque errors (discovered in governance track debugging). Rejected: same issue that caused PR #7 failures.
- *Create a shared `github-auth` crate* — Clean but over-engineers for 2 consumers. Can refactor later. Rejected for now.

**Rationale:** Copy the proven minting pattern. The duplication is minimal (~40 lines) and avoids adding a workspace crate for a single function.

### 3. Two-phase sync: hierarchy creation then membership sync

**Decision:** The GitHub sync runs in two phases:

**Phase 1 — Hierarchy creation (new, GitHub-specific):**
1. Fetch org info via `GET /orgs/{org}` → create Company unit if not exists
2. Fetch all teams via `GET /orgs/{org}/teams` with pagination → sort by parent relationship
3. Create top-level teams as Organization units, child teams as Team units
4. Store team-slug → unit-id mappings in `idp_group_mappings` table

**Phase 2 — Standard IdpSyncService flow:**
1. `list_users()` → `GET /orgs/{org}/members` with pagination → map to `IdpUser`
2. `list_groups()` → return cached teams from Phase 1 → map to `IdpGroup`
3. `get_group_members(team_slug)` → `GET /orgs/{org}/teams/{slug}/members` → map to `IdpUser`
4. Standard `sync_users()` + `sync_groups_and_memberships()` handle the rest

**Rationale:** Phase 1 is necessary because the standard `IdpSyncService` assumes groups already have team mappings. GitHub's nested team structure requires creating the organizational hierarchy first.

### 4. Admin API endpoint on existing router

**Decision:** Add `POST /api/v1/admin/sync/github` to the existing Axum router in `cli/src/server/router.rs`. The handler:
1. Validates admin role via existing auth middleware
2. Constructs `GitHubClient` from `AppState` config
3. Runs Phase 1 (hierarchy) then Phase 2 (standard sync) 
4. Returns `SyncReport` as JSON

**Alternatives considered:**
- *Separate microservice* — Unnecessary complexity for a periodic job. Rejected.
- *CLI-only trigger* — No programmatic access. Rejected.

### 5. Webhook handler extension for incremental sync

**Decision:** Extend `cli/src/server/webhooks.rs` (the existing GitHub webhook handler) to process these additional event types alongside the current `pull_request` events:

| GitHub Event | Action | Sync Behavior |
|---|---|---|
| `organization` | `member_added` | Create/activate user |
| `organization` | `member_removed` | Deactivate user |
| `team` | `created` | Create Org or Team unit |
| `team` | `deleted` | Deactivate unit (soft delete) |
| `team` | `edited` (parent changed) | Reparent unit in hierarchy |
| `membership` | `added` | Add team membership |
| `membership` | `removed` | Remove team membership |

The existing `X-Hub-Signature-256` verification applies to all events on the same webhook endpoint.

**Rationale:** Incremental sync avoids polling and provides near-real-time hierarchy updates. The GitHub App already receives webhooks for `pull_request` events — adding org/team events is a permissions change, not an architecture change.

### 6. GitHub role → Aeterna role mapping

**Decision:**

| GitHub Role | Aeterna Role | Rationale |
|---|---|---|
| Org owner | Admin | Full system access |
| Org member | Developer | Standard access |
| Team maintainer | TechLead | Team management capabilities |
| Team member | Developer | Standard team access |

A user's effective role is the highest role across all their memberships (org-level owner > team-level maintainer > member).

### 7. `GitHubConfig` variant in `IdpProvider` enum

**Decision:** Add to `idp-sync/src/config.rs`:

```rust
pub enum IdpProvider {
    Okta(OktaConfig),
    AzureAd(AzureAdConfig),
    GitHub(GitHubConfig),
}

pub struct GitHubConfig {
    pub org_name: String,
    pub app_id: u64,
    pub installation_id: u64,
    pub private_key_pem: String,  // PEM content or path
    pub team_filter: Option<String>,  // regex to filter team names
    pub sync_repos_as_projects: bool,  // future: map repos → Projects
}
```

## Risks / Trade-offs

- **GitHub API rate limits** (5000 req/hr for installation tokens) → Mitigation: batch fetches with pagination, cache team lists across phases, implement exponential backoff via existing `RetryConfig`
- **Team reparenting race condition** — webhook `team.edited` arrives while full sync is running → Mitigation: full sync acquires advisory lock; webhook sync is idempotent (upsert semantics)
- **Stale hierarchy on App permission change** — if GitHub App loses org:read permission, sync silently fails → Mitigation: health check endpoint validates permissions on startup, log warnings on 403
- **Large org pagination** — orgs with 500+ teams need careful pagination → Mitigation: `per_page=100` with link-header based pagination (octocrab handles this natively)
- **Token duplication** — same JWT minting logic exists in `knowledge` and `idp-sync` → Mitigation: accept duplication now, extract shared crate in future cleanup pass
- **Group type enum reuse** — `GroupType::OktaGroup`/`AppGroup` used for non-Okta semantics → Mitigation: clear documentation; consider adding `GitHubTeam`/`GitHubNestedTeam` variants to enum (low risk, additive change)

## Migration Plan

1. **GitHub App permissions update**: Add `Organization: Members (Read)`, `Organization: Administration (Read)` to the GitHub App settings. Reinstall on your org.
2. **Helm values update**: Add `github_org_sync` section to your deployment values with org_name, app_id, installation_id, and reference to existing PEM secret.
3. **Deploy**: Standard helm upgrade — new code is additive, no breaking changes.
4. **Initial sync**: Trigger `POST /api/v1/admin/sync/github` manually to verify mapping.
5. **Enable webhook events**: In GitHub App settings, subscribe to `Organization`, `Team`, and `Membership` events.
6. **Rollback**: Remove GitHub config from values, redeploy. No data migration needed — hierarchy units are additive.

## Open Questions

- Should repository → Project mapping be included in v1 or deferred? (Proposal says "optionally" — leaning toward deferring to keep scope tight)
- Should the GitHub provider support GitHub Enterprise Server (GHES) URLs in addition to github.com? (Low effort: make base URL configurable in `GitHubConfig`)
- Should `GroupType` enum get explicit `GitHubTeam`/`GitHubNestedTeam` variants, or keep reusing existing Okta/App variants with documentation?
