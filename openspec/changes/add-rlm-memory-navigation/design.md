# Design: Intelligent Memory Retrieval (RLM Infrastructure)

## Context

Aeterna's memory system provides a 7-layer hierarchy for enterprise AI agents. Current retrieval uses vector search with optional pre-retrieval reasoning (MemR³). For complex queries spanning millions of tokens across layers, we need recursive decomposition - but this must be **invisible to users**.

This design introduces RLM as **internal infrastructure**:
1. **ComplexityRouter** - Automatically routes queries based on complexity
2. **RlmExecutor** - Executes decomposition strategies internally
3. **DecompositionTrainer** - Learns optimal strategies from usage

### Key Constraint: UX-First

From `add-ux-first-governance`:
> "Implementation details are never exposed to end users."

RLM is infrastructure. Users interact with `aeterna_memory_search`. The system decides when to use RLM.

### Stakeholders
- AI Agents (use existing MCP tools - no change)
- Memory-R1 system (extends existing trainer)
- Context Architect (uses RLM internally for complex assembly)

### Constraints
- Must not change existing `memory_search` API
- No new user-facing tools or CLI commands
- Rust-native implementation (no Python REPL)
- Training must be invisible to users
- 80% test coverage requirement

---

## Goals / Non-Goals

### Goals
- Enable agents to query memory contexts 10-100x larger than context windows
- Learn optimal decomposition strategies from usage patterns
- Reduce context rot for information-dense tasks
- Maintain identical UX - users see better results, not different tools

### Non-Goals
- Exposing decomposition as a user-facing feature
- Python REPL or agent-written code
- New MCP tools for memory navigation
- Real-time sub-millisecond latency (RLM trades latency for capability)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                   User/Agent Interface (UNCHANGED)               │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │     aeterna_memory_search / aeterna memory search        │    │
│  │     (Natural language queries - same as today)           │    │
│  └─────────────────────────────┬───────────────────────────┘    │
└────────────────────────────────┼────────────────────────────────┘
                                 │
                                 ▼
┌────────────────────────────────────────────────────────────────────┐
│                      ComplexityRouter (NEW)                        │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  Analyzes query → computes complexity score → routes        │  │
│  │                                                              │  │
│  │  Simple (score < 0.3)         Complex (score >= 0.3)        │  │
│  │         │                              │                     │  │
│  └─────────┼──────────────────────────────┼─────────────────────┘  │
└────────────┼──────────────────────────────┼────────────────────────┘
             │                              │
             ▼                              ▼
┌────────────────────────┐    ┌─────────────────────────────────────┐
│   Standard Search      │    │          RlmExecutor (NEW)          │
│   (Existing Path)      │    │  ┌─────────────────────────────┐   │
│                        │    │  │   DecompositionStrategy     │   │
│  • Vector similarity   │    │  │   • SearchLayer             │   │
│  • Top-k retrieval     │    │  │   • DrillDown               │   │
│  • Layer precedence    │    │  │   • Filter                  │   │
│                        │    │  │   • RecursiveCall           │   │
│                        │    │  │   • Aggregate               │   │
│                        │    │  └─────────────────────────────┘   │
│                        │    │                                     │
│                        │    │  ┌─────────────────────────────┐   │
│                        │    │  │   TrajectoryRecorder        │   │
│                        │    │  │   (for training)            │   │
│                        │    │  └─────────────────────────────┘   │
└────────────────────────┘    └─────────────────────────────────────┘
             │                              │
             └──────────────┬───────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────────────────┐
│                      Training Layer (Internal)                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  CombinedMemoryTrainer                       │   │
│  │  ┌─────────────────────┐  ┌─────────────────────────────┐   │   │
│  │  │  MemoryR1Trainer    │  │  DecompositionTrainer       │   │   │
│  │  │  (existing)         │  │  (NEW)                      │   │   │
│  │  │                     │  │                             │   │   │
│  │  │  Learns: which      │  │  Learns: how to find        │   │   │
│  │  │  memories valuable  │  │  memories efficiently       │   │   │
│  │  └─────────────────────┘  └─────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Component Detail: RLM Module Structure

```
memory/src/rlm/
├── mod.rs              ← Public exports
├── router.rs           ← ComplexityRouter, ComplexitySignals
│   └── should_route_to_rlm(query) → bool
│   └── compute_complexity(query) → f32
├── executor.rs         ← RlmExecutor, RlmTrajectory, TrajectoryStep
│   └── execute(query, tenant) → (Vec<SearchResult>, RlmTrajectory)
├── strategy.rs         ← DecompositionAction, AggregationStrategy, StrategyExecutor
│   └── ActionExecutor trait
├── trainer.rs          ← DecompositionTrainer, PolicyState, RewardConfig
│   └── train(trajectory) → updates policy
│   └── select_action(actions) → best action
├── combined_trainer.rs ← CombinedMemoryTrainer
│   └── Integrates MemoryR1 + DecompositionTrainer
│   └── save_policy_state() / load_policy_state()
└── bootstrap.rs        ← BootstrapTrainer, BootstrapTaskTemplate
    └── generate_bootstrap_tasks(templates, tenant, multiplier)
    └── bootstrap(tenant, iterations) → BootstrapResult

knowledge/src/context_architect/assembler.rs
└── ContextAssembler
    └── with_rlm_handler(handler) → Self
    └── assemble_with_rlm(query, sources, tenant) → AssembledContext
    └── should_use_rlm(query) → bool
    └── compute_query_complexity(query) → f32

mk_core/src/traits.rs
└── RlmAssemblyService trait
    └── should_use_rlm(query) → bool
    └── compute_complexity(query) → f32
    └── execute_assembly(query, tenant) → RlmAssemblyResult
```

---

## Decisions

### Decision 1: No Python REPL - Rust-Native Strategies

**Original proposal:** Python REPL with agent-written code
**Problem:** Exposes implementation, security risk, violates UX-first

**New approach:** Rust-native structured strategies

**Rationale:**
- Strategies are internal - no user code execution needed
- Rust provides type safety and performance
- No sandbox complexity
- Simpler to test and maintain

**Implementation:**
```rust
pub enum DecompositionAction {
    // Navigation
    SearchLayer { layer: MemoryLayer, query: String },
    DrillDown { from: MemoryLayer, to: MemoryLayer, filter: String },
    
    // Filtering
    FilterByRelevance { min_score: f32 },
    FilterByRecency { max_age: Duration },
    
    // Recursion
    RecursiveCall { sub_query: String },
    
    // Aggregation
    Aggregate { strategy: AggregationStrategy },
}

pub enum AggregationStrategy {
    Combine,      // Merge results
    Compare,      // Side-by-side comparison
    Summarize,    // Condense to summary
}
```

### Decision 2: Transparent Complexity Routing

**Original proposal:** New `memory_navigate` tool
**Problem:** Exposes decomposition as user-facing feature

**New approach:** Transparent routing in existing search path

**Rationale:**
- Users shouldn't choose between "simple search" and "complex search"
- System should automatically use best strategy
- Better results without cognitive load

**Implementation:**
```rust
pub struct ComplexityRouter {
    threshold: f32,  // default: 0.3
}

impl ComplexityRouter {
    pub fn compute_complexity(&self, query: &str) -> f32 {
        let mut score = 0.0;
        
        // Multi-layer signals
        if contains_layer_keywords(query) { score += 0.15; }
        if mentions_multiple_teams(query) { score += 0.15; }
        
        // Aggregation signals  
        if contains_aggregation_keywords(query) { score += 0.25; }
        // "compare", "across", "all teams", "summarize"
        
        // Comparison signals
        if contains_comparison_keywords(query) { score += 0.25; }
        // "vs", "difference between", "compare"
        
        // Query complexity
        score += (query.split_whitespace().count() as f32 / 50.0).min(0.2);
        
        score.min(1.0)
    }
    
    pub fn route(&self, query: &str) -> RouteDecision {
        if self.compute_complexity(query) >= self.threshold {
            RouteDecision::Rlm
        } else {
            RouteDecision::StandardSearch
        }
    }
}
```

#### Complexity Scoring Algorithm (Implemented)

The actual implementation in `memory/src/rlm/router.rs` uses:

| Signal Category | Weight | Detection Method |
|----------------|--------|------------------|
| Query length | 0.20 max | `(length / 200).min(1.0) * 0.2` |
| Keyword density | 0.40 max | Regex matches for: compare, difference, trends, evolution, history, summarize, aggregate, impact, relationship, sequence, analyze, trace |
| Multi-hop indicators | 0.20 max | Detects: then, after, followed by, caused, leading to |
| Temporal constraints | 0.10 | Detects: last week/month/quarter/year, yesterday, since, before, period, over time |
| Aggregate operators | 0.10 | Detects: all, every, total, average, count |

**Threshold**: Default 0.3 (configurable via `RlmConfig.complexity_threshold`)

**Example scores**:
- "what is the login endpoint" → ~0.12 (Standard Search)
- "compare patterns across teams" → ~0.55 (RLM)
- "trace evolution of auth since last quarter" → ~0.72 (RLM)

### Decision 3: Implicit Training from Search Outcomes

**Original proposal:** Explicit trajectory recording tool parameter
**Problem:** Exposes training as user concern

**New approach:** Implicit learning from search success/failure

**Rationale:**
- Training is system optimization, not user feature
- Success signal from downstream usage (was result useful?)
- No `enable_learning` parameter needed

**Training signals:**
- Positive: Result used in context, led to task success
- Negative: User refined query, result ignored
- Neutral: No signal available

```rust
impl DecompositionTrainer {
    pub async fn record_outcome(&self, 
        trajectory_id: &str,
        outcome: TrainingOutcome,
    ) {
        // Called internally when search outcome is known
        // User never sees this
    }
}

pub enum TrainingOutcome {
    ResultUsed { quality_score: f32 },
    ResultIgnored,
    QueryRefined { new_query: String },
    NoSignal,
}
```

#### Training Signal Sources (Implemented)

| Signal Source | Outcome Type | When Triggered |
|---------------|--------------|----------------|
| Context assembly success | `ResultUsed { quality_score }` | Search result included in assembled context |
| Memory feedback API | `ResultUsed { quality_score }` | User provides explicit feedback via `memory_feedback` tool |
| Query refinement | `QueryRefined { new_query }` | User re-queries with modified text within session |
| Session abandonment | `ResultIgnored` | Search result retrieved but never referenced |
| Bootstrap training | Pre-configured | Synthetic trajectories with expected outcomes |
| No interaction | `NoSignal` | Result returned but no downstream signal available |

**Reward Computation (from `RewardConfig`):**
```
reward = success_weight * success_score + efficiency_weight * efficiency_score
where:
  success_score = quality_score (ResultUsed) | 0.3 (QueryRefined) | -0.5 (ResultIgnored) | 0.0 (NoSignal)
  efficiency_score = 1.0 - min(tokens_used / 100_000, 1.0)
```

**Default weights:** `success_weight=1.0`, `efficiency_weight=0.3`

### Decision 4: Simplified Reward Function

**Original proposal:** Complex multi-component reward with 6+ parameters
**New approach:** Simplified reward focused on outcome

```rust
pub struct RewardConfig {
    pub success_weight: f32,      // 1.0 - did query succeed?
    pub efficiency_weight: f32,   // 0.3 - token cost penalty
}

impl RewardConfig {
    pub fn compute(&self, trajectory: &Trajectory) -> f32 {
        let success = match trajectory.outcome {
            TrainingOutcome::ResultUsed { quality_score } => quality_score,
            TrainingOutcome::QueryRefined { .. } => 0.3,  // Partial success
            TrainingOutcome::ResultIgnored => -0.5,
            TrainingOutcome::NoSignal => 0.0,
        };
        
        let efficiency = 1.0 - (trajectory.tokens_used as f32 / 100_000.0).min(1.0);
        
        (self.success_weight * success + self.efficiency_weight * efficiency)
            .clamp(-1.0, 1.0)
    }
}
```

### Decision 5: Sub-LM Configuration

Same as original - configurable per-depth with sensible defaults:

```rust
pub struct RlmConfig {
    pub root_model: String,           // e.g., "gpt-4o"
    pub sub_call_model: String,       // e.g., "gpt-4o-mini"
    pub max_recursion_depth: u8,      // default: 3
    pub sub_call_timeout: Duration,   // default: 30s
}
```

---

## Data Structures

### DecompositionTrajectory (Internal)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DecompositionTrajectory {
    pub id: String,
    pub query: String,
    pub tenant_context: TenantContext,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    
    // Action sequence
    pub actions: Vec<TimestampedAction>,
    
    // Outcome
    pub result_count: usize,
    pub outcome: Option<TrainingOutcome>,
    
    // Costs
    pub tokens_used: usize,
    pub max_depth: u8,
    
    // Computed reward (after outcome known)
    pub reward: Option<f32>,
}
```

### ComplexitySignals (Internal)

```rust
#[derive(Debug)]
pub(crate) struct ComplexitySignals {
    pub layer_count: usize,
    pub has_aggregation: bool,
    pub has_comparison: bool,
    pub query_length: usize,
    pub computed_score: f32,
}
```

---

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Complexity routing misclassifies queries | Medium | Conservative threshold, fallback to standard search on RLM failure |
| Training instability | Medium | Conservative learning rate, EMA baseline, weight clamping |
| Cold start (no training data) | Medium | Bootstrapping phase with synthetic tasks (internal) |
| Latency increase for complex queries | Low | Only complex queries use RLM; simple queries unchanged |
| Cost increase for sub-calls | Medium | Cost-aware reward, depth limits |

---

## Migration Plan

### Phase 1: Foundation (No Visible Changes)
1. Add `ComplexityRouter` with conservative threshold (routes everything to standard search)
2. Add `RlmExecutor` skeleton
3. Add trajectory recording (internal)
4. No changes to user-facing behavior

### Phase 2: Gradual Rollout
1. Enable RLM for queries with complexity > 0.5 (very complex only)
2. Collect trajectories, begin training
3. Monitor success rates
4. Lower threshold gradually

### Phase 3: Full Integration
1. Enable RLM for all complex queries (threshold 0.3)
2. Integrate with ContextAssembler for complex assembly
3. Continue background training

### Rollback
- Feature flag to disable RLM entirely
- Routing returns to 100% standard search instantly
- No user-visible API changes to roll back

---

## Open Questions

1. **Training Signal Source**: How do we know if a search result was "useful"? Options:
   - Explicit feedback via existing `memory_feedback` tool
   - Implicit from context assembly (was memory included?)
   - Implicit from task outcome (if available)

2. **Multi-tenant Training**: Should policy weights be per-tenant or global?
   - Per-tenant: Better personalization, slower learning
   - Global: Faster learning, may not fit all usage patterns
   - Hybrid: Global base + per-tenant adjustments

3. **Complexity Threshold Tuning**: How do we tune the routing threshold?
   - A/B testing with success metrics
   - Per-tenant configuration
   - Self-adjusting based on outcomes

---

## Appendix: Example Query Routing

### Example A: Simple Query → Standard Search

```
Query: "What database does the API team use?"

ComplexitySignals:
  layer_count: 1 (team)
  has_aggregation: false
  has_comparison: false
  query_length: 8
  computed_score: 0.12

Decision: StandardSearch
Result: Vector search in team layer, top-5 results
```

### Example B: Complex Query → RLM

```
Query: "Compare error handling patterns across all Platform Engineering teams"

ComplexitySignals:
  layer_count: 2+ (org -> teams)
  has_aggregation: true ("across all")
  has_comparison: true ("compare")
  query_length: 9
  computed_score: 0.72

Decision: RLM

Internal Decomposition:
1. DrillDown(org, "Platform Engineering") → [API, Data, Infra, DevEx]
2. For each team:
   SearchLayer(team, "error handling patterns") → [results...]
3. Aggregate(strategy=Compare) → comparison table

Result: Structured comparison (user sees this, not the decomposition)
```

### Example C: Borderline Query → Conservative Routing

```
Query: "Show me recent authentication decisions"

ComplexitySignals:
  layer_count: 1 (unclear)
  has_aggregation: false
  has_comparison: false
  query_length: 5
  computed_score: 0.18

Decision: StandardSearch (below threshold)
```
