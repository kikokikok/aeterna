## ADDED Requirements

### Requirement: Sync State Persistence
The system SHALL maintain persistent state for tracking synchronization between memory and knowledge systems.

#### Scenario: Save sync state after operation
- **WHEN** a sync operation completes successfully
- **THEN** system SHALL save SyncState with lastSyncAt
- **AND** system SHALL save lastKnowledgeCommit hash
- **AND** system SHALL save knowledgeHashes mapping
- **AND** system SHALL save pointerMapping

#### Scenario: Load sync state on startup
- **WHEN** system starts
- **THEN** system SHALL load existing SyncState if available
- **AND** system SHALL initialize empty state if none exists

### Requirement: Delta Detection
The system SHALL detect changes between knowledge repository and last sync state using hash-based comparison.

#### Scenario: Detect new knowledge items
- **WHEN** knowledge manifest has new IDs not in stored hashes
- **THEN** system SHALL add items to delta.added
- **AND** system SHALL not include items in delta.updated or delta.unchanged

#### Scenario: Detect updated knowledge items
- **WHEN** knowledge manifest item ID exists but hash differs
- **THEN** system SHALL add items to delta.updated
- **AND** system SHALL compute hash from content, constraints, status

#### Scenario: Detect deleted knowledge items
- **WHEN** stored hash ID not found in knowledge manifest
- **THEN** system SHALL add ID to delta.deleted
- **AND** system SHALL not include in other delta fields

#### Scenario: Detect unchanged items
- **WHEN** knowledge manifest item ID exists and hash matches
- **THEN** system SHALL add ID to delta.unchanged

### Requirement: Pointer Memory Creation
The system SHALL generate pointer memories that summarize knowledge items for efficient storage and retrieval.

#### Scenario: Generate pointer content
- **WHEN** creating a pointer for a knowledge item
- **THEN** system SHALL include knowledge title in content
- **AND** system SHALL include knowledge summary in content
- **AND** system SHALL include type indicator ([ADR], [POLICY], [PATTERN], [SPEC])
- **AND** system SHALL include up to 3 blocking constraints
- **AND** system SHALL include knowledge item ID as reference

#### Scenario: Map knowledge layer to memory layer
- **WHEN** creating pointer for company knowledge
- **THEN** system SHALL set memory layer to 'company'
- **AND** system SHALL apply same mapping for all layers

### Requirement: Sync Operations
The system SHALL provide multiple sync methods for different use cases.

#### Scenario: Full sync all knowledge
- **WHEN** running full sync
- **THEN** system SHALL create checkpoint before starting
- **THEN** system SHALL load knowledge manifest
- **THEN** system SHALL detect delta from all items
- **THEN** system SHALL process additions, updates, deletions
- **THEN** system SHALL update sync state
- **THEN** system SHALL rollback on catastrophic failure

#### Scenario: Incremental sync since last commit
- **WHEN** running incremental sync
- **THEN** system SHALL fetch commits since lastKnowledgeCommit
- **THEN** system SHALL collect affected item IDs from commits
- **THEN** system SHALL process only affected items
- **AND** system SHALL respect maxItems limit if set

#### Scenario: Single item sync
- **WHEN** syncing single knowledge item by ID
- **THEN** system SHALL load knowledge item by ID
- **THEN** system SHALL update existing pointer if found
- **AND** system SHALL create new pointer if not found
- **AND** system SHALL mark pointer as orphaned if item deleted

### Requirement: Conflict Detection
The system SHALL detect and report conflicts between memory pointers and knowledge items.

#### Scenario: Detect hash mismatch conflict
- **WHEN** memory pointer contentHash differs from knowledge item hash
- **THEN** system SHALL create conflict with type='hash_mismatch'
- **AND** system SHALL suggest resolution='update_memory'

#### Scenario: Detect orphaned pointer
- **WHEN** memory pointer references deleted knowledge item
- **THEN** system SHALL create conflict with type='orphaned_pointer'
- **AND** system SHALL suggest resolution='delete_memory'

#### Scenario: Detect duplicate pointers
- **WHEN** multiple memories reference same knowledge item
- **THEN** system SHALL create conflict with type='duplicate_pointer'
- **AND** system SHALL suggest resolution='delete_memory' (keep newest)

#### Scenario: Detect status change
- **WHEN** knowledge item status changed to deprecated or superseded
- **THEN** system SHALL create conflict with type='status_change'
- **AND** system SHALL suggest resolution='update_memory'

### Requirement: Conflict Resolution
The system SHALL apply default resolution strategies to detected conflicts.

#### Scenario: Apply update_memory resolution
- **WHEN** resolving hash_mismatch or layer_mismatch conflict
- **THEN** system SHALL update memory pointer content
- **AND** system SHALL update memory pointer contentHash
- **AND** system SHALL update memory pointer timestamp

#### Scenario: Apply delete_memory resolution
- **WHEN** resolving orphaned_pointer or duplicate_pointer conflict
- **THEN** system SHALL delete memory pointer
- **AND** system SHALL remove from pointerMapping

#### Scenario: Custom resolution strategy
- **WHEN** user provides custom resolution config
- **THEN** system SHALL apply custom resolver for each conflict
- **AND** system SHALL fall back to default for unhandled types

### Requirement: Sync Triggers
The system SHALL evaluate conditions to determine when sync should run.

#### Scenario: Evaluate staleness threshold
- **WHEN** checking if sync should run with stalenessThreshold='6h'
- **THEN** system SHALL return true if lastSyncAt > 6 hours ago
- **AND** system SHALL return reason='threshold'

#### Scenario: Evaluate session threshold
- **WHEN** checking if sync should run with sessionThreshold=10
- **THEN** system SHALL return true if sessionsSinceSync >= 10
- **AND** system SHALL return reason='threshold'

#### Scenario: No trigger met
- **WHEN** checking if sync should run and no thresholds met
- **THEN** system SHALL return false with reason='manual'
- **AND** system SHALL not run sync automatically

### Requirement: Checkpoint and Rollback
The system SHALL create checkpoints before sync operations and support rollback on failure.

#### Scenario: Create checkpoint before sync
- **WHEN** starting a sync operation
- **THEN** system SHALL save current SyncState to checkpoint
- **AND** system SHALL return unique checkpoint ID

#### Scenario: Rollback on catastrophic failure
- **WHEN** sync operation fails catastrophically
- **THEN** system SHALL restore SyncState from checkpoint
- **AND** system SHALL restore pointerMapping
- **AND** system SHALL log rollback reason
- **AND** system SHALL not leave partial state

### Requirement: Sync Observability
The system SHALL emit metrics and logs for all sync operations.

#### Scenario: Emit sync operation metrics
- **WHEN** sync operation completes
- **THEN** system SHALL emit counter: sync.operations.total
- **AND** system SHALL emit histogram: sync.operations.duration
- **AND** system SHALL include sync type label (full, incremental, single)

#### Scenario: Emit item sync metrics
- **WHEN** processing items during sync
- **THEN** system SHALL emit counter: sync.items.added
- **AND** system SHALL emit counter: sync.items.updated
- **AND** system SHALL emit counter: sync.items.deleted

#### Scenario: Emit conflict metrics
- **WHEN** detecting conflicts during sync
- **THEN** system SHALL emit counter: sync.conflicts.total
- **AND** system SHALL include conflict type label

#### Scenario: Emit error metrics
- **WHEN** sync operation fails
- **THEN** system SHALL emit counter: sync.failures.total
- **AND** system SHALL include error code label

### Requirement: Sync Error Handling
The system SHALL provide specific error codes and recovery strategies.

#### Scenario: Knowledge unavailable error
- **WHEN** knowledge repository cannot be accessed
- **THEN** system SHALL return KNOWLEDGE_UNAVAILABLE error
- **AND** error SHALL be marked as retryable
- **AND** system SHALL retry with exponential backoff

#### Scenario: State corrupted error
- **WHEN** sync state file is corrupted
- **THEN** system SHALL return STATE_CORRUPTED error
- **AND** system SHALL attempt to regenerate from defaults
- **AND** system SHALL log corruption details

#### Scenario: Partial failure handling
- **WHEN** some items fail to sync
- **THEN** system SHALL return PARTIAL_FAILURE error
- **AND** system SHALL include failedItems in details
- **AND** system SHALL continue processing other items
- **AND** system SHALL not mark sync as complete

### Requirement: Performance Requirements
The system SHALL meet performance targets for sync operations.

#### Scenario: Delta detection performance
- **WHEN** detecting delta for 1000 knowledge items
- **THEN** operation SHALL complete in < 5 seconds
- **AND** system SHALL use O(n) algorithm

#### Scenario: Single item sync latency
- **WHEN** syncing single knowledge item
- **THEN** operation SHALL complete in < 100ms (excluding network latency)

#### Scenario: Full sync performance
- **WHEN** running full sync of 1000 items
- **THEN** operation SHALL complete in < 30 seconds
- **AND** system SHALL maintain <100 QPS throughput
