# Configuration Spec Delta: UX-First Architecture

## ADDED Requirements

### Requirement: Context File Configuration

The system SHALL support a `.aeterna/context.toml` configuration file for project-level context defaults with automatic detection fallbacks.

#### Scenario: Load context from file
- **WHEN** user runs any Aeterna command in directory with .aeterna/context.toml
- **THEN** system loads context values from file
- **AND** applies resolution priority (CLI flags override file values)

#### Scenario: Auto-generate context file on project init
- **WHEN** user runs aeterna project init
- **THEN** system creates .aeterna/context.toml with detected values
- **AND** includes comments explaining each field

### Requirement: Context File Schema

The system SHALL define a context.toml schema with the following sections: context, user, agent, defaults.

#### Scenario: Parse valid context file
- **WHEN** context file contains valid company, org, team, project fields
- **THEN** system parses all fields successfully
- **AND** validates values against known entities

#### Scenario: Reject invalid context file
- **WHEN** context file contains invalid or malformed values
- **THEN** system reports specific validation errors
- **AND** suggests corrections

### Requirement: Environment Variable Configuration

The system SHALL support environment variables for context override with defined precedence over auto-detection.

#### Scenario: Override context with environment variable
- **WHEN** AETERNA_TEAM environment variable is set
- **THEN** system uses environment value for team context
- **AND** environment value takes precedence over context.toml

#### Scenario: Agent authentication via environment
- **WHEN** AETERNA_AGENT_ID and AETERNA_AGENT_TOKEN are set
- **THEN** system authenticates as the specified agent
- **AND** applies agent's capability restrictions

### Requirement: Git Remote Pattern Configuration

The system SHALL support configurable git remote URL patterns for automatic project and team mapping.

#### Scenario: Match git remote to team
- **WHEN** git remote matches configured pattern with team capture group
- **THEN** system extracts team identifier from remote URL
- **AND** resolves team context automatically

#### Scenario: Configure custom remote pattern
- **WHEN** admin configures custom remote pattern for company
- **THEN** system uses pattern for all projects in company
- **AND** pattern supports named capture groups (company, org, team, project)

### Requirement: Governance Configuration Schema

The system SHALL define a governance configuration schema for meta-governance rules at each scope level.

#### Scenario: Configure policy approval requirements
- **WHEN** admin sets policy_approval.required_approvers to 2
- **THEN** system requires 2 approvals for policy activation at that scope
- **AND** validates approvers against allowed_approvers list

#### Scenario: Configure memory promotion rules
- **WHEN** admin sets memory_rules.auto_promote_threshold to 0.85
- **THEN** system auto-promotes memories with importance >= 0.85
- **AND** memories below threshold require approval for promotion

### Requirement: Delegation Rules Configuration

The system SHALL support delegation rules configuration defining what AI agents can do within their delegation scope.

#### Scenario: Configure agent proposal limits
- **WHEN** admin sets delegation_rules.max_agent_severity to "warn"
- **THEN** agents cannot propose policies with severity higher than warn
- **AND** attempts to exceed limit are rejected with explanation

#### Scenario: Configure agent approval requirements
- **WHEN** admin sets delegation_rules.agent_approval_requires_human_confirm to true
- **THEN** agent approvals require human confirmation before activation
- **AND** human confirmation is tracked in audit trail

### Requirement: Default Governance Levels

The system SHALL provide pre-defined governance levels (standard, strict, permissive) for quick organization setup.

#### Scenario: Apply standard governance
- **WHEN** admin selects "standard" governance level during init
- **THEN** system applies balanced defaults (1 approver for policies, auto-approve patterns)
- **AND** admin can customize individual settings afterward

#### Scenario: Apply strict governance
- **WHEN** admin selects "strict" governance level
- **THEN** system applies conservative defaults (2+ approvers, no auto-approve, longer review periods)
- **AND** enables additional audit requirements

### Requirement: SSO Provider Configuration

The system SHALL support SSO provider configuration for enterprise identity integration.

#### Scenario: Configure Okta SSO
- **WHEN** admin configures sso_provider as "okta" with domain
- **THEN** system enables Okta authentication
- **AND** maps Okta groups to Aeterna teams
- **AND** extracts user identity from JWT claims

#### Scenario: Map SSO claims to context
- **WHEN** user authenticates via SSO
- **THEN** system extracts org and team from JWT claims
- **AND** uses claims as context fallback in resolution priority
