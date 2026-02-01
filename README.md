# Aeterna

**Universal Memory & Knowledge Framework for Enterprise AI Agent Systems**

Aeterna provides hierarchical memory storage and governed organizational knowledge for AI agents at scale. Built for companies deploying AI coding assistants, autonomous agents, and intelligent automation across hundreds of engineers and thousands of projects.

---

## Why Aeterna?

Modern enterprises face critical challenges when deploying AI agents:

| Challenge | Impact | Aeterna Solution |
|-----------|--------|------------------|
| **Context window limits** | Agents forget previous interactions | Semantic memory with intelligent retrieval |
| **Knowledge fragmentation** | Decisions scattered across wikis, docs, Slack | Git-versioned knowledge repository |
| **No memory hierarchy** | All information treated equally | 7-layer memory with precedence rules |
| **Vendor lock-in** | Switching providers requires rewrites | Provider-agnostic adapter architecture |
| **Knowledge drift** | No audit trail for architectural decisions | Immutable commits, constraint enforcement |
| **Multi-tenant chaos** | Teams stepping on each other | Hierarchical isolation with policy inheritance |
| **Compliance gaps** | AI agents violating organizational standards | Cedar/Permit.io authorization + policy engine |
| **Agent coordination** | No shared context between agents | A2A protocol via Radkit integration |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           AI AGENT ECOSYSTEM                                 │
│                                                                              │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │  LangChain  │  │   AutoGen   │  │   CrewAI    │  │  OpenCode   │       │
│   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘       │
│          └────────────────┴────────────────┴────────────────┘               │
│                                    │                                         │
│                          ┌─────────▼─────────┐                              │
│                          │   MCP Tool API    │                              │
│                          │ (11 unified tools) │                              │
│                          └─────────┬─────────┘                              │
└────────────────────────────────────┼────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼────────────────────────────────────────┐
│                              AETERNA CORE                                    │
│                                                                              │
│   ┌────────────────────────┐              ┌────────────────────────┐        │
│   │     MEMORY SYSTEM      │              │  KNOWLEDGE REPOSITORY  │        │
│   │                        │              │                        │        │
│   │  • 7-layer hierarchy   │◄────────────►│  • Git-versioned       │        │
│   │  • Vector retrieval    │  Sync Bridge │  • Constraint DSL      │        │
│   │  • Memory-R1 rewards   │              │  • Policy enforcement  │        │
│   │  • DuckDB graph layer  │              │  • Natural language → │        │
│   └───────────┬────────────┘              │    Cedar translation   │        │
│               │                                       │                      │
│   ┌───────────▼────────────┐              ┌───────────▼────────────┐        │
│   │   GOVERNANCE ENGINE    │              │   AUTHORIZATION        │        │
│   │                        │              │                        │        │
│   │  • Policy inheritance  │              │  • Cedar policies      │        │
│   │  • Drift detection     │              │  • RBAC (5 roles)      │        │
│   │  • Merge strategies    │              │  • OPAL integration    │        │
│   │  • Multi-tenant ReBAC  │              │  • Tenant isolation    │        │
│   └────────────────────────┘              └────────────────────────┘        │
│                                                                              │
│   ┌────────────────────────────────────────────────────────────────────────┐ │
│   │                        CCA CAPABILITIES                                 │ │
│   │  • Context Architect: Hierarchical compression                         │ │
│   │  • Note-Taking Agent: Trajectory distillation                          │ │
│   │  • Hindsight Learning: Error capture & patterns                        │ │
│   │  • Meta-Agent: Build-test-improve loop                                 │ │
│   └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
                                     │
┌────────────────────────────────────▼────────────────────────────────────────┐
│                           STORAGE ADAPTERS                                   │
│                                                                              │
│   Vector Backends (Pluggable):                                              │
│   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│   │ Qdrant  │ │pgvector │ │Pinecone │ │Weaviate │ │ MongoDB │ │VertexAI │  │
│   └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘  │
│   ┌───────────┐                                                              │
│   │Databricks │                                                              │
│   └───────────┘                                                              │
│                                                                              │
│   Infrastructure:                                                            │
│   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────────┐  │
│   │  Redis  │ │ DuckDB  │ │Permit.io│ │   OPAL  │ │     Radkit A2A      │  │
│   └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Multi-Tenant Hierarchy

Aeterna's organizational hierarchy enables enterprise-scale deployment:

### Memory Layers (7 levels)

```
agent    ←── Per-agent instance (most specific)
   │         "Agent-specific learnings, tool preferences"
user         Per-user
   │         "User preferences, communication style"
session      Per-conversation
   │         "Current task context, recent decisions"
project      Per-repository
   │         "Project conventions, tech stack choices"
team         Per-team
   │         "Team standards, shared patterns"
org          Per-organization/department
   │         "Org-wide policies, compliance rules"
company  ←── Per-tenant (least specific)
             "Company standards, global policies"
```

### Knowledge Layers (4 levels)

```
Company (highest precedence)
    ↓ Policies flow DOWN
Organization
    ↓ Teams inherit + customize
Team
    ↓ Projects inherit + override
Project (lowest precedence)
```

### Example: 300-Engineer SaaS Platform

```
Acme Corp (Company)
├── Platform Engineering (Org)
│   ├── API Team (Team)
│   │   ├── payments-service (Project)
│   │   ├── auth-service (Project)
│   │   └── gateway-service (Project)
│   └── Data Platform Team (Team)
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

Each AI agent operating in `payments-service` automatically:
1. Inherits company-wide security policies
2. Applies Platform Engineering compliance rules
3. Follows API Team coding standards
4. Uses project-specific conventions

---

## Policy Inheritance & Governance

### Merge Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| **Override** | Child completely replaces parent | Project needs different rules |
| **Merge** | Combines rules from both | Adding project-specific rules |
| **Intersect** | Keeps only common rules | Stricter compliance |

### Example: Security Policy Flow

```rust
// Company-level: Security Baseline (Mandatory)
let company_policy = Policy {
    id: "security-baseline",
    layer: KnowledgeLayer::Company,
    mode: PolicyMode::Mandatory,
    merge_strategy: RuleMergeStrategy::Merge,
    rules: vec![
        rule!(MustNotUse, Dependency, "lodash < 4.17.21", Block, 
              "CVE-2021-23337: Prototype pollution"),
        rule!(MustExist, File, "SECURITY.md", Warn,
              "Security documentation required"),
    ],
};

// Org-level: Platform Engineering Standards
let org_policy = Policy {
    id: "platform-standards",
    layer: KnowledgeLayer::Org,
    mode: PolicyMode::Mandatory,
    merge_strategy: RuleMergeStrategy::Merge,
    rules: vec![
        rule!(MustUse, Dependency, "opentelemetry", Warn,
              "All services must emit traces"),
        rule!(MustMatch, Code, r"Result<.*, Error>", Info,
              "Use typed errors, not panics"),
    ],
};

// Team-level: API Team Conventions
let team_policy = Policy {
    id: "api-team-conventions",
    layer: KnowledgeLayer::Team,
    mode: PolicyMode::Optional,
    merge_strategy: RuleMergeStrategy::Merge,
    rules: vec![
        rule!(MustMatch, Config, r"\"timeout\":\s*\d+", Warn,
              "All API clients must specify timeouts"),
    ],
};
```

**Result**: An AI agent working on `payments-service` evaluates ALL policies:
- ❌ Blocked if using vulnerable lodash
- ⚠️ Warned if missing opentelemetry
- ℹ️ Informed if not using Result types
- ⚠️ Warned if API clients lack timeouts

---

## Role-Based Access Control

### Role Hierarchy

| Role | Precedence | Capabilities |
|------|------------|--------------|
| **Admin** | 4 | Full system access, manage all resources |
| **Architect** | 3 | Design policies, manage knowledge repository |
| **TechLead** | 2 | Manage team resources, enforce policies |
| **Developer** | 1 | Standard development, knowledge access |
| **Agent** | 0 | Delegated permissions from user context |

### Cedar Authorization + OPAL Integration

```cedar
// Allow users to view knowledge in their unit hierarchy
permit (
    principal,
    action == Action::"ViewKnowledge",
    resource
)
when {
    principal in resource.members
};

// AI agents inherit permissions from delegating user
permit (
    principal is Agent,
    action,
    resource
)
when {
    principal.delegatedBy in resource.members &&
    principal.delegatedBy has permission action on resource
};
```

---

## Memory-R1: Autonomous Optimization

Aeterna includes Memory-R1, an autonomous memory optimization system inspired by reinforcement learning:

### Reward-Based Promotion

Aeterna implements an autonomous reward propagation loop for its Recursive Language Model (RLM) search strategy. When the system decomposes complex queries:
- **Automatic Recognition**: Memories discovered via graph traversals or recursive steps are automatically identified.
- **Reward Propagation**: Successful search trajectories apply positive reward signals to all involved memories.
- **Dynamic Hierarchy**: Reward scores influence the `importance_score`, which determines if a memory is promoted to broader organizational layers (e.g., from Session to Project or Team).

```rust
// Rewards are applied automatically during RLM search
for step in trajectory.steps {
    if step.reward > 0.0 {
        for memory_id in step.involved_memory_ids {
            manager.record_reward(ctx, layer, &memory_id, reward_signal).await?;
        }
    }
}
```

### Feedback Loop

1. Agent uses memory during task
2. User provides feedback (explicit or implicit)
3. Memory receives reward signal
4. High-reward memories promoted to broader scope
5. Team/org benefits from individual learnings

---

## Graph Layer: DuckDB Integration

New with the **add-r1-graph-memory** change, Aeterna now includes a DuckDB-based graph storage layer:

```rust
// Graph relationship storage
pub struct GraphMemory {
    pub memory_id: MemoryId,
    pub relationships: Vec<Relationship>,
    pub embedding: Option<Vec<f32>>,
}

// Query traversals
let related = graph.query()
    .from_memory("payments-architecture")
    .follow_relationships("implements", "references")
    .depth(3)
    .execute().await?;
```

---

## CCA: Confucius Code Agent Capabilities

The **add-cca-capabilities** change introduces four specialized agents:

### Context Architect
Hierarchical context compression for efficient memory storage:
```rust
let compressed = context_architect.compress(session_memory)
    .with_hierarchy(true)
    .with_threshold(0.8)
    .execute().await?;
```

### Note-Taking Agent
Trajectory distillation to Markdown documentation:
```rust
let notes = note_taking.distill(agent_trajectory)
    .to_format(DocumentFormat::Markdown)
    .with_sections(&["decisions", "outcomes", "patterns"])
    .execute().await?;
```

### Hindsight Learning
Error capture and resolution pattern extraction:
```rust
let patterns = hindsight.extract_errors(failed_sessions)
    .identify_patterns()
    .suggest_improvements()
    .execute().await?;
```

### Meta-Agent
Build-test-improve self-refinement loop:
```rust
let improved = meta_agent.refine(agent_behavior)
    .with_feedback_loop(FedbackType::Hindsight)
    .with_iterations(3)
    .execute().await?;
```

---

## MCP Tool Interface

Aeterna exposes 11 unified tools via Model Context Protocol:

### Memory Tools

| Tool | Description |
|------|-------------|
| `memory_add` | Store new memory with layer targeting |
| `memory_search` | Semantic search across layers |
| `memory_delete` | Remove specific memory |
| `memory_feedback` | Provide reward signal for memory |
| `memory_optimize` | Trigger autonomous optimization |

### Knowledge Tools

| Tool | Description |
|------|-------------|
| `knowledge_query` | Search knowledge repository |
| `knowledge_check` | Validate against constraints |
| `knowledge_show` | Retrieve full knowledge item |

### Graph Tools

| Tool | Description |
|------|-------------|
| `graph_query` | Query memory relationships |
| `graph_neighbors` | Find related memories |
| `graph_path` | Discover connection paths |

---

## Roadmap: Active OpenSpec Changes

The following 2 changes are currently in development:

### 1. **add-helm-chart** - Kubernetes Deployment + CLI Setup Wizard
- **Status**: Implementation Phase
- **Key Features**: Helm chart with configurable dependencies, CLI setup wizard (`aeterna setup`)
- **Implemented**: Chart.yaml, values.yaml, values.schema.json, core templates (deployment, service, ingress, configmap, secret, HPA, PDB, NetworkPolicy, ServiceMonitor, migration job)
- **Remaining**: OPAL stack integration, deployment mode examples, testing, full documentation
- **Impact**: Production-ready enterprise deployment

### 2. **add-pluggable-vector-backends** - Enterprise Vector Database Support
- **Status**: Implementation Phase (90% complete)
- **Implemented Backends**: Qdrant, pgvector, Pinecone, Weaviate, MongoDB Atlas, Vertex AI, Databricks
- **Key Features**: Pluggable `VectorBackend` trait, tenant isolation, circuit breaker, observability
- **Remaining**: Integration tests (require live instances), backend-specific documentation
- **Impact**: Enterprise flexibility with cloud-native vector stores

---

## Recently Completed

The following changes have been archived (completed and deployed):

| Change | Description | Completed |
|--------|-------------|-----------|
| **add-opencode-plugin** | OpenCode plugin with MCP tools, hooks, SDK v1.1.36 | Jan 2026 |
| **add-radkit-integration** | Agent-to-Agent protocol via Radkit SDK | Jan 2026 |
| **add-ux-first-governance** | Natural language → Cedar policy, OPAL integration | Jan 2026 |
| **add-rlm-memory-navigation** | RLM memory navigation infrastructure | Jan 2026 |
| **add-cca-capabilities** | Context Architect, Note-Taking, Hindsight, Meta-Agent | Jan 2026 |
| **add-reflective-reasoning** | Pre-retrieval reasoning, multi-hop retrieval | Jan 2026 |
| **add-r1-graph-memory** | DuckDB graph layer, Memory-R1 optimization | Jan 2026 |
| **add-multi-tenant-governance** | ReBAC with Permit.io + Cedar, drift detection | Jan 2026 |

### OpenCode Plugin

The OpenCode plugin is published as `@kiko-aeterna/opencode-plugin`:

```bash
npm install -D @kiko-aeterna/opencode-plugin
```

```jsonc
{
  "plugin": ["@kiko-aeterna/opencode-plugin"]
}
```

**Features**: Memory tools, knowledge tools, graph tools, CCA tools, governance tools, automatic context injection, tool execution capture

---

## Production Readiness

Aeterna addresses **93 production readiness gaps** identified across enterprise deployments:

### Gap Distribution
- **Critical**: 19 gaps (data integrity, availability, cost control, security, stability)
- **High**: 47 gaps (performance, monitoring, scalability, compliance)
- **Medium**: 27 gaps (documentation, testing, tooling)

### Key Areas Addressed
- **Security**: Cedar/Permit.io authorization, policy enforcement, tenant isolation
- **Reliability**: Multi-tenant failover, data replication, disaster recovery
- **Performance**: Vector optimization, caching strategies, resource pooling
- **Compliance**: Audit trails, data retention, GDPR/CCPA support
- **Observability**: Metrics collection, distributed tracing, error tracking

📖 **[Full Details: Production Gaps Analysis](PRODUCTION_GAPS.md)**

---

## Quick Start

### Prerequisites

- **Rust**: 1.70+ (Edition 2024)
- **PostgreSQL**: 16+ with pgvector extension
- **Qdrant**: 1.12+
- **Redis**: 7+
- **DuckDB**: 0.9+ (for graph layer)

### Installation

```bash
git clone https://github.com/kikokikok/aeterna.git
cd aeterna

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Check coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Configuration

```toml
# config/aeterna.toml

[memory]
provider = "qdrant"
embedding_model = "text-embedding-3-small"

[memory.qdrant]
url = "http://localhost:6333"
collection_prefix = "aeterna"

[memory.graph]
provider = "duckdb"
path = "./data/graph.duckdb"

[knowledge]
backend = "git"
repository_path = "./knowledge-repo"

[governance]
authorization = "cedar"
policy_mode = "enforce"
opa_endpoint = "http://localhost:8181"
permit_sdk_key = "your-permit-io-key"

[governance.cedar]
schema_path = "./policies/cedar.cedarschema"

[cca]
enabled = true
context_architect = true
note_taking = true
hindsight_learning = true
meta_agent = true
```

### Basic Usage (Rust)

```rust
use aeterna_memory::{MemoryManager, MemoryLayer};
use aeterna_knowledge::{KnowledgeManager, KnowledgeQuery};
use aeterna_config::TenantContext;

// Create tenant context
let tenant = TenantContext::new("acme-corp")
    .with_org("platform-engineering")
    .with_team("api-team")
    .with_project("payments-service")
    .with_user("alice");

// Initialize memory manager
let memory = MemoryManager::new(config, tenant.clone()).await?;

// Store project-level memory
memory.add(
    "Use PostgreSQL for all new services per ADR-042",
    MemoryLayer::Project,
).await?;

// Search across all accessible layers
let results = memory.search("database selection").await?;

// Initialize knowledge manager
let knowledge = KnowledgeManager::new(config, tenant).await?;

// Query ADRs
let adrs = knowledge.query(KnowledgeQuery::new()
    .with_type(KnowledgeType::Adr)
    .with_tags(&["database"])
).await?;

// Check constraints before action
let violations = knowledge.check_constraints(
    ConstraintContext::new()
        .with_dependency("mysql")
).await?;

if violations.has_blocking() {
    // Agent stops, explains constraint
    return Err(violations.blocking_message());
}

// Graph query (new with add-r1-graph-memory)
let related = memory.graph_query()
    .from_memory_id("payments-architecture")
    .follow_relationships(&["implements", "references"])
    .depth(2)
    .execute().await?;
```

---

## Project Structure

```
aeterna/
├── adapters/           # Ecosystem integrations (OpenCode, LangChain, Radkit)
├── agent-a2a/          # Agent-to-Agent protocol implementation
├── config/             # Configuration management, hot reload
├── errors/             # Error handling framework
├── knowledge/          # Knowledge repository (Git-based)
├── memory/             # Memory system with R1 optimization + Graph layer
├── mk_core/            # Shared types and traits
├── storage/            # Storage layer (PostgreSQL, Qdrant, Redis, DuckDB)
├── sync/               # Memory-Knowledge sync bridge
├── tools/              # MCP tool interface
├── utils/              # Common utilities
├── specs/              # Detailed specifications (10 specs)
├── docs/               # Architecture documentation
├── openspec/           # Change proposals and versioning
├── test-project/       # Integration test project
└── agent-a2a/          # A2A protocol implementation
```

---

## Specifications

| Document | Description | Requirements |
|----------|-------------|-------------|
| [00-overview](specs/00-overview.md) | Executive summary and architecture | - |
| [01-core-concepts](specs/01-core-concepts.md) | Glossary and mental models | - |
| [02-memory-system](specs/02-memory-system.md) | Memory layers and operations | 21 requirements |
| [03-knowledge-repository](specs/03-knowledge-repository.md) | Git-based knowledge store | 17 requirements |
| [04-memory-knowledge-sync](specs/04-memory-knowledge-sync.md) | Pointer architecture | 11 requirements |
| [05-adapter-architecture](specs/05-adapter-architecture.md) | Provider abstraction | - |
| [06-tool-interface](specs/06-tool-interface.md) | MCP tool contracts | - |
| [07-configuration](specs/07-configuration.md) | Config schema | - |
| [08-deployment](specs/08-deployment.md) | Self-hosted vs cloud | - |
| [09-migration](specs/09-migration.md) | Data portability | - |
| [storage-spec](specs/storage/spec.md) | Storage layer specification | 10 requirements |

---

## Use Cases

### 1. Strangler Fig Platform Migration ⭐

**The flagship use case.** Transform a legacy monolith to microservices over 2-3 years with 300+ engineers:

```
┌─────────────────────────────────────────────────────────────────┐
│                  STRANGLER FIG WITH AETERNA                      │
│                                                                  │
│   KNOWLEDGE LAYER                    MEMORY LAYER               │
│   ━━━━━━━━━━━━━━━                   ━━━━━━━━━━━━━               │
│                                                                  │
│   ADRs:                              Team Learnings:            │
│   • Migration strategy               • "KApp has 20-char ID     │
│   • Tech debt payoffs                  limit - ACL must hash"   │
│   • API versioning                   • "Shadow test 2 weeks     │
│                                        before traffic shift"    │
│   Policies:                                                      │
│   • No new code in legacy            Agent Memory:              │
│   • Brick pattern required           • Tool preferences         │
│   • TigerBeetle for ledger           • What worked before       │
│                                                                  │
│   Patterns:                          Migration Memories:        │
│   • Strangler Facade                 • Gotchas discovered       │
│   • Anti-Corruption Layer            • Successful approaches    │
│   • Brick Specification              • Promoted to team/org     │
│                                                                  │
│   CCA Agents:                        Graph Relationships:       │
│   • Context Architect compression    • Service dependencies    │
│   • Note-taking trajectory docs      • Data flow mappings       │
│   • Hindsight error patterns         • Migration impact graph   │
└─────────────────────────────────────────────────────────────────┘
```

**What Aeterna provides:**
- **ADRs** capture migration decisions (Strangler Fig strategy, tech selections)
- **Policies** block legacy patterns from spreading (enforce at CI/CD)
- **Patterns** document reusable solutions (Strangler Facade, ACL, Bricks)
- **Memory** preserves team learnings (gotchas, workarounds, successes)
- **Agents** have full context for code generation and review
- **CCA** compresses context and learns from errors
- **Graph** discovers service dependencies and impact

📖 **[Full Example: Strangler Fig Migration Guide](docs/examples/strangler-fig-migration.md)**

### 2. Enterprise AI Coding Assistant with OpenCode Integration

Deploy AI coding assistants that:
- Remember individual developer preferences
- Apply team coding standards automatically
- Enforce company security policies
- Share learnings across the organization
- Integrate seamlessly with OpenCode via MCP plugin

### 3. Multi-Agent Platform with A2A Coordination

Build coordinated multi-agent systems where:
- Each agent has isolated memory
- Shared knowledge prevents conflicting decisions
- Policy constraints prevent dangerous actions
- Agents coordinate via Radkit A2A protocol
- Graph layer tracks agent relationships

### 4. AI-Powered DevOps with Helm Deployment

Automate infrastructure management with:
- Service-specific operational knowledge
- Team runbooks as enforceable constraints
- Incident learnings promoted across teams
- Compliance policies applied uniformly
- Production-ready Helm chart deployment

### 5. Knowledge-Augmented RAG with Reflective Reasoning

Enhance retrieval-augmented generation with:
- Hierarchical context from multiple scopes
- Constraint-guided response generation
- Version-controlled knowledge base
- Semantic deduplication
- Pre-retrieval reasoning for noise reduction
- Multi-hop retrieval for complex queries

---

## Development

### Testing

```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test -p aeterna-memory

# Run with coverage
cargo tarpaulin --out Html --all

# Run integration tests (requires Docker)
docker-compose up -d
cargo test --all -- --include-ignored
```

### Best Practices

- **Rust Edition**: 2024 (never 2021)
- **Error Handling**: `anyhow` for apps, `thiserror` for libs
- **Async**: Tokio runtime with proper cancellation
- **Safety**: Avoid `unsafe` unless necessary
- **Testing**: 80% coverage minimum (TDD/BDD enforced)

### Tech Stack

- **Language**: Rust (Edition 2024)
- **Memory Storage**: Redis 7+, PostgreSQL 16+ with pgvector, Qdrant 1.12+
- **Graph Storage**: DuckDB 0.9+
- **Embedding**: rust-genai 0.4+ (multi-provider)
- **Authorization**: Cedar + Permit.io + OPAL
- **Deployment**: Helm chart with Kubernetes support
- **A2A Protocol**: Radkit SDK integration
- **Testing**: TDD/BDD with 80% minimum coverage

---

## Contributing

1. Check existing [issues](../../issues) or [pull requests](../../pulls)
2. Follow [OpenSpec workflow](openspec/AGENTS.md) for changes
3. Ensure all tests pass and coverage targets met
4. Sign the [CLA](CLA.md)

---

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

---

## Acknowledgments

Built with insights from:
- [Mem0](https://mem0.ai) - Memory layer concepts
- [Letta](https://letta.com) - Agent memory patterns
- [Cedar](https://www.cedarpolicy.com) - Authorization language
- [OpenCode](https://opencode.ai) - AI coding assistant integration
- [Permit.io](https://permit.io) - ReBAC authorization
- [Radkit](https://radkit.dev) - Agent-to-Agent protocol
- [OPAL](https://opal.ac) - Policy administration