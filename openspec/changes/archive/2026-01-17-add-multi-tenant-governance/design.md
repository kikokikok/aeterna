# Design: Multi-Tenant Governance Architecture

## Context

Target: 300 developers using Aeterna in various configurations:
- **Local**: Developer runs full stack locally for experimentation
- **Hybrid**: Local for sessions, central server for team/org/company knowledge
- **Remote**: Fully centralized deployment

Key governance requirements:
- Architects can reject promotions, force corrections (including LLM-based agents as architects)
- Tenant = Company (multi-company SaaS model)
- Within company: access-control based permissions (not hard isolation)
- Near real-time drift detection + periodic batch analysis

## Goals / Non-Goals

### Goals
- Multi-tenant isolation at company level
- ReBAC for fine-grained permissions within company
- Semantic drift detection across hierarchy levels
- Real-time governance events + batch reporting
- Support for LLM agents as governance actors

### Non-Goals
- Cross-company knowledge sharing (hard isolation)
- Replacing CI/CD pipelines (complementary to, not replacement for)
- Full compliance framework (SOC2, HIPAA - future work)

## Decisions

### 1. Multi-Tenancy Model

**Company = Hard tenant boundary** (database-level isolation optional, logical isolation required)

```
Company A (Tenant)
├── Org: Engineering
│   ├── Team: Backend
│   │   ├── Project: API Service
│   │   └── Project: Auth Service
│   └── Team: Frontend
│       └── Project: Web App
└── Org: Platform
    └── Team: Infrastructure
        └── Project: Terraform Modules
```

**Deployment Modes**:

| Mode | Local Components | Central Components |
|------|------------------|-------------------|
| Local | All (Redis, PG, Qdrant, Aeterna) | None |
| Hybrid | Working/Session memory only | Episodic+, Knowledge, Governance |
| Remote | None | All |

### 2. ReBAC Implementation (Permit.io + OPA/Cedar)

Using Permit.io SDK with self-hosted OPA engine for relationship-based access control:

**Policy Model (OPA Rego)**:
```rego
package aeterna.authz

default allow = false

allow {
    input.action == "read"
    resource_owners[input.user]
}

allow {
    input.action == "write"
    input.resource_type == "knowledge_item"
    can_propose[input.user]
}

allow {
    input.action == "approve"
    input.resource_type == "knowledge_item"
    is_architect_or_lead
}

resource_owners[owner] {
    data.relationships[input.resource_type][input.resource_id].owner == owner
}

can_propose[user] {
    data.roles[user].type == "contributor"
}

is_architect_or_lead {
    data.roles[input.user].type in ["architect", "lead"]
}
```

**Alternative: Cedar Policy** (if Cedar chosen over OPA):
```cedar
permit(principal == User::"alice", action == Action::"read", resource == KnowledgeItem::"item-123");

permit(principal, action == Action::"write", resource)
    when { has_role(principal, "contributor", resource) };

permit(principal, action == Action::"approve", resource)
    when { has_role(principal, "architect", resource) || has_role(principal, "lead", resource) };
```

**Permit.io Integration**:
- Self-hosted Permit.io policy decision point (PDP) with OPA/Cedar backend
- Permit.io SDK for relationship management (user-role-resource tuples)
- Policy as code: Rego/Cedar files versioned in knowledge repository
- API for architects to update policies without deployment

**Roles & Permissions**:

| Role | Scope | Can Do |
|------|-------|--------|
| Developer | Project | Add memories, propose knowledge, view |
| Tech Lead | Team | Approve promotions, manage team knowledge |
| Architect | Org | Reject proposals, force corrections, drift review |
| Admin | Company | Full access, tenant management |
| Agent | Inherited | Same as user it acts on behalf of |

### 3. Drift Detection Engine

**What is Drift?**
Semantic divergence between a project's knowledge/memories and the standards at higher hierarchy levels.

**Drift Types**:
1. **Contradicting Knowledge**: Project ADR contradicts Org policy
2. **Missing Compliance**: Project lacks required policies from Company
3. **Stale References**: Memories point to deprecated knowledge
4. **Pattern Deviation**: Code patterns in memories differ significantly from approved patterns

**Detection Methods**:

| Type | Method | Timing |
|------|--------|--------|
| Contradicting | Vector similarity between conflicting items | Real-time on creation |
| Missing | Set difference of required vs present | Real-time on sync |
| Stale | Hash comparison + status check | Real-time on access |
| Pattern Deviation | LLM semantic analysis | Batch (hourly/daily) |

**Drift Score Formula**:
```
drift_score(project) = Σ(severity_weight × item_drift) / total_items

where:
  item_drift = 1 - cosine_similarity(project_embedding, reference_embedding)
  severity_weight = { block: 1.0, warn: 0.5, info: 0.1 }
```

### 4. Event Architecture

**Real-time Events** (Redis Streams / NATS):
```rust
enum GovernanceEvent {
    KnowledgeProposed { item_id, proposer, layer },
    KnowledgeApproved { item_id, approver, layer },
    KnowledgeRejected { item_id, rejector, reason },
    MemoryPromoted { memory_id, from_layer, to_layer },
    DriftDetected { project_id, drift_score, items },
    PolicyViolation { project_id, policy_id, violation },
}
```

**Batch Jobs** (Tokio cron / Kubernetes CronJob):
- Hourly: Quick drift scan for active projects
- Daily: Full drift analysis with LLM-based semantic comparison
- Weekly: Comprehensive governance report generation

### 5. Hybrid Deployment Architecture

```
 ┌──────────────────────────────────────────────────────────────────┐
 │                     CENTRAL SERVER                                │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                  │
 │  │ Knowledge  │  │ Governance │  │   Event    │                  │
 │  │ Repository │  │   Engine   │  │   Stream   │                  │
 │  └────────────┘  └────────────┘  └────────────┘                  │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                  │
 │  │  Episodic  │  │   OPA/Cedar│  │   Batch    │                  │
 │  │   Memory   │  │   Engine   │  │   Jobs     │                  │
 │  └────────────┘  └────────────┘  └────────────┘                  │
 │  ┌────────────┐                                                 │
 │  │ Permit.io  │  ← Policy Decision Point (PDP)                   │
 │  │    SDK     │                                                 │
 │  └────────────┘                                                 │
 └──────────────────────────────────────────────────────────────────┘
                               │
                     ┌─────────┴─────────┐
                     │   Sync Protocol   │
                     │   (gRPC/REST)     │
                     └─────────┬─────────┘
                               │
 ┌──────────────────────────────────────────────────────────────────┐
 │                     LOCAL DEVELOPER                               │
 │  ┌────────────┐  ┌────────────┐  ┌────────────┐                  │
 │  │  Working   │  │  Session   │  │   Local    │                  │
 │  │  Memory    │  │  Memory    │  │   Cache    │                  │
 │  └────────────┘  └────────────┘  └────────────┘                  │
 │  ┌────────────────────────────────────────────┐                  │
 │  │          OpenCode Plugin                    │                  │
 │  │     (MCP Server + Session Manager)          │                  │
 │  └────────────────────────────────────────────┘                  │
 └──────────────────────────────────────────────────────────────────┘
 ```

### Alternatives Considered

1. **Permit.io vs OpenFGA vs Casbin**: Permit.io chosen for managed policy-as-code, OPA/Cedar ecosystem support, and self-hosting flexibility
2. **OPA vs Cedar**: OPA chosen for maturity (Cedar available as alternative)
3. **Kafka vs Redis Streams**: Redis Streams chosen for simplicity and existing Redis dependency
4. **PostgreSQL RLS vs Application-level tenancy**: Application-level chosen for flexibility, RLS as optional enhancement

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Self-hosted OPA/Ceder complexity | Docker deployment with Helm charts, monitoring setup |
| Permit.io SDK learning curve | Clear docs, reference implementations |
| Drift detection latency | Tiered approach (real-time for simple, batch for complex) |
| Hybrid sync conflicts | Conflict resolution with clear precedence rules |
| LLM costs for semantic analysis | Sampling + caching + configurable frequency |

## Migration Plan

1. **Phase 1**: Add tenant context to all operations (non-breaking, optional)
2. **Phase 2**: Add Permit.io SDK + OPA/Cedar integration with default permissive policies
3. **Phase 3**: Add drift detection (real-time simple checks)
4. **Phase 4**: Add batch analysis and reporting
5. **Phase 5**: Add hybrid sync protocol

## Open Questions

- [ ] OPA vs Cedar for policy engine (OPA recommended)?
- [ ] Permit.io policy storage location (PostgreSQL vs knowledge repo)?
- [ ] Drift score thresholds for alerting?
- [ ] LLM provider for semantic analysis (same as embeddings)?
- [ ] Event retention policy?
