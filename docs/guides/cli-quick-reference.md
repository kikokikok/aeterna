# Aeterna CLI Quick Reference

**Fast command lookup for the Aeterna governance platform**

This is a concise cheat sheet. For detailed explanations and workflows, see [UX-First Governance Guide](ux-first-governance.md).

---

## Installation

```bash
# From source
git clone https://github.com/kikokikok/aeterna.git
cd aeterna
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

---

## Getting Started

| Command | Description |
|---------|-------------|
| `aeterna init` | Initialize company (interactive) |
| `aeterna init --company "Name" --admin email@example.com` | Initialize with defaults |
| `aeterna status` | Show current context and status |
| `aeterna whoami` | Show current user identity |
| `aeterna check` | Quick health check |

---

## Context Management

| Command | Description |
|---------|-------------|
| `aeterna context show` | Display current context |
| `aeterna context set --team TEAM` | Switch to team context |
| `aeterna context set --org ORG` | Switch to org context |
| `aeterna context clear` | Clear context overrides |

**Note**: Context auto-resolves from git config when available.

---

## Memory Operations

### Search and Browse

| Command | Description |
|---------|-------------|
| `aeterna memory search "QUERY"` | Semantic search across layers |
| `aeterna memory search "QUERY" --layer team` | Search specific layer |
| `aeterna memory search "QUERY" --last 30d` | Search recent memories |
| `aeterna memory search "QUERY" --min-relevance 0.8` | High-relevance only |
| `aeterna memory browse --layer team` | Browse team memories |
| `aeterna memory browse --layer team --category decisions` | Browse by category |

### Add and Manage

| Command | Description |
|---------|-------------|
| `aeterna memory add "TEXT" --layer project` | Add project memory |
| `aeterna memory add "TEXT" --layer team --tags "tag1,tag2"` | Add with tags |
| `aeterna memory promote ID --to team` | Promote to team layer |
| `aeterna memory promote ID --to team --reason "Why"` | Promote with justification |
| `aeterna memory where ID` | Show memory provenance |
| `aeterna memory attribution ID` | Show memory attribution |

### Feedback

| Command | Description |
|---------|-------------|
| `aeterna memory feedback ID --type helpful --score 0.9` | Positive feedback |
| `aeterna memory feedback ID --type not-helpful --score 0.2` | Negative feedback |

---

## Knowledge Operations

### Search and Browse

| Command | Description |
|---------|-------------|
| `aeterna knowledge search "QUERY"` | Search knowledge base |
| `aeterna knowledge search "QUERY" --type adr` | Search ADRs only |
| `aeterna knowledge search "QUERY" --layers company,org` | Search specific layers |
| `aeterna knowledge browse --type adr` | Browse all ADRs |
| `aeterna knowledge browse --type pattern --layer team` | Browse team patterns |
| `aeterna knowledge browse --type policy` | Browse policies |

### Retrieve and Explain

| Command | Description |
|---------|-------------|
| `aeterna knowledge get PATH` | Get knowledge item by path |
| `aeterna knowledge explain ID` | Explain knowledge item |
| `aeterna knowledge explain ID --verbose` | Detailed explanation |

### Propose

| Command | Description |
|---------|-------------|
| `aeterna knowledge propose "TEXT" --type adr` | Propose ADR |
| `aeterna knowledge propose "TEXT" --type pattern` | Propose pattern |
| `aeterna knowledge check` | Validate knowledge constraints |

---

## Policy Management

### Create and Draft

| Command | Description |
|---------|-------------|
| `aeterna policy create` | Create policy (interactive) |
| `aeterna policy create --interactive` | Same as above |
| `aeterna policy create "DESC" --scope project --severity block` | Create blocking policy |
| `aeterna policy create "DESC" --scope team --severity warn` | Create warning policy |

### List and Explain

| Command | Description |
|---------|-------------|
| `aeterna policy list` | List all active policies |
| `aeterna policy list --scope team` | List team policies |
| `aeterna policy list --scope org --include-inherited` | Include inherited |
| `aeterna policy list --severity block` | Show blocking policies |
| `aeterna policy list --format json` | JSON output |
| `aeterna policy explain ID` | Explain policy |
| `aeterna policy explain ID --verbose` | Detailed explanation |

### Simulate and Draft

| Command | Description |
|---------|-------------|
| `aeterna policy simulate DRAFT_ID` | Test policy draft |
| `aeterna policy simulate DRAFT_ID --scenario 'JSON'` | Test with scenario |
| `aeterna policy simulate DRAFT_ID --live` | Test against live project |
| `aeterna policy draft show DRAFT_ID` | View draft details |
| `aeterna policy draft list` | List all drafts |
| `aeterna policy draft submit DRAFT_ID` | Submit for approval |
| `aeterna policy draft submit DRAFT_ID --justification "Reason"` | Submit with reason |
| `aeterna policy draft delete DRAFT_ID` | Delete draft |

---

## Governance

### Configure

| Command | Description |
|---------|-------------|
| `aeterna govern configure --scope org --interactive` | Configure governance (interactive) |
| `aeterna govern configure --scope team --policy-approvers ROLES` | Set approvers |
| `aeterna govern configure --scope org --approval-count N` | Set approval count |
| `aeterna govern configure --scope org --review-period HOURS` | Set review period |

### Status and Roles

| Command | Description |
|---------|-------------|
| `aeterna govern status` | Show governance status |
| `aeterna govern status --scope company` | Company-wide status |
| `aeterna govern roles list` | List all roles |
| `aeterna govern roles list --scope team` | Team roles |
| `aeterna govern roles assign USER ROLE --scope SCOPE` | Assign role |
| `aeterna govern roles revoke USER ROLE --scope SCOPE` | Revoke role |

### Approvals

| Command | Description |
|---------|-------------|
| `aeterna govern pending` | Show pending approvals |
| `aeterna govern pending --scope org` | Org-level pending |
| `aeterna govern approve PROPOSAL_ID` | Approve proposal |
| `aeterna govern approve PROPOSAL_ID --comment "Comment"` | Approve with comment |
| `aeterna govern reject PROPOSAL_ID --reason "Reason"` | Reject proposal |

### Audit

| Command | Description |
|---------|-------------|
| `aeterna govern audit --last 7d` | Last 7 days |
| `aeterna govern audit --scope company --from DATE --to DATE` | Date range |
| `aeterna govern audit --scope org --event-type TYPE` | Filter by event type |
| `aeterna govern audit --format csv` | CSV output |
| `aeterna govern audit --format json` | JSON output |

---

## Organization Management

### Organization

| Command | Description |
|---------|-------------|
| `aeterna org create "NAME"` | Create organization |
| `aeterna org create "NAME" --inherit-from ORG` | Create with inheritance |

### Team

| Command | Description |
|---------|-------------|
| `aeterna team create "NAME" --org ORG` | Create team |
| `aeterna team create "NAME" --org ORG --lead EMAIL` | Create with lead |

### Project

| Command | Description |
|---------|-------------|
| `aeterna project init` | Initialize project (auto-detect) |
| `aeterna project init --team TEAM` | Initialize with team |
| `aeterna project init --path PATH` | Initialize at path |

### Users

| Command | Description |
|---------|-------------|
| `aeterna user register --email EMAIL` | Register user |
| `aeterna user register --email EMAIL --teams T1,T2 --role ROLE` | Register with details |
| `aeterna user list` | List all users |
| `aeterna user list --team TEAM` | List team users |
| `aeterna user whoami` | Show current user |

### Agents

| Command | Description |
|---------|-------------|
| `aeterna agent register --name NAME --delegated-by EMAIL` | Register agent |
| `aeterna agent register --agent-id ID --scope SCOPE` | Register with scope |
| `aeterna agent register --agent-id ID --max-severity LEVEL` | Limit severity |
| `aeterna agent list` | List all agents |
| `aeterna agent list --user EMAIL` | List user's agents |
| `aeterna agent revoke AGENT_ID` | Revoke agent |

---

## Administration

### Health and Validation

| Command | Description |
|---------|-------------|
| `aeterna admin health` | System health check |
| `aeterna admin health --verbose` | Detailed health check |
| `aeterna admin validate --all` | Validate all |
| `aeterna admin validate --scope SCOPE` | Validate scope |
| `aeterna admin validate --policy ID` | Validate policy |

### Migration

| Command | Description |
|---------|-------------|
| `aeterna admin migrate --from v1 --to v2 --dry-run` | Preview migration |
| `aeterna admin migrate --from v1 --to v2 --execute` | Execute migration |

### Export and Import

| Command | Description |
|---------|-------------|
| `aeterna admin export policies --scope SCOPE` | Export policies |
| `aeterna admin export policies --format yaml` | Export as YAML |
| `aeterna admin import policies < FILE` | Import policies |
| `aeterna admin import knowledge --from PATH` | Import knowledge |

### Drift Detection

| Command | Description |
|---------|-------------|
| `aeterna admin drift --scope SCOPE` | Detect drift |
| `aeterna admin drift --threshold N` | Set drift threshold |
| `aeterna admin drift --all` | Check all scopes |

### Sync

| Command | Description |
|---------|-------------|
| `aeterna sync` | Sync memory and knowledge |
| `aeterna sync --force` | Force sync |
| `aeterna sync --memory-knowledge` | Sync specific bridge |

---

## Common Workflows

### Developer Daily Workflow

```bash
# Morning: Check status
aeterna status

# Search for relevant knowledge
aeterna knowledge search "authentication patterns"

# Check if dependency is allowed
aeterna check dependency mysql

# Add project memory
aeterna memory add "Decided to use bcrypt for password hashing" --layer project

# Propose team policy
aeterna policy create "Require 2FA for admin endpoints" --scope project
```

### Tech Lead Approval Workflow

```bash
# Check pending approvals
aeterna govern pending

# Review policy draft
aeterna policy draft show draft_abc123

# Simulate policy
aeterna policy simulate draft_abc123 --live

# Approve proposal
aeterna govern approve prop_abc123 --comment "LGTM"

# Promote high-value memory
aeterna memory promote mem_xyz789 --to team --reason "Team-wide gotcha"
```

### Admin Onboarding Workflow

```bash
# Initialize company
aeterna init --company "Acme Corp" --admin admin@acme.com

# Create org structure
aeterna org create "Engineering"
aeterna team create "Backend" --org engineering --lead alice@acme.com

# Register users
aeterna user register --email bob@acme.com --teams backend --role developer

# Register AI agent
aeterna agent register --name "code-assistant" --delegated-by alice@acme.com

# Check system health
aeterna admin health --verbose
```

---

## Common Flags

| Flag | Description |
|------|-------------|
| `--help, -h` | Show help |
| `--version, -v` | Show version |
| `--json` | JSON output |
| `--verbose` | Verbose output |
| `--dry-run` | Preview without changes |
| `--scope SCOPE` | Target scope (company/org/team/project) |
| `--layer LAYER` | Memory layer (agent/user/session/project/team/org/company) |
| `--format FORMAT` | Output format (json/yaml/csv) |

---

## Scope Format

```
company:acme-corp
org:acme-corp:engineering
team:engineering:backend
project:backend:payments-service
```

---

## Layer Hierarchy

```
agent       (most specific)
user
session
project
team
org
company     (least specific)
```

Search precedence: agent → user → session → project → team → org → company

---

## Policy Severity Levels

| Level | Effect |
|-------|--------|
| `info` | Informational only |
| `warn` | Warning (allows action) |
| `block` | Blocking (prevents action) |

---

## Knowledge Types

| Type | Description |
|------|-------------|
| `adr` | Architecture Decision Records |
| `pattern` | Reusable patterns |
| `policy` | Governance policies |

---

## Role Hierarchy

| Role | Precedence | Capabilities |
|------|------------|--------------|
| `admin` | 4 | Full system access |
| `architect` | 3 | Design policies, manage knowledge |
| `tech_lead` | 2 | Manage team resources, approve proposals |
| `developer` | 1 | Standard development, propose changes |
| `agent` | 0 | Delegated permissions only |

---

## Environment Variables

```bash
export AETERNA_CONFIG_PATH=/path/to/config.toml
export AETERNA_AGENT_ID=agent:opencode-alice
export AETERNA_AGENT_TOKEN=aeterna_agent_abc123xyz
```

---

## Quick Troubleshooting

| Problem | Solution |
|---------|----------|
| Context not detected | Run `aeterna project init` in project root |
| Permission denied | Check role with `aeterna govern roles list` |
| Policy not syncing | Run `aeterna sync --force` |
| Health check fails | Run `aeterna admin health --verbose` |

---

## Further Reading

- [UX-First Governance Guide](ux-first-governance.md) - Complete guide with workflows
- [Strangler Fig Migration](../examples/strangler-fig-migration.md) - Real-world example
- [Architecture Overview](../../README.md) - System architecture

---

**Quick Links:**
- GitHub: https://github.com/kikokikok/aeterna
- Issues: https://github.com/kikokikok/aeterna/issues
- License: Apache 2.0
