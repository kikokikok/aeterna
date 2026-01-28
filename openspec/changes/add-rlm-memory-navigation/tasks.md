# Tasks: Intelligent Memory Retrieval (RLM Infrastructure)

## 1. Complexity Router

- [x] 1.1 Create `memory/src/rlm/mod.rs` module structure
- [x] 1.2 Implement `ComplexitySignals` struct
- [x] 1.3 Implement `ComplexityRouter` with keyword detection
- [x] 1.4 Implement complexity scoring algorithm
- [x] 1.5 Add configurable routing threshold
- [x] 1.6 Add unit tests for complexity scoring (80%+ coverage)
- [x] 1.7 Add integration tests for routing decisions

## 2. Decomposition Strategy

- [x] 2.1 Define `DecompositionAction` enum (SearchLayer, DrillDown, Filter, RecursiveCall, Aggregate)
- [x] 2.2 Define `AggregationStrategy` enum (Combine, Compare, Summarize)
- [x] 2.3 Implement `ActionExecutor` trait
- [x] 2.4 Implement `SearchLayerExecutor`
- [x] 2.5 Implement `DrillDownExecutor`
- [x] 2.6 Implement `FilterExecutor` (relevance, recency)
- [x] 2.7 Implement `RecursiveCallExecutor` with depth tracking
- [x] 2.8 Implement `AggregateExecutor` with strategies
- [x] 2.9 Add unit tests for all executors (80%+ coverage)

## 3. RLM Executor

- [x] 3.1 Create `RlmExecutor` struct with configuration
- [x] 3.2 Implement strategy selection based on query analysis
- [x] 3.3 Implement sub-LM calling infrastructure with model routing
- [x] 3.4 Implement depth limiting and circuit breaker
- [x] 3.5 Implement `execute()` main entry point
- [x] 3.6 Add trajectory recording during execution (internal)
- [x] 3.7 Add integration tests for RLM execution flows

## 4. Decomposition Trainer

- [x] 4.1 Define `DecompositionTrajectory` struct (internal)
- [x] 4.2 Define `TrainingOutcome` enum
- [x] 4.3 Define `RewardConfig` (simplified: success + efficiency)
- [x] 4.4 Implement `compute_reward()` function
- [x] 4.5 Implement `DecompositionTrainer` struct
- [x] 4.6 Implement `compute_returns()` with discount factor
- [x] 4.7 Implement `update_policy()` with policy gradient
- [x] 4.8 Implement exploration/exploitation action selection
- [x] 4.9 Implement `export_state()` / `import_state()` for persistence
- [x] 4.10 Add unit tests for trainer (80%+ coverage)

## 5. Combined Trainer Integration

- [x] 5.1 Extend `CombinedMemoryTrainer` to include decomposition training
- [x] 5.2 Implement unified `train_step()` method
- [x] 5.3 Add PostgreSQL schema for persisting decomposition weights
- [x] 5.4 Implement weight persistence (save/load from DB)
- [x] 5.5 Add integration tests for combined training

## 6. Memory Search Enhancement

- [x] 6.1 Integrate `ComplexityRouter` into `MemoryManager::search()`
- [x] 6.2 Add internal routing to RLM executor for complex queries
- [x] 6.3 Ensure API compatibility (no signature changes)
- [x] 6.4 Add fallback to standard search on RLM failure
- [x] 6.5 Add integration tests for transparent routing

## 7. Context Architect Integration

- [x] 7.1 Add `assemble_with_rlm()` method to ContextAssembler (internal)
- [x] 7.2 Implement complexity detection for automatic routing
- [x] 7.3 Add fallback from RLM to standard assembly on failure
- [x] 7.4 Add integration tests for enhanced assembly

## 8. Bootstrapping (Internal)

- [x] 8.1 Define synthetic task templates for bootstrapping
- [x] 8.2 Implement `generate_bootstrap_tasks()` from memory schema
- [x] 8.3 Implement `bootstrap_trainer()` offline training pipeline
- [x] 8.4 Add integration tests for bootstrapping pipeline

## 9. Observability

- [x] 9.1 Add metrics: `rlm.routing.decision` counter (standard vs rlm)
- [x] 9.2 Add metrics: `rlm.execution.duration_ms` histogram
- [x] 9.3 Add metrics: `rlm.execution.depth` histogram
- [x] 9.4 Add metrics: `rlm.training.reward` histogram
- [x] 9.5 Add tracing spans for RLM execution phases
- [x] 9.6 Add structured logging for internal debugging

## 10. Documentation

- [x] 10.1 Add rustdoc for all public APIs (internal module, but document interfaces)
- [x] 10.2 Add architecture diagram to design doc
- [x] 10.3 Document complexity routing algorithm
- [x] 10.4 Document training signal sources

## 11. Testing & Coverage

- [x] 11.1 Ensure 80%+ coverage on `memory/src/rlm/` module
- [x] 11.2 Add property-based tests for complexity scoring
- [x] 11.3 Add mutation tests for reward computation
- [x] 11.4 Run full test suite and verify no regressions
