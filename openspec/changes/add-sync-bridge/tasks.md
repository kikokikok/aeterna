# Implementation Tasks

## 1. Sync State Management
- [ ] 1.1 Implement SyncState struct
- [ ] 1.2 Implement lastSyncAt field (ISO8601)
- [ ] 1.3 Implement lastKnowledgeCommit field (git hash)
- [ ] 1.4 Implement knowledgeHashes field (Map<id, hash>)
- [ ] 1.5 Implement pointerMapping field (Map<memoryId, knowledgeId>)
- [ ] 1.6 Implement SyncStats struct
- [ ] 1.7 Implement SyncFailure struct
- [ ] 1.8 Implement state serialization/deserialization
- [ ] 1.9 Write unit tests for state persistence

## 2. State Persister
- [ ] 2.1 Create state_persister.rs in sync/ crate
- [ ] 2.2 Implement SyncStatePersister trait
- [ ] 2.3 Implement FilePersister (JSON file)
- [ ] 2.4 Implement DatabasePersister (PostgreSQL)
- [ ] 2.5 Implement checkpoint() method
- [ ] 2.6 Implement rollback() method
- [ ] 2.7 Implement load() method
- [ ] 2.8 Implement save() method
- [ ] 2.9 Write unit tests for persister

## 3. Pointer Generation
- [ ] 3.1 Implement KnowledgePointer struct
- [ ] 3.2 Implement generate_pointer_content() function
- [ ] 3.3 Include title + summary in content
- [ ] 3.4 Include type indicator ([ADR], [POLICY], etc.)
- [ ] 3.5 Include up to 3 blocking constraints
- [ ] 3.6 Include reference ID
- [ ] 3.7 Write unit tests for pointer generation

## 4. Delta Detection
- [ ] 4.1 Implement DeltaResult struct
- [ ] 4.2 Implement detect_delta() function
- [ ] 4.3 Compare manifest IDs vs stored hashes
- [ ] 4.4 Identify new items (delta.added)
- [ ] 4.5 Identify updated items (delta.updated)
- [ ] 4.6 Identify deleted items (delta.deleted)
- [ ] 4.7 Identify unchanged items (delta.unchanged)
- [ ] 4.8 Use compute_knowledge_hash() for comparison
- [ ] 4.9 Write unit tests for delta detection
- [ ] 4.10 Write property-based tests for delta algorithm

## 5. Conflict Detection
- [ ] 5.1 Implement ConflictType enum
- [ ] 5.2 Implement ConflictResolution enum
- [ ] 5.3 Implement Conflict struct
- [ ] 5.4 Implement detect_conflicts() function
- [ ] 5.5 Check for hash_mismatch conflicts
- [ ] 5.6 Check for orphaned_pointer conflicts
- [ ] 5.7 Check for duplicate_pointer conflicts
- [ ] 5.8 Check for layer_mismatch conflicts
- [ ] 5.9 Check for status_change conflicts
- [ ] 5.10 Write unit tests for each conflict type

## 6. Conflict Resolution
- [ ] 6.1 Implement default resolution strategies
- [ ] 6.2 Implement resolve_conflict() function
- [ ] 6.3 Apply update_memory for hash_mismatch
- [ ] 6.4 Apply delete_memory for orphaned_pointer
- [ ] 6.5 Apply delete_memory for duplicate_pointer
- [ ] 6.6 Apply update_memory for layer_mismatch
- [ ] 6.7 Apply update_memory for status_change
- [ ] 6.8 Implement ConflictResolutionConfig
- [ ] 6.9 Support custom resolvers
- [ ] 6.10 Write unit tests for resolution logic

## 7. Sync Manager Core
- [ ] 7.1 Create sync_manager.rs in sync/ crate
- [ ] 7.2 Implement SyncManager struct
- [ ] 7.3 Implement new() constructor with dependencies
- [ ] 7.4 Implement initialize() method
- [ ] 7.5 Implement shutdown() method

## 8. Sync Operations - Full Sync
- [ ] 8.1 Implement full_sync() method
- [ ] 8.2 Create checkpoint before sync
- [ ] 8.3 Get knowledge manifest
- [ ] 8.4 Filter by types/layers if specified
- [ ] 8.5 Detect delta (or force sync = all added)
- [ ] 8.6 Process additions: create pointer memory
- [ ] 8.7 Process updates: update pointer memory + hash
- [ ] 8.8 Process deletions: mark as orphaned
- [ ] 8.9 Update sync state (timestamp, commit, stats)
- [ ] 8.10 Save state (rollback on catastrophic failure)
- [ ] 8.11 Write integration tests for full sync

## 9. Sync Operations - Incremental Sync
- [ ] 9.1 Implement incremental_sync() method
- [ ] 9.2 Fetch commits since lastKnowledgeCommit
- [ ] 9.3 Collect affected item IDs from commits
- [ ] 9.4 Process only affected items (respect maxItems limit)
- [ ] 9.5 Same processing as full sync for affected items
- [ ] 9.6 Write integration tests for incremental sync

## 10. Sync Operations - Single Item Sync
- [ ] 10.1 Implement single_item_sync() method
- [ ] 10.2 Load knowledge item by ID
- [ ] 10.3 If not found: mark pointer as orphaned
- [ ] 10.4 If existing pointer: update content + hash
- [ ] 10.5 If no pointer: create new pointer
- [ ] 10.6 Update sync state
- [ ] 10.7 Write unit tests for single item sync

## 11. Sync Triggers
- [ ] 11.1 Implement SyncTrigger enum
- [ ] 11.2 Implement SyncTriggerConfig struct
- [ ] 11.3 Implement should_trigger_sync() function
- [ ] 11.4 Check staleness threshold
- [ ] 11.5 Check session count threshold
- [ ] 11.6 Return shouldSync and reason
- [ ] 11.7 Write unit tests for trigger evaluation

## 12. Sync Scheduler
- [ ] 12.1 Implement scheduled_sync() method
- [ ] 12.2 Use tokio::time::interval for scheduling
- [ ] 12.3 Evaluate triggers before each run
- [ ] 12.4 Run incremental_sync if triggered
- [ ] 12.5 Log sync results and duration
- [ ] 12.6 Write integration tests for scheduled sync

## 13. Error Handling
- [ ] 13.1 Implement SyncError enum
- [ ] 13.2 Define all error codes from spec
- [ ] 13.3 Implement retry logic with exponential backoff
- [ ] 13.4 Implement partial failure handling
- [ ] 13.5 Implement checkpoint recovery
- [ ] 13.6 Implement rollback on catastrophic failure
- [ ] 13.7 Write unit tests for error handling

## 14. Checkpoint & Rollback
- [ ] 14.1 Implement create_checkpoint() method
- [ ] 14.2 Save current state to checkpoint location
- [ ] 14.3 Return checkpoint ID
- [ ] 14.4 Implement rollback() method
- [ ] 14.5 Load state from checkpoint
- [ ] 14.6 Restore state and mappings
- [ ] 14.7 Write integration tests for checkpoint/rollback

## 15. Observability
- [ ] 15.1 Integrate OpenTelemetry for sync operations
- [ ] 15.2 Add Prometheus metrics
- [ ] 15.3 Emit metrics: sync.operations.total, sync.operations.duration
- [ ] 15.4 Emit metrics: sync.items.added, sync.items.updated, sync.items.deleted
- [ ] 15.5 Emit metrics: sync.conflicts.total, sync.failures.total
- [ ] 15.6 Emit metrics: sync.state.age
- [ ] 15.7 Add structured logging with tracing spans
- [ ] 15.8 Configure metric histograms

## 16. Integration Tests
- [ ] 16.1 Create full sync workflow test suite
- [ ] 16.2 Test full sync with 100+ knowledge items
- [ ] 16.3 Test incremental sync with commits
- [ ] 16.4 Test single item sync
- [ ] 16.5 Test conflict detection and resolution
- [ ] 16.6 Test checkpoint creation and rollback
- [ ] 16.7 Test partial failure recovery
- [ ] 16.8 Test trigger evaluation
- [ ] 16.9 Test scheduled sync behavior
- [ ] 16.10 Ensure 85%+ test coverage

## 17. Performance Tests
- [ ] 17.1 Benchmark delta detection with 1000 items
- [ ] 17.2 Benchmark single item sync latency
- [ ] 17.3 Benchmark full sync with 100 items
- [ ] 17.4 Benchmark incremental sync with 10 changed items
- [ ] 17.5 Verify P95 targets met

## 18. Documentation
- [ ] 18.1 Document SyncManager public API
- [ ] 18.2 Document pointer architecture
- [ ] 18.3 Document delta sync algorithm
- [ ] 18.4 Document conflict resolution strategies
- [ ] 18.5 Document sync trigger configuration
- [ ] 18.6 Add inline examples for all operations
- [ ] 18.7 Write architecture documentation
- [ ] 18.8 Update crate README
