# UX-First Governance: Natural Language Policy Management

**Enterprise-scale knowledge governance without the complexity**

This guide documents Aeterna's UX-First Governance system, a revolutionary approach to enterprise policy management that uses natural language instead of complex policy DSLs. With OPAL-powered organizational referential and Cedar-based authorization, Aeterna makes governance accessible to everyone, from developers to executives.

---

## Table of Contents

- [Executive Summary](#executive-summary)
- [Architecture Overview](#architecture-overview)
- [Core Concepts](#core-concepts)
- [Getting Started](#getting-started)
- [Persona Workflows](#persona-workflows)
  - [Developer Workflow](#developer-workflow)
  - [Tech Lead Workflow](#tech-lead-workflow)
  - [Architect Workflow](#architect-workflow)
  - [Admin Workflow](#admin-workflow)
- [CLI Command Reference](#cli-command-reference)
- [Integration Scenarios](#integration-scenarios)
- [Advanced Topics](#advanced-topics)
- [Troubleshooting](#troubleshooting)

---

## Executive Summary

### The Problem

Traditional enterprise governance systems require deep technical expertise:

| Pain Point | Traditional Approach | Aeterna UX-First |
|------------|---------------------|------------------|
| Policy creation | Learn Cedar DSL syntax | "Block MySQL in this project" |
| Onboarding | Manual TOML configs | `aeterna init` (auto-detects) |
| Context resolution | Specify org/team/project flags | Auto-resolves from git |
| Knowledge discovery | Navigate file hierarchies | Natural language search |
| Administration | Scattered across tools | Unified CLI and API |

### What is UX-First Governance?

UX-First Governance is an architectural approach where **every capability is accessible through natural language, simple APIs, and intuitive CLI commands**. Implementation details like Cedar policies, TOML configs, and layer hierarchies are completely hidden from end users.

### Key Capabilities

1. **Natural Language Policies**: "Block MySQL" → Cedar policy
2. **Zero-Config Onboarding**: Auto-detect git context, initialize in seconds
3. **Automatic Context Resolution**: No more `--company --org --team` flags
4. **Semantic Search**: Find memories and knowledge without knowing structure
5. **OPAL Integration**: Real-time organizational referential with Cedar Agent
6. **Meta-Governance**: Policies about policies, with approval workflows
7. **AI Agent Integration**: Agents propose, simulate, and track policies autonomously

### Success Metrics

After deploying UX-First Governance:
- **Developer onboarding**: 5 minutes (from 2+ hours)
- **Policy creation**: <1 minute (from 30+ minutes)
- **Context errors**: Near zero (from frequent)
- **Governance adoption**: 10x increase across teams
- **AI agent autonomy**: 80% of routine governance tasks automated

---

## Architecture Overview

### System Layers

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         USER INTERACTION LAYER                               │
│                                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐ │
│   │   Human     │    │  AI Agent   │    │    CLI      │    │  Web API    │ │
│   │  (Chat)     │    │   (LLM)     │    │  (Script)   │    │  (REST)     │ │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘    └──────┬──────┘ │
│          └──────────────────┴────────────────────┴────────────────┘         │
│                                    │                                         │
└────────────────────────────────────┼─────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼─────────────────────────────────────────┐
│                         NATURAL LANGUAGE LAYER                               │
│                                                                              │
│   "Block MySQL in this project"                                              │
│   "Only architects can approve org policies"                                 │
│   "Require 2 approvers for company-level changes"                            │
│                                                                              │
└────────────────────────────────────┬─────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼─────────────────────────────────────────┐
│                         SKILL / TOOL LAYER                                   │
│                                                                              │
│   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│   │  Policy Skill   │  │ Governance Skill│  │  Onboarding Skill│             │
│   │                 │  │                 │  │                 │             │
│   │ • draft         │  │ • configure     │  │ • org_init      │             │
│   │ • validate      │  │ • approve       │  │ • team_create   │             │
│   │ • propose       │  │ • reject        │  │ • project_init  │             │
│   │ • explain       │  │ • audit         │  │ • user_register │             │
│   │ • simulate      │  │ • roles         │  │ • agent_register│             │
│   │ • list          │  │                 │  │                 │             │
│   └────────┬────────┘  └────────┬────────┘  └────────┬────────┘             │
│            │                    │                    │                       │
│            └────────────────────┼────────────────────┘                       │
│                                 │                                            │
└─────────────────────────────────┼────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────▼────────────────────────────────────────────┐
│                         TRANSLATION LAYER                                    │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                  LLM-Powered Translator                          │       │
│   │                                                                  │       │
│   │   Natural Language ──────▶ Structured Intent ──────▶ Cedar      │       │
│   │                                                                  │       │
│   │   "Block MySQL" ──▶ {target: dep, op: deny, value: "mysql"}     │       │
│   │                 ──▶ forbid(principal, action, resource)          │       │
│   │                     when { resource.dependency == "mysql" }      │       │
│   │                                                                  │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                  Cedar Validator                                 │       │
│   │                                                                  │       │
│   │   • Syntax validation (cedar-policy crate)                       │       │
│   │   • Schema compliance                                            │       │
│   │   • Conflict detection                                           │       │
│   │   • Simulation against test scenarios                            │       │
│   │                                                                  │       │
│   └─────────────────────────────────────────────────────────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────▼────────────────────────────────────────────┐
│                    GOVERNANCE ENGINE + OPAL/CEDAR                            │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                        OPAL SERVER                               │       │
│   │                                                                  │       │
│   │   PostgreSQL (Referential)          Git (Policies)              │       │
│   │   ┌─────────────────────┐          ┌─────────────────────┐     │       │
│   │   │ companies           │          │ cedar-policies/     │     │       │
│   │   │ organizations       │          │   company.cedar     │     │       │
│   │   │ teams               │          │   org.cedar         │     │       │
│   │   │ projects            │          │   meta-governance/  │     │       │
│   │   │ users               │          └─────────────────────┘     │       │
│   │   │ agents              │                                       │       │
│   │   │ memberships         │          PubSub Channel              │       │
│   │   └─────────────────────┘          (WebSocket)                 │       │
│   │                                           │                      │       │
│   └───────────────────────────────────────────┼──────────────────────┘       │
│                                               │                              │
│               ┌─────────────────────────────┬─┴──────────────┐              │
│               │                             │                │              │
│               ▼                             ▼                ▼              │
│   ┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────┐    │
│   │   OPAL Client + Cedar   │  │   OPAL Client + Cedar   │  │  OPAL   │    │
│   │   Agent (Region A)      │  │   Agent (Region B)      │  │  Agent  │    │
│   │                         │  │                         │  │ (CI/CD) │    │
│   │   Topics:               │  │   Topics:               │  │         │    │
│   │   - company:acme        │  │   - company:acme        │  │ Topics: │    │
│   │   - org:platform        │  │   - org:data            │  │   *     │    │
│   └─────────────────────────┘  └─────────────────────────┘  └─────────┘    │
│                                                                              │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│   │   Proposal   │  │   Approval   │  │    Cedar     │  │    Audit     │   │
│   │    Store     │  │   Workflow   │  │   Authorizer │  │     Log      │   │
│   └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### OPAL: The Organizational Referential

OPAL (Open Policy Administration Layer) provides the source of truth for organizational topology:

**What it stores:**
- Company → Organization → Team → Project hierarchy
- User memberships and roles
- Agent delegation chains
- Policy files from git repositories

**Why it matters:**
- **Real-time sync**: Changes propagate to all Cedar Agents instantly
- **Context resolution**: Automatic detection of who/where without flags
- **Self-hosted**: Apache 2.0 licensed, runs in your infrastructure
- **IdP integration**: Syncs with Okta, Azure AD, Google Workspace

**Data flow:**
```
PostgreSQL (Referential) ──┐
                           ├──► OPAL Server ──► WebSocket PubSub
Git (Cedar Policies)  ─────┘                           │
                                                       │
                    ┌──────────────────────────────────┘
                    │
                    ▼
        OPAL Client + Cedar Agent
                    │
                    ▼
        Authorization Decisions
        Context Resolution Queries
```

### Governance Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE WORKFLOW                                  │
│                                                                              │
│  1. NATURAL LANGUAGE INPUT                                                   │
│     ┌────────────────────────────────────────────┐                           │
│     │ Developer: "Block MySQL in this project"  │                           │
│     └─────────────────────┬──────────────────────┘                           │
│                           │                                                  │
│  2. INTENT EXTRACTION     ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ LLM Translator:                                      │                 │
│     │ {                                                    │                 │
│     │   "action": "deny",                                  │                 │
│     │   "target_type": "dependency",                       │                 │
│     │   "target_value": "mysql",                           │                 │
│     │   "severity": "block"                                │                 │
│     │ }                                                    │                 │
│     └─────────────────────┬───────────────────────────────┘                 │
│                           │                                                  │
│  3. CEDAR GENERATION      ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ forbid(                                              │                 │
│     │   principal,                                         │                 │
│     │   action == Action::"UseDependency",                 │                 │
│     │   resource                                           │                 │
│     │ )                                                    │                 │
│     │ when {                                               │                 │
│     │   resource.dependency == "mysql"                     │                 │
│     │ };                                                   │                 │
│     └─────────────────────┬───────────────────────────────┘                 │
│                           │                                                  │
│  4. VALIDATION            ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ Cedar Validator:                                     │                 │
│     │ ✅ Syntax valid                                      │                 │
│     │ ✅ Schema compliant                                  │                 │
│     │ ⚠️  Warning: Consider blocking mysql2 as well        │                 │
│     └─────────────────────┬───────────────────────────────┘                 │
│                           │                                                  │
│  5. SIMULATION            ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ Test against current project:                        │                 │
│     │ ✅ Pass (no MySQL dependencies found)                │                 │
│     │                                                      │                 │
│     │ Test against hypothetical:                           │                 │
│     │ ❌ Block (would block if MySQL added)                │                 │
│     └─────────────────────┬───────────────────────────────┘                 │
│                           │                                                  │
│  6. APPROVAL WORKFLOW     ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ Proposal Created: prop_abc123                        │                 │
│     │ Required Approvers: 1 (tech lead or architect)       │                 │
│     │ Notified: alice@company.com                          │                 │
│     │ Review Period: 24 hours                              │                 │
│     └─────────────────────┬───────────────────────────────┘                 │
│                           │                                                  │
│  7. ACTIVATION            ▼                                                  │
│     ┌─────────────────────────────────────────────────────┐                 │
│     │ alice@company.com approves                           │                 │
│     │ Policy ID: no-mysql                                  │                 │
│     │ Status: ACTIVE                                       │                 │
│     │ Pushed to OPAL → synced to all Cedar Agents          │                 │
│     └─────────────────────────────────────────────────────┘                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### Memory Layer Hierarchy

Aeterna organizes memories in a 7-layer hierarchy:

```
agent    ←── Per-agent instance (most specific)
   │         "This agent prefers Rust for new services"
user         Per-user
   │         "Alice prefers snake_case for API fields"
session      Per-conversation
   │         "Current task: Implement payment API"
project      Per-repository
   │         "payments-service uses PostgreSQL"
team         Per-team
   │         "API Team decided on REST over GraphQL"
org          Per-organization/department
   │         "Platform Engineering standardized on Kubernetes"
company  ←── Per-tenant (least specific)
             "Acme Corp mandates TLS 1.3+"
```

**Search precedence**: Agent → User → Session → Project → Team → Org → Company

**Promotion flow**: Memories with high reward scores automatically promote upward

### Knowledge Layers

Knowledge (ADRs, patterns, policies) follows a 4-layer hierarchy:

```
Company (highest precedence)
    ↓ Policies flow DOWN
Organization
    ↓ Teams inherit + customize
Team
    ↓ Projects inherit + override
Project (lowest precedence)
```

**Merge strategies:**
- **Override**: Child completely replaces parent
- **Merge**: Combines rules from both
- **Intersect**: Keeps only common rules (stricter)

### Roles and Permissions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ROLE HIERARCHY                                       │
│                                                                              │
│   Admin (precedence: 4)                                                      │
│   ━━━━━━━━━━━━━━━━━━━━━                                                      │
│   • Full system access                                                       │
│   • Configure meta-governance                                                │
│   • Manage all resources                                                     │
│                                                                              │
│        Architect (precedence: 3)                                             │
│        ━━━━━━━━━━━━━━━━━━━━━                                                 │
│        • Design policies                                                     │
│        • Manage knowledge repository                                         │
│        • Approve org-level proposals                                         │
│                                                                              │
│             Tech Lead (precedence: 2)                                        │
│             ━━━━━━━━━━━━━━━━━━━━━                                            │
│             • Manage team resources                                          │
│             • Approve team-level proposals                                   │
│             • Enforce policies                                               │
│                                                                              │
│                  Developer (precedence: 1)                                   │
│                  ━━━━━━━━━━━━━━━━━━━━━                                       │
│                  • Standard development                                      │
│                  • Propose policies                                          │
│                  • Access knowledge                                          │
│                                                                              │
│                       Agent (precedence: 0)                                  │
│                       ━━━━━━━━━━━━━━━━━━                                     │
│                       • Delegated permissions from user                      │
│                       • Cannot exceed user's capabilities                    │
│                       • Auto-proposal (with limits)                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Approval Workflow State Machine

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     PROPOSAL STATE MACHINE                                   │
│                                                                              │
│                          ┌───────────┐                                       │
│                          │  DRAFTED  │                                       │
│                          └─────┬─────┘                                       │
│                                │                                             │
│                         submit │                                             │
│                                ▼                                             │
│                     ┌────────────────────┐                                   │
│                     │ PENDING_APPROVAL   │                                   │
│                     └─────┬────────┬─────┘                                   │
│                           │        │                                         │
│                   approve │        │ reject                                  │
│                           │        │                                         │
│            ┌──────────────┘        └──────────────┐                          │
│            ▼                                      ▼                          │
│     ┌───────────┐                          ┌──────────┐                      │
│     │ APPROVED  │                          │ REJECTED │                      │
│     └─────┬─────┘                          └─────┬────┘                      │
│           │                                      │                           │
│    activate                              revise/abandon                      │
│           │                                      │                           │
│           ▼                                      ▼                           │
│     ┌───────────┐                          ┌──────────┐                      │
│     │  ACTIVE   │                          │ ABANDONED│                      │
│     └─────┬─────┘                          └──────────┘                      │
│           │                                                                  │
│      deprecate                                                               │
│           │                                                                  │
│           ▼                                                                  │
│     ┌────────────┐                                                           │
│     │ DEPRECATED │                                                           │
│     └────────────┘                                                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Organization Structure

```
Acme Corp (Company)
├── Platform Engineering (Org)
│   ├── API Team (Team)
│   │   ├── alice@acme.com (Tech Lead)
│   │   ├── bob@acme.com (Developer)
│   │   ├── payments-service (Project)
│   │   ├── auth-service (Project)
│   │   └── gateway-service (Project)
│   └── Data Platform Team (Team)
│       ├── carol@acme.com (Architect)
│       ├── analytics-pipeline (Project)
│       └── ml-inference (Project)
├── Product Engineering (Org)
│   ├── Web Team (Team)
│   │   ├── dashboard-ui (Project)
│   │   └── admin-portal (Project)
│   └── Mobile Team (Team)
│       ├── ios-app (Project)
│       └── android-app (Project)
└── Security (Org)
    └── SecOps Team (Team)
        └── security-scanner (Project)
```

---

## Getting Started

### Prerequisites

- **OPAL Server**: Deployed and accessible
- **PostgreSQL**: For organizational referential
- **Cedar Agent**: Deployed via OPAL Client
- **Aeterna CLI**: Installed and configured

### Quick Start (5 Minutes)

#### 1. Initialize Company

```bash
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
✅ OPAL Server synchronized

Next steps:
  aeterna org create "Engineering"
  aeterna team create "Platform" --org engineering
  aeterna project init  # In your git repository
```

#### 2. Create Organization and Team

```bash
$ aeterna org create "Platform Engineering"

✅ Organization 'Platform Engineering' created in 'Acme Corp'
   ID: org:acme-corp:platform-engineering
   Inherited: 2 company-level policies
   Members: 0 (invite users with 'aeterna user invite')

$ aeterna team create "API Team" --org platform-engineering --lead alice@acme.com

✅ Team 'API Team' created in 'Platform Engineering'
   ID: team:platform-engineering:api-team
   Lead: alice@acme.com (tech_lead)
   Inherited: 3 policies from org
   Members: 1
```

#### 3. Initialize Project (Auto-Detection)

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

#### 4. Check Status

```bash
$ aeterna status

📍 Current Context
   Company:  acme-corp (from context.toml)
   Org:      platform-engineering (from context.toml)
   Team:     api-team (from git remote)
   Project:  payments-service (from git remote)
   User:     alice@acme.com (from git user.email)

🚦 Governance Status
   Active policies: 5
   Pending approvals: 0
   Recent violations: 0

📚 Knowledge
   ADRs: 12
   Patterns: 8
   Policies: 5

💾 Memory
   Project memories: 15
   Team memories: 47
   Org memories: 123
```

---

## Persona Workflows

### Developer Workflow

#### Daily Tasks

**Morning: Check context and status**

```bash
$ aeterna status

📍 Current Context: acme-corp / platform-engineering / api-team / payments-service
🚦 Status: All systems operational
📌 Pending for you: None

Recent team learnings:
  • "PostgreSQL connection pooling: max 20 connections" (2 hours ago)
  • "Use snake_case for API field names" (1 day ago)
```

**Task: Search for relevant knowledge**

```bash
$ aeterna knowledge search "how do we handle authentication"

Found 3 results:

[95%] ADR-015: JWT Authentication Strategy
   "Use JWT tokens with 1-hour expiration, refresh tokens for long sessions"
   Layer: org:platform-engineering
   
[88%] Pattern: OAuth2 Integration
   "Standard OAuth2 flow for third-party integrations"
   Layer: company:acme-corp
   
[72%] Policy: Authentication Requirements
   "All APIs must implement authentication, no anonymous access"
   Layer: company:acme-corp
```

**Task: Search memory for past decisions**

```bash
$ aeterna memory search "database decisions we made last month"

Found 2 memories:

[95%] team:api-team - 2024-01-10
   "Decided to use PostgreSQL for all new services per ADR-042"
   by alice@acme.com
   
[82%] org:platform-engineering - 2024-01-05
   "Redis for caching, but not for primary data storage"
   by bob@acme.com
```

**Task: Check if a dependency is allowed**

```bash
$ aeterna check dependency mysql

❌ BLOCKED

Policy: security-baseline (company)
Rule: no-mysql
Severity: block
Message: MySQL is prohibited. Use PostgreSQL instead.
Reference: ADR-042

Allowed alternatives:
  • postgresql
  • pg (Node.js client)
```

**Task: Add memory about a decision**

```bash
$ aeterna memory add "Decided to use bcrypt for password hashing, cost factor 12" \
  --layer project \
  --tags "security,authentication"

✅ Memory added: mem_abc123
   Layer: project:payments-service
   Tags: security, authentication
   
Tip: If this becomes team-wide, promote with:
  aeterna memory promote mem_abc123 --to team
```

**Task: Propose a new policy**

```bash
$ aeterna policy create "Require 2FA for admin endpoints" --scope project --severity warn

📋 Draft Policy Created: draft_xyz789

Name: require-2fa
Scope: project (payments-service)
Severity: warn
Effect: Requires two-factor authentication for admin endpoints

Simulating against current project...
✅ Current project passes (2FA already implemented)

Submit for approval? [y/N]: y
Justification: Security best practice for admin access

✅ Proposal submitted: prop_abc123
Notified: alice@acme.com (Tech Lead)
Review period: 24 hours
```

### Tech Lead Workflow

#### Approval Management

**Morning: Check pending approvals**

```bash
$ aeterna govern pending

You have 3 pending approvals:

[POLICY] prop_abc123 - "Require 2FA for admin endpoints"
  Proposed by: bob@acme.com
  Scope: project:payments-service
  Created: 2 hours ago
  View: aeterna policy draft show draft_xyz789

[KNOWLEDGE] prop_def456 - "Add pattern for API pagination"
  Proposed by: charlie@acme.com
  Scope: team:api-team
  Created: 1 day ago

[MEMORY PROMOTION] prom_ghi789 - Promote to team layer
  Memory: "KApp has 20-char ID limit"
  Proposed by: alice@acme.com
  Reason: "All team members should know this gotcha"
```

**Task: Review and approve policy proposal**

```bash
$ aeterna policy draft show draft_xyz789

📋 Policy Draft: require-2fa

Natural Language:
  "Require 2FA for admin endpoints"

Generated Cedar:
  permit(
    principal,
    action == Action::"AccessAdminEndpoint",
    resource
  )
  when {
    principal.has_2fa == true
  };

Human Readable:
  This policy requires two-factor authentication for all admin endpoint access.
  Violations will generate warnings but not block access.

Validation:
  ✅ Syntax valid
  ✅ Schema compliant
  ⚠️  Warning: Consider making this blocking (severity: block) for production

Simulation Results:
  Current project: ✅ Pass (2FA implemented)
  Hypothetical without 2FA: ⚠️  Warn

$ aeterna govern approve prop_abc123 --comment "Good security practice, approved"

✅ Proposal approved: prop_abc123
   Policy activated: require-2fa
   Effective immediately
   Audit trail updated
```

**Task: Promote memory to team layer**

```bash
$ aeterna memory promote mem_abc123 --to team --reason "Critical gotcha for all team members"

✅ Memory promoted: mem_abc123
   From: project:payments-service
   To: team:api-team
   All team members will now see this memory in search results
   Audit log: promotion recorded
```

**Task: Configure team governance**

```bash
$ aeterna govern configure --scope team:api-team --interactive

🔧 Governance Configuration for team:api-team

Policy Approval Settings:
  Required approvers [1]: 1
  Allowed approvers [tech_lead,architect]: tech_lead,architect
  Auto-approve for roles [none]: 
  Review period (hours) [24]: 48

Knowledge Proposal Settings:
  Required approvers [1]: 1
  Allowed proposers [developer,tech_lead,architect]: developer,tech_lead,architect
  Auto-approve types [none]: pattern

Memory Promotion Settings:
  Auto-promote threshold [0.9]: 0.85
  Require approval above layer [team]: team

Save configuration? [y/N]: y

✅ Governance configured for team:api-team

Effective rules:
- Policies require 1 approval from tech_lead or architect
- Knowledge patterns auto-approve, ADRs require review
- Memory auto-promotes at 0.85 importance threshold
```

### Architect Workflow

#### Organization-Wide Policy Management

**Task: Create organization-wide policy**

```bash
$ aeterna policy create

What should this policy do? All services must use OpenTelemetry for tracing
Scope? [project/team/org/company]: org
Severity? [info/warn/block]: warn

📋 Draft Policy Created: draft_otel_001

Name: require-opentelemetry
Scope: org:platform-engineering
Severity: warn
Effect: Requires opentelemetry dependency in all services

⚠️ Impact Analysis:
   12 services in scope
   8 already compliant
   4 would receive warnings:
     - auth-service
     - gateway
     - legacy-api
     - batch-processor

Would you like to:
1. Keep as warning (recommended for migration)
2. Change to blocking
3. Add grace period
4. Exclude specific projects

Choice: 4
Exclude which projects? legacy-api

Updated policy excludes legacy-api.

Simulating...
✅ Simulation complete

Submit for approval? [y/N]: y
Justification: Standardizing on OpenTelemetry for observability

✅ Proposal submitted: prop_org_001
Required approvers: 2 (org-level requires quorum)
Notified: bob@acme.com (Admin), carol@acme.com (Architect)
Review period: 48 hours (org-level policy)
```

**Task: Explain existing policy to developer**

```bash
$ aeterna policy explain security-baseline

📋 Policy: security-baseline

Summary:
  This company-wide mandatory policy enforces core security requirements
  across all projects.

Scope: company:acme-corp
Mode: mandatory (cannot be overridden)
Created by: admin@acme.com on 2024-01-15

Rules:

1. no-vulnerable-lodash
   Effect: Blocks any project from using lodash versions below 4.17.21
   Reason: CVE-2021-23337 (prototype pollution vulnerability)
   Severity: block (prevents deployment)
   
2. require-security-doc
   Effect: Requires every project to have a SECURITY.md file
   Severity: warn (generates warning, allows action)

3. tls-1-3-required
   Effect: All network connections must use TLS 1.3 or higher
   Severity: block

Applies to: All projects in Acme Corp
Cannot be overridden by lower scopes

Related:
  • ADR-008: Security Baseline Requirements
  • Pattern: Secure Configuration
```

**Task: Audit governance activity**

```bash
$ aeterna govern audit --scope org:platform-engineering --last 30d

Governance Audit Report
Scope: org:platform-engineering
Period: 2024-01-01 to 2024-01-31

Summary:
┌─────────────────────┬───────┬──────────┬──────────┐
│ Event Type          │ Count │ Approved │ Rejected │
├─────────────────────┼───────┼──────────┼──────────┤
│ Policy Proposals    │ 15    │ 12       │ 3        │
│ Knowledge Proposals │ 28    │ 26       │ 2        │
│ Memory Promotions   │ 67    │ 67       │ 0        │
│ Role Changes        │ 8     │ 8        │ 0        │
└─────────────────────┴───────┴──────────┴──────────┘

Recent Events:

[2024-01-28 10:23] POLICY_APPROVED
  Actor: alice@acme.com
  Proposal: prop_org_001 (require-opentelemetry)
  Comment: "LGTM, aligns with observability standards"

[2024-01-27 15:45] POLICY_REJECTED
  Actor: carol@acme.com
  Proposal: prop_org_002 (mandate-graphql)
  Reason: "Too prescriptive, teams should choose API style"

[2024-01-26 09:12] ROLE_ASSIGNED
  Actor: admin@acme.com
  User: david@acme.com
  Role: architect
  Scope: org:platform-engineering

Export full audit log? [y/N]: y
Format [csv/json]: csv

✅ Exported to: governance-audit-2024-01.csv
```

### Admin Workflow

#### System Setup and Management

**Task: Bootstrap entire organization**

```bash
$ aeterna init --company "Acme Corp" --admin admin@acme.com --governance strict

✅ Company initialized: acme-corp

Created:
  • Company: Acme Corp
  • Default org: default
  • Admin user: admin@acme.com
  • OPAL Server connection: established
  • Cedar Agent: synchronized

Default policies applied:
  • security-baseline (blocking)
  • compliance-requirements (blocking)
  • coding-standards (warning)

$ aeterna org create "Platform Engineering"
$ aeterna org create "Product Engineering"
$ aeterna org create "Security"

$ aeterna team create "API Team" --org platform-engineering --lead alice@acme.com
$ aeterna team create "Data Platform" --org platform-engineering --lead bob@acme.com
$ aeterna team create "Web Team" --org product-engineering --lead carol@acme.com
$ aeterna team create "SecOps" --org security --lead dave@acme.com

✅ Organization structure created
   Total orgs: 3
   Total teams: 4
   Total users: 4
```

**Task: Register user and assign roles**

```bash
$ aeterna user register \
  --email eve@acme.com \
  --display-name "Eve Johnson" \
  --teams api-team,data-platform \
  --role developer

✅ User registered: user:eve@acme.com

Memberships:
  • team:api-team (developer)
  • team:data-platform (developer)

Capabilities:
  Memory: read, write, promote to team
  Knowledge: read, propose
  Policy: read, propose

$ aeterna govern roles assign eve@acme.com tech_lead --scope team:api-team

✅ Role assigned
   User: eve@acme.com
   Role: tech_lead
   Scope: team:api-team
   Granted by: admin@acme.com
   
Updated capabilities for eve@acme.com in team:api-team:
  • Can approve team-level proposals
  • Can manage team resources
  • Can promote memories to org layer
```

**Task: Register AI agent with delegation**

```bash
$ aeterna agent register \
  --agent-id "agent:opencode-alice" \
  --delegated-by alice@acme.com \
  --scope project:payments-service \
  --max-severity warn \
  --expires 2024-12-31

✅ Agent registered: agent:opencode-alice

Delegation chain:
  user:alice@acme.com
    → team:api-team
    → org:platform-engineering
    → company:acme-corp

Capabilities (delegated from alice@acme.com):
  • Read memory: yes
  • Write memory: yes
  • Propose policies: yes (max severity: warn)
  • Approve policies: no (requires human)
  • Promote memories: team layer only

Token: aeterna_agent_abc123xyz (save securely)

Configure your AI assistant:
  export AETERNA_AGENT_ID="agent:opencode-alice"
  export AETERNA_AGENT_TOKEN="aeterna_agent_abc123xyz"
```

**Task: System health check**

```bash
$ aeterna admin health --verbose

🏥 Aeterna System Health

Core Services:
  ✅ OPAL Server: healthy (latency: 23ms)
  ✅ PostgreSQL: healthy (connections: 12/100)
  ✅ Cedar Agent: healthy (policies: 47 synced)
  ✅ Redis: healthy (memory: 45MB/2GB)
  ✅ Qdrant: healthy (collections: 7)

Governance:
  ✅ Active policies: 47
  ✅ Pending proposals: 3
  ⚠️  Expired proposals: 1 (auto-archived)
  ✅ Recent approvals: 12 (last 24h)

Knowledge:
  ✅ ADRs: 45
  ✅ Patterns: 52
  ✅ Policies: 47
  ✅ Git sync: up to date (last sync: 5 min ago)

Memory:
  ✅ Total memories: 1,247
  ✅ Promoted this week: 89
  ✅ Average reward: 0.78
  ⚠️  Low-reward memories: 23 (candidates for pruning)

Users & Agents:
  ✅ Registered users: 127
  ✅ Active agents: 45
  ✅ Team memberships: 312
  ⚠️  Expired agent tokens: 3 (renewal recommended)

Recommendations:
  • Renew 3 expired agent tokens
  • Review 1 expired proposal
  • Consider pruning 23 low-reward memories
```

---

## CLI Command Reference

### Core Commands

#### Status and Context

```bash
# Show current status
aeterna status
aeterna status --scope company
aeterna status --json

# Show/set context
aeterna context show
aeterna context set --team backend
aeterna context set --org engineering
aeterna context clear

# Quick health check
aeterna check
aeterna check dependency mysql
aeterna check --dry-run
```

#### Memory Commands

```bash
# Search memories
aeterna memory search "database decisions"
aeterna memory search "auth" --layer team --last 30d
aeterna memory search "performance" --min-relevance 0.8

# Browse memories
aeterna memory browse --layer team
aeterna memory browse --layer team --category decisions
aeterna memory browse --layer org --page 2

# Add memory
aeterna memory add "Decided to use PostgreSQL" --layer project
aeterna memory add "Use snake_case for APIs" --layer team --tags "style,convention"

# Promote memory
aeterna memory promote mem_abc123 --to team --reason "Team consensus"
aeterna memory promote mem_def456 --to org

# Memory provenance
aeterna memory where mem_abc123
aeterna memory attribution mem_abc123

# Provide feedback
aeterna memory feedback mem_abc123 --type helpful --score 0.9
aeterna memory feedback mem_def456 --type not-helpful --score 0.2
```

#### Knowledge Commands

```bash
# Search knowledge
aeterna knowledge search "authentication approaches"
aeterna knowledge search "database" --type adr
aeterna knowledge search "security" --layers company,org

# Browse knowledge
aeterna knowledge browse --type adr
aeterna knowledge browse --type pattern --layer team
aeterna knowledge browse --type policy

# Propose knowledge
aeterna knowledge propose "We should use JWT for auth" --type adr
aeterna knowledge propose "API rate limiting pattern" --type pattern

# Explain knowledge
aeterna knowledge explain ADR-042
aeterna knowledge explain security-baseline

# Get knowledge item
aeterna knowledge get company/adrs/adr-042.md
aeterna knowledge get team/patterns/api-pagination.md
```

#### Policy Commands

```bash
# Create policy (interactive)
aeterna policy create
aeterna policy create --interactive

# Create policy (non-interactive)
aeterna policy create "Block MySQL usage" --scope project --severity block
aeterna policy create "Require README" --scope team --severity warn

# List policies
aeterna policy list
aeterna policy list --scope team
aeterna policy list --scope org --include-inherited
aeterna policy list --severity block
aeterna policy list --format json

# Explain policy
aeterna policy explain security-baseline
aeterna policy explain no-mysql --verbose

# Simulate policy
aeterna policy simulate draft_abc123
aeterna policy simulate draft_abc123 --scenario '{"dependencies": ["mysql"]}'
aeterna policy simulate draft_abc123 --live

# Draft management
aeterna policy draft show draft_abc123
aeterna policy draft list
aeterna policy draft submit draft_abc123 --justification "Per ADR-042"
aeterna policy draft delete draft_abc123
```

#### Governance Commands

```bash
# Configure governance
aeterna govern configure --scope org --interactive
aeterna govern configure --scope team --policy-approvers tech_lead,architect
aeterna govern configure --scope org --approval-count 2 --review-period 48h

# View governance status
aeterna govern status
aeterna govern status --scope company

# Manage roles
aeterna govern roles list
aeterna govern roles list --scope team
aeterna govern roles assign alice@acme.com tech_lead --scope team:backend
aeterna govern roles revoke alice@acme.com tech_lead --scope team:backend

# Approval workflow
aeterna govern pending
aeterna govern pending --scope org
aeterna govern approve prop_abc123 --comment "LGTM"
aeterna govern reject prop_abc123 --reason "Needs revision"

# Audit
aeterna govern audit --last 7d
aeterna govern audit --scope company --from 2024-01-01 --to 2024-01-31
aeterna govern audit --scope org --event-type policy_approved
aeterna govern audit --scope team --format csv > audit.csv
aeterna govern audit --scope company --format json | jq '.events[]'
```

#### Organization Management

```bash
# Initialize company
aeterna init
aeterna init --company "Acme Corp" --admin admin@acme.com
aeterna init --company "Acme" --governance strict

# Create organization
aeterna org create "Platform Engineering"
aeterna org create "Product" --inherit-from platform-engineering

# Create team
aeterna team create "API Team" --org platform-engineering
aeterna team create "Backend" --org engineering --lead alice@acme.com

# Initialize project
aeterna project init
aeterna project init --team api-team
aeterna project init --path /path/to/repo

# User management
aeterna user register --email bob@acme.com
aeterna user register --email carol@acme.com --teams api-team,data-team --role developer
aeterna user list
aeterna user list --team api-team
aeterna user whoami

# Agent management
aeterna agent register --name "code-assistant" --delegated-by alice@acme.com
aeterna agent register --agent-id "agent:ci-bot" --scope project --max-severity warn
aeterna agent list
aeterna agent list --user alice@acme.com
aeterna agent revoke agent:opencode-alice
```

#### Admin Commands

```bash
# Health check
aeterna admin health
aeterna admin health --verbose

# Validate all policies
aeterna admin validate --all
aeterna admin validate --scope project
aeterna admin validate --policy security-baseline

# Migration
aeterna admin migrate --from v1 --to v2 --dry-run
aeterna admin migrate --from v1 --to v2 --execute

# Export/Import
aeterna admin export policies --scope company > policies.json
aeterna admin export policies --scope org --format yaml > org-policies.yaml
aeterna admin import policies < policies.json
aeterna admin import knowledge --from ./knowledge-export/

# Drift detection
aeterna admin drift --scope project
aeterna admin drift --scope org --threshold 0.3
aeterna admin drift --all

# Sync
aeterna sync
aeterna sync --force
aeterna sync --memory-knowledge
```

---

## Integration Scenarios

### CI/CD Integration

#### GitHub Actions Example

```yaml
name: Aeterna Governance Check

on: [pull_request]

jobs:
  governance-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Aeterna CLI
        run: |
          curl -sSL https://get.aeterna.dev | sh
          
      - name: Context Resolution
        env:
          AETERNA_API_URL: ${{ secrets.AETERNA_API_URL }}
          AETERNA_AGENT_ID: agent:ci-bot
          AETERNA_AGENT_TOKEN: ${{ secrets.AETERNA_AGENT_TOKEN }}
        run: |
          aeterna context show
          
      - name: Check Dependencies
        run: |
          # Extract dependencies and check each
          cat package.json | jq -r '.dependencies | keys[]' | while read dep; do
            if ! aeterna check dependency "$dep"; then
              echo "::error::Dependency $dep violates policy"
              exit 1
            fi
          done
          
      - name: Validate Against Policies
        run: |
          aeterna check --all
          
      - name: Post Results to PR
        if: failure()
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: '❌ Governance check failed. See logs for details.'
            })
```

#### GitLab CI Example

```yaml
governance_check:
  stage: test
  image: aeterna/cli:latest
  variables:
    AETERNA_API_URL: $AETERNA_API_URL
    AETERNA_AGENT_ID: agent:ci-bot
    AETERNA_AGENT_TOKEN: $AETERNA_AGENT_TOKEN
  script:
    - aeterna context show
    - aeterna check --all
    - aeterna admin drift --scope project
  only:
    - merge_requests
```

### AI Assistant Integration

#### OpenCode Plugin

```typescript
// .opencode/config.ts
import { AeternaPlugin } from '@kiko-aeterna/opencode-plugin';

export default {
  plugins: [
    new AeternaPlugin({
      apiUrl: process.env.AETERNA_API_URL,
      agentId: 'agent:opencode-alice',
      agentToken: process.env.AETERNA_AGENT_TOKEN,
      
      // Auto-inject context into agent prompts
      autoInjectContext: true,
      
      // Check policies before code generation
      policyCheckBeforeGeneration: true,
      
      // Add memories after successful tasks
      autoMemoryCapture: true,
      
      // Search knowledge for relevant ADRs/patterns
      knowledgeSearchEnabled: true,
    }),
  ],
};
```

#### Agent Workflow

```python
# Example: AI agent autonomously managing governance

async def agent_workflow(user_request: str):
    # Step 1: Search for relevant knowledge
    knowledge = await aeterna_knowledge_search(
        query=user_request,
        layers=["project", "team", "org", "company"]
    )
    
    # Step 2: Search for relevant memories
    memories = await aeterna_memory_search(
        query=user_request,
        layers=["auto"]  # All accessible layers
    )
    
    # Step 3: Generate code with context
    code = await generate_code(
        request=user_request,
        knowledge_context=knowledge,
        memory_context=memories
    )
    
    # Step 4: Check policies before returning
    validation = await aeterna_check(
        code=code,
        dependencies=extract_dependencies(code)
    )
    
    if validation.has_blocking_violations:
        # Explain violation and regenerate
        return f"Cannot proceed: {validation.blocking_message}"
    
    # Step 5: Add memory about decision
    if code_quality_high(code):
        await aeterna_memory_add(
            content=f"Successfully implemented {user_request}",
            layer="session",
            tags=["success", "implementation"]
        )
    
    return code
```

### Pre-Commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Check if Aeterna is available
if ! command -v aeterna &> /dev/null; then
    echo "Aeterna CLI not found, skipping governance check"
    exit 0
fi

echo "Running Aeterna governance check..."

# Get staged files
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)

# Check dependencies if package.json changed
if echo "$STAGED_FILES" | grep -q "package.json"; then
    echo "Checking dependencies..."
    if ! aeterna check --dependencies; then
        echo "❌ Dependency check failed"
        exit 1
    fi
fi

# Check policies
if ! aeterna check --staged; then
    echo "❌ Policy check failed"
    echo "Override with: git commit --no-verify"
    exit 1
fi

echo "✅ Governance check passed"
exit 0
```

---

## Advanced Topics

### Natural Language to Cedar Translation

#### How It Works

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     TRANSLATION PIPELINE                                     │
│                                                                              │
│  INPUT: "Block MySQL in this project"                                        │
│                                                                              │
│  Step 1: Intent Extraction (LLM)                                             │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                                           │
│  Prompt: Extract policy intent from natural language                         │
│  Output: {                                                                   │
│    "action": "deny",                                                         │
│    "target_type": "dependency",                                              │
│    "target_value": "mysql",                                                  │
│    "severity": "block"                                                       │
│  }                                                                           │
│                                                                              │
│  Step 2: Cedar Generation                                                    │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━                                                   │
│  Template match: Deny + Dependency → Use forbid() pattern                    │
│  Output: forbid(                                                             │
│            principal,                                                        │
│            action == Action::"UseDependency",                                │
│            resource                                                          │
│          )                                                                   │
│          when {                                                              │
│            resource.dependency == "mysql"                                    │
│          };                                                                  │
│                                                                              │
│  Step 3: Validation                                                          │
│  ━━━━━━━━━━━━━━━━━━                                                          │
│  Cedar parser: ✅ Syntax valid                                               │
│  Schema check: ✅ Compliant                                                  │
│  Conflict check: ⚠️  Warning: Consider blocking mysql2, mariadb as well      │
│                                                                              │
│  Step 4: Human-Readable Explanation (LLM)                                    │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                                   │
│  Prompt: Explain this Cedar policy in plain English                          │
│  Output: "This policy blocks any code that uses MySQL as a dependency.       │
│           Violations will prevent the action from proceeding."               │
│                                                                              │
│  OUTPUT: PolicyDraft (ready for review and proposal)                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Common Translation Patterns

| Natural Language | Structured Intent | Cedar Template |
|------------------|-------------------|----------------|
| "Block MySQL" | `{action: deny, target: dependency, value: "mysql"}` | `forbid() when { resource.dependency == "mysql" }` |
| "Require README" | `{action: allow, target: file, value: "README.md", condition: must_exist}` | `permit() when { file.exists("README.md") }` |
| "No console.log in production" | `{action: deny, target: code, pattern: "console\\.log"}` | `forbid() when { code.matches("console\\.log") }` |
| "Only architects can approve" | `{action: allow, principal: role, value: "architect"}` | `permit() when { principal.role == Role::"Architect" }` |

### Meta-Governance: Policies About Policies

Meta-governance defines **who can create, approve, and enforce policies**.

#### Example: Company-Level Meta-Governance

```bash
$ aeterna govern configure --scope company --interactive

🔧 Meta-Governance Configuration

Who can propose company-level policies?
  [architects, admins]: admins

Who can approve company-level policies?
  [architect, admin]: admin

Required approvers: 2

Auto-approve for roles: none

Review period (hours): 72

What happens if no response?
  [escalate, auto-reject]: escalate

Escalate to: ceo@acme.com

Save? [y/N]: y

✅ Meta-governance configured

Effective rules:
- Only admins can propose company policies
- Requires 2 admin approvals
- 72-hour review period
- Auto-escalates to CEO if no response
```

#### Cedar Meta-Policy Example

```cedar
// Meta-policy: Only admins can configure governance
permit(
  principal,
  action == Action::"ConfigureGovernance",
  resource
)
when {
  principal.role == Role::"Admin" &&
  principal.scope.contains(resource.scope)
};

// Meta-policy: Agents cannot approve blocking policies
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
}
only_if {
  principal.role == Role::"Admin"
};
```

### Context Resolution Deep Dive

#### Resolution Priority Order

```
1. Explicit CLI flags
   --company acme --org platform --team api --project payments
   ↓ HIGHEST PRECEDENCE
   
2. Environment variables
   AETERNA_COMPANY=acme
   AETERNA_ORG=platform
   AETERNA_TEAM=api
   AETERNA_PROJECT=payments
   ↓
   
3. Local context file
   .aeterna/context.toml
   ↓
   
4. Parent directory traversal
   Walk up from current dir looking for .aeterna/
   ↓
   
5. Git remote detection
   git remote -v → github.com/acme/payments-service
   ↓
   
6. Git user detection
   git config user.email → alice@acme.com
   ↓
   
7. SSO/JWT claims
   JWT token → { company: "acme", email: "alice@acme.com" }
   ↓
   
8. Interactive prompt
   Ask user to select company/org/team
   ↓ LOWEST PRECEDENCE
```

#### Context File Schema

```toml
# .aeterna/context.toml

[context]
company = "acme-corp"
org = "platform-engineering"
team = "api-team"
project = "payments-service"

[user]
email = "alice@acme.com"

[agent]
# Populated if running as agent
id = "agent:opencode-alice"
delegated_by = "alice@acme.com"

[defaults]
memory_layer = "project"
policy_scope = "project"
auto_sync = true

[overrides]
# Override auto-detection
# team = "backend-team"  # Uncomment to force team context
```

#### Git Remote Mapping

Aeterna maps git remote URLs to organizational structure:

| Git Remote | Detected Context |
|------------|------------------|
| `github.com/acme/payments-service` | company: acme, project: payments-service |
| `gitlab.com/acme-corp/platform/api-team/gateway` | company: acme-corp, org: platform, team: api-team, project: gateway |
| `bitbucket.org/acme/data/analytics-pipeline` | company: acme, org: data, project: analytics-pipeline |

Patterns are configurable in OPAL Server.

---

## Troubleshooting

### Common Issues

#### Issue: Context Resolution Fails

```bash
$ aeterna status
ERROR: Unable to resolve context

Possible causes:
1. Not in a git repository
2. No .aeterna/context.toml found
3. Git remote not mapped to organization
4. Environment variables not set

Solutions:
  # Option 1: Initialize project
  aeterna project init --team api-team
  
  # Option 2: Set context explicitly
  aeterna context set --company acme --org platform --team api
  
  # Option 3: Use environment variables
  export AETERNA_COMPANY=acme
  export AETERNA_ORG=platform
  export AETERNA_TEAM=api
  
  # Option 4: Use CLI flags
  aeterna status --company acme --org platform --team api
```

#### Issue: Policy Validation Fails

```bash
$ aeterna policy create "Block MySQL" --scope project
ERROR: Policy validation failed

Validation errors:
  • Schema error: Unknown attribute 'dep' on resource
    Line 3: resource.dep == "mysql"
    Suggestion: Did you mean 'dependency'?

Solution:
  The natural language translator made an error.
  Try being more specific:
  
  aeterna policy create "Block MySQL database dependency" --scope project
```

#### Issue: Approval Workflow Stuck

```bash
$ aeterna govern pending
You have 1 pending approval:

[POLICY] prop_abc123 - "Require 2FA"
  Status: PENDING
  Created: 5 days ago
  Expires: EXPIRED
  Required approvers: 1
  Current approvals: 0

$ aeterna govern status
⚠️  Warning: 1 expired proposal

Solution:
  # Option 1: Manually approve (if authorized)
  aeterna govern approve prop_abc123
  
  # Option 2: Reject and re-propose
  aeterna govern reject prop_abc123 --reason "Expired, will re-propose"
  
  # Option 3: Configure auto-escalation
  aeterna govern configure --scope team --auto-escalate-after 48h
```

#### Issue: Agent Token Expired

```bash
$ aeterna check
ERROR: Agent authentication failed
Reason: Token expired

Solution:
  # Renew agent token
  aeterna agent renew agent:opencode-alice
  
  # Or register new agent
  aeterna agent register --name "code-assistant" --delegated-by alice@acme.com
  
  # Update environment variable
  export AETERNA_AGENT_TOKEN="<new-token>"
```

#### Issue: OPAL Sync Failure

```bash
$ aeterna status
⚠️  Warning: OPAL Server unreachable

$ aeterna admin health
❌ OPAL Server: connection refused

Solution:
  # Check OPAL Server status
  curl http://opal-server:8181/health
  
  # Check network connectivity
  ping opal-server
  
  # Verify configuration
  cat ~/.aeterna/config.toml | grep opal_url
  
  # Update OPAL URL if needed
  aeterna config set opal_url "https://opal.acme.com"
```

### Debugging Tips

**Enable verbose logging:**

```bash
export AETERNA_LOG_LEVEL=debug
aeterna policy create "Block MySQL" --verbose
```

**Check Cedar Agent sync status:**

```bash
$ aeterna admin health --component cedar-agent

Cedar Agent Status:
  Connected to OPAL: ✅
  Last sync: 2 minutes ago
  Synced policies: 47
  Synced data: companies (1), orgs (3), teams (12), users (127)
  
Subscribed topics:
  - company:acme-corp
  - org:platform-engineering
  - team:api-team
```

**View raw Cedar policy:**

```bash
$ aeterna policy draft show draft_abc123 --format cedar

forbid(
  principal,
  action == Action::"UseDependency",
  resource
)
when {
  resource.dependency == "mysql"
};
```

**Test policy simulation manually:**

```bash
$ aeterna policy simulate draft_abc123 \
  --scenario '{"dependencies": ["mysql", "pg", "redis"]}' \
  --verbose

Simulation Results:

Scenario: custom
Context:
  dependencies: ["mysql", "pg", "redis"]

Policy Evaluation:
  Rule: forbid-mysql-dependency
    Target: dependency
    Operator: ==
    Value: "mysql"
    Match: dependencies[0] = "mysql"
    Result: VIOLATION

Outcome: BLOCK
Violations:
  • MySQL dependency is prohibited (severity: block)
```

---

## Summary

Aeterna's UX-First Governance revolutionizes enterprise policy management by:

1. **Eliminating complexity**: Natural language instead of Cedar DSL
2. **Zero-config onboarding**: Auto-detect context, initialize in seconds
3. **Real-time sync**: OPAL + Cedar Agent keep policies consistent globally
4. **AI-first design**: Agents propose, simulate, and manage governance autonomously
5. **Meta-governance**: Policies about policies ensure proper oversight
6. **Complete audit trail**: Every action logged from intent to enforcement

**The result**: Governance becomes an enabler, not a blocker. Developers stay in flow, architects maintain standards, and AI agents operate with safety guardrails.

---

## Next Steps

- **For Developers**: Run `aeterna status` and explore your current context
- **For Tech Leads**: Configure team governance with `aeterna govern configure`
- **For Architects**: Create org-wide policies with `aeterna policy create`
- **For Admins**: Bootstrap your organization with `aeterna init`

**Need help?**
- Documentation: https://docs.aeterna.dev
- Community: https://discord.gg/aeterna
- Support: support@aeterna.dev

---

**Document Version**: 1.0.0
**Last Updated**: January 2024
**Change**: add-ux-first-governance
