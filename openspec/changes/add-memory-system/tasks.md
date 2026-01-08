# Implementation Tasks

## 1. Provider Adapter Interface
- [ ] 1.1 Define `MemoryProviderAdapter` trait in `core/` crate
- [ ] 1.2 Define `ProviderCapabilities` struct
- [ ] 1.3 Define `ProviderConfig` struct
- [ ] 1.4 Define all input/output structs for provider methods
- [ ] 1.5 Define `HealthCheckResult` struct
- [ ] 1.6 Define `MemoryErrorCode` enum

## 2. Mock Provider Implementation
- [ ] 2.1 Create `mock_provider.rs` in `memory/` crate
- [ ] 2.2 Implement in-memory storage using HashMap
- [ ] 2.3 Implement all `MemoryProviderAdapter` methods
- [ ] 2.4 Implement embedding generation (return fixed vectors)
- [ ] 2.5 Add metrics collection (latency, operations, errors)
- [ ] 2.6 Write unit tests for mock provider

## 3. Memory Manager Core
- [ ] 3.1 Create `memory_manager.rs` in `memory/` crate
- [ ] 3.2 Implement `MemoryManager` struct with provider field
- [ ] 3.3 Implement `new()` constructor
- [ ] 3.4 Implement `initialize()` method
- [ ] 3.5 Implement `shutdown()` method
- [ ] 3.6 Implement `health_check()` method

## 4. Layer Resolution
- [ ] 4.1 Implement `get_accessible_layers(identifiers: &LayerIdentifiers) -> Vec<MemoryLayer>`
- [ ] 4.2 Implement `get_layer_precedence(layer: &MemoryLayer) -> u8`
- [ ] 4.3 Implement `merge_search_results()` function
- [ ] 4.4 Implement layer access control validation
- [ ] 4.5 Write unit tests for layer resolution logic

## 5. Memory Operations - Add
- [ ] 5.1 Implement validation for `AddMemoryInput`
- [ ] 5.2 Implement layer identifier validation
- [ ] 5.3 Implement `add()` method with embedding generation
- [ ] 5.4 Implement metadata merging
- [ ] 5.5 Return `AddMemoryOutput` with generated memory ID
- [ ] 5.6 Write unit tests for add operation

## 6. Memory Operations - Search
- [ ] 6.1 Implement query embedding generation
- [ ] 6.2 Implement concurrent layer search using `tokio::spawn`
- [ ] 6.3 Implement similarity threshold filtering
- [ ] 6.4 Implement metadata filtering
- [ ] 6.5 Implement result deduplication by content similarity
- [ ] 6.6 Implement result sorting by layer precedence and score
- [ ] 6.7 Write unit tests for search operation

## 7. Memory Operations - Get
- [ ] 7.1 Implement `get()` method to fetch memory by ID
- [ ] 7.2 Handle not found case (return null)
- [ ] 7.3 Write unit tests for get operation

## 8. Memory Operations - Update
- [ ] 8.1 Implement `update()` method with partial updates
- [ ] 8.2 Re-generate embedding if content changed
- [ ] 8.3 Merge metadata with existing metadata
- [ ] 8.4 Update timestamp
- [ ] 8.5 Handle memory not found error
- [ ] 8.6 Write unit tests for update operation

## 9. Memory Operations - Delete
- [ ] 9.1 Implement `delete()` method
- [ ] 9.2 Remove from provider
- [ ] 9.3 Return success status
- [ ] 9.4 Write unit tests for delete operation

## 10. Memory Operations - List
- [ ] 10.1 Implement `list()` method with pagination
- [ ] 10.2 Implement cursor-based pagination
- [ ] 10.3 Implement metadata filtering
- [ ] 10.4 Return `ListMemoriesOutput` with next cursor
- [ ] 10.5 Write unit tests for list operation

## 11. Qdrant Provider
- [ ] 11.1 Create `qdrant_provider.rs` in `storage/` crate
- [ ] 11.2 Initialize Qdrant client
- [ ] 11.3 Create collections per memory layer
- [ ] 11.4 Implement `add()` method with vector upsert
- [ ] 11.5 Implement `search()` method with vector similarity
- [ ] 11.6 Implement `get()` method with point retrieval
- [ ] 11.7 Implement `update()` method with point update
- [ ] 11.8 Implement `delete()` method with point deletion
- [ ] 11.9 Implement metadata filtering via Qdrant filters
- [ ] 11.10 Implement layer isolation via collections
- [ ] 11.11 Write integration tests for Qdrant provider

## 12. Embedding Service
- [ ] 12.1 Create `embedding_service.rs` in `utils/` crate
- [ ] 12.2 Define `EmbeddingProvider` trait
- [ ] 12.3 Implement OpenAI embedding provider
- [ ] 12.4 Implement caching for embeddings (Redis)
- [ ] 12.5 Implement batch embedding for efficiency
- [ ] 12.6 Write unit tests for embedding service

## 13. Error Handling
- [ ] 13.1 Implement all error variants from spec
- [ ] 13.2 Add retry logic with exponential backoff
- [ ] 13.3 Implement error translation from provider errors
- [ ] 13.4 Add detailed error context in `details` field
- [ ] 13.5 Set retryable flags on each error type
- [ ] 13.6 Write unit tests for error handling

## 14. Observability
- [ ] 14.1 Integrate OpenTelemetry for distributed tracing
- [ ] 14.2 Add Prometheus metrics for operations
- [ ] 14.3 Emit metrics: memory.operations.total, memory.operations.errors, memory.operations.latency
- [ ] 14.4 Emit metrics: memory.search.results, memory.storage.size by layer
- [ ] 14.5 Add structured logging with tracing spans
- [ ] 14.6 Configure metric histograms with appropriate buckets

## 15. Integration Tests
- [ ] 15.1 Create integration test suite
- [ ] 15.2 Test full add -> search -> update -> delete workflow
- [ ] 15.3 Test layer resolution and merging
- [ ] 15.4 Test concurrent operations
- [ ] 15.5 Test error handling and recovery
- [ ] 15.6 Test performance against P95 targets
- [ ] 15.7 Ensure 85%+ test coverage

## 16. Documentation
- [ ] 16.1 Document `MemoryManager` public API
- [ ] 16.2 Document `MemoryProviderAdapter` trait
- [ ] 16.3 Add inline examples for all operations
- [ ] 16.4 Write architecture documentation
- [ ] 16.5 Update crate README
