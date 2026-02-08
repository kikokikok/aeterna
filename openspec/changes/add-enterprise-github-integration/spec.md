# Change: Enterprise GitHub Integration & Platform Readiness

## Why

To support enterprise-scale organizations (e.g., 300+ engineers) that rely on **GitHub** as their source of truth, Aeterna needs deep integration with the GitHub ecosystem.
Currently, admins must manually replicate their GitHub organization structure (teams, repositories, users) into Aeterna. This is inefficient and error-prone.
Additionally, governance policies (who owns what) are often already defined in `CODEOWNERS` files, and Aeterna should respect and enforce these automatically.

## What Changes

### Major Features

#### 1. GitHub App Integration (Identity & Structure)
- **GitHub App Auth**: Support for GitHub App authentication (private key) in `idp-sync`.
- **Org/Team Sync**: Automatically mirror GitHub Organizations and Teams into Aeterna's `organizations` and `teams` tables.
- **User Provisioning**: Auto-create Aeterna users from GitHub identities when they log in or are synced via team membership.
- **SSO**: "Login with GitHub" flow using the App's OAuth capabilities.

#### 2. Repository Discovery & Registration
- **Auto-Discovery**: Listen to `repository.created` webhooks to instantly register new repositories in Aeterna.
- **Backfill Job**: A "Scanner" job to import existing repositories from the GitHub Org, mapping them to the correct Aeterna Team based on ownership.

#### 3. Governance as Code (CODEOWNERS Sync)
- **Policy Generation**: Parse `CODEOWNERS` files from repositories.
- **Cedar Translation**: valid `CODEOWNERS` rules are translated into Aeterna Cedar policies (e.g., `@org/api-team owns /api/*` -> `permit(principal == Team::"api-team", action == Action::"ApproveChange", resource == Project::"api-service")`).
- **Drift Detection**: Alert when Aeterna policies drift from the `CODEOWNERS` source of truth.

#### 4. CI/CD Enforcement CLI
- **New Binary**: `aeterna-ci` (lightweight, minimal dependencies).
- **Pipeline Guardrails**:
    - `aeterna-ci check-policy`: Validates PRs against Cedar policies (e.g., "Dependency allowed?", "Compliance check passed?").
    - `aeterna-ci report-coverage`: Uploads test/security coverage data to Aeterna for holistic views.

#### 5. Observability & Audit Export
- **Audit Streaming**: Stream `referential_audit_log` events to external SIEMs (Datadog, Splunk, SumoLogic) via HTTP Webhooks or Vector.
- **Compliance Reports**: Generate PDF/CSV reports on "Who has access to what" based on the synced state.

### Database Schema Updates

| Table | Description |
|-------|-------------|
| `github_app_config` | Stores App ID, Install ID, Private Key (encrypted) per tenant |
| `github_team_mappings` | Maps GitHub Team Slugs/IDs to Aeterna Team UUIDs |
| `policy_sources` | Tracks where a policy came from (e.g., `source_type='codeowners', source_url='github.com/org/repo/CODEOWNERS'`) |

### New CLI Commands

- `aeterna github install`: Wizard to setup the GitHub App.
- `aeterna github sync`: Manually trigger a full sync (Teams + Repos).
- `aeterna ci`: The entry point for CI/CD operations.

## Impact

- **Onboarding**: "Zero-touch" onboarding for engineers. They just log in with GitHub and inherit all their team permissions.
- **Governance**: Security teams can rely on `CODEOWNERS` (which engineers already understand) to drive Aeterna's rigorous policy engine.
- **Scale**: Supports organizations with thousands of repos and engineers without manual admin toil.

## Success Metrics

- < 5 minutes to onboard a 300-person organization (via Sync).
- 100% of Cedar policies for repo ownership match `CODEOWNERS`.
- CI/CD check latency < 2 seconds.
