## ADDED Requirements

### Requirement: Remote Git Synchronization
The knowledge repository SHALL support synchronization with a remote Git repository so that all Aeterna replicas share a single source of truth.

#### Scenario: Clone on first start
- **WHEN** the repository initializes with a configured remote URL and the local path is empty
- **THEN** the system SHALL clone the remote repository to the local path
- **AND** the system SHALL authenticate using the configured SSH key

#### Scenario: Pull on subsequent start
- **WHEN** the repository initializes with a configured remote URL and the local path contains a valid clone
- **THEN** the system SHALL pull from the remote to synchronize with the latest state

#### Scenario: Pull before read
- **WHEN** any read operation (get, list, search, query) is executed with a remote URL configured
- **THEN** the system SHALL pull from the remote before reading the local filesystem
- **AND** the pull SHALL use fast-forward only merge strategy

#### Scenario: Local-only fallback
- **WHEN** no remote URL is configured (AETERNA_KNOWLEDGE_REPO_URL is empty or unset)
- **THEN** the system SHALL operate in local-only mode with no remote operations
- **AND** behavior SHALL be identical to the current implementation

#### Scenario: SSH key authentication
- **WHEN** authenticating with the remote repository
- **THEN** the system SHALL use the SSH private key content from the AETERNA_KNOWLEDGE_REPO_SSH_KEY environment variable
- **AND** the key SHALL be loaded from memory without writing to disk

### Requirement: Two-Track Write Model
The knowledge repository SHALL route write operations through one of two tracks based on the knowledge layer and status.

#### Scenario: Fast track for Project-layer drafts
- **WHEN** a write operation targets a knowledge entry with layer=Project and status=Draft
- **THEN** the system SHALL commit locally and push directly to the main branch
- **AND** the system SHALL retry up to 3 times on push conflict with pull-rebase-push

#### Scenario: Governance track for status transitions
- **WHEN** a status change operation transitions an entry to a non-Draft status (e.g., Draft to Accepted)
- **THEN** the system SHALL create a governance branch, commit the change, and open a pull request
- **AND** the pull request title SHALL describe the status transition
- **AND** the system SHALL NOT modify the main branch directly

#### Scenario: Governance track for higher-layer writes
- **WHEN** a write operation targets a knowledge entry with layer=Team, Org, or Company
- **THEN** the system SHALL create a governance branch, commit the change, and open a pull request
- **AND** the pull request body SHALL include the entry metadata and change description

#### Scenario: Governance track for promotions
- **WHEN** a knowledge entry is promoted from one layer to a higher layer
- **THEN** the system SHALL create a governance branch, commit the promoted entry at the target layer path, and open a pull request

#### Scenario: Governance track for deletions
- **WHEN** a delete operation is requested for any knowledge entry with a remote URL configured
- **THEN** the system SHALL create a governance branch, commit the file removal, and open a pull request

#### Scenario: Local-only mode bypasses governance
- **WHEN** no remote URL is configured
- **THEN** all write operations SHALL commit locally without governance track routing
- **AND** no branches or pull requests SHALL be created

### Requirement: PR Lifecycle Governance Events
The system SHALL emit GovernanceEvents corresponding to pull request lifecycle transitions.

#### Scenario: PR opened emits RequestCreated
- **WHEN** a pull request is opened for a governance-track write
- **THEN** the system SHALL emit a GovernanceEvent::RequestCreated event
- **AND** the event SHALL include the PR number, title, entry ID, and requesting user

#### Scenario: PR merged emits RequestApproved
- **WHEN** a pull request is merged (via webhook or polling)
- **THEN** the system SHALL emit a GovernanceEvent::RequestApproved event
- **AND** the system SHALL update the local knowledge entry status accordingly
- **AND** the event SHALL include the PR number, merge commit SHA, and approver

#### Scenario: PR closed without merge emits RequestRejected
- **WHEN** a pull request is closed without being merged
- **THEN** the system SHALL emit a GovernanceEvent::RequestRejected event
- **AND** the system SHALL clean up the local branch reference
- **AND** the event SHALL include the PR number and reason

### Requirement: GitHub Webhook Endpoint
The system SHALL expose an HTTP endpoint that receives GitHub webhook notifications for immediate knowledge synchronization.

#### Scenario: Webhook receives PR merge event
- **WHEN** the webhook endpoint receives a pull_request event with action=closed and merged=true
- **THEN** the system SHALL validate the X-Hub-Signature-256 HMAC signature
- **AND** the system SHALL trigger an immediate git pull on the knowledge repository
- **AND** the system SHALL emit a GovernanceEvent::RequestApproved
- **AND** the system SHALL notify the sync bridge of a CommitMismatch

#### Scenario: Webhook receives PR closed event (not merged)
- **WHEN** the webhook endpoint receives a pull_request event with action=closed and merged=false
- **THEN** the system SHALL validate the HMAC signature
- **AND** the system SHALL emit a GovernanceEvent::RequestRejected

#### Scenario: Webhook receives PR opened event
- **WHEN** the webhook endpoint receives a pull_request event with action=opened
- **THEN** the system SHALL validate the HMAC signature
- **AND** the system SHALL emit a GovernanceEvent::RequestCreated

#### Scenario: Webhook with invalid signature
- **WHEN** the webhook endpoint receives a request with an invalid or missing X-Hub-Signature-256
- **THEN** the system SHALL return HTTP 401 Unauthorized
- **AND** the system SHALL NOT process the event

#### Scenario: Webhook disabled
- **WHEN** AETERNA_WEBHOOK_SECRET is not configured
- **THEN** the webhook endpoint SHALL return HTTP 404 Not Found
- **AND** no webhook processing SHALL occur

### Requirement: Git Provider Abstraction
The system SHALL define an abstract GitProvider trait to decouple knowledge governance from a specific Git hosting platform.

#### Scenario: GitHub provider implementation
- **WHEN** the system is configured with a GitHub repository
- **THEN** the system SHALL use the GitHubProvider implementation of the GitProvider trait
- **AND** all branch, PR, and webhook operations SHALL use the octocrab crate

#### Scenario: Provider trait extensibility
- **WHEN** a new Git hosting platform (e.g., GitLab) needs to be supported
- **THEN** a new implementation of the GitProvider trait SHALL be sufficient
- **AND** no changes to the knowledge repository or governance engine SHALL be required

### Requirement: Governance Branch Naming
The system SHALL use a consistent branch naming convention for governance-track operations.

#### Scenario: Branch name for status change
- **WHEN** creating a governance branch for a status change operation
- **THEN** the branch name SHALL follow the pattern governance/{verb}-{slug}-{yyyymmdd}
- **AND** the verb SHALL match the operation (accept, deprecate, reject)

#### Scenario: Branch name for promotion
- **WHEN** creating a governance branch for a layer promotion
- **THEN** the branch name SHALL follow the pattern governance/promote-{slug}-{yyyymmdd}

#### Scenario: Branch name uniqueness
- **WHEN** a branch name conflicts with an existing branch
- **THEN** the system SHALL append a numeric suffix to ensure uniqueness

### Requirement: PR Idempotency
The system SHALL ensure that duplicate governance operations do not create duplicate pull requests.

#### Scenario: Duplicate PR prevention
- **WHEN** a governance-track write is initiated for an operation that already has an open PR
- **THEN** the system SHALL return the existing PR information without creating a new one

#### Scenario: Closed PR allows new creation
- **WHEN** a governance-track write is initiated for an operation whose previous PR was merged or closed
- **THEN** the system SHALL create a new PR with a new date suffix

### Requirement: PR-Backed Proposal Storage
The system SHALL replace the in-memory knowledge proposal storage with a PR-backed implementation for persistence across pod restarts.

#### Scenario: Propose creates governance branch
- **WHEN** a knowledge proposal is submitted via MCP tools
- **THEN** the system SHALL create a governance branch and commit the proposed entry
- **AND** the proposal SHALL persist across pod restarts via the remote repository

#### Scenario: Submit opens pull request
- **WHEN** a proposed knowledge entry is submitted for review
- **THEN** the system SHALL open a pull request from the governance branch to main
- **AND** the PR body SHALL include the proposal metadata

#### Scenario: List pending reads open PRs
- **WHEN** listing pending proposals
- **THEN** the system SHALL query open pull requests with the governance/ branch prefix
- **AND** the system SHALL return proposal metadata extracted from PR details

## MODIFIED Requirements

### Requirement: Git-based Versioning
Every change to the repository SHALL result in an immutable Git commit with a full audit trail. For remote-enabled repositories, commits on the main branch SHALL be synchronized with the remote, and governance-track changes SHALL be committed to feature branches.

#### Scenario: Trace item history
- **WHEN** an item is updated
- **THEN** the system SHALL allow retrieving the full commit history for that specific item

#### Scenario: Fast-track commit pushed to remote
- **WHEN** a fast-track write (Project-layer Draft) is committed
- **THEN** the commit SHALL be pushed to the remote main branch
- **AND** the push SHALL use fast-forward only

#### Scenario: Governance-track commit on branch
- **WHEN** a governance-track write is committed
- **THEN** the commit SHALL be created on a governance branch, not on main
- **AND** the commit SHALL only reach main after PR merge

### Requirement: Lifecycle Management
Knowledge items SHALL follow a defined lifecycle (Draft -> Proposed -> Accepted -> Deprecated/Superseded). Status transitions beyond Draft SHALL require governance review when a remote repository is configured.

#### Scenario: Supersede an item
- **WHEN** a new item supersedes an existing one
- **THEN** the status of the old item SHALL be updated to 'superseded' and link to the new item

#### Scenario: Status transition requires governance review
- **WHEN** a status transition from Draft to Proposed or Accepted is requested with a remote URL configured
- **THEN** the system SHALL route the transition through the governance track (branch + PR)
- **AND** the status SHALL only be updated on main after the PR is merged

#### Scenario: Status transition in local-only mode
- **WHEN** a status transition is requested without a remote URL configured
- **THEN** the system SHALL update the status directly via local commit

### Requirement: Status Update Operation
The system SHALL provide a method to update knowledge item status with governance approval workflows. When a remote repository is configured, status updates to non-Draft states SHALL be routed through the governance track.

#### Scenario: Update status with tenant context and authorization
- **WHEN** updating knowledge item status with TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL verify user has appropriate role (TechLead, Architect, Admin)
- **AND** system SHALL enforce governance approval workflow
- **AND** system SHALL create Git commit with status change
- **AND** system SHALL emit governance event (KnowledgeApproved/KnowledgeRejected)

#### Scenario: Update status without required role
- **WHEN** updating knowledge item status with insufficient role permissions
- **THEN** system SHALL return INSUFFICIENT_PERMISSIONS error
- **AND** status SHALL NOT be changed

#### Scenario: Update status routed through governance track
- **WHEN** updating status to Accepted with a remote URL configured
- **THEN** the system SHALL create a governance branch with the status change
- **AND** the system SHALL open a PR for review
- **AND** the status SHALL only be updated on main after PR merge

### Requirement: Governance Event Emission
Knowledge operations SHALL emit governance events for audit and real-time monitoring. PR lifecycle transitions SHALL be the primary source of governance events when a remote repository is configured.

#### Scenario: Emit event on knowledge proposal
- **WHEN** a knowledge item is proposed
- **THEN** system SHALL emit a `KnowledgeProposed` event with tenant context
- **AND** event SHALL be published to Redis Streams for real-time consumption

#### Scenario: Emit event on knowledge approval
- **WHEN** a knowledge item is approved
- **THEN** system SHALL emit a `KnowledgeApproved` event with tenant context
- **AND** event SHALL include approver identity and timestamp

#### Scenario: Emit event on PR merge
- **WHEN** a governance PR is merged (detected via webhook or polling)
- **THEN** system SHALL emit a `GovernanceEvent::RequestApproved` event
- **AND** the event SHALL include the PR number, merge commit SHA, and entry metadata

#### Scenario: Emit event on PR rejection
- **WHEN** a governance PR is closed without merging
- **THEN** system SHALL emit a `GovernanceEvent::RequestRejected` event
- **AND** the event SHALL include the PR number and the entry that was rejected
