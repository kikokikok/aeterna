# Tasks: Intelligent Memory Retrieval (RLM Infrastructure)

## 1. Complexity Router

- [x] 1.1 Create `memory/src/rlm/mod.rs` module structure
- [x] 1.2 Implement `ComplexitySignals` struct
- [x] 1.3 Implement `ComplexityRouter` with keyword detection
- [x] 1.4 Implement complexity scoring algorithm
- [x] 1.5 Add configurable routing threshold
- [x] 1.6 Add unit tests for complexity scoring (80%+ coverage)
- [ ] 1.7 Add integration tests for routing decisions

## 2. Decomposition Strategy

- [x] 2.1 Define `DecompositionAction` enum (SearchLayer, DrillDown, Filter, RecursiveCall, Aggregate)
- [x] 2.2 Define `AggregationStrategy` enum (Combine, Compare, Summarize)
- [ ] 2.3 Implement `ActionExecutor` trait
- [ ] 2.4 Implement `SearchLayerExecutor`
- [ ] 2.5 Implement `DrillDownExecutor`
- [ ] 2.6 Implement `FilterExecutor` (relevance, recency)
- [ ] 2.7 Implement `RecursiveCallExecutor` with depth tracking
- [ ] 2.8 Implement `AggregateExecutor` with strategies
- [ ] 2.9 Add unit tests for all executors (80%+ coverage)

## 3. RLM Executor

- [ ] 3.1 Create `RlmExecutor` struct with configuration
- [ ] 3.2 Implement strategy selection based on query analysis
- [ ] 3.3 Implement sub-LM calling infrastructure with model routing
- [ ] 3.4 Implement depth limiting and circuit breaker
- [ ] 3.5 Implement `execute()` main entry point
- [ ] 3.6 Add trajectory recording during execution (internal)
- [ ] 3.7 Add integration tests for RLM execution flows

## 4. Decomposition Trainer

- [ ] 4.1 Define `DecompositionTrajectory` struct (internal)
- [ ] 4.2 Define `TrainingOutcome` enum
- [ ] 4.3 Define `RewardConfig` (simplified: success + efficiency)
- [ ] 4.4 Implement `compute_reward()` function
- [ ] 4.5 Implement `DecompositionTrainer` struct
- [ ] 4.6 Implement `compute_returns()` with discount factor
- [ ] 4.7 Implement `update_policy()` with policy gradient
- [ ] 4.8 Implement exploration/exploitation action selection
- [ ] 4.9 Implement `export_state()` / `import_state()` for persistence
- [ ] 4.10 Add unit tests for trainer (80%+ coverage)

## 5. Combined Trainer Integration

- [ ] 5.1 Extend `CombinedMemoryTrainer` to include decomposition training
- [ ] 5.2 Implement unified `train_step()` method
- [ ] 5.3 Add PostgreSQL schema for persisting decomposition weights
- [ ] 5.4 Implement weight persistence (save/load from DB)
- [ ] 5.5 Add integration tests for combined training

## 6. Memory Search Enhancement

- [ ] 6.1 Integrate `ComplexityRouter` into `MemoryManager::search()`
- [ ] 6.2 Add internal routing to RLM executor for complex queries
- [ ] 6.3 Ensure API compatibility (no signature changes)
- [ ] 6.4 Add fallback to standard search on RLM failure
- [ ] 6.5 Add integration tests for transparent routing

## 7. Context Architect Integration

- [ ] 7.1 Add `assemble_with_rlm()` method to ContextAssembler (internal)
- [ ] 7.2 Implement complexity detection for automatic routing
- [ ] 7.3 Add fallback from RLM to standard assembly on failure
- [ ] 7.4 Add integration tests for enhanced assembly

## 8. Bootstrapping (Internal)

- [ ] 8.1 Define synthetic task templates for bootstrapping
- [ ] 8.2 Implement `generate_bootstrap_tasks()` from memory schema
- [ ] 8.3 Implement `bootstrap_trainer()` offline training pipeline
- [ ] 8.4 Add integration tests for bootstrapping pipeline

## 9. Observability

- [ ] 9.1 Add metrics: `rlm.routing.decision` counter (standard vs rlm)
- [ ] 9.2 Add metrics: `rlm.execution.duration_ms` histogram
- [ ] 9.3 Add metrics: `rlm.execution.depth` histogram
- [ ] 9.4 Add metrics: `rlm.training.reward` histogram
- [ ] 9.5 Add tracing spans for RLM execution phases
- [ ] 9.6 Add structured logging for internal debugging

## 10. Documentation

- [ ] 10.1 Add rustdoc for all public APIs (internal module, but document interfaces)
- [ ] 10.2 Add architecture diagram to design doc
- [ ] 10.3 Document complexity routing algorithm
- [ ] 10.4 Document training signal sources

## 11. Testing & Coverage

- [ ] 11.1 Ensure 80%+ coverage on `memory/src/rlm/` module
- [ ] 11.2 Add property-based tests for complexity scoring
- [ ] 11.3 Add mutation tests for reward computation
- [ ] 11.4 Run full test suite and verify no regressions
