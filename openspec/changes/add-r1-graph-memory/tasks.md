## 1. Memory-R1 Implementation
- [x] 1.1 Implement `RewardFunction` for memory trajectory evaluation
- [x] 1.2 Add `prune()` and `compress()` methods to `MemoryManager`
- [x] 1.3 Create `MemoryR1Trainer` for outcome-driven policy updates

## 2. Dynamic Knowledge Graph
- [ ] 2.1 Implement `GraphStore` using PostgreSQL/Apache Age or simple relational tables
- [ ] 2.2 Create `EntityExtractor` using LLM-based parsing
- [ ] 2.3 Implement local search (neighbor traversal) and global search (community summaries)

## 3. Integration & Tooling
- [ ] 3.1 Add `graph_query` tool to `tools/`
- [ ] 3.2 Add `memory_optimize` tool for R1-led pruning
- [ ] 3.3 Write E2E tests for graph-based reasoning
