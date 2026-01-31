# Tool Interface Spec Delta: UX-First Architecture

## ADDED Requirements

### Requirement: Policy Draft Tool

The system SHALL provide an `aeterna_policy_draft` tool that translates natural language policy intent into Cedar policy drafts without requiring users to know Cedar syntax.

#### Scenario: Create policy from natural language
- **WHEN** user provides intent "Block MySQL in this project" with scope "project" and severity "block"
- **THEN** system generates a valid Cedar policy draft
- **AND** returns a human-readable summary of the policy effect
- **AND** returns validation results (syntax, schema, conflicts)
- **AND** returns suggested next steps

#### Scenario: Handle ambiguous intent
- **WHEN** user provides ambiguous intent like "no bad dependencies"
- **THEN** system requests clarification with specific questions
- **AND** suggests possible interpretations

### Requirement: Policy Validate Tool

The system SHALL provide an `aeterna_policy_validate` tool that validates Cedar policy syntax and semantics without LLM involvement.

#### Scenario: Validate correct Cedar policy
- **WHEN** user submits valid Cedar policy text
- **THEN** system returns validation success
- **AND** reports any warnings (non-blocking issues)

#### Scenario: Validate incorrect Cedar policy
- **WHEN** user submits invalid Cedar policy text
- **THEN** system returns validation failure
- **AND** provides specific error messages with line numbers
- **AND** suggests corrections where possible

### Requirement: Policy Propose Tool

The system SHALL provide an `aeterna_policy_propose` tool that submits validated policy drafts into the governance approval workflow.

#### Scenario: Submit policy for approval
- **WHEN** user submits a validated draft with justification
- **THEN** system creates a proposal record
- **AND** notifies required approvers
- **AND** returns proposal ID and status
- **AND** returns approval requirements and expiration

#### Scenario: Reject invalid draft submission
- **WHEN** user submits an unvalidated or invalid draft
- **THEN** system rejects the submission
- **AND** provides validation errors

### Requirement: Policy List Tool

The system SHALL provide an `aeterna_policy_list` tool that lists active policies with human-readable summaries.

#### Scenario: List policies for current scope
- **WHEN** user requests policy list with scope "project" and include_inherited=true
- **THEN** system returns all active policies affecting the project
- **AND** indicates which policies are inherited from higher scopes
- **AND** provides natural language summary for each policy

#### Scenario: Filter policies by severity
- **WHEN** user requests policy list filtered by severity "block"
- **THEN** system returns only blocking policies

### Requirement: Policy Explain Tool

The system SHALL provide an `aeterna_policy_explain` tool that translates Cedar policies into natural language explanations.

#### Scenario: Explain policy in plain English
- **WHEN** user requests explanation for policy "security-baseline"
- **THEN** system returns summary of policy purpose
- **AND** explains each rule in natural language
- **AND** describes the impact of violations
- **AND** shows scope and applicability

### Requirement: Policy Simulate Tool

The system SHALL provide an `aeterna_policy_simulate` tool that tests policies against scenarios without applying them.

#### Scenario: Simulate against current project
- **WHEN** user simulates policy against live project state
- **THEN** system evaluates policy against actual project dependencies and files
- **AND** returns pass/fail outcome
- **AND** lists specific violations if any

#### Scenario: Simulate against hypothetical scenario
- **WHEN** user provides synthetic scenario with specific dependencies
- **THEN** system evaluates policy against the hypothetical context
- **AND** returns what would happen if policy were active

### Requirement: Governance Configure Tool

The system SHALL provide an `aeterna_governance_configure` tool for configuring meta-governance rules at each scope.

#### Scenario: Configure policy approval rules
- **WHEN** admin configures governance for org scope
- **THEN** system accepts approval requirements (required approvers, allowed roles, review period)
- **AND** validates configuration against inherited rules
- **AND** reports any conflicts with parent scope policies

#### Scenario: Configure memory promotion rules
- **WHEN** admin configures memory promotion thresholds
- **THEN** system accepts auto-promote threshold and approval requirements
- **AND** applies rules to future promotion requests

### Requirement: Governance Roles Tool

The system SHALL provide an `aeterna_governance_roles` tool for managing role assignments within scopes.

#### Scenario: List roles for scope
- **WHEN** user requests role list for team scope
- **THEN** system returns all users and their roles in that scope
- **AND** shows who granted each role and when

#### Scenario: Assign role to user
- **WHEN** authorized user assigns role "architect" to user in org scope
- **THEN** system creates role assignment
- **AND** validates the assigner has permission to grant the role
- **AND** returns confirmation with effective permissions

#### Scenario: Revoke role from user
- **WHEN** authorized user revokes role from user
- **THEN** system removes role assignment
- **AND** logs the revocation in audit trail

### Requirement: Governance Approve Tool

The system SHALL provide an `aeterna_governance_approve` tool for approving governance proposals.

#### Scenario: Approve policy proposal
- **WHEN** authorized approver approves policy proposal with comment
- **THEN** system records approval
- **AND** activates policy if approval requirements met
- **AND** logs approval in audit trail

#### Scenario: Reject unauthorized approval
- **WHEN** non-authorized user attempts to approve proposal
- **THEN** system rejects the action
- **AND** explains required permissions

### Requirement: Governance Reject Tool

The system SHALL provide an `aeterna_governance_reject` tool for rejecting governance proposals.

#### Scenario: Reject policy proposal with reason
- **WHEN** authorized approver rejects proposal with reason
- **THEN** system records rejection
- **AND** notifies proposer of rejection with reason
- **AND** offers option for revision

### Requirement: Governance Audit Tool

The system SHALL provide an `aeterna_governance_audit` tool for viewing governance activity and audit trail.

#### Scenario: Query audit log by time range
- **WHEN** user queries audit log for scope with date range
- **THEN** system returns all governance events in range
- **AND** includes actor, timestamp, action, and details for each event

#### Scenario: Export audit log for compliance
- **WHEN** admin exports audit log in CSV format
- **THEN** system generates compliant export with all required fields
- **AND** includes before/after state for changes

### Requirement: Organization Init Tool

The system SHALL provide an `aeterna_org_init` tool for initializing company or organization hierarchy with sensible defaults.

#### Scenario: Initialize new company
- **WHEN** admin initializes company with name, admin email, and governance level
- **THEN** system creates company entity
- **AND** creates default organization
- **AND** assigns admin role to provided email
- **AND** applies default governance policies based on selected level
- **AND** returns next steps for setup completion

#### Scenario: Initialize organization within company
- **WHEN** admin creates organization within existing company
- **THEN** system creates org entity
- **AND** inherits company governance rules
- **AND** allows customization of org-specific rules

### Requirement: Team Create Tool

The system SHALL provide an `aeterna_team_create` tool for creating teams within organizations.

#### Scenario: Create team with lead
- **WHEN** admin creates team with name, org, and lead email
- **THEN** system creates team entity
- **AND** assigns tech_lead role to specified lead
- **AND** inherits org governance rules
- **AND** returns inherited policies count

### Requirement: Project Init Tool

The system SHALL provide an `aeterna_project_init` tool for initializing Aeterna in a project with automatic context detection.

#### Scenario: Initialize project with auto-detection
- **WHEN** user runs project init in git repository
- **THEN** system detects git remote URL and user email
- **AND** matches to existing team/org based on remote patterns
- **AND** creates .aeterna/context.toml with detected context
- **AND** returns inherited policies and knowledge items count

#### Scenario: Initialize project with explicit team
- **WHEN** user runs project init with explicit team parameter
- **THEN** system creates project with specified team association
- **AND** validates user has access to the team

### Requirement: User Register Tool

The system SHALL provide an `aeterna_user_register` tool for registering user identities.

#### Scenario: Register new user with team membership
- **WHEN** admin registers user with email, teams, and role
- **THEN** system creates user entity
- **AND** establishes team memberships
- **AND** returns effective capabilities based on role

### Requirement: Agent Register Tool

The system SHALL provide an `aeterna_agent_register` tool for registering AI agents with delegation chains.

#### Scenario: Register agent with delegation
- **WHEN** user registers agent with scope and capability limits
- **THEN** system creates agent entity
- **AND** establishes delegation chain from user
- **AND** limits agent capabilities to not exceed user's permissions
- **AND** returns agent token for authentication

#### Scenario: Reject over-permissioned agent
- **WHEN** user attempts to register agent with capabilities exceeding their own
- **THEN** system rejects registration
- **AND** explains which capabilities exceeded user's permissions

### Requirement: Context Resolve Tool

The system SHALL provide an `aeterna_context_resolve` tool that resolves and displays current execution context.

#### Scenario: Resolve context automatically
- **WHEN** user requests context resolution in git repository
- **THEN** system checks resolution priority order (CLI flags, env vars, context file, git detection)
- **AND** returns resolved company, org, team, project, user
- **AND** indicates source of each resolved value

#### Scenario: Show effective permissions
- **WHEN** user requests context with permissions
- **THEN** system returns context plus effective permissions for memory, knowledge, and policy operations

### Requirement: Context Set Tool

The system SHALL provide an `aeterna_context_set` tool for overriding auto-detected context.

#### Scenario: Override team context
- **WHEN** user sets team context to different team
- **THEN** system validates user has access to specified team
- **AND** saves override to .aeterna/context.toml
- **AND** confirms context update

### Requirement: Context Clear Tool

The system SHALL provide an `aeterna_context_clear` tool for resetting context to auto-detection.

#### Scenario: Clear context overrides
- **WHEN** user clears context
- **THEN** system removes overrides from context file
- **AND** confirms next operation will use auto-detection

### Requirement: Enhanced Memory Search Tool

The system SHALL provide an enhanced `aeterna_memory_search` tool with natural language query support across all accessible layers.

#### Scenario: Search memories with natural language
- **WHEN** user searches "database decisions we made last month"
- **THEN** system interprets query intent
- **AND** searches across all accessible memory layers
- **AND** returns results ranked by relevance
- **AND** shows layer and source for each result

#### Scenario: Search with time filter
- **WHEN** user searches with time_range filter
- **THEN** system limits results to specified time period

### Requirement: Memory Browse Tool

The system SHALL provide an `aeterna_memory_browse` tool for interactive exploration of memories by layer and category.

#### Scenario: Browse team memories
- **WHEN** user browses memories for team layer
- **THEN** system returns total count
- **AND** shows categorized breakdown (decisions, learnings, conventions, etc.)
- **AND** returns paginated list of memories

### Requirement: Memory Promote Tool

The system SHALL provide an `aeterna_memory_promote` tool for promoting memories to broader scope with governance.

#### Scenario: Promote memory with approval
- **WHEN** user promotes project memory to team layer with reason
- **THEN** system checks if promotion requires approval based on governance rules
- **AND** creates promotion request if approval required
- **AND** notifies approvers
- **AND** returns promotion status

#### Scenario: Auto-promote high-value memory
- **WHEN** memory meets auto-promotion threshold and target layer allows auto-promotion
- **THEN** system promotes memory automatically
- **AND** logs promotion in audit trail

### Requirement: Memory Attribution Tool

The system SHALL provide an `aeterna_memory_attribute` tool for showing memory provenance.

#### Scenario: Show memory provenance
- **WHEN** user requests attribution for memory
- **THEN** system returns creation context (who, when, where)
- **AND** shows promotion history if any
- **AND** shows inheritance chain

### Requirement: Enhanced Knowledge Search Tool

The system SHALL provide an enhanced `aeterna_knowledge_search` tool with semantic search across the knowledge repository.

#### Scenario: Semantic knowledge search
- **WHEN** user searches "how do we handle authentication"
- **THEN** system performs semantic search across ADRs, patterns, policies, specs
- **AND** returns results ranked by relevance
- **AND** shows type and layer for each result

### Requirement: Knowledge Browse Tool

The system SHALL provide an `aeterna_knowledge_browse` tool for exploring the knowledge repository by type and layer.

#### Scenario: Browse ADRs across layers
- **WHEN** user browses type "adr" across all layers
- **THEN** system returns ADR count by layer
- **AND** returns paginated list sorted by date

### Requirement: Knowledge Propose Tool

The system SHALL provide an `aeterna_knowledge_propose` tool for proposing new knowledge items from natural language descriptions.

#### Scenario: Propose ADR from description
- **WHEN** user proposes "We should document that all new APIs must use GraphQL"
- **THEN** system interprets as ADR proposal
- **AND** generates draft ADR structure
- **AND** returns draft for editing
- **AND** provides submission instructions

### Requirement: Knowledge Explain Tool

The system SHALL provide an `aeterna_knowledge_explain` tool for getting plain-English explanations of knowledge items.

#### Scenario: Explain ADR in plain English
- **WHEN** user requests explanation for ADR-042
- **THEN** system returns context, decision, and consequences in plain language
- **AND** explains impact on current development

### Requirement: CLI Status Command

The system SHALL provide an `aeterna status` CLI command for quick health and context overview.

#### Scenario: Show status for current project
- **WHEN** user runs aeterna status
- **THEN** CLI displays current context
- **AND** shows pending items requiring attention
- **AND** shows recent team learnings
- **AND** shows active constraints

### Requirement: CLI Check Command

The system SHALL provide an `aeterna check` CLI command for validating against constraints.

#### Scenario: Check specific dependency
- **WHEN** user runs aeterna check dependency mysql
- **THEN** CLI evaluates dependency against active policies
- **AND** returns allow/block status with explanation

### Requirement: CLI Sync Command

The system SHALL provide an `aeterna sync` CLI command for synchronizing memory and knowledge.

#### Scenario: Sync memory-knowledge bridge
- **WHEN** user runs aeterna sync
- **THEN** CLI triggers memory-knowledge synchronization
- **AND** reports sync results
