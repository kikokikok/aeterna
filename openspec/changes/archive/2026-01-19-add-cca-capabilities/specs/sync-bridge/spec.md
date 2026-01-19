## ADDED Requirements

### Requirement: Summary Synchronization
The system SHALL synchronize layer summaries between memory and knowledge systems.

#### Scenario: Sync summary on knowledge change
- **WHEN** knowledge item is updated
- **AND** item has associated memory pointer
- **THEN** system SHALL regenerate summary for affected layer
- **AND** system SHALL update memory pointer with new summary content

#### Scenario: Incremental summary sync
- **WHEN** running incremental sync
- **AND** layer content has changed
- **THEN** system SHALL regenerate summaries only for affected layers
- **AND** system SHALL preserve unchanged layer summaries

### Requirement: Summary Pointer Extension
The system SHALL extend pointer metadata to include summary information.

#### Scenario: Store summary with pointer
- **WHEN** creating or updating memory pointer
- **THEN** system SHALL include layer_summaries in pointer metadata
- **AND** system SHALL include summary_depth_available flags
- **AND** system SHALL include summary_generated_at timestamp

#### Scenario: Retrieve summary from pointer
- **WHEN** fetching memory pointer for context assembly
- **THEN** system SHALL return associated layer summaries
- **AND** system SHALL indicate if summary is stale

### Requirement: Summary Conflict Detection
The system SHALL detect conflicts in summary data between systems.

#### Scenario: Detect summary staleness
- **WHEN** checking sync state
- **AND** knowledge summary differs from memory summary
- **THEN** system SHALL create conflict with type='summary_mismatch'
- **AND** system SHALL include summary generation timestamps

#### Scenario: Resolve summary conflict
- **WHEN** resolving summary_mismatch conflict
- **THEN** system SHALL prefer knowledge-side summary (source of truth)
- **AND** system SHALL update memory pointer summary

### Requirement: Hindsight Pointer Sync
The system SHALL sync hindsight knowledge items to memory with resolution metadata.

#### Scenario: Create hindsight pointer
- **WHEN** syncing hindsight knowledge item
- **THEN** system SHALL create pointer with type='knowledge_pointer'
- **AND** system SHALL include resolution_count in metadata
- **AND** system SHALL include best_resolution_success_rate

#### Scenario: Update hindsight pointer
- **WHEN** hindsight item resolutions are updated
- **THEN** system SHALL update pointer metadata with new metrics
- **AND** system SHALL regenerate pointer content with updated summary

### Requirement: Summary Sync Triggers
The system SHALL support triggers for summary synchronization.

#### Scenario: Trigger summary sync on session start
- **WHEN** agent session starts
- **AND** summary staleness threshold exceeded
- **THEN** system SHALL trigger summary sync for session layers
- **AND** system SHALL use incremental sync for performance

#### Scenario: Trigger summary sync on content change
- **WHEN** layer content changes exceed change threshold
- **THEN** system SHALL queue summary regeneration
- **AND** system SHALL batch regeneration for efficiency

### Requirement: Summary Sync Observability
The system SHALL emit metrics for summary sync operations.

#### Scenario: Emit summary sync metrics
- **WHEN** summary sync completes
- **THEN** system SHALL emit counter: sync.summary.total with labels (layer, trigger)
- **AND** system SHALL emit histogram: sync.summary.duration_ms
- **AND** system SHALL emit counter: sync.summary.conflicts with labels (conflict_type)
