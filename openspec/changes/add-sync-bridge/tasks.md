# Implementation Tasks

## 1. Sync State Management
- [x] 1.1 Implement SyncState struct
- [x] 1.2 Implement lastSyncAt field (ISO8601)
- [x] 1.3 Implement lastKnowledgeCommit field (git hash)
- [x] 1.4 Implement knowledgeHashes field (Map<id, hash>)
- [x] 1.5 Implement pointerMapping field (Map<memoryId, knowledgeId>)
- [x] 1.6 Implement SyncStats struct
- [x] 1.7 Implement SyncFailure struct
- [x] 1.8 Implement state serialization/deserialization
- [x] 1.9 Write unit tests for state persistence

## 2. State Persister
- [x] 2.1 Create state_persister.rs in sync/ crate
- [x] 2.2 Implement SyncStatePersister trait
- [x] 2.3 Implement FilePersister (JSON file)
- [x] 2.4 Implement DatabasePersister (PostgreSQL)
- [x] 2.5 Implement checkpoint() method
- [x] 2.6 Implement rollback() method
- [x] 2.7 Implement load() method
- [x] 2.8 Implement save() method
- [x] 2.9 Write unit tests for persister

## 3. Pointer Generation
- [x] 3.1 Implement KnowledgePointer struct
- [x] 3.2 Implement generate_pointer_content() function
- [x] 3.3 Include title + summary in content
- [x] 3.4 Include type indicator ([ADR], [POLICY], etc.)
- [x] 3.5 Include up to 3 blocking constraints
- [x] 4.1 DeltaResult struct
- [x] 4.2 detect_delta() function
- [x] 4.4 Identify new items (delta.added)
- [x] 4.5 Identify updated items (delta.updated)
- [x] 4.6 Identify deleted items (delta.deleted)
- [x] 4.7 Identify unchanged items (delta.unchanged)
- [x] 4.9 Write unit tests for delta detection
- [x] 4.10 Write property-based tests for delta algorithm

## 5. Conflict Detection
- [x] 5.1 Implement ConflictType enum
- [x] 5.2 Implement ConflictResolution enum
- [x] 5.3 Implement Conflict struct
- [x] 5.4 Implement detect_conflicts() function
- [x] 5.5 Check for hash_mismatch conflicts
- [x] 5.6 Check for orphaned_pointer conflicts
- [x] 5.7 Check for duplicate_pointer conflicts
- [x] 5.8 Check for layer_mismatch conflicts
- [x] 5.9 Check for status_change conflicts
- [x] 5.10 Write unit tests for each conflict type

## 6. Conflict Resolution
- [x] 6.1 Implement default resolution strategies
- [x] 6.2 Implement resolve_conflict() function
- [x] 6.3 Apply update_memory for hash_mismatch
- [x] 6.4 Apply delete_memory for orphaned_pointer
- [x] 6.5 Apply delete_memory for duplicate_pointer
- [x] 6.6 Apply update_memory for layer_mismatch
- [x] 6.7 Apply update_memory for status_change
- [x] 6.8 Implement ConflictResolutionConfig
- [x] 6.9 Support custom resolvers
- [x] 6.10 Write unit tests for resolution logic

## 7. Sync Manager Core
- [x] 7.1 Create sync_manager.rs in sync/ crate
- [x] 7.2 Implement SyncManager struct
- [x] 7.3 Implement new() constructor with dependencies
- [x] 7.4 Implement initialize() method
- [x] 7.5 Implement shutdown() method

## 8. Sync Operations - Full Sync
- [x] 8.1 Implement full_sync() method
- [x] 8.2 Create checkpoint before sync
- [x] 8.3 Get knowledge manifest
- [x] 8.4 Filter by types/layers if specified
- [x] 8.5 Detect delta (or force sync = all added)
- [x] 8.6 Process additions: create pointer memory
- [x] 8.7 Process updates: update pointer memory + hash
- [x] 8.8 Process deletions: mark as orphaned
- [x] 8.9 Update sync state (timestamp, commit, stats)
- [x] 8.10 Save state (rollback on catastrophic failure)
- [x] 8.11 Write integration tests for full sync

## 9. Sync Operations - Incremental Sync
- [x] 9.1 Implement incremental_sync() method
- [x] 9.2 Fetch commits since lastKnowledgeCommit
- [x] 9.3 Collect affected item IDs from commits
- [x] 9.4 Process only affected items (respect maxItems limit)
- [x] 9.5 Same processing as full sync for affected items
- [x] 9.6 Write integration tests for incremental sync

## 10. Sync Operations - Single Item Sync
- [x] 10.1 Implement single_item_sync() method
- [x] 10.2 Load knowledge item by ID
- [x] 10.3 If not found: mark pointer as orphaned
- [x] 10.4 If existing pointer: update content + hash
- [x] 10.5 If no pointer: create new pointer
- [x] 10.6 Update sync state
- [x] 10.7 Write unit tests for single item sync

## 11. Sync Triggers
- [x] 11.1 Implement SyncTrigger enum
- [x] 11.2 Implement SyncTriggerConfig struct
- [x] 13.1 Implement SyncError enum
- [x] 13.2 Define all error codes from spec
- [x] 15.2 Add Prometheus metrics
- [x] 15.8 Configure metric histograms

## 16. Integration Tests
- [x] 16.1 Create full sync workflow test suite
- [x] 16.2 Test full sync with 100+ knowledge items
- [x] 16.3 Test incremental sync with commits
- [x] 16.4 Test single item sync
- [x] 16.5 Test conflict detection and resolution
- [x] 16.6 Test checkpoint creation and rollback
- [x] 16.7 Test partial failure recovery
- [x] 16.8 Test trigger evaluation
- [x] 16.9 Test scheduled sync behavior
- [x] 16.10 Ensure 85%+ test coverage

## 17. Performance Tests
- [x] 17.1 Benchmark delta detection with 1000 items
- [x] 17.2 Benchmark single item sync latency
- [x] 17.3 Benchmark full sync with 100 items
- [x] 17.4 Benchmark incremental sync with 10 changed items
- [x] 17.5 Verify P95 targets met

## 18. Documentation
- [x] 18.1 Document SyncManager public API
- [x] 18.2 Document pointer architecture
- [x] 18.3 Document delta sync algorithm
- [x] 18.4 Document conflict resolution strategies
- [x] 18.5 Document sync trigger configuration
- [x] 18.6 Add inline examples for all operations
- [x] 18.7 Write architecture documentation
- [x] 18.8 Update crate README
