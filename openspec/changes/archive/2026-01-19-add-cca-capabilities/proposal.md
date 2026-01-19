# Change: Confucius Code Agent (CCA) Capabilities

## Why

Current AI coding agents suffer from context limitations and lack persistent learning mechanisms. The Confucius Code Agent (CCA) research from Meta/Harvard (arxiv.org/html/2512.10398v5) demonstrates that hierarchical context management, trajectory-based note-taking, and hindsight learning significantly improve agent performance:

- **+7.6% improvement** from rich tool handling vs simple (ablation study on SWE-Bench Pro: 44.0% → 51.6%)
- **Hierarchical working memory** with adaptive compression prevents context overflow
- **Note-taking agent** distills trajectories into persistent, retrievable knowledge
- **Hindsight learning** captures error patterns and resolutions for future reference

Aeterna's existing memory and knowledge systems provide the foundation, but lack the intelligent compression, trajectory distillation, and self-improvement loops that CCA demonstrates.

## What Changes

### New Capabilities (5 new specs)

1. **Context Architect** - Hierarchical context compression at EVERY layer
   - Company → Org → Team → Project → Multi-Session → Session → Query
   - Pre-computed summaries at multiple depths (1-sentence, 1-paragraph, detailed)
   - Adaptive token budgeting based on query relevance
   - AX/UX/DX separation (agent sees compressed, user sees rich, dev gets observability)

2. **Note-Taking Agent** - Trajectory distillation into persistent Markdown notes
   - Captures successful patterns from tool execution sequences
   - Generates structured notes with context, actions, and outcomes
   - Stores in knowledge repository for cross-session retrieval

3. **Hindsight Learning** - Error capture and resolution patterns
   - Captures failure modes with error signatures
   - Records successful resolutions linked to error patterns
   - Enables "learn from mistakes" behavior across sessions

4. **Meta-Agent** - Build-test-improve self-refinement loop
   - Iterative improvement cycles for generated code
   - Test execution feedback integration
   - Quality gates before committing changes

5. **Extension System** - Typed callbacks with state management
   - `on_input_messages`, `on_plain_text`, `on_tag`, `on_llm_output` hooks
   - Per-extension state management and prompt wiring
   - Advanced context features for tool selection and sequencing

### Modified Capabilities (3 existing specs)

6. **Memory System** (MODIFIED)
   - Add hierarchical summary fields to memory entries
   - Add summary update triggers (time-based OR change-based)
   - Add summary depth configuration per layer

7. **Knowledge Repository** (MODIFIED)
   - Add summary storage as source of truth
   - Add summary caching for fast access
   - Add personalization configuration per layer

8. **Sync Bridge** (MODIFIED)
   - Add summary synchronization between memory and knowledge
   - Add incremental summary updates on content changes

## Impact

- **Affected specs**: `memory-system`, `knowledge-repository`, `sync-bridge`
- **New specs**: `context-architect`, `note-taking-agent`, `hindsight-learning`, `meta-agent`, `extension-system`
- **Affected code**: `memory/`, `knowledge/`, `sync/`, `tools/` (new CCA tools)
- **Integration**: Amends `add-opencode-plugin` for client-side CCA hooks
- **Dependencies**: LLM provider for summarization (via existing rust-genai)

## Key Design Decisions

### Hierarchical Context Compression (HCC)

Every layer maintains:
- Full content (source of truth)
- Pre-computed summaries at multiple depths
- Context vectors for semantic matching
- Relevance signals for adaptive selection

### Configuration Philosophy

| Layer | Personalization | Summary Update Trigger |
|-------|-----------------|----------------------|
| Company | Generic (org-wide) | Hourly OR every 10 changes |
| Org | Generic (org-wide) | Hourly OR every 10 changes |
| Team | Contextualized (user concerns) | Hourly OR every 5 changes |
| Project | Contextualized (user concerns) | Every 30 min OR every 3 changes |
| Session | Highly contextualized | Every 5 min OR every change |

All thresholds are configurable per deployment.

### Execution Location (Hybrid)

- **Client-side (OpenCode Plugin)**: Extension callbacks, context assembly
- **Server-side (Aeterna)**: LLM agents for summarization, note generation, hindsight analysis
