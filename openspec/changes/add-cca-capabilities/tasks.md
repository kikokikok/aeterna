# Tasks: Confucius Code Agent (CCA) Capabilities

Implementation checklist organized by migration phases from design.md.

---

## Phase 1: Summary Schema Extensions (Backward Compatible)

### 1.1 Core Types
- [ ] 1.1.1 Add `SummaryDepth` enum to `mk_core/src/types.rs` (Sentence ~50 tokens, Paragraph ~200 tokens, Detailed ~500 tokens)
- [ ] 1.1.2 Add `LayerSummary` struct to `mk_core/src/types.rs` (depth, content, token_count, generated_at, source_hash, personalized, personalization_context)
- [ ] 1.1.3 Add `SummaryConfig` struct to `mk_core/src/types.rs` (layer, update_interval, update_on_changes, skip_if_unchanged, personalized, depths)
- [ ] 1.1.4 Add `ContextVector` type alias (Vec<f32>) for semantic matching
- [ ] 1.1.5 Write unit tests for new types (serialization, default values)

### 1.2 Memory System Extensions
- [ ] 1.2.1 Add `summaries: HashMap<SummaryDepth, LayerSummary>` field to memory entry structs in `memory/src/lib.rs`
- [ ] 1.2.2 Add `context_vector: Option<ContextVector>` field to memory entries
- [ ] 1.2.3 Add `summary_config: Option<SummaryConfig>` to memory layer configuration
- [ ] 1.2.4 Implement `needs_summary_update()` method based on triggers (time OR change count)
- [ ] 1.2.5 Add PostgreSQL migration for summary columns in memory tables
- [ ] 1.2.6 Add Redis schema for summary caching (key pattern: `summary:{tenant}:{layer}:{entry_id}:{depth}`)
- [ ] 1.2.7 Write integration tests for memory summary storage/retrieval

### 1.3 Knowledge Repository Extensions
- [ ] 1.3.1 Add `HindsightNote` knowledge type to `knowledge/src/types.rs`
- [ ] 1.3.2 Add `ErrorSignature` struct (error_type, message_pattern, stack_patterns, context_patterns, embedding)
- [ ] 1.3.3 Add `Resolution` struct (id, error_signature_id, description, changes, success_rate, application_count, last_success_at)
- [ ] 1.3.4 Add summary storage fields to knowledge item struct
- [ ] 1.3.5 Implement `KnowledgeType::Hindsight` variant in knowledge type enum
- [ ] 1.3.6 Add PostgreSQL migration for hindsight tables (error_signatures, resolutions, hindsight_notes)
- [ ] 1.3.7 Write integration tests for hindsight note CRUD operations

### 1.4 Sync Bridge Extensions
- [ ] 1.4.1 Add summary sync events to `sync/src/events.rs` (SummarySyncEvent, SummaryInvalidated, SummaryUpdated)
- [ ] 1.4.2 Implement summary pointer tracking in `sync/src/pointer.rs`
- [ ] 1.4.3 Add hindsight pointer type for knowledge→memory error pattern references
- [ ] 1.4.4 Implement incremental summary sync (only changed layers)
- [ ] 1.4.5 Add summary invalidation logic when source content changes
- [ ] 1.4.6 Write tests for summary sync scenarios (create, update, invalidate)

---

## Phase 2: Context Architect Implementation

### 2.1 Summary Generator
- [ ] 2.1.1 Create `knowledge/src/context_architect/mod.rs` module structure
- [ ] 2.1.2 Implement `SummaryGenerator` struct with LLM client dependency
- [ ] 2.1.3 Add `generate_summary(content: &str, depth: SummaryDepth, context: Option<&str>) -> LayerSummary` method
- [ ] 2.1.4 Implement configurable LLM provider (via existing rust-genai integration)
- [ ] 2.1.5 Add prompt templates for each summary depth (1-sentence, 1-paragraph, detailed)
- [ ] 2.1.6 Implement batch summarization for efficiency (process multiple entries in one LLM call)
- [ ] 2.1.7 Add token counting using tiktoken-rs or similar
- [ ] 2.1.8 Write unit tests for summary generation (mock LLM responses)

### 2.2 Trigger System
- [ ] 2.2.1 Create `SummaryTriggerMonitor` struct in `knowledge/src/context_architect/triggers.rs`
- [ ] 2.2.2 Implement time-based trigger (check interval configurable per layer)
- [ ] 2.2.3 Implement change-count trigger (track modifications since last summary)
- [ ] 2.2.4 Implement hash-based invalidation (detect content changes via source_hash)
- [ ] 2.2.5 Add `should_update_summary(entry: &MemoryEntry, config: &SummaryConfig) -> bool` method
- [ ] 2.2.6 Integrate with existing event system for change notifications
- [ ] 2.2.7 Write tests for trigger edge cases (boundary conditions, concurrent updates)

### 2.3 Context Assembler
- [ ] 2.3.1 Create `ContextAssembler` struct in `knowledge/src/context_architect/assembler.rs`
- [ ] 2.3.2 Implement `assemble_context(query: &str, token_budget: u32) -> AssembledContext` method
- [ ] 2.3.3 Add relevance scoring using context vectors (cosine similarity)
- [ ] 2.3.4 Implement adaptive depth selection based on relevance score and token budget
- [ ] 2.3.5 Add layer priority configuration (session > project > team > org > company)
- [ ] 2.3.6 Implement token budget distribution algorithm (allocate tokens proportionally to relevance)
- [ ] 2.3.7 Add context assembly caching (cache assembled context for repeated queries)
- [ ] 2.3.8 Write integration tests for context assembly scenarios

### 2.4 Hierarchical Compression
- [ ] 2.4.1 Implement `HierarchicalCompressor` in `knowledge/src/context_architect/compressor.rs`
- [ ] 2.4.2 Add layer inheritance logic (child layers inherit compressed parent context)
- [ ] 2.4.3 Implement progressive compression (use shorter summaries as budget decreases)
- [ ] 2.4.4 Add fallback to full content when summaries unavailable
- [ ] 2.4.5 Implement AX/UX/DX separation (three view modes with different verbosity)
- [ ] 2.4.6 Write tests for compression edge cases (empty layers, missing summaries)

---

## Phase 3: Note-Taking Agent

### 3.1 Trajectory Capture
- [ ] 3.1.1 Create `knowledge/src/note_taking/mod.rs` module structure
- [ ] 3.1.2 Define `TrajectoryEvent` struct (timestamp, tool_name, input, output, success, duration)
- [ ] 3.1.3 Implement `TrajectoryCapture` struct with event buffer
- [ ] 3.1.4 Add `capture_tool_execution(event: TrajectoryEvent)` method
- [ ] 3.1.5 Implement trajectory serialization for LLM consumption
- [ ] 3.1.6 Add trajectory filtering (exclude sensitive data, truncate large outputs)
- [ ] 3.1.7 Write unit tests for trajectory capture (event ordering, buffer overflow)

### 3.2 Distillation Engine
- [ ] 3.2.1 Create `Distiller` struct in `knowledge/src/note_taking/distiller.rs`
- [ ] 3.2.2 Implement `distill(trajectory: &[TrajectoryEvent]) -> DistillationResult` method
- [ ] 3.2.3 Add LLM prompt for extracting problem, solution, and patterns
- [ ] 3.2.4 Implement structured output parsing (extract Context, Solution, Tags sections)
- [ ] 3.2.5 Add quality scoring for generated notes (completeness, specificity)
- [ ] 3.2.6 Implement distillation triggers (on session end, on significant success, manual)
- [ ] 3.2.7 Write tests with sample trajectories and expected notes

### 3.3 Note Generation
- [ ] 3.3.1 Create `NoteGenerator` in `knowledge/src/note_taking/generator.rs`
- [ ] 3.3.2 Implement Markdown note template generation
- [ ] 3.3.3 Add automatic tag extraction from distillation result
- [ ] 3.3.4 Implement code snippet extraction and formatting
- [ ] 3.3.5 Add note deduplication (detect similar existing notes)
- [ ] 3.3.6 Implement note storage via knowledge repository API
- [ ] 3.3.7 Write integration tests for end-to-end note generation

### 3.4 Note Retrieval
- [ ] 3.4.1 Add semantic search for notes via existing Qdrant integration
- [ ] 3.4.2 Implement `retrieve_relevant_notes(query: &str, limit: usize) -> Vec<Note>` method
- [ ] 3.4.3 Add filtering by tags, recency, and success rate
- [ ] 3.4.4 Implement note ranking (relevance + recency + success rate)
- [ ] 3.4.5 Write tests for note retrieval scenarios

---

## Phase 4: Hindsight Learning

### 4.1 Error Capture
- [ ] 4.1.1 Create `knowledge/src/hindsight/mod.rs` module structure
- [ ] 4.1.2 Implement `ErrorCapture` struct for collecting error events
- [ ] 4.1.3 Add `capture_error(error: &Error, context: &ErrorContext)` method
- [ ] 4.1.4 Implement error signature generation (normalize error messages, extract patterns)
- [ ] 4.1.5 Add error embedding generation for semantic matching
- [ ] 4.1.6 Implement error deduplication (group similar errors)
- [ ] 4.1.7 Write unit tests for error capture and signature generation

### 4.2 Resolution Tracking
- [ ] 4.2.1 Create `ResolutionTracker` in `knowledge/src/hindsight/resolution.rs`
- [ ] 4.2.2 Implement `record_resolution(error_id: &str, resolution: Resolution)` method
- [ ] 4.2.3 Add resolution success/failure tracking
- [ ] 4.2.4 Implement success rate calculation (rolling window)
- [ ] 4.2.5 Add code change extraction for resolutions
- [ ] 4.2.6 Write tests for resolution tracking scenarios

### 4.3 Hindsight Note Generation
- [ ] 4.3.1 Create `HindsightNoteGenerator` in `knowledge/src/hindsight/note_gen.rs`
- [ ] 4.3.2 Implement LLM-based hindsight note generation from error + resolution pairs
- [ ] 4.3.3 Add Markdown formatting for hindsight notes
- [ ] 4.3.4 Implement hindsight note storage in knowledge repository
- [ ] 4.3.5 Add automatic tagging based on error type and context
- [ ] 4.3.6 Write integration tests for hindsight note generation

### 4.4 Hindsight Retrieval
- [ ] 4.4.1 Implement `query_hindsight(error: &Error) -> Vec<HindsightNote>` method
- [ ] 4.4.2 Add semantic matching using error embeddings
- [ ] 4.4.3 Implement pattern matching using error signatures
- [ ] 4.4.4 Add ranking by success rate and recency
- [ ] 4.4.5 Write tests for hindsight retrieval scenarios

### 4.5 Hindsight Promotion
- [ ] 4.5.1 Implement `HindsightPromoter` for layer promotion logic
- [ ] 4.5.2 Add configurable promotion thresholds (N successful applications)
- [ ] 4.5.3 Implement governance-aware promotion (check layer approval rules)
- [ ] 4.5.4 Add promotion request workflow (pending → approved → promoted)
- [ ] 4.5.5 Write tests for promotion scenarios (threshold met, governance rejection)

---

## Phase 5: Meta-Agent Loop

### 5.1 Build Phase
- [ ] 5.1.1 Create `knowledge/src/meta_agent/mod.rs` module structure
- [ ] 5.1.2 Implement `BuildPhase` struct with code generation context
- [ ] 5.1.3 Add `execute_build(requirements: &str, context: &Context) -> BuildResult` method
- [ ] 5.1.4 Integrate note retrieval for relevant patterns
- [ ] 5.1.5 Integrate hindsight retrieval for known pitfalls
- [ ] 5.1.6 Write tests for build phase (mock LLM code generation)

### 5.2 Test Phase
- [ ] 5.2.1 Create `TestPhase` struct in `knowledge/src/meta_agent/test.rs`
- [ ] 5.2.2 Implement test execution via subprocess (cargo test, pytest, etc.)
- [ ] 5.2.3 Add test output parsing (extract failures, stack traces)
- [ ] 5.2.4 Implement `execute_tests(build_result: &BuildResult) -> TestResult` method
- [ ] 5.2.5 Add test result classification (pass, fail, error, timeout)
- [ ] 5.2.6 Write tests for test phase (mock subprocess execution)

### 5.3 Improve Phase
- [ ] 5.3.1 Create `ImprovePhase` struct in `knowledge/src/meta_agent/improve.rs`
- [ ] 5.3.2 Implement failure analysis using LLM
- [ ] 5.3.3 Add `improve(test_result: &TestResult, context: &Context) -> ImproveResult` method
- [ ] 5.3.4 Integrate hindsight query for known resolutions
- [ ] 5.3.5 Implement fix suggestion generation
- [ ] 5.3.6 Write tests for improve phase (mock failure scenarios)

### 5.4 Loop Orchestration
- [ ] 5.4.1 Create `MetaAgentLoop` struct in `knowledge/src/meta_agent/loop.rs`
- [ ] 5.4.2 Implement iteration loop (build → test → improve → repeat)
- [ ] 5.4.3 Add max iteration limit (default: 3)
- [ ] 5.4.4 Implement quality gates (configurable pass criteria)
- [ ] 5.4.5 Add loop state persistence for resumption
- [ ] 5.4.6 Implement escalation to user after max iterations
- [ ] 5.4.7 Write integration tests for full loop scenarios

### 5.5 Result Handling
- [ ] 5.5.1 Implement success handling (commit message generation, PR creation hints)
- [ ] 5.5.2 Implement failure handling (store in hindsight, detailed report)
- [ ] 5.5.3 Add telemetry for loop performance (iterations, success rate)
- [ ] 5.5.4 Write tests for result handling scenarios

---

## Phase 6: Extension System

### 6.1 Callback Infrastructure
- [ ] 6.1.1 Create `tools/src/extensions/mod.rs` module structure
- [ ] 6.1.2 Define `ExtensionCallback` trait with async methods
- [ ] 6.1.3 Implement `on_input_messages` callback type
- [ ] 6.1.4 Implement `on_plain_text` callback type
- [ ] 6.1.5 Implement `on_tag` callback type
- [ ] 6.1.6 Implement `on_llm_output` callback type
- [ ] 6.1.7 Add callback error handling and timeout logic
- [ ] 6.1.8 Write unit tests for callback invocation

### 6.2 Extension Context
- [ ] 6.2.1 Create `ExtensionContext` struct in `tools/src/extensions/context.rs`
- [ ] 6.2.2 Implement state management (get_state, set_state, clear_state)
- [ ] 6.2.3 Add session context integration
- [ ] 6.2.4 Add tool registry access
- [ ] 6.2.5 Implement context serialization for Redis persistence
- [ ] 6.2.6 Write tests for context state management

### 6.3 Extension Registration
- [ ] 6.3.1 Create `ExtensionRegistry` in `tools/src/extensions/registry.rs`
- [ ] 6.3.2 Implement `register_extension(registration: ExtensionRegistration)` method
- [ ] 6.3.3 Add extension validation (check for conflicts, valid callbacks)
- [ ] 6.3.4 Implement extension priority ordering
- [ ] 6.3.5 Add extension enable/disable functionality
- [ ] 6.3.6 Write tests for registration scenarios

### 6.4 Prompt Wiring
- [ ] 6.4.1 Create `PromptWiring` struct in `tools/src/extensions/prompt.rs`
- [ ] 6.4.2 Implement prompt addition injection points
- [ ] 6.4.3 Add tool override configuration
- [ ] 6.4.4 Implement tool sequencing hints (suggest next tool)
- [ ] 6.4.5 Add advanced context features (tool selection hints)
- [ ] 6.4.6 Write tests for prompt wiring scenarios

### 6.5 State Persistence
- [ ] 6.5.1 Implement Redis-based state persistence
- [ ] 6.5.2 Add session TTL for automatic state cleanup
- [ ] 6.5.3 Implement state migration for extension updates
- [ ] 6.5.4 Add state size limits and compression
- [ ] 6.5.5 Write integration tests for state persistence

---

## Phase 7: OpenCode Plugin Integration

### 7.1 Tool Updates
- [ ] 7.1.1 Add `context_assemble` tool to `tools/src/tools/context.rs`
- [ ] 7.1.2 Add `note_capture` tool for manual trajectory capture trigger
- [ ] 7.1.3 Add `hindsight_query` tool for error pattern lookup
- [ ] 7.1.4 Add `meta_loop_status` tool for loop progress reporting
- [ ] 7.1.5 Update existing tools to emit trajectory events
- [ ] 7.1.6 Write tool tests with mock dependencies

### 7.2 Hook Integration (amends add-opencode-plugin)
- [ ] 7.2.1 Document required amendments to `add-opencode-plugin` change
- [ ] 7.2.2 Add `chat.context_assembled` hook for context injection
- [ ] 7.2.3 Add `tool.trajectory_captured` event emission
- [ ] 7.2.4 Add `session.ended` hook for note distillation trigger
- [ ] 7.2.5 Add `error.captured` event for hindsight capture
- [ ] 7.2.6 Document hook→CCA agent mapping for plugin implementers

### 7.3 Configuration
- [ ] 7.3.1 Add CCA configuration section to `config/src/cca.rs`
- [ ] 7.3.2 Add per-layer summary configuration (update triggers, depths)
- [ ] 7.3.3 Add hindsight promotion thresholds
- [ ] 7.3.4 Add meta-agent loop limits and timeouts
- [ ] 7.3.5 Add extension enable/disable flags
- [ ] 7.3.6 Write configuration validation tests

### 7.4 End-to-End Testing
- [ ] 7.4.1 Write E2E test: Context assembly with hierarchical compression
- [ ] 7.4.2 Write E2E test: Note generation from tool trajectory
- [ ] 7.4.3 Write E2E test: Hindsight capture and retrieval
- [ ] 7.4.4 Write E2E test: Meta-agent loop with test failure recovery
- [ ] 7.4.5 Write E2E test: Extension callback chain execution
- [ ] 7.4.6 Add performance benchmarks for context assembly (target: <100ms)

---

## Cross-Cutting Concerns

### CC.1 Observability
- [ ] CC.1.1 Add tracing spans for context architect operations
- [ ] CC.1.2 Add metrics for summary generation (latency, token counts)
- [ ] CC.1.3 Add metrics for note distillation (frequency, quality scores)
- [ ] CC.1.4 Add metrics for hindsight queries (hit rate, success rate)
- [ ] CC.1.5 Add metrics for meta-agent loop (iterations, outcomes)

### CC.2 Documentation
- [ ] CC.2.1 Update README with CCA capabilities overview
- [ ] CC.2.2 Add architecture diagram for hybrid execution model
- [ ] CC.2.3 Document configuration options with examples
- [ ] CC.2.4 Add API reference for new tools
- [ ] CC.2.5 Document extension development guide

### CC.3 Migration
- [ ] CC.3.1 Create database migration scripts for PostgreSQL
- [ ] CC.3.2 Add Redis schema documentation
- [ ] CC.3.3 Write data migration guide for existing deployments
- [ ] CC.3.4 Add rollback procedures for each migration

---

## Summary

| Phase | Tasks | Description |
|-------|-------|-------------|
| 1 | 28 | Summary schema extensions to memory/knowledge |
| 2 | 30 | Context Architect implementation |
| 3 | 21 | Note-Taking Agent with trajectory capture |
| 4 | 21 | Hindsight Learning with error patterns |
| 5 | 20 | Meta-Agent loop integration |
| 6 | 21 | Extension system for callbacks |
| 7 | 17 | OpenCode Plugin integration |
| CC | 14 | Cross-cutting concerns |
| **Total** | **172** | |

**Estimated effort**: 6-8 weeks with 80% test coverage target
