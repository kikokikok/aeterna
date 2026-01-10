# Change: Implement Memory System

## Why
The Memory System is the core component for storing and retrieving agent experiences. It implements the 7-layer hierarchy with semantic search, which is foundational for all other components (Knowledge Repository, Sync Bridge, Tool Interface).

## What Changes

### Memory System
- Implement `MemoryManager` with all CRUD operations
- Implement 7-layer memory hierarchy with proper scoping
- Implement semantic search with vector embeddings
- Implement layer resolution and merge algorithm
- Implement metadata filtering
- Implement provider adapter interface
- Implement mock provider for testing

### Storage Abstraction
- Define `MemoryProviderAdapter` trait
- Implement at least one real provider (Qdrant)
- Implement provider capabilities negotiation
- Implement layer isolation strategies

### Operations
- `add(input: AddMemoryInput) -> AddMemoryOutput`
- `search(input: SearchMemoryInput) -> SearchMemoryOutput`
- `get(input: GetMemoryInput) -> GetMemoryOutput`
- `update(input: UpdateMemoryInput) -> UpdateMemoryOutput`
- `delete(input: DeleteMemoryInput) -> DeleteMemoryOutput`
- `list(input: ListMemoriesInput) -> ListMemoriesOutput`

### Performance
- Working memory: <10ms (P95)
- Session memory: <50ms (P95)
- Semantic memory: <200ms (P95)
- Throughput: >100 QPS

## Impact

### Affected Specs
- `memory-system` - Complete implementation
- `adapter-layer` - Implement provider adapter interface

### Affected Code
- New `memory` crate
- Update `core` crate with provider traits
- Update `storage` crate with Qdrant implementation

### Dependencies
- Qdrant Rust SDK
- tokio for async runtime
- rayon for parallel queries
- thiserror for error handling

## Breaking Changes
None - this is greenfield work building on foundation
