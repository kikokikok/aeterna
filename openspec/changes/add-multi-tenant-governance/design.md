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

### 2. ReBAC Implementation (OpenFGA)

Using OpenFGA for relationship-based access control:

```fga
model
  schema 1.1

type user

type company
  relations
    define admin: [user]
    define member: [user] or admin

type organization
  relations
    define parent: [company]
    define admin: [user] or admin from parent
    define architect: [user] or admin
    define member: [user] or architect

type team
  relations
    define parent: [organization]
    define lead: [user] or admin from parent
    define architect: [user, agent] or architect from parent
    define member: [user] or lead

type project
  relations
    define parent: [team]
    define owner: [user] or lead from parent
    define contributor: [user] or owner
    define viewer: [user] or contributor

type knowledge_item
  relations
    define parent: [project, team, organization, company]
    define can_propose: contributor from parent
    define can_approve: architect from parent or lead from parent
    define can_reject: architect from parent
    define can_view: viewer from parent

type memory_entry
  relations
    define parent: [project, team, user]
    define can_promote: contributor from parent
    define can_view: viewer from parent

type agent
  relations
    define acts_as: [user]  # Agent can act on behalf of user
```

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
│  │  Episodic  │  │  OpenFGA   │  │   Batch    │                  │
│  │   Memory   │  │   Server   │  │   Jobs     │                  │
│  └────────────┘  └────────────┘  └────────────┘                  │
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

1. **Casbin vs OpenFGA**: OpenFGA chosen for native relationship modeling and Google Zanzibar lineage
2. **Kafka vs Redis Streams**: Redis Streams chosen for simplicity and existing Redis dependency
3. **PostgreSQL RLS vs Application-level tenancy**: Application-level chosen for flexibility, RLS as optional enhancement

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| OpenFGA operational complexity | Embed mode for small deployments, external for scale |
| Drift detection latency | Tiered approach (real-time for simple, batch for complex) |
| Hybrid sync conflicts | Conflict resolution with clear precedence rules |
| LLM costs for semantic analysis | Sampling + caching + configurable frequency |

## Migration Plan

1. **Phase 1**: Add tenant context to all operations (non-breaking, optional)
2. **Phase 2**: Add OpenFGA integration with default permissive policies
3. **Phase 3**: Add drift detection (real-time simple checks)
4. **Phase 4**: Add batch analysis and reporting
5. **Phase 5**: Add hybrid sync protocol

## Open Questions

- [ ] OpenFGA embedded vs external deployment threshold?
- [ ] Drift score thresholds for alerting?
- [ ] LLM provider for semantic analysis (same as embeddings)?
- [ ] Event retention policy?
