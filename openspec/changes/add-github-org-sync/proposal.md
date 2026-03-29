## Why

Aeterna's organizational hierarchy (Company → Organization → Team → Project) is manually managed today. The `idp-sync` crate supports Okta and Azure AD for user/group synchronization, but there is no GitHub provider. Since the project already uses a GitHub App (`aeterna-knowledge-bot`) with certificate-based authentication for the knowledge repository, extending it to also sync the GitHub Organization's teams and members into Aeterna's hierarchy is a natural fit. This eliminates manual org setup, keeps the hierarchy in sync with the actual engineering structure, and leverages existing infrastructure.

## What Changes

- Add a **GitHub provider** to the `idp-sync` crate implementing the existing `IdpClient` trait, using GitHub App installation tokens via `octocrab`
- Map GitHub Organization structure to Aeterna's 4-level hierarchy:
  - **GitHub Org** → Company (tenant root)
  - **Top-level GitHub Teams** (no parent) → Organization (business unit)
  - **Child GitHub Teams** (nested under a parent) → Team (working group)
  - **GitHub Org Members** → Users with appropriate roles (admin → Admin, member → Developer)
  - **GitHub Repositories** → optionally mapped to Projects under their team
- Add a `GitHubConfig` variant to the `IdpProvider` enum with app_id, installation_id, and private_key_path
- Add a **REST API endpoint** `POST /api/v1/admin/sync/github` to trigger on-demand GitHub org sync from the deployed server
- Add a **CLI command** `aeterna admin sync github` to trigger sync via the API
- Reuse the existing `SyncReport` structure for reporting sync results
- Add **webhook handler** for GitHub `organization` and `team` events to trigger incremental sync

## Capabilities

### New Capabilities
- `github-org-sync`: GitHub Organization synchronization — maps GitHub Org/Teams/Members to Aeterna's Company/Org/Team/User hierarchy using GitHub App authentication

### Modified Capabilities
- `multi-tenant-governance`: Add requirement for IdP-synced hierarchy initialization — the hierarchy MUST be bootstrappable from an external identity provider (GitHub, Okta, Azure AD)
- `deployment`: Add requirement for GitHub App configuration in Helm values when GitHub org sync is enabled

## Impact

- **Code**: `idp-sync/src/github.rs` (new), `idp-sync/src/config.rs` (new variant), `cli/src/server/router.rs` (new admin route), `cli/src/server/webhooks.rs` (new org/team event handlers), `cli/src/commands/admin.rs` (new sync subcommand)
- **Dependencies**: `octocrab` (already in workspace), `jsonwebtoken` (for JWT signing — already in workspace)
- **Config**: New `idp_sync.github` section in Helm values, reuse of `aeterna-github-app-pem` K8s secret
- **API**: New `POST /api/v1/admin/sync/github` endpoint
- **Webhook**: Extended GitHub webhook handler to process `organization` and `team` events alongside existing `pull_request` events
