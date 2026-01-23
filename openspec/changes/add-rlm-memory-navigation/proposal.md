# Change: Intelligent Memory Retrieval (RLM Infrastructure)

## Why

Current memory retrieval approaches face fundamental limitations at enterprise scale:

1. **Context Window Limits**: Even with 272K token windows, agents cannot process millions of tokens of organizational memory
2. **Context Rot**: LLM quality degrades significantly as context grows, even within window limits
3. **Flat Search Limitations**: Vector search returns top-k results but cannot perform multi-hop reasoning across memory layers
4. **Information Density Scaling**: Tasks requiring pairwise comparisons scale quadratically - impossible with context stuffing

Research on Recursive Language Models (RLMs) demonstrates that treating prompts as external environment variables enables:
- Processing inputs 2 orders of magnitude beyond context windows
- 28-58% improvement on information-dense tasks vs base models
- Comparable or lower costs than context stuffing approaches

**However, this complexity must remain invisible to users and agents.**

## Design Principle: UX-First

Per Aeterna's UX-first architecture:

> "Every capability must be accessible through natural language. Implementation details (Cedar, TOML, layer enums, RLM internals) are **never exposed** to end users."

RLM is **infrastructure**, not a user-facing feature. Users and agents continue using:
- `aeterna_memory_search` (natural language)
- `aeterna memory search` (CLI)

The system **automatically** detects complex queries and routes to RLM internally.

## What Changes

### Internal Infrastructure (Not User-Visible)

1. **`RlmExecutor`** (`memory/src/rlm/executor.rs`)
   - Manages recursive decomposition with depth limits
   - Tracks token costs and execution state
   - Supports async/parallel sub-calls for efficiency
   - **Not exposed via MCP or CLI**

2. **`DecompositionStrategy`** (`memory/src/rlm/strategy.rs`)
   - Rust-native structured actions (no Python REPL)
   - SearchLayer, DrillDown, Filter, Aggregate, RecursiveCall
   - Selected automatically based on query analysis

3. **`DecompositionTrainer`** (`memory/src/rlm/trainer.rs`)
   - Learns optimal strategies from usage patterns
   - Policy gradient on action selection
   - Silent background training from search outcomes

4. **`ComplexityRouter`** (`memory/src/rlm/router.rs`)
   - Analyzes incoming queries for complexity signals
   - Routes simple queries to standard vector search
   - Routes complex queries to RLM executor
   - **Completely transparent to callers**

### Enhanced Existing Tools (User-Visible Interface Unchanged)

5. **`aeterna_memory_search`** enhancement
   - Same natural language interface
   - Same CLI: `aeterna memory search "query"`
   - Internally routes based on query complexity
   - Returns results - user never sees decomposition strategy

6. **`ContextAssembler`** enhancement
   - Uses RLM for complex multi-layer assembly
   - Falls back when standard assembly exceeds budget
   - **No new API surface**

### What We're NOT Doing

- ~~`memory_navigate` MCP tool~~ - Violates UX-first (exposes decomposition)
- ~~Python REPL~~ - Unnecessary complexity, security risk
- ~~Agent-written traversal code~~ - Shifts complexity to wrong layer
- ~~Exposed action types~~ - Implementation detail

## User Experience

### Before (Current)
```
User: "Compare testing frameworks across Platform Engineering teams"
Agent: Uses memory_search, gets top-k results from all layers
       Results are noisy, may miss relevant team-specific memories
```

### After (With RLM Infrastructure)
```
User: "Compare testing frameworks across Platform Engineering teams"
Agent: Uses aeterna_memory_search (same as before)
       
System (internal, invisible to user):
  1. ComplexityRouter detects: multi-hop, cross-team, comparison -> complex
  2. Routes to RlmExecutor
  3. RlmExecutor decomposes:
     - DrillDown(org -> teams, filter="Platform Engineering")
     - For each team: SearchLayer(team, "testing framework")
     - Aggregate(strategy=compare)
  4. Returns unified results
       
User sees: Clean comparison table, not the decomposition
```

## Impact

- **Affected specs**: `memory-system` (internal enhancement, no API change)
- **Affected code**: `memory/src/rlm/` (new internal module)
- **New dependencies**: None (Rust-native, no Python)
- **User-facing changes**: None - existing tools work better automatically
- **Performance**: Adds latency for complex queries only; simple queries unchanged
- **Cost**: Automatic cost optimization via learned decomposition

## Success Criteria

1. `aeterna_memory_search` handles complex queries without user awareness of RLM
2. Simple queries route to fast vector search (no RLM overhead)
3. Complex queries (cross-layer, comparison, aggregation) produce better results
4. No new MCP tools or CLI commands exposed
5. Training happens silently based on search success/failure signals
6. 80% test coverage on RLM infrastructure

## References

- RLM Paper: "Recursive Language Models" (2025) - Inference-time scaling
- UX-First Proposal: `openspec/changes/add-ux-first-governance/proposal.md`
- Memory-R1 Trainer: `memory/src/trainer.rs`
