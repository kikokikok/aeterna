## 1. Memory-R1 Implementation
- [x] 1.1 Implement `RewardFunction` for memory trajectory evaluation
- [x] 1.2 Add `prune()` and `compress()` methods to `MemoryManager`
- [x] 1.3 Create `MemoryR1Trainer` for outcome-driven policy updates

## 2. Dynamic Knowledge Graph (DuckDB + DuckPGQ)
- [ ] 2.1 Add `duckdb` crate to `storage/Cargo.toml` with bundled feature
- [ ] 2.2 Implement `GraphStore` struct with DuckDB connection management
- [ ] 2.3 Create SQL schema for memory_nodes, memory_edges, entities, entity_edges
- [ ] 2.4 Define SQL/PGQ property graphs (memory_graph, entity_graph)
- [ ] 2.5 Implement `find_related()` - neighbor traversal within N hops
- [ ] 2.6 Implement `shortest_path()` - path finding between memories
- [ ] 2.7 Add S3 persistence support (load_from_s3, persist_to_s3)
- [ ] 2.8 Create `EntityExtractor` trait with LLM-based implementation
- [ ] 2.9 Implement community detection for memory clustering
- [ ] 2.10 Write unit tests for GraphStore (>80% coverage)

## 3. Integration & Tooling
- [ ] 3.1 Add `graph_query` tool to `tools/` with MCP interface
- [ ] 3.2 Add `graph_neighbors` tool for related memory lookup
- [ ] 3.3 Add `memory_optimize` tool for R1-led pruning
- [ ] 3.4 Integrate GraphStore with MemoryManager (auto-update on add/delete)
- [ ] 3.5 Write E2E tests for graph-based reasoning
- [ ] 3.6 Add GraphStore configuration to `config/` module
