# Change: Memory-R1 and Dynamic Knowledge Graph Layer

## Why
To achieve state-of-the-art performance in 2026, memory systems must move beyond static vector retrieval. This proposal combines two major advancements:

1. **Memory-R1**: Enables outcome-driven self-improvement of the memory bank. Agents learn to prune useless memories and compress redundant ones based on task success (reward signals).
2. **Dynamic Knowledge Graph**: Adds a structured semantic layer to memory. Instead of just finding "similar" fragments, agents can traverse relationships (Entity -> Relation -> Entity), enabling complex reasoning like "Find all Python memory leak resolutions applied to Project X".

## What Changes
- **Memory-R1**: Implementation of `MemoryManagerAgent` for active pruning/compression.
- **Graph Layer**: Implementation of `GraphStore` and `EntityExtractor`.
- **New Tools**: `graph_query` and `memory_prune` for agent-led memory optimization.

## Impact
- Affected specs: `memory-system`
- Affected code: `mk_core/` (types added), `memory/`, `storage/`
- Performance: Pruning improves retrieval speed by reducing search space; Graph traversal adds O(depth) overhead.
