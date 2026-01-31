# Change: UX-First Architecture for Aeterna

## Why

Current Aeterna design exposes implementation complexity (Cedar DSL, TOML configs, Rust structs) directly to users. This creates barriers for both humans and AI agents across **all interactions**, not just governance:

### Core Problems

1. **Policy Creation**: Humans shouldn't need to learn Cedar policy language to express "Block MySQL in this project"
2. **Onboarding**: Organizations shouldn't need to manually configure TOML files and directory structures
3. **Context Resolution**: Users shouldn't need to specify `--company acme --org platform --team api` on every command
4. **Memory Discovery**: Finding relevant memories shouldn't require knowing internal layer names
5. **Knowledge Discovery**: Searching ADRs/patterns/policies shouldn't require file path knowledge
6. **Administration**: Governance is scattered across config files, API calls, and manual processes
7. **Day-to-Day Workflows**: No clear path from "I want to use Aeterna" to productive operation
8. **Organizational Referential**: No source of truth for who belongs to what team, which teams own which projects

### Missing Infrastructure: Organizational Referential

Aeterna currently lacks a **source of truth** for organizational topology:
- Company → Organization → Team → Project hierarchy
- User ↔ Team memberships (with roles)
- Agent ↔ User delegation chains
- Project ↔ Team ownership

Without this, context resolution is guesswork. We need a **real-time synchronized referential** that:
- Stores organizational structure
- Pushes updates to policy agents instantly
- Integrates with enterprise IdPs (Okta, Azure AD)
- Supports self-hosted deployment

**Solution: OPAL (Open Policy Administration Layer)** - Apache 2.0 licensed, self-hostable, with native Cedar Agent support.

### Design Principle

**Every Aeterna capability must be accessible through:**
- Natural language (for humans and LLM agents)
- Simple API/MCP tools (for programmatic access)
- CLI commands (for automation and scripts)

Implementation details (Cedar, TOML, layer enums, OPAL internals) are never exposed to end users.

## What Changes

### New Capabilities

#### 1. Policy Skill (Natural Language → Cedar)

- `aeterna_policy_draft` - Intent → Cedar (returns for review, not applied)
- `aeterna_policy_validate` - Validate Cedar syntax/semantics
- `aeterna_policy_propose` - Submit for approval workflow
- `aeterna_policy_list` - View active policies (natural language summaries)
- `aeterna_policy_explain` - Cedar → Natural language explanation
- `aeterna_policy_simulate` - Test policy against hypothetical scenarios

#### 2. Governance Administration

- `aeterna_governance_configure` - Set up governance rules
- `aeterna_governance_roles` - Manage role assignments
- `aeterna_governance_audit` - View governance activity
- `aeterna_governance_approve` / `_reject` - Approval workflow actions

#### 3. Meta-Governance (Policies about Policies)

- Who can propose policies at each layer
- Approval requirements (single approver, quorum, auto-approve)
- Review periods and escalation paths
- Delegation rules for AI agents

#### 4. Onboarding Skill (Zero-Config Bootstrap)

- `aeterna_org_init` - Initialize company/organization
- `aeterna_team_create` - Create team within organization
- `aeterna_project_init` - Initialize project (auto-detects git context)
- `aeterna_user_register` - Register user identity
- `aeterna_agent_register` - Register AI agent with delegation chain

**CLI equivalents:**
```bash
aeterna init                    # Auto-detect and bootstrap
aeterna org init "Acme Corp"    # Create company
aeterna team create "API Team"  # Create team in current org
aeterna project init            # Init from current git repo
aeterna user register           # Register current user
aeterna agent register          # Register agent with delegation
```

#### 5. Context Resolution (Automatic Identity Detection)

- **Git-based detection**: Remote URL → project, user.email → user identity
- **Environment-based**: `AETERNA_COMPANY`, `AETERNA_ORG`, `AETERNA_TEAM`
- **SSO/JWT-based**: Claims from enterprise identity providers
- **Explicit override**: `--scope company:acme/org:platform/team:api`
- **Context file**: `.aeterna/context.toml` for project-level defaults

**Auto-resolution order:**
1. Explicit CLI flags (highest precedence)
2. Environment variables
3. `.aeterna/context.toml` in current/parent directories
4. Git remote detection
5. SSO/JWT claims
6. Interactive prompt (lowest precedence)

#### 6. Memory Discovery Skill (Enhanced Search & Exploration)

- `aeterna_memory_search` - Natural language search across all accessible layers
- `aeterna_memory_browse` - Interactive exploration by layer/category
- `aeterna_memory_promote` - Promote memory to broader scope with reason
- `aeterna_memory_attribute` - Explain where a memory came from

**CLI equivalents:**
```bash
aeterna memory search "database decisions"
aeterna memory browse --layer team
aeterna memory promote <id> --to org --reason "Team consensus"
aeterna memory where <id>  # Show provenance
```

#### 7. Knowledge Discovery Skill (Semantic Search & Exploration)

- `aeterna_knowledge_search` - Natural language semantic search
- `aeterna_knowledge_browse` - Explore by type (ADR, pattern, policy, spec)
- `aeterna_knowledge_propose` - Propose new knowledge item with NL description
- `aeterna_knowledge_explain` - Get plain-English explanation of any item

**CLI equivalents:**
```bash
aeterna knowledge search "authentication approaches"
aeterna knowledge browse --type adr
aeterna knowledge propose "We should use JWT for auth"
aeterna knowledge explain ADR-042
```

#### 8. CLI Skill Interface (Human-Friendly Commands)

Every MCP tool has a CLI equivalent:

```bash
# Policy management
aeterna policy create "Block MySQL usage in this project"
aeterna policy list --scope team
aeterna policy simulate --intent "use Redis for caching"
aeterna policy explain POL-001

# Governance
aeterna govern approve <proposal-id>
aeterna govern reject <proposal-id> --reason "Need security review"
aeterna govern status
aeterna govern audit --since 7d

# Context
aeterna context show           # Display resolved context
aeterna context set --team api # Override context
aeterna context clear          # Reset to auto-detection

# Day-to-day
aeterna status                 # Quick health check
aeterna sync                   # Sync memory-knowledge
aeterna check                  # Run constraint validation
```

### Modified Capabilities

- **All existing tools** - Get natural language descriptions and context auto-injection
- **Knowledge proposal flow** - Integrated with governance approval workflow
- **Memory promotion** - Governance-aware promotion paths with audit trail
- **Constraint checking** - Natural language violation messages

### New Infrastructure: OPAL + Cedar Agent

#### OPAL Server (Organizational Referential)
- **Data sources**: PostgreSQL tables for Company/Org/Team/Project/User/Agent
- **Policy sources**: Git repository for Cedar policies, Aeterna knowledge repo
- **Real-time sync**: WebSocket PubSub to all Cedar Agents
- **IdP integration**: Fetch providers for Okta, Azure AD, Google Workspace

#### Cedar Agent (Authorization Decisions)
- Deployed via OPAL Client (one per service/region)
- Receives real-time policy and data updates from OPAL Server
- Answers authorization queries: "Can user X do action Y on resource Z?"
- Context resolution queries: "What is user X's team/org/company?"

#### Architecture Overview
```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              OPAL SERVER                                     │
│                                                                              │
│   PostgreSQL (Referential)          Git (Policies)                          │
│   ┌─────────────────────┐          ┌─────────────────────┐                 │
│   │ companies           │          │ cedar-policies/     │                 │
│   │ organizations       │          │   company.cedar     │                 │
│   │ teams               │          │   org.cedar         │                 │
│   │ projects            │          │   team.cedar        │                 │
│   │ users               │          │   meta-governance/  │                 │
│   │ agents              │          └─────────────────────┘                 │
│   │ memberships         │                                                   │
│   │ delegations         │          PubSub Channel                          │
│   └─────────────────────┘          (WebSocket)                             │
│                                           │                                  │
└───────────────────────────────────────────┼──────────────────────────────────┘
                                            │
              ┌─────────────────────────────┼─────────────────────────────┐
              │                             │                             │
              ▼                             ▼                             ▼
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│   OPAL Client + Cedar   │  │   OPAL Client + Cedar   │  │   OPAL Client + Cedar   │
│   Agent (Region A)      │  │   Agent (Region B)      │  │   Agent (CI/CD)         │
│                         │  │                         │  │                         │
│   Topics:               │  │   Topics:               │  │   Topics:               │
│   - company:acme        │  │   - company:acme        │  │   - company:acme        │
│   - org:platform        │  │   - org:data            │  │   - *                   │
└─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘
              │                             │                             │
              └─────────────────────────────┼─────────────────────────────┘
                                            │
                                            ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              AETERNA CORE                                    │
│                                                                              │
│   Memory System          Knowledge Repository         Tools/CLI             │
│   ┌─────────────┐       ┌─────────────────────┐      ┌─────────────┐       │
│   │ Qdrant      │       │ Git + Constraints   │      │ MCP Server  │       │
│   │ PostgreSQL  │       │ Policy Evaluation   │◄────►│ CLI         │       │
│   │ Redis       │       │ (via Cedar Agent)   │      │ Skills      │       │
│   └─────────────┘       └─────────────────────┘      └─────────────┘       │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Impact

### Affected Specs
- `tool-interface` - All new tools added
- `configuration` - Onboarding config, context resolution schema, OPAL config
- `deployment` - Organization bootstrap workflow, OPAL deployment

### New Infrastructure
- **OPAL Server** - Policy and data administration (self-hosted)
- **Cedar Agent** - Authorization engine (via OPAL Client)
- **PostgreSQL schema** - Organizational referential tables

### New Code
- `governance-ux/` crate - Policy translation, governance administration
- `onboarding/` crate - Organization/team/project initialization
- `context/` crate - Context resolution via Cedar Agent queries
- `cli/` crate - CLI skill interface (wraps MCP tools)
- `opal-fetcher/` crate - Custom OPAL data fetcher for Aeterna referential

### Documentation
- Complete UX guide with all personas
- Quick-start for each role (Developer, Tech Lead, Architect, Admin, Agent)
- Migration guide from raw Cedar/TOML
- OPAL deployment and configuration guide

## Success Criteria

### Policy & Governance
1. A developer can create a policy using only natural language
2. An AI agent can propose, validate, and track policy approval without generating Cedar
3. All governance actions have CLI equivalents
4. Meta-governance is self-configurable (governance rules are themselves governed)
5. Audit trail captures full intent-to-policy journey

### Onboarding & Context
6. `aeterna init` in a git repo fully bootstraps without manual config
7. Context is automatically resolved for 90%+ of operations
8. New team member can be productive within 5 minutes
9. AI agents can operate without explicit context parameters

### Organizational Referential (NEW)
10. Company/Org/Team/Project hierarchy is stored in PostgreSQL
11. User and Agent memberships are queryable in real-time
12. Changes to org structure propagate to all Cedar Agents within 1 second
13. IdP changes (Okta/Azure AD) sync to referential automatically
14. Context resolution uses Cedar Agent queries (not guesswork)

### Memory & Knowledge Discovery
15. Natural language search returns relevant results across all accessible layers
16. Users can explore and understand the knowledge graph without knowing internals
17. Memory promotion captures reason and creates audit trail
18. Knowledge proposals flow through governance automatically

### Day-to-Day Workflows
19. Each persona has documented, tested workflow covering daily tasks
20. CLI provides autocomplete and helpful error messages
21. No operation requires knowledge of Cedar, TOML, OPAL, or internal layer names
