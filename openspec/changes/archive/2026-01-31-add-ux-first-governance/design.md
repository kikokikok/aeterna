# Design: UX-First Governance & Policy Skills

## Context

Aeterna governance must serve three distinct user types with different interaction patterns:

| User Type | Primary Interface | Expectation |
|-----------|-------------------|-------------|
| **Human Developer** | CLI, Chat (via AI assistant) | Natural language, minimal learning curve |
| **Human Admin/Architect** | CLI, Web Dashboard | Bulk operations, audit views, approval workflows |
| **AI Agent (LLM)** | MCP Tools, Skills | Structured inputs, clear success/failure, no DSL generation |

**Key Insight**: All three should use the SAME underlying tools/skills - the interface adapts, not the capability.

## Goals / Non-Goals

### Goals
- Natural language policy creation (human and AI agent)
- Zero Cedar knowledge required for users
- Self-service governance configuration
- Complete audit trail from intent to enforcement
- CLI-first automation support
- Meta-governance (govern the governance)

### Non-Goals
- Replacing Cedar as policy engine (Cedar remains internal)
- Building a web UI (API-first, UI can be built on top)
- Supporting non-Cedar policy engines (pluggable in future)
- Real-time policy enforcement in CI/CD (separate integration)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER INTERACTION LAYER                               │
│                                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                     │
│   │   Human     │    │  AI Agent   │    │    CLI      │                     │
│   │  (Chat)     │    │   (LLM)     │    │  (Script)   │                     │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                     │
│          │                  │                  │                             │
│          ▼                  ▼                  ▼                             │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                    NATURAL LANGUAGE LAYER                        │       │
│   │                                                                  │       │
│   │   "Block MySQL in this project"                                  │       │
│   │   "Only architects can approve org policies"                     │       │
│   │   "Require 2 approvers for company-level changes"                │       │
│   │                                                                  │       │
│   └──────────────────────────┬──────────────────────────────────────┘       │
│                              │                                               │
└──────────────────────────────┼───────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SKILL / TOOL LAYER                                   │
│                                                                              │
│   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│   │  Policy Skill   │  │ Governance Skill│  │  Admin Skill    │             │
│   │                 │  │                 │  │                 │             │
│   │ • draft         │  │ • configure     │  │ • roles         │             │
│   │ • validate      │  │ • approve       │  │ • audit         │             │
│   │ • propose       │  │ • reject        │  │ • health        │             │
│   │ • explain       │  │ • escalate      │  │ • migrate       │             │
│   │ • simulate      │  │ • delegate      │  │                 │             │
│   │ • list          │  │                 │  │                 │             │
│   └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
│            │                    │                    │                       │
│            └────────────────────┼────────────────────┘                       │
│                                 │                                            │
└─────────────────────────────────┼────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TRANSLATION LAYER                                    │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                  LLM-Powered Translator                          │       │
│   │                                                                  │       │
│   │   Natural Language ──────▶ Structured Intent ──────▶ Cedar      │       │
│   │                                                                  │       │
│   │   "Block MySQL" ──▶ {target: dep, op: deny, value: "mysql"}     │       │
│   │                 ──▶ forbid(principal, action, resource)          │       │
│   │                     when { resource.dependencies.contains("mysql") } │   │
│   │                                                                  │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                  Cedar Validator                                 │       │
│   │                                                                  │       │
│   │   • Syntax validation (cedar-policy crate)                       │       │
│   │   • Schema compliance                                            │       │
│   │   • Conflict detection with existing policies                    │       │
│   │   • Simulation against test scenarios                            │       │
│   │                                                                  │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE ENGINE                                    │
│                                                                              │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│   │   Proposal   │  │   Approval   │  │    Cedar     │  │    Audit     │   │
│   │    Store     │  │   Workflow   │  │   Authorizer │  │     Log      │   │
│   └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Policy Skill

### 1.1 Tool: `aeterna_policy_draft`

**Purpose**: Translate natural language intent into a Cedar policy draft.

**Input (Natural Language)**:
```json
{
  "intent": "Block any usage of MySQL in this project",
  "scope": "project",
  "severity": "block"
}
```

**Output (Structured + Cedar)**:
```json
{
  "draft_id": "draft_abc123",
  "status": "pending_review",
  "intent": {
    "original": "Block any usage of MySQL in this project",
    "interpreted": "Deny usage of MySQL database dependency in project scope"
  },
  "policy": {
    "name": "no-mysql",
    "scope": "project",
    "severity": "block",
    "cedar": "forbid(\n  principal,\n  action == Action::\"UseDependency\",\n  resource\n)\nwhen {\n  resource.dependency == \"mysql\"\n};",
    "human_readable": "This policy blocks any code that uses MySQL as a dependency. Violations will prevent the action from proceeding."
  },
  "validation": {
    "syntax_valid": true,
    "schema_valid": true,
    "conflicts": [],
    "warnings": ["Consider also blocking 'mysql2' and 'mysqlclient' variants"]
  },
  "next_steps": [
    "Review the generated policy",
    "Use aeterna_policy_simulate to test against scenarios",
    "Use aeterna_policy_propose to submit for approval"
  ]
}
```

**UX Flow (Human via AI Assistant)**:
```
Human: Block MySQL in this project

AI: I'll create a policy to block MySQL usage.

[Uses aeterna_policy_draft]

Here's the draft policy:

📋 **Policy: no-mysql**
- **Scope**: Project
- **Severity**: Block (prevents action)
- **Effect**: Blocks any code using MySQL as a dependency

⚠️ **Suggestions**:
- Consider also blocking 'mysql2' and 'mysqlclient' variants

Would you like me to:
1. Simulate this against your current codebase
2. Adjust the policy (e.g., add more MySQL variants)
3. Submit for approval
```

**UX Flow (AI Agent)**:
```python
# Agent workflow
draft = await aeterna_policy_draft(
    intent="Block MySQL usage",
    scope="project",
    severity="block"
)

if draft.validation.syntax_valid and not draft.validation.conflicts:
    # Simulate first
    simulation = await aeterna_policy_simulate(
        draft_id=draft.draft_id,
        scenarios=["current_dependencies"]
    )
    
    if simulation.would_block_existing:
        # Inform user before proposing
        return f"This policy would affect {len(simulation.affected_items)} existing items"
    
    # Submit for approval
    proposal = await aeterna_policy_propose(draft_id=draft.draft_id)
```

### 1.2 Tool: `aeterna_policy_validate`

**Purpose**: Validate Cedar policy syntax and semantics without LLM involvement.

**Input**:
```json
{
  "cedar": "forbid(principal, action, resource) when { resource.dep == \"mysql\" };",
  "scope": "project"
}
```

**Output**:
```json
{
  "valid": false,
  "errors": [
    {
      "type": "schema_error",
      "message": "Unknown attribute 'dep' on resource. Did you mean 'dependency'?",
      "line": 1,
      "suggestion": "resource.dependency"
    }
  ],
  "warnings": []
}
```

### 1.3 Tool: `aeterna_policy_propose`

**Purpose**: Submit a validated draft for approval workflow.

**Input**:
```json
{
  "draft_id": "draft_abc123",
  "justification": "MySQL is prohibited per ADR-042, need enforcement",
  "notify": ["@tech-lead", "@architect"]
}
```

**Output**:
```json
{
  "proposal_id": "prop_xyz789",
  "status": "pending_approval",
  "required_approvers": 1,
  "current_approvals": 0,
  "approvers_notified": ["alice@company.com", "bob@company.com"],
  "expires_at": "2024-01-20T10:00:00Z",
  "approval_url": "https://aeterna.company.com/proposals/prop_xyz789"
}
```

### 1.4 Tool: `aeterna_policy_list`

**Purpose**: List active policies with human-readable summaries.

**Input**:
```json
{
  "scope": "project",
  "include_inherited": true
}
```

**Output**:
```json
{
  "policies": [
    {
      "id": "security-baseline",
      "name": "Security Baseline",
      "scope": "company",
      "inherited": true,
      "severity": "block",
      "summary": "Blocks vulnerable dependencies (lodash <4.17.21) and requires SECURITY.md",
      "rules_count": 5
    },
    {
      "id": "no-mysql",
      "name": "No MySQL",
      "scope": "project",
      "inherited": false,
      "severity": "block",
      "summary": "Blocks MySQL database dependencies",
      "rules_count": 1
    }
  ],
  "total": 2,
  "inherited_from": ["company: security-baseline", "org: platform-standards"]
}
```

### 1.5 Tool: `aeterna_policy_explain`

**Purpose**: Explain a Cedar policy in natural language.

**Input**:
```json
{
  "policy_id": "security-baseline"
}
```

**Output**:
```json
{
  "policy_id": "security-baseline",
  "name": "Security Baseline",
  "explanation": {
    "summary": "This company-wide mandatory policy enforces core security requirements across all projects.",
    "rules": [
      {
        "rule_id": "no-vulnerable-lodash",
        "natural_language": "Blocks any project from using lodash versions below 4.17.21 due to CVE-2021-23337 (prototype pollution vulnerability)",
        "severity": "block",
        "impact": "Prevents deployment if violated"
      },
      {
        "rule_id": "require-security-doc",
        "natural_language": "Requires every project to have a SECURITY.md file documenting security practices",
        "severity": "warn",
        "impact": "Generates warning but allows action"
      }
    ],
    "scope_explanation": "Applies to all projects in the company. Cannot be overridden by lower scopes.",
    "created_by": "alice@company.com",
    "created_at": "2024-01-15T10:00:00Z",
    "last_modified": "2024-01-18T14:30:00Z"
  }
}
```

### 1.6 Tool: `aeterna_policy_simulate`

**Purpose**: Test a policy against scenarios without applying it.

**Input**:
```json
{
  "draft_id": "draft_abc123",
  "scenarios": [
    {
      "name": "current_project",
      "type": "live",
      "target": "."
    },
    {
      "name": "hypothetical_mysql",
      "type": "synthetic",
      "context": {
        "dependencies": ["mysql", "express", "lodash"]
      }
    }
  ]
}
```

**Output**:
```json
{
  "draft_id": "draft_abc123",
  "results": [
    {
      "scenario": "current_project",
      "outcome": "pass",
      "violations": [],
      "message": "No MySQL dependencies found in current project"
    },
    {
      "scenario": "hypothetical_mysql",
      "outcome": "block",
      "violations": [
        {
          "rule": "no-mysql",
          "target": "mysql",
          "message": "MySQL dependency is prohibited"
        }
      ],
      "message": "Policy would block this configuration"
    }
  ],
  "summary": "Policy passes current project, would block MySQL if added"
}
```

---

## Part 2: Governance Administration

### 2.1 Tool: `aeterna_governance_configure`

**Purpose**: Configure governance rules (meta-governance).

**Input**:
```json
{
  "scope": "org",
  "settings": {
    "policy_approval": {
      "required_approvers": 1,
      "allowed_approvers": ["architect", "tech_lead"],
      "auto_approve_for": ["admin"],
      "review_period_hours": 24
    },
    "knowledge_proposal": {
      "required_approvers": 1,
      "allowed_proposers": ["developer", "tech_lead", "architect"],
      "auto_approve_types": ["pattern"]
    },
    "memory_promotion": {
      "auto_promote_threshold": 0.9,
      "require_approval_above": "team"
    }
  }
}
```

**Output**:
```json
{
  "status": "configured",
  "scope": "org",
  "effective_settings": {
    "policy_approval": {
      "required_approvers": 1,
      "allowed_approvers": ["architect", "tech_lead"],
      "review_period_hours": 24
    }
  },
  "inherited_from": {
    "company": ["base_governance_rules"]
  },
  "conflicts": []
}
```

**CLI Equivalent**:
```bash
# Interactive configuration
aeterna govern configure --scope org

# Non-interactive
aeterna govern configure --scope org \
  --policy-approvers architect,tech_lead \
  --policy-approval-count 1 \
  --review-period 24h
```

### 2.2 Tool: `aeterna_governance_roles`

**Purpose**: Manage role assignments.

**Input (List)**:
```json
{
  "action": "list",
  "scope": "team"
}
```

**Output**:
```json
{
  "roles": [
    {
      "user": "alice@company.com",
      "role": "tech_lead",
      "scope": "team:backend",
      "granted_by": "bob@company.com",
      "granted_at": "2024-01-10T10:00:00Z"
    },
    {
      "user": "charlie@company.com",
      "role": "developer",
      "scope": "team:backend",
      "granted_by": "alice@company.com",
      "granted_at": "2024-01-12T14:00:00Z"
    }
  ]
}
```

**Input (Assign)**:
```json
{
  "action": "assign",
  "user": "david@company.com",
  "role": "architect",
  "scope": "org:engineering"
}
```

**CLI Equivalent**:
```bash
# List roles
aeterna govern roles --scope team:backend

# Assign role
aeterna govern roles assign david@company.com architect --scope org:engineering

# Revoke role
aeterna govern roles revoke david@company.com architect --scope org:engineering
```

### 2.3 Tool: `aeterna_governance_approve` / `_reject`

**Purpose**: Approve or reject governance proposals.

**Input (Approve)**:
```json
{
  "proposal_id": "prop_xyz789",
  "comment": "LGTM, aligns with ADR-042"
}
```

**Output**:
```json
{
  "proposal_id": "prop_xyz789",
  "status": "approved",
  "approved_by": "alice@company.com",
  "approved_at": "2024-01-19T15:30:00Z",
  "policy_activated": true,
  "policy_id": "no-mysql",
  "effective_at": "2024-01-19T15:30:00Z"
}
```

**Input (Reject)**:
```json
{
  "proposal_id": "prop_xyz789",
  "reason": "Too broad - should only block mysql, not mysql2",
  "suggest_revision": true
}
```

**CLI Equivalent**:
```bash
# List pending approvals
aeterna govern pending

# Approve
aeterna govern approve prop_xyz789 --comment "LGTM"

# Reject
aeterna govern reject prop_xyz789 --reason "Too broad"
```

### 2.4 Tool: `aeterna_governance_audit`

**Purpose**: View governance activity and audit trail.

**Input**:
```json
{
  "scope": "project",
  "from": "2024-01-01",
  "to": "2024-01-20",
  "event_types": ["policy_created", "policy_approved", "policy_rejected"]
}
```

**Output**:
```json
{
  "events": [
    {
      "id": "evt_001",
      "type": "policy_proposed",
      "timestamp": "2024-01-18T10:00:00Z",
      "actor": "charlie@company.com",
      "details": {
        "proposal_id": "prop_xyz789",
        "policy_name": "no-mysql",
        "intent": "Block MySQL usage"
      }
    },
    {
      "id": "evt_002",
      "type": "policy_approved",
      "timestamp": "2024-01-19T15:30:00Z",
      "actor": "alice@company.com",
      "details": {
        "proposal_id": "prop_xyz789",
        "comment": "LGTM, aligns with ADR-042"
      }
    }
  ],
  "total": 2,
  "audit_complete": true
}
```

**CLI Equivalent**:
```bash
# View recent audit log
aeterna govern audit --scope project --last 7d

# Export for compliance
aeterna govern audit --scope company --from 2024-01-01 --format csv > audit.csv
```

---

## Part 3: CLI Skill Interface

### 3.1 Policy Commands

```bash
# Create policy (interactive)
aeterna policy create
> What should this policy do? Block MySQL usage
> Scope? [project/team/org/company]: project
> Severity? [info/warn/block]: block
> 
> Generated policy:
> Name: no-mysql
> Effect: Blocks MySQL dependencies
> 
> Options:
> 1. Simulate against current project
> 2. Submit for approval
> 3. Modify
> 4. Cancel

# Create policy (non-interactive)
aeterna policy create "Block MySQL usage" --scope project --severity block --submit

# List policies
aeterna policy list
aeterna policy list --scope team --include-inherited
aeterna policy list --format json

# Explain policy
aeterna policy explain security-baseline

# Simulate policy
aeterna policy simulate draft_abc123
aeterna policy simulate draft_abc123 --scenario '{"dependencies": ["mysql"]}'

# View draft
aeterna policy draft show draft_abc123

# Submit draft
aeterna policy draft submit draft_abc123 --justification "Per ADR-042"
```

### 3.2 Governance Commands

```bash
# Configure governance
aeterna govern configure --scope org --interactive
aeterna govern configure --scope team --policy-approvers architect

# View status
aeterna govern status
aeterna govern status --scope company

# Manage roles
aeterna govern roles list --scope team
aeterna govern roles assign user@company.com tech_lead --scope team:backend
aeterna govern roles revoke user@company.com tech_lead --scope team:backend

# Approvals
aeterna govern pending
aeterna govern approve prop_xyz789 --comment "LGTM"
aeterna govern reject prop_xyz789 --reason "Needs revision"

# Audit
aeterna govern audit --last 30d
aeterna govern audit --scope company --from 2024-01-01 --to 2024-01-31
```

### 3.3 Admin Commands

```bash
# Health check
aeterna admin health
aeterna admin health --verbose

# Validate all policies
aeterna admin validate --all
aeterna admin validate --scope project

# Migrate governance data
aeterna admin migrate --from v1 --to v2 --dry-run
aeterna admin migrate --from v1 --to v2 --execute

# Export/Import
aeterna admin export policies --scope company > company-policies.json
aeterna admin import policies < company-policies.json

# Drift detection
aeterna admin drift --scope project
aeterna admin drift --scope org --threshold 0.3
```

---

## Part 4: Complete Use Cases

### Use Case 1: Developer Creates Project Policy

**Scenario**: Developer wants to enforce PostgreSQL usage in their project.

**Via AI Assistant**:
```
Developer: Enforce PostgreSQL for database connections in this project

AI: I'll create a policy to enforce PostgreSQL usage.

[Uses aeterna_policy_draft]

📋 **Draft Policy: require-postgresql**
- **Scope**: Project (payments-service)
- **Severity**: Block
- **Effect**: Requires PostgreSQL for database connections, blocks MySQL/MariaDB/SQLite

🔍 **Simulation Results**:
- Current project: ✅ Pass (uses pg driver)
- Would block: mysql, mysql2, mariadb, sqlite3

Shall I submit this for approval? Your tech lead will be notified.

Developer: Yes, submit it

AI: [Uses aeterna_policy_propose]

✅ **Proposal Submitted**: prop_abc123
- Required approvers: 1 (tech lead or architect)
- Notified: alice@company.com (Tech Lead)
- Review period: 24 hours

You'll be notified when it's approved.
```

**Via CLI**:
```bash
$ aeterna policy create "Require PostgreSQL for databases" --scope project --severity block

📋 Draft Policy Created: draft_xyz789

Name: require-postgresql
Scope: project (payments-service)
Severity: block
Effect: Requires PostgreSQL for database connections

Simulating against current project...
✅ Current project passes (uses pg driver)

Submit for approval? [y/N]: y
Justification: Standardizing on PostgreSQL per team decision

✅ Proposal submitted: prop_abc123
Notified: alice@company.com (Tech Lead)
```

### Use Case 2: Architect Creates Org-Wide Policy

**Scenario**: Architect wants to mandate OpenTelemetry for all services.

**Via AI Assistant**:
```
Architect: All services in Platform Engineering must use OpenTelemetry

AI: I'll create an organization-level policy for Platform Engineering.

[Uses aeterna_policy_draft with scope="org"]

📋 **Draft Policy: require-opentelemetry**
- **Scope**: Organization (Platform Engineering)
- **Severity**: Warn (I suggest warn instead of block for gradual adoption)
- **Effect**: Requires opentelemetry dependency in all services

⚠️ **Impact Analysis**:
- 12 services in scope
- 8 already compliant
- 4 would receive warnings: auth-service, gateway, legacy-api, batch-processor

Would you like to:
1. Keep as warning (recommended for migration)
2. Change to blocking
3. Add grace period
4. Exclude specific projects

Architect: Keep as warning, but exclude legacy-api

AI: [Modifies draft]

Updated policy excludes legacy-api. Submitting for approval...

[Uses aeterna_policy_propose]

✅ **Proposal Submitted**: prop_org_001
- Required approvers: 2 (org-level requires quorum)
- Notified: bob@company.com (Admin), carol@company.com (Architect)
- Review period: 48 hours (org-level policy)
```

### Use Case 3: Admin Configures Meta-Governance

**Scenario**: Admin sets up governance rules for a new organization.

**Via CLI**:
```bash
$ aeterna govern configure --scope org:data-platform --interactive

🔧 Governance Configuration for org:data-platform

Policy Approval Settings:
  Required approvers [1]: 2
  Allowed approvers [architect,admin]: architect,tech_lead,admin
  Auto-approve for roles [admin]: admin
  Review period (hours) [24]: 48

Knowledge Proposal Settings:
  Required approvers [1]: 1
  Allowed proposers [developer,tech_lead,architect]: developer,tech_lead,architect
  Auto-approve types [none]: pattern,reference

Memory Promotion Settings:
  Auto-promote threshold [0.9]: 0.85
  Require approval above layer [team]: team

Save configuration? [y/N]: y

✅ Governance configured for org:data-platform

Effective rules:
- Policies require 2 approvals from architect/tech_lead/admin
- Knowledge proposals auto-approve patterns and references
- Memory auto-promotes at 0.85 importance, needs approval for org+ scope
```

### Use Case 4: AI Agent Proposes Policy Autonomously

**Scenario**: AI agent detects repeated pattern violations and proposes policy.

```python
# Agent detects pattern: developers keep using console.log in production code
# Agent decides to propose a policy

async def agent_governance_workflow():
    # Step 1: Draft policy from observation
    draft = await aeterna_policy_draft(
        intent="Prevent console.log statements in production TypeScript files",
        scope="team",
        severity="warn",
        context={
            "observation": "Detected 47 console.log statements across 12 PRs this week",
            "rationale": "Console.log in production affects performance and leaks info"
        }
    )
    
    # Step 2: Validate
    if not draft.validation.syntax_valid:
        logger.error(f"Draft validation failed: {draft.validation.errors}")
        return
    
    # Step 3: Simulate
    simulation = await aeterna_policy_simulate(
        draft_id=draft.draft_id,
        scenarios=[{"type": "live", "target": "."}]
    )
    
    # Step 4: Inform human before proposing
    if simulation.results[0].outcome == "block":
        # Would affect existing code - need human decision
        await notify_human(
            message=f"Proposed policy would flag {len(simulation.violations)} existing issues",
            action_required="approve_or_modify_policy",
            draft_id=draft.draft_id
        )
        return
    
    # Step 5: Submit proposal (if no existing violations)
    proposal = await aeterna_policy_propose(
        draft_id=draft.draft_id,
        justification="Automated proposal based on repeated pattern violations",
        notify=["@tech-lead"]
    )
    
    logger.info(f"Policy proposed: {proposal.proposal_id}")
```

### Use Case 5: Governance Audit for Compliance

**Scenario**: Security team needs audit trail for SOC2 compliance.

**Via CLI**:
```bash
$ aeterna govern audit --scope company --from 2024-01-01 --to 2024-03-31 --format csv

Generating audit report for company scope...

Events found: 342
- Policy changes: 45
- Role assignments: 89
- Approvals/Rejections: 156
- Drift detections: 52

Exporting to CSV...
✅ Saved to: governance-audit-2024-Q1.csv

Summary:
┌─────────────────────┬───────┬──────────┬──────────┐
│ Event Type          │ Count │ Approved │ Rejected │
├─────────────────────┼───────┼──────────┼──────────┤
│ Policy Proposals    │ 45    │ 38       │ 7        │
│ Knowledge Proposals │ 89    │ 82       │ 7        │
│ Role Changes        │ 52    │ 52       │ 0        │
│ Drift Resolutions   │ 156   │ 140      │ 16       │
└─────────────────────┴───────┴──────────┴──────────┘

All events include:
- Timestamp (ISO 8601)
- Actor (user or agent ID)
- Action taken
- Before/After state
- Justification (if provided)
```

---

## Part 5: Translation Layer Implementation

### 5.1 LLM-Powered Natural Language to Cedar

```rust
pub struct PolicyTranslator {
    llm: Box<dyn LlmProvider>,
    schema: CedarSchema,
    examples: Vec<TranslationExample>,
}

impl PolicyTranslator {
    pub async fn translate(&self, intent: &str, context: &TranslationContext) -> Result<PolicyDraft> {
        // Step 1: Extract structured intent
        let structured = self.extract_intent(intent, context).await?;
        
        // Step 2: Generate Cedar from structured intent
        let cedar = self.generate_cedar(&structured).await?;
        
        // Step 3: Validate generated Cedar
        let validation = self.validate_cedar(&cedar, &self.schema)?;
        
        // Step 4: Generate human-readable explanation
        let explanation = self.explain_cedar(&cedar).await?;
        
        Ok(PolicyDraft {
            intent: structured,
            cedar,
            validation,
            explanation,
        })
    }
    
    async fn extract_intent(&self, natural: &str, ctx: &TranslationContext) -> Result<StructuredIntent> {
        let prompt = format!(r#"
Extract policy intent from natural language.

Input: "{natural}"
Context: scope={}, project={}

Output JSON:
{{
  "action": "allow|deny",
  "target_type": "dependency|file|code|import|config",
  "target_value": "specific value or pattern",
  "condition": "optional condition",
  "severity": "info|warn|block"
}}

Examples:
- "Block MySQL" -> {{"action": "deny", "target_type": "dependency", "target_value": "mysql", "severity": "block"}}
- "Require README" -> {{"action": "allow", "target_type": "file", "target_value": "README.md", "condition": "must_exist", "severity": "warn"}}
"#, ctx.scope, ctx.project);
        
        self.llm.complete(&prompt).await
    }
    
    async fn generate_cedar(&self, intent: &StructuredIntent) -> Result<String> {
        // Use templates for common patterns, LLM for complex cases
        match (&intent.action, &intent.target_type) {
            (Action::Deny, TargetType::Dependency) => {
                Ok(format!(r#"
forbid(
  principal,
  action == Action::"UseDependency",
  resource
)
when {{
  resource.dependency == "{}"
}};
"#, intent.target_value))
            }
            // ... other patterns
            _ => self.llm_generate_cedar(intent).await
        }
    }
}
```

### 5.2 Cedar Validation Pipeline

```rust
pub struct CedarValidator {
    engine: cedar_policy::Authorizer,
    schema: cedar_policy::Schema,
}

impl CedarValidator {
    pub fn validate(&self, cedar_text: &str) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Step 1: Parse Cedar
        let policy = match cedar_policy::Policy::parse(None, cedar_text) {
            Ok(p) => p,
            Err(e) => {
                errors.push(ValidationError::Syntax(e.to_string()));
                return ValidationResult { valid: false, errors, warnings };
            }
        };
        
        // Step 2: Validate against schema
        if let Err(e) = policy.validate(&self.schema) {
            errors.push(ValidationError::Schema(e.to_string()));
        }
        
        // Step 3: Check for conflicts with existing policies
        let conflicts = self.check_conflicts(&policy);
        if !conflicts.is_empty() {
            warnings.extend(conflicts.into_iter().map(ValidationError::Conflict));
        }
        
        // Step 4: Semantic analysis
        let semantic_issues = self.analyze_semantics(&policy);
        warnings.extend(semantic_issues);
        
        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}
```

---

## Part 6: Meta-Governance Schema

### 6.1 Governance Configuration Model

```rust
pub struct GovernanceConfig {
    pub scope: GovernanceScope,
    pub policy_rules: PolicyGovernanceRules,
    pub knowledge_rules: KnowledgeGovernanceRules,
    pub memory_rules: MemoryGovernanceRules,
    pub delegation_rules: DelegationRules,
}

pub struct PolicyGovernanceRules {
    pub required_approvers: u32,
    pub allowed_approvers: Vec<Role>,
    pub auto_approve_for: Vec<Role>,
    pub review_period: Duration,
    pub escalation_after: Duration,
    pub escalation_to: Vec<Role>,
}

pub struct DelegationRules {
    pub agents_can_propose: bool,
    pub agents_can_approve: bool,
    pub agent_approval_requires_human_confirm: bool,
    pub max_agent_severity: Severity,  // Agents can only propose up to this severity
}
```

### 6.2 Default Meta-Governance Policies

```cedar
// Meta-policy: Who can configure governance
permit(
  principal,
  action == Action::"ConfigureGovernance",
  resource
)
when {
  principal.role in [Role::"Admin", Role::"Architect"] &&
  principal.scope.contains(resource.scope)
};

// Meta-policy: Agents cannot approve blocking policies without human
forbid(
  principal is Agent,
  action == Action::"ApprovePolicy",
  resource
)
when {
  resource.policy.severity == Severity::"Block" &&
  !resource.has_human_confirmation
};

// Meta-policy: Company policies require admin approval
permit(
  principal,
  action == Action::"ApprovePolicy",
  resource
)
when {
  resource.scope == Scope::"Company"
} only_if {
  principal.role == Role::"Admin"
};
```

---

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| LLM translation errors | Validation pipeline catches syntax/schema errors; simulation prevents bad policies |
| Over-permissive meta-governance | Default-secure configuration; audit trail for all changes |
| Agent abuse (autonomous policy creation) | Delegation rules limit agent capabilities; human confirmation for high-severity |
| Translation consistency | Example library + fine-tuned prompts; fallback to templates for common patterns |
| Cedar complexity exposed via errors | Translate Cedar errors to natural language explanations |

---

## Migration Plan

1. **Phase 1**: Policy Skill tools (draft, validate, propose, list, explain)
2. **Phase 2**: Governance administration tools (configure, roles, approve/reject)
3. **Phase 3**: CLI commands mirroring all tools
4. **Phase 4**: Meta-governance (policies about policies)
5. **Phase 5**: Agent delegation rules and autonomous proposal support

---

## Open Questions

- [ ] Should policy simulation include cost estimation (how many items affected)?
- [ ] Should there be a "policy sandbox" for testing without proposal workflow?
- [ ] How do we handle policy versioning and rollback?
- [ ] Should agents be able to auto-escalate blocked proposals?
- [ ] Integration with external approval systems (Slack, GitHub PR reviews)?

---

## Part 7: Onboarding & Deployment UX

The onboarding experience must be zero-friction. A new organization, team, or project should be productive within minutes, not hours.

### 7.1 Onboarding Hierarchy

```
Company (root)
   └── Organization (department/division)
         └── Team (working group)
               └── Project (repository)
                     └── User (individual)
                           └── Agent (AI delegated by user)
```

### 7.2 Tool: `aeterna_org_init`

**Purpose**: Initialize a company or organization with sensible defaults.

**Input**:
```json
{
  "name": "Acme Corp",
  "type": "company",
  "admin_email": "admin@acme.com",
  "settings": {
    "default_governance": "standard",  // standard, strict, permissive
    "sso_provider": "okta",            // optional
    "knowledge_backend": "git"
  }
}
```

**Output**:
```json
{
  "id": "company:acme-corp",
  "status": "initialized",
  "created": {
    "company": "acme-corp",
    "default_org": "default",
    "admin_user": "admin@acme.com"
  },
  "defaults_applied": {
    "governance": "standard",
    "policies": ["security-baseline"],
    "roles": {
      "admin@acme.com": "admin"
    }
  },
  "next_steps": [
    "Create organizations: aeterna org create 'Engineering'",
    "Create teams: aeterna team create 'Platform'",
    "Invite users: aeterna user invite user@acme.com"
  ]
}
```

**CLI Equivalent**:
```bash
# Interactive setup wizard
$ aeterna init
Welcome to Aeterna!

Let's set up your organization.

Company name: Acme Corp
Admin email: admin@acme.com
SSO Provider (optional, press Enter to skip): okta
Governance level [standard/strict/permissive]: standard

Initializing...
✅ Company 'Acme Corp' created
✅ Default security policies applied
✅ Admin role assigned to admin@acme.com

Next steps:
  aeterna org create "Engineering"
  aeterna team create "Platform" --org engineering
  aeterna project init  # In your git repository

# Non-interactive
$ aeterna init --company "Acme Corp" --admin admin@acme.com --governance standard
```

### 7.3 Tool: `aeterna_team_create`

**Purpose**: Create a team within an organization.

**Input**:
```json
{
  "name": "API Team",
  "org": "platform-engineering",
  "lead": "alice@acme.com",
  "inherit_governance": true
}
```

**Output**:
```json
{
  "id": "team:platform-engineering:api-team",
  "status": "created",
  "inherited_policies": 3,
  "inherited_roles": {
    "alice@acme.com": "tech_lead"
  },
  "governance": {
    "policy_approval_required": 1,
    "allowed_approvers": ["tech_lead", "architect"]
  }
}
```

**CLI Equivalent**:
```bash
$ aeterna team create "API Team" --org platform-engineering --lead alice@acme.com

✅ Team 'API Team' created in 'Platform Engineering'
   ID: team:platform-engineering:api-team
   Lead: alice@acme.com (tech_lead)
   Inherited: 3 policies from org
```

### 7.4 Tool: `aeterna_project_init`

**Purpose**: Initialize Aeterna in a project (auto-detects git context).

**Input**:
```json
{
  "path": ".",
  "team": "auto",  // auto-detect or explicit
  "settings": {
    "memory_layer": "project",
    "knowledge_sync": true
  }
}
```

**Auto-Detection Logic**:
```
1. Read git remote URL → extract org/repo name
2. Read git user.email → identify user
3. Check environment variables → AETERNA_TEAM, AETERNA_ORG
4. Search existing team mappings for this repo
5. If no match → prompt for team association
```

**Output**:
```json
{
  "project_id": "project:platform-engineering:api-team:payments-service",
  "detected": {
    "git_remote": "github.com/acme/payments-service",
    "git_user": "alice@acme.com",
    "team": "api-team",
    "org": "platform-engineering"
  },
  "created_files": [
    ".aeterna/context.toml",
    ".aeterna/config.toml"
  ],
  "inherited": {
    "policies": 5,
    "knowledge_items": 23
  },
  "status": "ready"
}
```

**CLI Equivalent**:
```bash
$ cd payments-service
$ aeterna project init

🔍 Detecting context...
   Git remote: github.com/acme/payments-service
   Git user: alice@acme.com
   
📍 Matched to: Platform Engineering → API Team

✅ Project initialized: payments-service
   Config: .aeterna/context.toml
   Policies inherited: 5
   Knowledge items: 23
   
Your project is ready! Try:
   aeterna status          # Check current state
   aeterna memory search   # Search memories
   aeterna knowledge search # Search knowledge base
```

### 7.5 Tool: `aeterna_user_register`

**Purpose**: Register a user identity with Aeterna.

**Input**:
```json
{
  "email": "bob@acme.com",
  "display_name": "Bob Smith",
  "teams": ["api-team"],
  "role": "developer"
}
```

**Output**:
```json
{
  "user_id": "user:bob@acme.com",
  "status": "registered",
  "memberships": [
    {
      "team": "api-team",
      "role": "developer",
      "granted_by": "alice@acme.com"
    }
  ],
  "capabilities": {
    "can_create_policies": true,
    "can_approve_policies": false,
    "can_promote_memory": "team"
  }
}
```

### 7.6 Tool: `aeterna_agent_register`

**Purpose**: Register an AI agent with delegation chain.

**Input**:
```json
{
  "agent_id": "agent:opencode-alice",
  "delegated_by": "alice@acme.com",
  "scope": "project:payments-service",
  "capabilities": {
    "can_read_memory": true,
    "can_write_memory": true,
    "can_propose_policies": true,
    "can_approve_policies": false,
    "max_policy_severity": "warn"
  },
  "expires": "2024-12-31T23:59:59Z"
}
```

**Output**:
```json
{
  "agent_id": "agent:opencode-alice",
  "status": "registered",
  "delegation_chain": [
    "user:alice@acme.com",
    "team:api-team",
    "org:platform-engineering",
    "company:acme-corp"
  ],
  "effective_capabilities": {
    "can_read_memory": true,
    "can_write_memory": true,
    "can_propose_policies": true,
    "can_approve_policies": false,
    "max_policy_severity": "warn"
  },
  "token": "aeterna_agent_xxxxx"
}
```

**Key Principle**: Agents NEVER have more permissions than their delegating user.

---

## Part 8: Context Resolution

Context resolution determines WHO you are and WHERE you're operating, automatically.

### 8.1 Resolution Priority

```
1. Explicit CLI flags (--company, --org, --team, --project)
   ↓ if not specified
2. Environment variables (AETERNA_COMPANY, AETERNA_ORG, etc.)
   ↓ if not set
3. Local config file (.aeterna/context.toml)
   ↓ if not present
4. Parent directory traversal (walk up looking for .aeterna/)
   ↓ if not found
5. Git remote detection (remote URL → project mapping)
   ↓ if no match
6. Git user detection (user.email → user mapping)
   ↓ if no match
7. SSO/JWT claims (if authenticated)
   ↓ if no auth
8. Interactive prompt (ask user)
```

### 8.2 Context File Schema

**.aeterna/context.toml**:
```toml
[context]
company = "acme-corp"
org = "platform-engineering"
team = "api-team"
project = "payments-service"

[user]
# Can be auto-populated from git user.email
email = "alice@acme.com"
# Override for specific project if different from global
# email = "alice.contractor@acme.com"

[agent]
# If running as an agent
id = "agent:opencode-alice"
delegated_by = "alice@acme.com"

[defaults]
# Default layer for memory operations
memory_layer = "project"
# Default scope for policy commands
policy_scope = "project"
# Auto-sync memory-knowledge
auto_sync = true
```

### 8.3 Tool: `aeterna_context_resolve`

**Purpose**: Resolve and display current context.

**Input**:
```json
{
  "format": "detailed"
}
```

**Output**:
```json
{
  "resolved": {
    "company": {
      "value": "acme-corp",
      "source": "context.toml"
    },
    "org": {
      "value": "platform-engineering",
      "source": "context.toml"
    },
    "team": {
      "value": "api-team",
      "source": "git_remote"
    },
    "project": {
      "value": "payments-service",
      "source": "git_remote"
    },
    "user": {
      "value": "alice@acme.com",
      "source": "git_user.email"
    }
  },
  "effective_scope": "company:acme-corp/org:platform-engineering/team:api-team/project:payments-service",
  "permissions": {
    "memory": ["read", "write", "promote:team"],
    "knowledge": ["read", "propose"],
    "policy": ["read", "propose"]
  }
}
```

**CLI Equivalent**:
```bash
$ aeterna context show

📍 Current Context
   Company:  acme-corp (from context.toml)
   Org:      platform-engineering (from context.toml)
   Team:     api-team (from git remote)
   Project:  payments-service (from git remote)
   User:     alice@acme.com (from git user.email)

🔑 Permissions
   Memory:    read, write, promote to team
   Knowledge: read, propose
   Policy:    read, propose

$ aeterna context set --team backend
✅ Context updated: team = backend (saved to .aeterna/context.toml)

$ aeterna context clear
✅ Context cleared, will auto-detect on next operation
```

### 8.4 Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `AETERNA_COMPANY` | Company identifier | `acme-corp` |
| `AETERNA_ORG` | Organization identifier | `platform-engineering` |
| `AETERNA_TEAM` | Team identifier | `api-team` |
| `AETERNA_PROJECT` | Project identifier | `payments-service` |
| `AETERNA_USER` | User email (override git) | `alice@acme.com` |
| `AETERNA_AGENT_ID` | Agent identifier (for CI/CD) | `agent:ci-bot` |
| `AETERNA_AGENT_TOKEN` | Agent auth token | `aeterna_agent_xxxxx` |
| `AETERNA_API_URL` | API endpoint | `https://aeterna.acme.com` |

### 8.5 Git Remote Mapping

```rust
pub struct GitContextDetector {
    remote_patterns: Vec<RemotePattern>,
}

impl GitContextDetector {
    pub fn detect(&self, repo_path: &Path) -> Option<DetectedContext> {
        // Get git remote URL
        let remote = self.get_remote_url(repo_path)?;
        
        // Match against registered patterns
        // e.g., "github.com/acme-corp/payments-service" 
        //       → company: acme-corp, project: payments-service
        
        for pattern in &self.remote_patterns {
            if let Some(captures) = pattern.regex.captures(&remote) {
                return Some(DetectedContext {
                    company: captures.name("company").map(|m| m.as_str().to_string()),
                    org: captures.name("org").map(|m| m.as_str().to_string()),
                    team: captures.name("team").map(|m| m.as_str().to_string()),
                    project: captures.name("project").map(|m| m.as_str().to_string()),
                    source: ContextSource::GitRemote,
                });
            }
        }
        
        // Fallback: extract org/repo from URL
        self.parse_generic_remote(&remote)
    }
}
```

---

## Part 9: Memory & Knowledge Discovery UX

Discovery tools let users find and explore memories and knowledge without knowing internal structure.

### 9.1 Tool: `aeterna_memory_search` (Enhanced)

**Purpose**: Natural language search across all accessible memory layers.

**Input**:
```json
{
  "query": "database decisions we made last month",
  "filters": {
    "time_range": "last_30d",
    "layers": "auto",  // auto = all accessible layers
    "min_relevance": 0.7
  }
}
```

**Output**:
```json
{
  "results": [
    {
      "id": "mem_abc123",
      "content": "Decided to use PostgreSQL for all new services per ADR-042",
      "layer": "team",
      "relevance": 0.95,
      "created_at": "2024-01-10T10:00:00Z",
      "created_by": "alice@acme.com",
      "source": "team:api-team"
    },
    {
      "id": "mem_def456",
      "content": "Redis for caching, but not for primary data storage",
      "layer": "org",
      "relevance": 0.82,
      "created_at": "2024-01-05T14:00:00Z",
      "created_by": "bob@acme.com",
      "source": "org:platform-engineering"
    }
  ],
  "total": 2,
  "searched_layers": ["agent", "user", "session", "project", "team", "org"],
  "query_interpretation": "Looking for memories about database technology decisions from the past 30 days"
}
```

**CLI Equivalent**:
```bash
$ aeterna memory search "database decisions last month"

Found 2 memories:

[95%] team:api-team - 2024-01-10
   "Decided to use PostgreSQL for all new services per ADR-042"
   by alice@acme.com

[82%] org:platform-engineering - 2024-01-05
   "Redis for caching, but not for primary data storage"
   by bob@acme.com
```

### 9.2 Tool: `aeterna_memory_browse`

**Purpose**: Interactive exploration of memories by layer or category.

**Input**:
```json
{
  "mode": "by_layer",
  "layer": "team",
  "pagination": {
    "page": 1,
    "per_page": 10
  }
}
```

**Output**:
```json
{
  "layer": "team",
  "layer_name": "API Team",
  "total_memories": 47,
  "categories": [
    {"name": "decisions", "count": 15},
    {"name": "learnings", "count": 12},
    {"name": "conventions", "count": 10},
    {"name": "gotchas", "count": 7},
    {"name": "other", "count": 3}
  ],
  "memories": [
    {
      "id": "mem_001",
      "content": "PostgreSQL for new services...",
      "category": "decisions",
      "created_at": "2024-01-10"
    }
    // ... more
  ],
  "pagination": {
    "page": 1,
    "per_page": 10,
    "total_pages": 5
  }
}
```

**CLI Equivalent**:
```bash
$ aeterna memory browse --layer team

📂 Team: API Team (47 memories)

Categories:
   decisions (15)  learnings (12)  conventions (10)  gotchas (7)  other (3)

Recent memories:
   1. [decisions] PostgreSQL for new services per ADR-042
   2. [learnings] KApp has 20-char ID limit
   3. [conventions] Use snake_case for API fields
   ...

Use 'aeterna memory browse --layer team --category decisions' to filter
```

### 9.3 Tool: `aeterna_memory_promote`

**Purpose**: Promote a memory to a broader scope with governance.

**Input**:
```json
{
  "memory_id": "mem_abc123",
  "to_layer": "org",
  "reason": "This decision applies to all teams in Platform Engineering",
  "notify": ["@platform-leads"]
}
```

**Output**:
```json
{
  "original_id": "mem_abc123",
  "promoted_id": "mem_org_xyz789",
  "status": "pending_approval",  // or "promoted" if auto-approved
  "from_layer": "team",
  "to_layer": "org",
  "approval_required": true,
  "approvers_notified": ["bob@acme.com"],
  "reason": "This decision applies to all teams in Platform Engineering"
}
```

**CLI Equivalent**:
```bash
$ aeterna memory promote mem_abc123 --to org --reason "Applies to all Platform teams"

⬆️ Promoting memory to org layer...

Original: team:api-team → Target: org:platform-engineering

Content: "Decided to use PostgreSQL for all new services per ADR-042"

Promotion requires approval from: bob@acme.com (org lead)
Submit promotion request? [y/N]: y

✅ Promotion requested (pending approval)
   Notified: bob@acme.com
```

### 9.4 Tool: `aeterna_knowledge_search` (Enhanced)

**Purpose**: Semantic search across knowledge repository.

**Input**:
```json
{
  "query": "how do we handle authentication",
  "types": ["adr", "pattern", "spec"],
  "include_inherited": true
}
```

**Output**:
```json
{
  "results": [
    {
      "id": "adr-015",
      "type": "adr",
      "title": "Use JWT for API Authentication",
      "summary": "All internal APIs use JWT tokens with RS256 signing",
      "layer": "org",
      "relevance": 0.94,
      "status": "accepted"
    },
    {
      "id": "pattern-auth-middleware",
      "type": "pattern",
      "title": "Authentication Middleware Pattern",
      "summary": "Standard middleware pattern for validating JWT in Express/Fastify",
      "layer": "team",
      "relevance": 0.87
    }
  ],
  "total": 2,
  "query_interpretation": "Looking for authentication-related architectural decisions and patterns"
}
```

### 9.5 Tool: `aeterna_knowledge_browse`

**Purpose**: Explore knowledge repository by type and layer.

**Input**:
```json
{
  "type": "adr",
  "layer": "all",
  "status": "accepted"
}
```

**Output**:
```json
{
  "type": "adr",
  "total": 23,
  "by_layer": {
    "company": 5,
    "org": 12,
    "team": 4,
    "project": 2
  },
  "items": [
    {
      "id": "adr-001",
      "title": "Use Rust for Core Services",
      "layer": "company",
      "status": "accepted",
      "date": "2023-06-15"
    }
    // ... more
  ]
}
```

### 9.6 Tool: `aeterna_knowledge_propose`

**Purpose**: Propose new knowledge item from natural language.

**Input**:
```json
{
  "description": "We should document that all new APIs must use GraphQL instead of REST",
  "suggested_type": "auto",
  "layer": "team"
}
```

**Output**:
```json
{
  "proposal_id": "prop_k_001",
  "interpreted_as": {
    "type": "adr",
    "title": "Use GraphQL for New APIs",
    "summary": "All new API endpoints should be implemented in GraphQL rather than REST",
    "rationale": "Extracted from proposal: 'We should document that all new APIs must use GraphQL instead of REST'"
  },
  "draft_content": "## Status\nProposed\n\n## Context\n[To be filled]\n\n## Decision\nAll new API endpoints should be implemented in GraphQL rather than REST.\n\n## Consequences\n[To be filled]",
  "next_steps": [
    "Review and edit the draft",
    "Add context and consequences",
    "Submit for team approval"
  ]
}
```

**CLI Equivalent**:
```bash
$ aeterna knowledge propose "All new APIs must use GraphQL"

🔍 Analyzing your proposal...

Interpreted as: ADR (Architectural Decision Record)
   Title: Use GraphQL for New APIs
   Layer: team (api-team)
   
Draft created: .aeterna/drafts/adr-graphql-apis.md

Edit the draft and submit:
   aeterna knowledge submit .aeterna/drafts/adr-graphql-apis.md
```

---

## Part 10: Day-to-Day Workflows by Persona

### 10.1 Developer Workflow

**Start of Day**:
```bash
# Quick status check
$ aeterna status

📊 Aeterna Status for payments-service

Context: acme-corp → platform-engineering → api-team → payments-service
User: alice@acme.com (developer)

📝 Pending for you:
   • 1 policy proposal awaiting your review
   • 2 knowledge items marked for attention

🧠 Recent team learnings:
   • "Use batch inserts for bulk operations" (2d ago)
   • "PostgreSQL EXPLAIN ANALYZE for slow queries" (3d ago)

⚠️ Active constraints:
   • No MySQL dependencies (project policy)
   • Require opentelemetry (org policy)
```

**During Development**:
```bash
# Search for relevant knowledge
$ aeterna knowledge search "how to handle database migrations"

# Check if a dependency is allowed
$ aeterna check dependency mysql
❌ BLOCKED: MySQL dependencies prohibited by policy 'no-mysql'
   Reason: Standardized on PostgreSQL per ADR-042
   Alternatives: pg, postgres

# Record a learning
$ aeterna memory add "Discovered that batch size > 1000 causes timeout in payment processor"
✅ Memory saved to project layer

# Search memories
$ aeterna memory search "payment processor limits"
```

**AI Agent Workflow (via MCP)**:
```python
# Agent starts task
context = await aeterna_context_resolve()

# Agent searches for relevant knowledge before coding
knowledge = await aeterna_knowledge_search(
    query="database migration patterns in this project"
)

# Agent checks constraints before suggesting code
check = await aeterna_knowledge_check(
    context={"dependencies": ["mysql2"]},
    scope="project"
)
if check.violations:
    # Inform user, don't proceed
    return f"Cannot add mysql2: {check.violations[0].message}"

# Agent records successful approach
if task_completed_successfully:
    await aeterna_memory_add(
        content="Used pg-migrate for schema versioning, worked well",
        layer="project"
    )
```

### 10.2 Tech Lead Workflow

**Daily Governance**:
```bash
# Review pending approvals
$ aeterna govern pending

📋 Pending Approvals for Tech Lead

Policies:
   1. [prop_001] no-console-log (team) - proposed by agent:opencode-charlie
      "Prevent console.log in production code"
      → aeterna govern approve prop_001

Knowledge:
   2. [prop_k_002] ADR: GraphQL for New APIs (team) - proposed by bob@acme.com
      → aeterna govern approve prop_k_002

Memory Promotions:
   3. [prom_003] "Batch size limit" project → team - proposed by alice@acme.com
      → aeterna govern approve prom_003

# Approve policy
$ aeterna govern approve prop_001 --comment "Good catch, LGTM"
✅ Policy 'no-console-log' activated for team:api-team

# Review and approve knowledge
$ aeterna knowledge show prop_k_002
$ aeterna govern approve prop_k_002
```

**Team Oversight**:
```bash
# View team activity
$ aeterna govern audit --scope team --last 7d

📊 Team Governance Activity (Last 7 Days)

Policies: 3 proposed, 2 approved, 1 rejected
Knowledge: 5 proposed, 5 approved
Memory Promotions: 8 requested, 7 approved, 1 pending

Top Contributors:
   alice@acme.com: 12 contributions
   bob@acme.com: 8 contributions
   agent:opencode-alice: 5 proposals

# Check policy drift
$ aeterna admin drift --scope team
✅ No policy drift detected
```

### 10.3 Architect Workflow

**Organization Governance**:
```bash
# Set up governance for new org
$ aeterna govern configure --scope org:data-platform --interactive

# Create organization-wide policy
$ aeterna policy create "All services must implement health checks" \
    --scope org --severity warn

# Review cross-team knowledge
$ aeterna knowledge browse --type pattern --layer org

# Promote successful team pattern to org
$ aeterna knowledge promote pattern-circuit-breaker --from team:api --to org \
    --reason "Proven effective, should be standard"
```

**Architecture Oversight**:
```bash
# View all ADRs
$ aeterna knowledge list --type adr --layer company,org

# Check constraint compliance across org
$ aeterna admin validate --scope org

📋 Validation Report for org:platform-engineering

Teams: 4
Projects: 12

Compliance:
   ✅ security-baseline: 12/12 compliant
   ⚠️ require-opentelemetry: 10/12 compliant
      - legacy-api: missing opentelemetry
      - batch-processor: missing opentelemetry
   ✅ no-mysql: 12/12 compliant

# Create company-wide policy
$ aeterna policy create "Block AWS SDK v2, use v3 only" \
    --scope company --severity block --justification "v2 EOL in 2024"
```

### 10.4 Admin Workflow

**Organization Setup**:
```bash
# Initialize new company
$ aeterna init --company "Acme Corp" --admin admin@acme.com

# Create organizational structure
$ aeterna org create "Engineering"
$ aeterna org create "Product"
$ aeterna org create "Security"

# Create teams
$ aeterna team create "Platform" --org engineering --lead alice@acme.com
$ aeterna team create "Mobile" --org product --lead bob@acme.com

# Configure company-wide governance
$ aeterna govern configure --scope company \
    --policy-approvers admin \
    --review-period 72h
```

**User Management**:
```bash
# Invite users
$ aeterna user invite new-hire@acme.com --teams platform --role developer

# Manage roles
$ aeterna govern roles assign alice@acme.com architect --scope org:engineering

# Register CI/CD agent
$ aeterna agent register --id ci-bot \
    --delegated-by admin@acme.com \
    --scope company \
    --capabilities read_memory,read_knowledge,check_constraints

# View all agents
$ aeterna agent list
```

**Compliance & Audit**:
```bash
# Generate compliance report
$ aeterna admin audit --scope company --from 2024-01-01 --format pdf \
    > compliance-q1-2024.pdf

# Export policies for review
$ aeterna admin export policies --scope company --format yaml \
    > company-policies.yaml

# Health check
$ aeterna admin health --verbose
```

### 10.5 AI Agent Autonomous Workflow

**Agent Registration & Capabilities**:
```python
# Agent startup (e.g., in OpenCode)
async def agent_startup():
    # Register with Aeterna
    registration = await aeterna_agent_register(
        agent_id="agent:opencode-alice",
        delegated_by="alice@acme.com",
        scope="project:payments-service",
        capabilities={
            "can_read_memory": True,
            "can_write_memory": True,
            "can_propose_policies": True,
            "can_approve_policies": False,
            "max_policy_severity": "warn"
        }
    )
    
    # Agent now operates with alice's permissions, limited by delegation
    return registration.token
```

**Agent Task Execution**:
```python
async def agent_coding_task(task: str):
    # 1. Get context automatically
    context = await aeterna_context_resolve()
    
    # 2. Search for relevant knowledge before starting
    knowledge = await aeterna_knowledge_search(
        query=f"relevant to: {task}"
    )
    
    # 3. Search for team learnings
    memories = await aeterna_memory_search(
        query=task,
        filters={"layers": ["team", "project"]}
    )
    
    # 4. Check constraints for planned changes
    check = await aeterna_knowledge_check(
        context=planned_changes
    )
    
    if check.has_blocking_violations:
        # Cannot proceed, inform user
        return f"Blocked by policy: {check.blocking_message}"
    
    # 5. Execute task...
    result = await execute_task(task)
    
    # 6. Record learnings if successful
    if result.success and result.learnings:
        await aeterna_memory_add(
            content=result.learnings,
            layer="project"
        )
    
    return result
```

**Agent Policy Proposal**:
```python
async def agent_propose_policy_from_observation():
    # Agent detected repeated pattern violation
    observation = "47 console.log statements in 12 PRs this week"
    
    # Agent can propose (but not approve) policies
    draft = await aeterna_policy_draft(
        intent="Prevent console.log in production TypeScript",
        scope="team",
        severity="warn",
        context={"observation": observation}
    )
    
    # Simulate impact
    simulation = await aeterna_policy_simulate(
        draft_id=draft.draft_id
    )
    
    if simulation.would_affect_existing:
        # Inform human, don't auto-propose
        await notify_human(
            message=f"Would affect {len(simulation.affected)} existing items",
            action="review_policy_draft",
            draft_id=draft.draft_id
        )
    else:
        # Safe to propose
        proposal = await aeterna_policy_propose(
            draft_id=draft.draft_id,
            justification="Automated observation of repeated violations"
        )
        # Human approval still required
```

---

## Summary: Tool Inventory

### Onboarding Tools
| Tool | Purpose |
|------|---------|
| `aeterna_org_init` | Initialize company/organization |
| `aeterna_team_create` | Create team within org |
| `aeterna_project_init` | Initialize project (auto-detect git) |
| `aeterna_user_register` | Register user identity |
| `aeterna_agent_register` | Register AI agent with delegation |

### Context Tools
| Tool | Purpose |
|------|---------|
| `aeterna_context_resolve` | Resolve current context |
| `aeterna_context_set` | Override context |
| `aeterna_context_clear` | Reset to auto-detection |

### Policy Tools (Part 1)
| Tool | Purpose |
|------|---------|
| `aeterna_policy_draft` | Natural language → Cedar |
| `aeterna_policy_validate` | Validate Cedar syntax |
| `aeterna_policy_propose` | Submit for approval |
| `aeterna_policy_list` | List active policies |
| `aeterna_policy_explain` | Cedar → Natural language |
| `aeterna_policy_simulate` | Test against scenarios |

### Governance Tools (Part 2)
| Tool | Purpose |
|------|---------|
| `aeterna_governance_configure` | Set meta-governance rules |
| `aeterna_governance_roles` | Manage role assignments |
| `aeterna_governance_approve` | Approve proposals |
| `aeterna_governance_reject` | Reject proposals |
| `aeterna_governance_audit` | View audit trail |

### Memory Discovery Tools (Part 9)
| Tool | Purpose |
|------|---------|
| `aeterna_memory_search` | Natural language search |
| `aeterna_memory_browse` | Interactive exploration |
| `aeterna_memory_promote` | Promote to broader scope |
| `aeterna_memory_attribute` | Show provenance |

### Knowledge Discovery Tools (Part 9)
| Tool | Purpose |
|------|---------|
| `aeterna_knowledge_search` | Semantic search |
| `aeterna_knowledge_browse` | Explore by type/layer |
| `aeterna_knowledge_propose` | Propose from NL |
| `aeterna_knowledge_explain` | Plain English explanation |

### CLI Quick Reference
```bash
# Onboarding
aeterna init                     # Full setup wizard
aeterna org create NAME          # Create organization
aeterna team create NAME         # Create team
aeterna project init             # Initialize current directory

# Context
aeterna context show             # Display resolved context
aeterna context set --team X     # Override context
aeterna context clear            # Reset to auto-detection

# Memory
aeterna memory search QUERY      # Natural language search
aeterna memory browse            # Interactive exploration
aeterna memory add CONTENT       # Add new memory
aeterna memory promote ID --to LAYER

# Knowledge
aeterna knowledge search QUERY   # Semantic search
aeterna knowledge browse         # Explore repository
aeterna knowledge propose DESC   # Propose from description

# Policy
aeterna policy create DESC       # Create from NL
aeterna policy list              # List active policies
aeterna policy explain ID        # Explain in plain English
aeterna policy simulate DRAFT_ID # Test impact

# Governance
aeterna govern status            # Quick overview
aeterna govern pending           # Show pending approvals
aeterna govern approve ID        # Approve proposal
aeterna govern audit             # View activity log

# Admin
aeterna admin health             # System health check
aeterna admin validate           # Check all constraints
aeterna admin export             # Export configuration
```

---

## Part 11: OPAL Integration & Organizational Referential

### 11.1 The Missing Piece: Organizational Topology

Previous parts assume we know **who the user is** and **what hierarchy they belong to**. But where does this come from?

**The Problem:**
- Git remote URL tells us `github.com/acme-corp/payments-service` but not:
  - Which team owns `payments-service`?
  - Which org does that team belong to?
  - What role does `alice@acme.com` have?
  - Can agent `build-agent-7` act on behalf of `alice`?

**Without a source of truth**, context resolution is guesswork. OPAL provides that source of truth.

### 11.2 Why OPAL?

[OPAL (Open Policy Administration Layer)](https://github.com/permitio/opal) solves the "data + policy sync" problem:

| Feature | Benefit for Aeterna |
|---------|---------------------|
| **Real-time sync** | Organization changes propagate instantly to all agents |
| **Native Cedar support** | [cedar-agent](https://github.com/permitio/cedar-agent) via OPAL Client |
| **Pluggable data fetchers** | PostgreSQL, APIs, IdPs as data sources |
| **WebSocket PubSub** | No polling; instant updates |
| **Apache 2.0** | Self-hostable, no vendor lock-in |
| **Battle-tested** | Used by Permit.io, production-ready |

**Architecture:**
```
┌─────────────────────────────────────────────────────────────────┐
│                         OPAL SERVER                              │
│                                                                  │
│   Data Sources:                  Policy Sources:                │
│   • PostgreSQL (org hierarchy)   • Git repo (Cedar policies)    │
│   • IdP (Okta, Azure AD)         • Knowledge repo policies      │
│                                                                  │
│              WebSocket PubSub → Real-time updates               │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          ▼                       ▼                       ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  OPAL Client 1  │     │  OPAL Client 2  │     │  OPAL Client N  │
│  + Cedar Agent  │     │  + Cedar Agent  │     │  + Cedar Agent  │
│                 │     │                 │     │                 │
│  (Pod 1)        │     │  (Pod 2)        │     │  (Pod N)        │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                         AETERNA CORE                             │
│                                                                  │
│   • Queries Cedar Agent for authorization                       │
│   • Queries Cedar Agent for context resolution                  │
│   • Writes to PostgreSQL (the referential)                      │
│   • Never exposes OPAL/Cedar to users                           │
└─────────────────────────────────────────────────────────────────┘
```

### 11.3 Organizational Referential Schema (PostgreSQL)

This is the **source of truth** for "who belongs to what":

```sql
-- ============================================================================
-- ORGANIZATIONAL REFERENTIAL SCHEMA
-- ============================================================================
-- This schema is the authoritative source for organizational topology.
-- OPAL fetches this data and syncs it to Cedar Agents in real-time.
-- ============================================================================

-- Companies (root of hierarchy, maps to tenant)
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) UNIQUE NOT NULL,  -- 'acme-corp'
    name VARCHAR(255) NOT NULL,          -- 'Acme Corporation'
    settings JSONB DEFAULT '{}',         -- Company-wide config
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_companies_slug ON companies(slug);

-- Organizations (departments within company)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    slug VARCHAR(255) NOT NULL,          -- 'platform-engineering'
    name VARCHAR(255) NOT NULL,          -- 'Platform Engineering'
    settings JSONB DEFAULT '{}',         -- Org-specific config
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(company_id, slug)
);

CREATE INDEX idx_organizations_company ON organizations(company_id);

-- Teams (working groups within org)
CREATE TABLE teams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    slug VARCHAR(255) NOT NULL,          -- 'api-team'
    name VARCHAR(255) NOT NULL,          -- 'API Team'
    settings JSONB DEFAULT '{}',         -- Team-specific config
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(org_id, slug)
);

CREATE INDEX idx_teams_org ON teams(org_id);

-- Projects (repositories, owned by teams)
CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    slug VARCHAR(255) NOT NULL,          -- 'payments-service'
    name VARCHAR(255) NOT NULL,          -- 'Payments Service'
    git_remote VARCHAR(512),             -- 'git@github.com:acme-corp/payments-service.git'
    settings JSONB DEFAULT '{}',         -- Project-specific config
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(team_id, slug)
);

CREATE INDEX idx_projects_team ON projects(team_id);
CREATE INDEX idx_projects_git_remote ON projects(git_remote);

-- Users (humans who interact with the system)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,  -- 'alice@acme.com'
    display_name VARCHAR(255),           -- 'Alice Smith'
    idp_subject VARCHAR(255),            -- From SSO (sub claim)
    idp_provider VARCHAR(100),           -- 'okta', 'azure-ad', 'github'
    settings JSONB DEFAULT '{}',         -- User preferences
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_idp_subject ON users(idp_subject);

-- Agents (AI agents with delegated authority)
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(255) UNIQUE NOT NULL,   -- 'build-agent-7', 'review-bot'
    display_name VARCHAR(255),           -- 'Build Agent #7'
    delegated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    scope_type VARCHAR(50) NOT NULL,     -- 'company', 'org', 'team', 'project'
    scope_id UUID NOT NULL,              -- Reference to company/org/team/project
    capabilities JSONB NOT NULL DEFAULT '[]',  -- ['memory_read', 'memory_write', 'knowledge_read']
    max_role VARCHAR(50) DEFAULT 'developer',  -- Maximum role agent can assume
    expires_at TIMESTAMPTZ,              -- Optional expiration
    revoked_at TIMESTAMPTZ,              -- Soft-delete for revocation
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT valid_scope_type CHECK (scope_type IN ('company', 'org', 'team', 'project'))
);

CREATE INDEX idx_agents_slug ON agents(slug);
CREATE INDEX idx_agents_delegated_by ON agents(delegated_by);
CREATE INDEX idx_agents_scope ON agents(scope_type, scope_id);

-- Memberships (user ↔ team with role)
CREATE TABLE memberships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL,           -- 'admin', 'architect', 'tech_lead', 'developer'
    granted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    granted_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,              -- Optional role expiration
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, team_id),
    CONSTRAINT valid_role CHECK (role IN ('admin', 'architect', 'tech_lead', 'developer'))
);

CREATE INDEX idx_memberships_user ON memberships(user_id);
CREATE INDEX idx_memberships_team ON memberships(team_id);

-- Git remote patterns (for auto-detection of project context)
CREATE TABLE git_remote_patterns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pattern VARCHAR(512) NOT NULL,       -- Regex: '^git@github\.com:acme-corp/.*$'
    company_id UUID REFERENCES companies(id) ON DELETE CASCADE,
    org_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    priority INT DEFAULT 0,              -- Higher = matched first
    created_at TIMESTAMPTZ DEFAULT NOW(),
    -- At least one scope must be set
    CONSTRAINT at_least_one_scope CHECK (
        company_id IS NOT NULL OR org_id IS NOT NULL OR 
        team_id IS NOT NULL OR project_id IS NOT NULL
    )
);

CREATE INDEX idx_git_remote_patterns_priority ON git_remote_patterns(priority DESC);

-- Email domain patterns (for auto-detection of user company)
CREATE TABLE email_domain_patterns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_pattern VARCHAR(255) NOT NULL,  -- '@acme.com', '@*.acme.com'
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    priority INT DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_email_domain_patterns_priority ON email_domain_patterns(priority DESC);

-- Audit log (for governance trail)
CREATE TABLE referential_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    action VARCHAR(50) NOT NULL,         -- 'create', 'update', 'delete', 'grant', 'revoke'
    entity_type VARCHAR(50) NOT NULL,    -- 'company', 'org', 'team', 'project', 'user', 'agent', 'membership'
    entity_id UUID NOT NULL,
    actor_type VARCHAR(50) NOT NULL,     -- 'user', 'agent', 'system'
    actor_id UUID,
    changes JSONB,                        -- JSON diff of changes
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_log_entity ON referential_audit_log(entity_type, entity_id);
CREATE INDEX idx_audit_log_actor ON referential_audit_log(actor_type, actor_id);
CREATE INDEX idx_audit_log_created ON referential_audit_log(created_at DESC);

-- ============================================================================
-- VIEWS FOR OPAL DATA FETCHER
-- ============================================================================

-- Denormalized view of full hierarchy (for Cedar entity data)
CREATE VIEW v_hierarchy AS
SELECT 
    c.id AS company_id,
    c.slug AS company_slug,
    o.id AS org_id,
    o.slug AS org_slug,
    t.id AS team_id,
    t.slug AS team_slug,
    p.id AS project_id,
    p.slug AS project_slug,
    p.git_remote
FROM companies c
LEFT JOIN organizations o ON o.company_id = c.id
LEFT JOIN teams t ON t.org_id = o.id
LEFT JOIN projects p ON p.team_id = t.id;

-- User permissions view (for Cedar authorization)
CREATE VIEW v_user_permissions AS
SELECT 
    u.id AS user_id,
    u.email,
    m.team_id,
    m.role,
    t.org_id,
    o.company_id
FROM users u
JOIN memberships m ON m.user_id = u.id
JOIN teams t ON t.id = m.team_id
JOIN organizations o ON o.id = t.org_id
WHERE m.expires_at IS NULL OR m.expires_at > NOW();

-- Agent permissions view (for Cedar authorization)
CREATE VIEW v_agent_permissions AS
SELECT 
    a.id AS agent_id,
    a.slug AS agent_slug,
    a.delegated_by AS delegating_user_id,
    u.email AS delegating_user_email,
    a.scope_type,
    a.scope_id,
    a.capabilities,
    a.max_role
FROM agents a
LEFT JOIN users u ON u.id = a.delegated_by
WHERE a.revoked_at IS NULL
  AND (a.expires_at IS NULL OR a.expires_at > NOW());
```

### 11.4 OPAL Server Configuration

**docker-compose.yml addition:**

```yaml
services:
  # ... existing services ...

  opal-server:
    image: permitio/opal-server:latest
    ports:
      - "7002:7002"  # OPAL Server API
    environment:
      # Broadcast channel (for multi-instance deployments)
      OPAL_BROADCAST_URI: "postgres://aeterna:aeterna@postgres:5432/aeterna"
      
      # Policy repository (Cedar policies from knowledge repo)
      OPAL_POLICY_REPO_URL: "https://github.com/acme-corp/aeterna-policies.git"
      OPAL_POLICY_REPO_MAIN_BRANCH: "main"
      OPAL_POLICY_REPO_POLLING_INTERVAL: 30
      
      # Data configuration
      OPAL_DATA_CONFIG_SOURCES: '{"config": {"entries": [{"url": "http://opal-fetcher:8080/hierarchy", "topics": ["hierarchy"], "dst_path": "hierarchy"}, {"url": "http://opal-fetcher:8080/users", "topics": ["users"], "dst_path": "users"}, {"url": "http://opal-fetcher:8080/agents", "topics": ["agents"], "dst_path": "agents"}]}}'
      
      # Authentication
      OPAL_AUTH_PRIVATE_KEY_PATH: "/app/keys/opal_private.pem"
      OPAL_AUTH_PUBLIC_KEY_PATH: "/app/keys/opal_public.pem"
    volumes:
      - ./keys:/app/keys:ro
    depends_on:
      - postgres
      - opal-fetcher

  opal-fetcher:
    build:
      context: ./opal-fetcher
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: "postgres://aeterna:aeterna@postgres:5432/aeterna"
      OPAL_SERVER_URL: "http://opal-server:7002"
    depends_on:
      - postgres

  cedar-agent:
    image: permitio/cedar-agent:latest
    ports:
      - "8180:8180"  # Cedar Agent API
    environment:
      # Connect to OPAL Server
      OPAL_SERVER_URL: "http://opal-server:7002"
      
      # Cedar Agent configuration
      CEDAR_AGENT_ADDR: "0.0.0.0:8180"
      
      # Authentication token (generated by OPAL Server)
      OPAL_CLIENT_TOKEN: "${OPAL_CLIENT_TOKEN}"
    depends_on:
      - opal-server
```

### 11.5 Custom OPAL Data Fetcher

The data fetcher queries PostgreSQL and formats data for Cedar Agent:

**`opal-fetcher/src/main.rs`:**

```rust
//! OPAL Data Fetcher for Aeterna
//! 
//! Queries PostgreSQL organizational referential and formats
//! data for Cedar Agent consumption via OPAL.

use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct HierarchyData {
    companies: Vec<CompanyEntity>,
    organizations: Vec<OrgEntity>,
    teams: Vec<TeamEntity>,
    projects: Vec<ProjectEntity>,
}

#[derive(Debug, Serialize)]
struct CompanyEntity {
    uid: String,  // "Company::\"acme-corp\""
    attrs: CompanyAttrs,
}

#[derive(Debug, Serialize)]
struct CompanyAttrs {
    slug: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct OrgEntity {
    uid: String,  // "Organization::\"platform-engineering\""
    attrs: OrgAttrs,
    parents: Vec<String>,  // ["Company::\"acme-corp\""]
}

// ... similar for TeamEntity, ProjectEntity

#[derive(Debug, Serialize)]
struct UsersData {
    users: Vec<UserEntity>,
    memberships: Vec<MembershipEntity>,
}

#[derive(Debug, Serialize)]
struct UserEntity {
    uid: String,  // "User::\"alice@acme.com\""
    attrs: UserAttrs,
    parents: Vec<String>,  // Teams user belongs to
}

#[derive(Debug, Serialize)]
struct UserAttrs {
    email: String,
    display_name: Option<String>,
    roles: HashMap<String, String>,  // team_uid -> role
}

#[derive(Debug, Serialize)]
struct AgentsData {
    agents: Vec<AgentEntity>,
}

#[derive(Debug, Serialize)]
struct AgentEntity {
    uid: String,  // "Agent::\"build-agent-7\""
    attrs: AgentAttrs,
    parents: Vec<String>,  // Delegating user + scope
}

#[derive(Debug, Serialize)]
struct AgentAttrs {
    slug: String,
    delegated_by: Option<String>,  // User UID
    scope_type: String,
    scope_uid: String,
    capabilities: Vec<String>,
    max_role: String,
}

async fn get_hierarchy(pool: &PgPool) -> Json<HierarchyData> {
    // Query v_hierarchy view
    let rows = sqlx::query_as!(
        HierarchyRow,
        r#"SELECT * FROM v_hierarchy"#
    )
    .fetch_all(pool)
    .await
    .unwrap();

    // Transform to Cedar entity format
    let mut companies = HashMap::new();
    let mut organizations = HashMap::new();
    let mut teams = HashMap::new();
    let mut projects = vec![];

    for row in rows {
        // Build entities with proper Cedar UIDs
        // Company::\"acme-corp\"
        // Organization::\"acme-corp/platform-engineering\"
        // Team::\"acme-corp/platform-engineering/api-team\"
        // Project::\"acme-corp/platform-engineering/api-team/payments-service\"
        // ...
    }

    Json(HierarchyData {
        companies: companies.into_values().collect(),
        organizations: organizations.into_values().collect(),
        teams: teams.into_values().collect(),
        projects,
    })
}

async fn get_users(pool: &PgPool) -> Json<UsersData> {
    let rows = sqlx::query_as!(
        UserPermissionRow,
        r#"SELECT * FROM v_user_permissions"#
    )
    .fetch_all(pool)
    .await
    .unwrap();

    // Transform to Cedar entity format
    // ...

    Json(UsersData { users, memberships })
}

async fn get_agents(pool: &PgPool) -> Json<AgentsData> {
    let rows = sqlx::query_as!(
        AgentPermissionRow,
        r#"SELECT * FROM v_agent_permissions"#
    )
    .fetch_all(pool)
    .await
    .unwrap();

    // Transform to Cedar entity format with delegation chain
    // ...

    Json(AgentsData { agents })
}

// Webhook endpoint for PostgreSQL NOTIFY
async fn handle_data_update(
    pool: &PgPool,
    opal_client: &OpalClient,
    notification: PgNotification,
) {
    match notification.channel() {
        "hierarchy_change" => {
            opal_client.publish_update("hierarchy").await;
        }
        "user_change" => {
            opal_client.publish_update("users").await;
        }
        "agent_change" => {
            opal_client.publish_update("agents").await;
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    let app = Router::new()
        .route("/hierarchy", get(|| get_hierarchy(&pool)))
        .route("/users", get(|| get_users(&pool)))
        .route("/agents", get(|| get_agents(&pool)));

    // Start PostgreSQL LISTEN for real-time updates
    tokio::spawn(listen_for_changes(pool.clone()));

    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
```

**PostgreSQL triggers for real-time sync:**

```sql
-- Notify OPAL on hierarchy changes
CREATE OR REPLACE FUNCTION notify_hierarchy_change()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('hierarchy_change', json_build_object(
        'table', TG_TABLE_NAME,
        'action', TG_OP,
        'id', COALESCE(NEW.id, OLD.id)
    )::text);
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER companies_change AFTER INSERT OR UPDATE OR DELETE ON companies
    FOR EACH ROW EXECUTE FUNCTION notify_hierarchy_change();

CREATE TRIGGER organizations_change AFTER INSERT OR UPDATE OR DELETE ON organizations
    FOR EACH ROW EXECUTE FUNCTION notify_hierarchy_change();

CREATE TRIGGER teams_change AFTER INSERT OR UPDATE OR DELETE ON teams
    FOR EACH ROW EXECUTE FUNCTION notify_hierarchy_change();

CREATE TRIGGER projects_change AFTER INSERT OR UPDATE OR DELETE ON projects
    FOR EACH ROW EXECUTE FUNCTION notify_hierarchy_change();

-- Notify OPAL on user/membership changes
CREATE OR REPLACE FUNCTION notify_user_change()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('user_change', json_build_object(
        'table', TG_TABLE_NAME,
        'action', TG_OP,
        'id', COALESCE(NEW.id, OLD.id)
    )::text);
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_change AFTER INSERT OR UPDATE OR DELETE ON users
    FOR EACH ROW EXECUTE FUNCTION notify_user_change();

CREATE TRIGGER memberships_change AFTER INSERT OR UPDATE OR DELETE ON memberships
    FOR EACH ROW EXECUTE FUNCTION notify_user_change();

-- Notify OPAL on agent changes
CREATE OR REPLACE FUNCTION notify_agent_change()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('agent_change', json_build_object(
        'table', TG_TABLE_NAME,
        'action', TG_OP,
        'id', COALESCE(NEW.id, OLD.id)
    )::text);
    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agents_change AFTER INSERT OR UPDATE OR DELETE ON agents
    FOR EACH ROW EXECUTE FUNCTION notify_agent_change();
```

### 11.6 Cedar Schema for Aeterna

**`policies/aeterna.cedarschema`:**

```cedar
// ============================================================================
// AETERNA CEDAR SCHEMA
// ============================================================================
// Defines entity types and actions for organizational authorization.
// ============================================================================

namespace Aeterna {
    // Entity types matching our hierarchy
    entity Company = {
        slug: String,
        name: String,
    };

    entity Organization in [Company] = {
        slug: String,
        name: String,
    };

    entity Team in [Organization] = {
        slug: String,
        name: String,
    };

    entity Project in [Team] = {
        slug: String,
        name: String,
        git_remote?: String,
    };

    // Principal types
    entity User in [Team] = {
        email: String,
        display_name?: String,
        roles: Map<String, String>,  // team_uid -> role
    };

    entity Agent in [User, Team, Organization, Company, Project] = {
        slug: String,
        delegated_by?: User,
        scope_type: String,
        capabilities: Set<String>,
        max_role: String,
    };

    // Actions
    action ViewKnowledge appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action EditKnowledge appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action ProposeKnowledge appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action ApproveKnowledge appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action ReadMemory appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action WriteMemory appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action PromoteMemory appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action ManageGovernance appliesTo {
        principal: [User, Agent],
        resource: [Company, Organization, Team, Project],
    };

    action ManageRoles appliesTo {
        principal: [User],
        resource: [Team],
    };

    action DelegateToAgent appliesTo {
        principal: [User],
        resource: [Agent],
    };
}
```

### 11.7 Cedar Policies for Role-Based Access

**`policies/rbac.cedar`:**

```cedar
// ============================================================================
// RBAC POLICIES
// ============================================================================

// Admins can do everything within their scope
permit (
    principal is Aeterna::User,
    action,
    resource
)
when {
    principal.roles.contains(resource) &&
    principal.roles[resource] == "admin"
};

// Architects can manage knowledge and governance
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"ManageGovernance"
    ],
    resource
)
when {
    principal.roles.contains(resource) &&
    principal.roles[resource] in ["admin", "architect"]
};

// Tech leads can approve proposals and promote memory
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"ReadMemory",
        Aeterna::Action::"WriteMemory",
        Aeterna::Action::"PromoteMemory"
    ],
    resource
)
when {
    principal.roles.contains(resource) &&
    principal.roles[resource] in ["admin", "architect", "tech_lead"]
};

// Developers can read, write, and propose
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"ReadMemory",
        Aeterna::Action::"WriteMemory"
    ],
    resource
)
when {
    principal in resource
};

// ============================================================================
// AGENT DELEGATION POLICIES
// ============================================================================

// Agents inherit permissions from delegating user, capped by max_role
permit (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    principal.delegated_by is Aeterna::User &&
    principal.delegated_by in resource &&
    // Check capability allows this action
    principal.capabilities.contains(actionToCapability(action)) &&
    // Check max_role allows this action
    roleAllowsAction(principal.max_role, action)
};

// Agents can only act within their scope
permit (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    // Project-scoped agent can only access that project
    (principal.scope_type == "project" && resource == principal in) ||
    // Team-scoped agent can access team and its projects
    (principal.scope_type == "team" && resource in principal) ||
    // Org-scoped agent can access org, teams, and projects
    (principal.scope_type == "org" && resource in principal) ||
    // Company-scoped agent can access everything in company
    (principal.scope_type == "company" && resource in principal)
};

// ============================================================================
// EXPLICIT DENIES
// ============================================================================

// Agents cannot manage roles (human-only)
forbid (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"ManageRoles",
    resource
);

// Agents cannot delegate to other agents
forbid (
    principal is Aeterna::Agent,
    action == Aeterna::Action::"DelegateToAgent",
    resource
);

// Revoked agents are denied everything
forbid (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    principal.revoked == true
};
```

### 11.8 Context Resolution via Cedar Agent

When Aeterna needs to resolve "who am I, where am I", it queries Cedar Agent:

```rust
//! Context resolution using Cedar Agent
//!
//! Replaces heuristic-based context detection with authoritative
//! queries to Cedar Agent via OPAL.

use cedar_policy::{Context, EntityUid, Request};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct CedarContextResolver {
    cedar_agent_url: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct AuthorizationRequest {
    principal: String,
    action: String,
    resource: String,
    context: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AuthorizationResponse {
    decision: String,  // "Allow" or "Deny"
    diagnostics: Option<Diagnostics>,
}

#[derive(Debug, Deserialize)]
struct EntityQueryResponse {
    entities: Vec<EntityData>,
}

#[derive(Debug, Deserialize)]
struct EntityData {
    uid: String,
    attrs: serde_json::Value,
    parents: Vec<String>,
}

impl CedarContextResolver {
    /// Resolve user identity from email
    pub async fn resolve_user(&self, email: &str) -> Result<ResolvedUser, Error> {
        // Query Cedar Agent for user entity
        let response: EntityQueryResponse = self.client
            .get(&format!("{}/entities", self.cedar_agent_url))
            .query(&[("type", "Aeterna::User"), ("filter", &format!("email == \"{}\"", email))])
            .send()
            .await?
            .json()
            .await?;

        if let Some(entity) = response.entities.first() {
            let roles: HashMap<String, String> = serde_json::from_value(
                entity.attrs.get("roles").cloned().unwrap_or_default()
            )?;

            Ok(ResolvedUser {
                uid: entity.uid.clone(),
                email: email.to_string(),
                teams: entity.parents.iter()
                    .filter(|p| p.starts_with("Aeterna::Team"))
                    .cloned()
                    .collect(),
                roles,
            })
        } else {
            Err(Error::UserNotFound(email.to_string()))
        }
    }

    /// Resolve project from git remote
    pub async fn resolve_project(&self, git_remote: &str) -> Result<ResolvedContext, Error> {
        // Query Cedar Agent for project matching git remote
        let response: EntityQueryResponse = self.client
            .get(&format!("{}/entities", self.cedar_agent_url))
            .query(&[("type", "Aeterna::Project"), ("filter", &format!("git_remote == \"{}\"", git_remote))])
            .send()
            .await?
            .json()
            .await?;

        if let Some(project) = response.entities.first() {
            // Walk up hierarchy from parents
            let team_uid = project.parents.iter()
                .find(|p| p.starts_with("Aeterna::Team"))
                .cloned();

            let org_uid = self.get_parent_of_type(&team_uid, "Aeterna::Organization").await?;
            let company_uid = self.get_parent_of_type(&org_uid, "Aeterna::Company").await?;

            Ok(ResolvedContext {
                project: Some(project.uid.clone()),
                team: team_uid,
                org: org_uid,
                company: company_uid,
            })
        } else {
            Err(Error::ProjectNotFound(git_remote.to_string()))
        }
    }

    /// Check if principal can perform action on resource
    pub async fn check_authorization(
        &self,
        principal: &str,
        action: &str,
        resource: &str,
    ) -> Result<bool, Error> {
        let request = AuthorizationRequest {
            principal: principal.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            context: serde_json::json!({}),
        };

        let response: AuthorizationResponse = self.client
            .post(&format!("{}/is_authorized", self.cedar_agent_url))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.decision == "Allow")
    }

    /// Get all accessible layers for a principal
    pub async fn get_accessible_layers(
        &self,
        principal: &str,
    ) -> Result<Vec<AccessibleLayer>, Error> {
        // Query all entities where principal has at least ReadMemory permission
        let mut layers = vec![];

        // Check company access
        for company in self.get_all_companies().await? {
            if self.check_authorization(principal, "Aeterna::Action::\"ReadMemory\"", &company.uid).await? {
                layers.push(AccessibleLayer {
                    layer_type: "company".to_string(),
                    uid: company.uid,
                    slug: company.attrs.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                });
            }
        }

        // Similar for org, team, project...

        Ok(layers)
    }

    async fn get_parent_of_type(
        &self,
        child_uid: &Option<String>,
        parent_type: &str,
    ) -> Result<Option<String>, Error> {
        if let Some(uid) = child_uid {
            let response: EntityQueryResponse = self.client
                .get(&format!("{}/entities/{}", self.cedar_agent_url, uid))
                .send()
                .await?
                .json()
                .await?;

            if let Some(entity) = response.entities.first() {
                return Ok(entity.parents.iter()
                    .find(|p| p.starts_with(parent_type))
                    .cloned());
            }
        }
        Ok(None)
    }
}

#[derive(Debug)]
pub struct ResolvedUser {
    pub uid: String,
    pub email: String,
    pub teams: Vec<String>,
    pub roles: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ResolvedContext {
    pub project: Option<String>,
    pub team: Option<String>,
    pub org: Option<String>,
    pub company: Option<String>,
}

#[derive(Debug)]
pub struct AccessibleLayer {
    pub layer_type: String,
    pub uid: String,
    pub slug: String,
}
```

### 11.9 Integration with Aeterna Context Resolution (Part 8)

Update the context resolution from Part 8 to use Cedar Agent:

```rust
// In Part 8's ContextResolver, replace heuristic detection with Cedar queries

impl ContextResolver {
    pub async fn resolve(&self) -> Result<ResolvedContext, Error> {
        let cedar_resolver = CedarContextResolver::new(&self.config.cedar_agent_url);

        // 1. Check explicit context.toml first (unchanged)
        if let Some(explicit) = self.try_explicit_context().await? {
            return Ok(explicit);
        }

        // 2. Check environment variables (unchanged)
        if let Some(env_context) = self.try_env_context()? {
            return Ok(env_context);
        }

        // 3. Auto-detect from git + email using Cedar Agent (NEW)
        let git_remote = self.detect_git_remote()?;
        let user_email = self.detect_user_email()?;

        // Query Cedar Agent for authoritative context
        let project_context = cedar_resolver.resolve_project(&git_remote).await?;
        let user_context = cedar_resolver.resolve_user(&user_email).await?;

        // Verify user has access to this project
        if let Some(ref project_uid) = project_context.project {
            if !cedar_resolver.check_authorization(
                &user_context.uid,
                "Aeterna::Action::\"ReadMemory\"",
                project_uid
            ).await? {
                return Err(Error::AccessDenied {
                    user: user_email,
                    project: git_remote,
                });
            }
        }

        Ok(ResolvedContext {
            user: Some(user_context),
            project: project_context.project,
            team: project_context.team,
            org: project_context.org,
            company: project_context.company,
            source: ContextSource::CedarAgent,
        })
    }
}
```

### 11.10 IdP Sync (Okta, Azure AD)

For enterprises with existing identity providers, sync users automatically:

```rust
//! IdP Sync for Aeterna
//!
//! Syncs users from Okta/Azure AD to PostgreSQL referential.
//! Runs as a scheduled job or webhook receiver.

use sqlx::PgPool;

pub struct IdpSyncer {
    pool: PgPool,
    okta_client: Option<OktaClient>,
    azure_client: Option<AzureAdClient>,
}

impl IdpSyncer {
    /// Sync all users from configured IdPs
    pub async fn sync_all(&self) -> Result<SyncReport, Error> {
        let mut report = SyncReport::default();

        if let Some(ref okta) = self.okta_client {
            report.merge(self.sync_okta(okta).await?);
        }

        if let Some(ref azure) = self.azure_client {
            report.merge(self.sync_azure(azure).await?);
        }

        Ok(report)
    }

    async fn sync_okta(&self, client: &OktaClient) -> Result<SyncReport, Error> {
        let okta_users = client.list_users().await?;
        let mut report = SyncReport::default();

        for okta_user in okta_users {
            let result = sqlx::query!(
                r#"
                INSERT INTO users (email, display_name, idp_subject, idp_provider, updated_at)
                VALUES ($1, $2, $3, 'okta', NOW())
                ON CONFLICT (email) DO UPDATE SET
                    display_name = EXCLUDED.display_name,
                    idp_subject = EXCLUDED.idp_subject,
                    updated_at = NOW()
                RETURNING id
                "#,
                okta_user.profile.email,
                okta_user.profile.display_name,
                okta_user.id
            )
            .fetch_one(&self.pool)
            .await?;

            // Sync group memberships to team memberships
            for group in client.get_user_groups(&okta_user.id).await? {
                if let Some(team_mapping) = self.get_team_mapping(&group.id).await? {
                    sqlx::query!(
                        r#"
                        INSERT INTO memberships (user_id, team_id, role, granted_by)
                        VALUES ($1, $2, $3, NULL)
                        ON CONFLICT (user_id, team_id) DO UPDATE SET
                            role = EXCLUDED.role
                        "#,
                        result.id,
                        team_mapping.team_id,
                        team_mapping.default_role
                    )
                    .execute(&self.pool)
                    .await?;

                    report.memberships_synced += 1;
                }
            }

            report.users_synced += 1;
        }

        Ok(report)
    }
}

#[derive(Default)]
pub struct SyncReport {
    pub users_synced: u32,
    pub users_created: u32,
    pub users_updated: u32,
    pub memberships_synced: u32,
    pub errors: Vec<String>,
}
```

### 11.11 Deployment Topology

**Single-node (Development):**
```
┌─────────────────────────────────────────┐
│               Docker Compose             │
│                                          │
│  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │PostgreSQL│  │  Redis   │  │ Qdrant │ │
│  └────┬─────┘  └────┬─────┘  └────┬───┘ │
│       │             │             │      │
│  ┌────┴─────────────┴─────────────┴────┐ │
│  │           OPAL Server               │ │
│  └────────────────┬────────────────────┘ │
│                   │                      │
│  ┌────────────────┴────────────────────┐ │
│  │   Cedar Agent (embedded in OPAL)    │ │
│  └────────────────┬────────────────────┘ │
│                   │                      │
│  ┌────────────────┴────────────────────┐ │
│  │          Aeterna Core               │ │
│  └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

**Production (Kubernetes):**
```
┌─────────────────────────────────────────────────────────────────┐
│                         Kubernetes Cluster                       │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    OPAL Server (Deployment)                │  │
│  │                    Replicas: 2 (HA)                        │  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │                 Cedar Agent (DaemonSet)                    │  │
│  │                 One per node for locality                  │  │
│  └───────────────────────────┬───────────────────────────────┘  │
│                              │                                   │
│  ┌───────────────────────────┴───────────────────────────────┐  │
│  │                 Aeterna Core (Deployment)                  │  │
│  │                 Replicas: 3+                               │  │
│  │                 Connects to local Cedar Agent              │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ PostgreSQL  │  │    Redis    │  │   Qdrant    │             │
│  │  (Managed)  │  │  (Managed)  │  │  (Managed)  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

### 11.12 Migration Path

For existing Aeterna deployments without OPAL:

**Phase 1: Deploy OPAL (Non-blocking)**
1. Deploy OPAL Server + Cedar Agent alongside existing setup
2. Migrate organizational data to PostgreSQL referential
3. Configure OPAL fetcher to read referential
4. Verify Cedar Agent has correct data

**Phase 2: Switch Context Resolution**
1. Update Aeterna config to use Cedar Agent for context
2. Test context resolution returns same results
3. Monitor for discrepancies
4. Disable fallback heuristics

**Phase 3: Enforce Authorization**
1. Enable Cedar authorization in "audit" mode (log-only)
2. Review denied requests, adjust policies
3. Switch to "enforce" mode
4. Monitor and tune

### 11.13 Failure Modes & Recovery

| Failure | Impact | Recovery |
|---------|--------|----------|
| OPAL Server down | No policy/data updates | Cedar Agents use cached data; deploy HA |
| Cedar Agent down | Auth queries fail | Aeterna falls back to cached context (degraded) |
| PostgreSQL down | No referential updates | OPAL uses cached data; restore PostgreSQL |
| Network partition | Split-brain risk | Cedar Agents operate independently; eventual consistency |

**Circuit breaker in Aeterna:**
```rust
impl CedarContextResolver {
    pub async fn resolve_with_fallback(&self, ...) -> Result<ResolvedContext, Error> {
        match self.resolve_from_cedar(...).await {
            Ok(context) => {
                self.cache.store(&context).await;
                Ok(context)
            }
            Err(e) if self.circuit_breaker.is_open() => {
                warn!("Cedar Agent unavailable, using cached context");
                self.cache.get(&cache_key).await
                    .ok_or(Error::NoCachedContext)
            }
            Err(e) => Err(e),
        }
    }
}
```
