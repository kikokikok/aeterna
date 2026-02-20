# codesearch Specification

## Purpose
TBD - created by archiving change add-codesearch-repo-management. Update Purpose after archive.
## Requirements
### Requirement: Request Repository via CLI
The system MUST allow users to request repository indexing via a CLI command.

#### Scenario: Requesting a remote repository
- **WHEN** user executes `aeterna codesearch repo request --url <url> --type remote`
- **THEN** a new request is created in `codesearch_requests` table
- **AND** the state is set to `PENDING` (or `APPROVED` if auto-approval applies)

---

### Requirement: Request Repository via MCP
The system MUST provide an MCP tool named `codesearch_repo_request` for agents to request repository access.

#### Scenario: Agent requests access to a repo
- **WHEN** agent calls `codesearch_repo_request` with repo URL
- **THEN** the system evaluates policies and creates a request
- **AND** returns the request status to the agent

---

### Requirement: Policy-Based Approval (Cedar)
The system MUST evaluate indexing requests against Cedar policies before approval.

#### Scenario: Auto-approval for local repos
- **WHEN** a local repository is requested
- **THEN** the policy evaluator returns `permit`
- **AND** the repository status moves directly to `APPROVED`

---

### Requirement: Incremental Indexing Strategy
The system MUST support three strategies for triggering incremental re-indexing: `hook`, `job`, and `manual`.

#### Scenario: Configure hook strategy
- **WHEN** a repository is added with `--strategy hook`
- **THEN** the system listens for merge webhooks to trigger re-indexing
- **AND** re-indexes only the changed files on merge

#### Scenario: Periodic job strategy
- **WHEN** a repository is added with `--strategy job --interval 15m`
- **THEN** a background job checks for changes every 15 minutes
- **AND** triggers incremental indexing if the remote branch has new commits

---

### Requirement: Usage-Based Auto-Cleanup
The system SHALL automatically remove repository indexes that have not been searched for a configurable duration.

#### Scenario: Cleanup inactive repo
- **WHEN** a repository index hasn't been used for 30 days
- **THEN** the cleanup job deletes the vectors and metadata for that repo
- **AND** logs the action in `codesearch_cleanup_log`

---

### Requirement: GitHub Owner Auto-Detection
The system MUST attempt to detect the owner of a GitHub repository to apply ownership-based policies.

#### Scenario: Detect owner via CODEOWNERS
- **WHEN** a GitHub repo is indexed
- **THEN** the system parses the `CODEOWNERS` file
- **AND** associates the repository with the detected users/teams in `codesearch_repositories`

---

### Requirement: PR Delta Indexing
The system MUST support indexing individual pull request deltas to provide 99%+ faster updates.

#### Scenario: Indexing a PR merge
- **WHEN** a PR is merged into a tracked branch
- **THEN** the system calculates the delta using `git diff` (or GraphQL)
- **AND** updates only the embeddings for changed files

