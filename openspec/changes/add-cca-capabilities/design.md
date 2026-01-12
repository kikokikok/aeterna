# Design: Confucius Code Agent (CCA) Capabilities

## Context

The Confucius Code Agent (CCA) from Meta/Harvard research demonstrates state-of-the-art performance on software engineering benchmarks through:

1. **Hierarchical Working Memory**: Architect agent that manages context compression
2. **Note-Taking Agent**: Distills trajectories into persistent Markdown notes
3. **Hindsight Learning**: Captures error patterns and successful resolutions
4. **Meta-Agent Loop**: Build-test-improve cycles for self-refinement
5. **Extension System**: Rich tool handling with typed callbacks

Aeterna provides the foundation (8-layer memory, Git-based knowledge, sync bridge) but lacks these intelligent orchestration layers.

## Goals / Non-Goals

### Goals
- Implement hierarchical context compression at every layer (Company → Query)
- Add trajectory distillation for cross-session learning
- Capture error patterns with resolution metadata
- Enable self-improvement loops for generated code
- Provide extension callbacks for advanced tool handling
- Integrate with existing OpenCode plugin architecture
- Maintain backward compatibility with existing Aeterna APIs

### Non-Goals
- Replacing OpenCode's native features
- Building a standalone agent (Aeterna provides infrastructure, not the agent itself)
- Real-time streaming (batch summarization is acceptable)
- Multi-modal content (text-only for v1)

## Decisions

### 1. Hierarchical Context Compression (HCC) Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    HIERARCHICAL CONTEXT COMPRESSION                      │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Company Layer                                                       │ │
│  │ ├─ Full Content: "Security-first approach, all data encrypted..."  │ │
│  │ ├─ Summary (1-sentence): "Security-first, PostgreSQL standard"     │ │
│  │ ├─ Summary (1-para): "Security-first organization with..."        │ │
│  │ ├─ Summary (detailed): "Comprehensive security policy..."          │ │
│  │ └─ Context Vector: [0.23, 0.87, ...] (for relevance matching)     │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Org Layer (inherits + extends)                                      │ │
│  │ ├─ Full Content: "Microservices architecture, gRPC preferred..."   │ │
│  │ ├─ Summaries at 3 depths...                                        │ │
│  │ └─ Context Vector: [0.45, 0.12, ...]                               │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Team Layer                                                          │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Project Layer                                                       │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Multi-Session Layer (user's history across sessions)               │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Session Layer (current conversation)                                │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │ Query Assembly                                                      │ │
│  │ ├─ Token Budget: 8000 tokens                                       │ │
│  │ ├─ Relevance Scores: Company(0.3), Project(0.9), Session(0.95)    │ │
│  │ └─ Assembled Context: Adaptive blend based on relevance + budget   │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Decision**: Each layer maintains full content + 3 summary depths + context vector.
**Rationale**: Pre-computed summaries eliminate runtime LLM calls for context assembly. Multiple depths allow adaptive selection based on token budget.

### 2. Summary Schema Extension

```rust
/// Summary at a specific depth for a layer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerSummary {
    /// Depth: "sentence", "paragraph", "detailed"
    pub depth: SummaryDepth,
    
    /// The summary content
    pub content: String,
    
    /// Token count for budget calculation
    pub token_count: u32,
    
    /// When this summary was generated
    pub generated_at: DateTime<Utc>,
    
    /// Hash of source content at generation time
    pub source_hash: String,
    
    /// Whether personalized for user context
    pub personalized: bool,
    
    /// Personalization context (if personalized)
    pub personalization_context: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SummaryDepth {
    Sentence,   // ~50 tokens
    Paragraph,  // ~200 tokens
    Detailed,   // ~500 tokens
}

/// Configuration for summary generation per layer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SummaryConfig {
    /// Layer this config applies to
    pub layer: MemoryLayer,
    
    /// Update trigger: time-based
    pub update_interval: Option<Duration>,
    
    /// Update trigger: change-based
    pub update_on_changes: Option<u32>,
    
    /// Skip update if no changes since last summary
    pub skip_if_unchanged: bool,
    
    /// Whether to personalize summaries
    pub personalized: bool,
    
    /// Depths to generate
    pub depths: Vec<SummaryDepth>,
}
```

### 3. Context Architect Agent

The Context Architect is a server-side LLM agent that:

1. **Monitors** layer content for changes
2. **Generates** summaries when triggers fire
3. **Assembles** context for queries based on relevance and budget

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CONTEXT ARCHITECT                                │
│                                                                          │
│  ┌─────────────────────┐    ┌─────────────────────┐                     │
│  │  Summary Generator  │    │  Context Assembler  │                     │
│  │                     │    │                     │                     │
│  │  • Watches triggers │    │  • Receives query   │                     │
│  │  • Calls LLM        │    │  • Computes scores  │                     │
│  │  • Stores summaries │    │  • Selects depths   │                     │
│  └──────────┬──────────┘    │  • Assembles final  │                     │
│             │               └──────────┬──────────┘                     │
│             │                          │                                 │
│             ▼                          ▼                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Summary Storage (Redis + PostgreSQL)          │   │
│  │  • Redis: Hot cache for fast retrieval                          │   │
│  │  • PostgreSQL: Source of truth for summaries                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Decision**: Hybrid storage (Redis cache + PostgreSQL truth).
**Rationale**: Fast reads for context assembly (Redis), durable storage for audit trail (PostgreSQL).

### 4. Note-Taking Agent Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         NOTE-TAKING AGENT                                │
│                                                                          │
│  Input: Tool Execution Trajectory                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  1. User asks: "Fix the authentication bug"                      │   │
│  │  2. Agent searches codebase → finds auth.rs                      │   │
│  │  3. Agent reads file → identifies issue                          │   │
│  │  4. Agent edits file → adds null check                           │   │
│  │  5. Agent runs tests → all pass                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
│                                    ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  Distillation (LLM-powered)                                      │   │
│  │                                                                  │   │
│  │  • Extract: What was the problem?                                │   │
│  │  • Extract: What solution worked?                                │   │
│  │  • Extract: What patterns emerged?                               │   │
│  │  • Generate: Structured Markdown note                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                    │                                     │
│                                    ▼                                     │
│  Output: Structured Note (stored in Knowledge Repository)                │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  # Authentication Null Check Pattern                             │   │
│  │                                                                  │   │
│  │  ## Context                                                      │   │
│  │  When handling JWT tokens in auth.rs, null checks are required   │   │
│  │  before accessing token fields.                                  │   │
│  │                                                                  │   │
│  │  ## Solution                                                     │   │
│  │  ```rust                                                         │   │
│  │  if let Some(token) = token_option {                            │   │
│  │      // proceed with token                                       │   │
│  │  }                                                               │   │
│  │  ```                                                             │   │
│  │                                                                  │   │
│  │  ## Tags                                                         │   │
│  │  authentication, rust, null-safety                               │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Decision**: Notes stored as knowledge items (type: `pattern`) in knowledge repository.
**Rationale**: Leverages existing knowledge infrastructure, Git versioning, and sync bridge.

### 5. Hindsight Learning Schema

```rust
/// Error signature for pattern matching
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorSignature {
    /// Error type/category
    pub error_type: String,
    
    /// Error message pattern (regex-capable)
    pub message_pattern: String,
    
    /// Stack trace patterns
    pub stack_patterns: Vec<String>,
    
    /// Context patterns (file types, frameworks)
    pub context_patterns: Vec<String>,
    
    /// Embedding for semantic matching
    pub embedding: Vec<f32>,
}

/// Successful resolution linked to error
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resolution {
    /// Unique ID
    pub id: String,
    
    /// Error signature this resolves
    pub error_signature_id: String,
    
    /// Description of the fix
    pub description: String,
    
    /// Code diff or changes made
    pub changes: Vec<CodeChange>,
    
    /// Success rate (0.0 - 1.0)
    pub success_rate: f32,
    
    /// Number of times applied
    pub application_count: u32,
    
    /// Last successful application
    pub last_success_at: DateTime<Utc>,
}

/// Hindsight note stored in knowledge repository
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HindsightNote {
    /// Knowledge item type = "hindsight"
    pub knowledge_type: String,
    
    /// Error signature
    pub error_signature: ErrorSignature,
    
    /// Successful resolutions
    pub resolutions: Vec<Resolution>,
    
    /// Generated Markdown content
    pub content: String,
}
```

**Decision**: Hindsight notes are a new knowledge type alongside ADR, policy, pattern, spec.
**Rationale**: Keeps error patterns queryable through existing knowledge APIs.

### 6. Meta-Agent Loop

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           META-AGENT LOOP                                │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                         BUILD PHASE                              │   │
│  │  • Generate code based on requirements                           │   │
│  │  • Apply patterns from note-taking agent                         │   │
│  │  • Use hindsight notes to avoid known pitfalls                   │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
│                                 │                                        │
│                                 ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                         TEST PHASE                               │   │
│  │  • Run test suite                                                │   │
│  │  • Capture test output                                           │   │
│  │  • Analyze failures                                              │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
│                                 │                                        │
│                    ┌────────────┴────────────┐                          │
│                    │                         │                          │
│               Tests Pass              Tests Fail                        │
│                    │                         │                          │
│                    ▼                         ▼                          │
│  ┌─────────────────────────┐   ┌─────────────────────────────────┐    │
│  │       COMMIT            │   │        IMPROVE PHASE            │    │
│  │  • Quality gate passed  │   │  • Analyze failure              │    │
│  │  • Generate commit msg  │   │  • Query hindsight notes        │    │
│  │  • Create PR if needed  │   │  • Apply resolution             │    │
│  └─────────────────────────┘   │  • Loop back to TEST            │    │
│                                 │  • Max iterations: 3            │    │
│                                 └─────────────────────────────────┘    │
│                                                                          │
│  After 3 failed iterations:                                             │
│  • Store failure in hindsight learning                                  │
│  • Report to user with analysis                                         │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Decision**: Max 3 improvement iterations before escalating to user.
**Rationale**: Prevents infinite loops while allowing self-correction.

### 7. Extension System Callbacks

```typescript
/// Extension callback types (inspired by CCA paper)
interface ExtensionCallbacks {
  /// Called when input messages are received
  on_input_messages: (messages: Message[]) => Promise<Message[]>;
  
  /// Called for plain text processing
  on_plain_text: (text: string, context: ExtensionContext) => Promise<string>;
  
  /// Called when specific XML tags are encountered
  on_tag: (tag: string, content: string, context: ExtensionContext) => Promise<string>;
  
  /// Called after LLM generates output
  on_llm_output: (output: string, context: ExtensionContext) => Promise<string>;
}

/// Extension context with state management
interface ExtensionContext {
  /// Unique extension ID
  extension_id: string;
  
  /// Extension-specific state (persisted across calls)
  state: Map<string, unknown>;
  
  /// Session context
  session: SessionContext;
  
  /// Available tools
  tools: ToolRegistry;
  
  /// State management methods
  get_state<T>(key: string): T | undefined;
  set_state<T>(key: string, value: T): void;
  clear_state(): void;
}

/// Extension registration
interface ExtensionRegistration {
  /// Extension identifier
  id: string;
  
  /// Callbacks to register
  callbacks: Partial<ExtensionCallbacks>;
  
  /// Prompt additions for this extension
  prompt_additions: PromptAddition[];
  
  /// Tool overrides/additions
  tool_config: ToolConfig;
}
```

**Decision**: Extension state persisted in Redis with session TTL.
**Rationale**: Fast state access during conversation, automatic cleanup on session end.

### 8. Hybrid Execution Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      HYBRID EXECUTION MODEL                              │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    CLIENT (OpenCode Plugin)                      │   │
│  │                                                                  │   │
│  │  • Extension callback registration                               │   │
│  │  • Context assembly (using pre-computed summaries)               │   │
│  │  • Tool execution capture                                        │   │
│  │  • Session state management                                      │   │
│  │  • Real-time hooks (chat.message, tool.execute)                 │   │
│  └──────────────────────────────┬──────────────────────────────────┘   │
│                                 │                                        │
│                                 │ gRPC/HTTP                             │
│                                 │                                        │
│                                 ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    SERVER (Aeterna Backend)                      │   │
│  │                                                                  │   │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │   │
│  │  │ Context         │  │ Note-Taking     │  │ Hindsight       │ │   │
│  │  │ Architect       │  │ Agent           │  │ Learning        │ │   │
│  │  │                 │  │                 │  │                 │ │   │
│  │  │ • Summarization │  │ • Distillation  │  │ • Error capture │ │   │
│  │  │ • LLM calls     │  │ • Note gen      │  │ • Resolution    │ │   │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────┘ │   │
│  │                                                                  │   │
│  │  ┌─────────────────┐  ┌─────────────────────────────────────┐   │   │
│  │  │ Meta-Agent      │  │ Storage Layer                       │   │   │
│  │  │                 │  │                                     │   │   │
│  │  │ • Build-test    │  │ • PostgreSQL (knowledge, hindsight)│   │   │
│  │  │ • Improve loop  │  │ • Redis (summaries, state)         │   │   │
│  │  │ • Quality gates │  │ • Qdrant (embeddings)              │   │   │
│  │  └─────────────────┘  └─────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Decision**: LLM-intensive operations (summarization, distillation, analysis) run server-side.
**Rationale**: Centralizes LLM costs, enables caching, simplifies client.

### 9. AX/UX/DX Separation

From CCA paper: Agent Experience, User Experience, Developer Experience should be separate.

| Aspect | Agent (AX) | User (UX) | Developer (DX) |
|--------|------------|-----------|----------------|
| **Context View** | Compressed summaries | Full trajectory logs | Debug traces |
| **Notes** | Query results only | Rendered Markdown | Raw JSON + metadata |
| **Errors** | Resolution suggestions | Friendly messages | Stack traces + metrics |
| **State** | Relevant state slice | Activity indicator | Full state dump |

**Decision**: Three separate view layers with configurable verbosity.
**Rationale**: Each stakeholder needs different information density.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| LLM costs for summarization | Batch processing, caching, configurable triggers |
| Summary staleness | Hash-based invalidation, trigger thresholds |
| Storage overhead | Compression, TTL policies, selective summarization |
| Latency in context assembly | Pre-computed summaries, Redis caching |
| Extension callback complexity | Clear documentation, typed interfaces |

## Migration Plan

1. **Phase 1**: Summary schema extensions to memory/knowledge (backward compatible)
2. **Phase 2**: Context Architect implementation (server-side only)
3. **Phase 3**: Note-Taking Agent with trajectory capture
4. **Phase 4**: Hindsight Learning with error patterns
5. **Phase 5**: Meta-Agent loop integration
6. **Phase 6**: Extension system for OpenCode plugin
7. **Phase 7**: Integration with `add-opencode-plugin` change

## Open Questions

- [x] Summary update frequency: Configurable per layer (hourly OR every X changes)
- [x] Summary storage: Hybrid (Redis cache + PostgreSQL truth)
- [x] Personalization: Session/Project/Team personalized; Company/Org generic; ALL configurable
- [x] Summary depths: Multiple (sentence, paragraph, detailed) + adaptive selection
- [x] LLM model for summarization: Configurable separately - OpenCode config for client-side operations, Aeterna server config for server-side agents
- [x] Hindsight note promotion: Auto-promote after N successful applications, respecting governance rules and requiring acceptance per layer (e.g., project→team promotion requires team-level approval)
