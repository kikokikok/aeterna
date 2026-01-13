## 1. Foundation
- [ ] 1.1 Add `ReasoningStrategy` enum to `mk_core/src/types.rs`
- [ ] 1.2 Implement `ReflectiveReasoner` trait in `memory/src/reasoning.rs`
- [ ] 1.3 Add unit tests for reasoning strategies

## 2. Implementation
- [ ] 2.1 Implement LLM-based query expansion logic
- [ ] 2.2 Add `memory_reason` tool to `tools/src/memory_tools.rs`
- [ ] 2.3 Integrate reasoning step into `MemoryManager::search`
- [ ] 2.4 Add integration tests for reflective retrieval

## 3. Verification
- [ ] 3.1 Benchmark retrieval precision with vs without reasoning
- [ ] 3.2 Run `openspec validate add-reflective-reasoning --strict`
