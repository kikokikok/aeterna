## ADDED Requirements

### Requirement: Layer Summary Storage
The system SHALL store pre-computed summaries at multiple depths for each memory layer to enable efficient context assembly.

#### Scenario: Store summary with depth levels
- **WHEN** a summary is generated for a layer
- **THEN** system SHALL store summaries at three depths: sentence (~50 tokens), paragraph (~200 tokens), detailed (~500 tokens)
- **AND** system SHALL include token_count for budget calculation
- **AND** system SHALL include source_hash for staleness detection
- **AND** system SHALL include generated_at timestamp

#### Scenario: Store personalized summary
- **WHEN** personalization is enabled for a layer
- **THEN** system SHALL store personalization_context with summary
- **AND** system SHALL set personalized=true flag
- **AND** system SHALL scope personalization to user or session

### Requirement: Summary Configuration
The system SHALL support configurable summary generation triggers per memory layer.

#### Scenario: Configure time-based update
- **WHEN** configuring summary for a layer with update_interval
- **THEN** system SHALL trigger summary regeneration after interval elapses
- **AND** system SHALL skip regeneration if source unchanged (skip_if_unchanged=true)

#### Scenario: Configure change-based update
- **WHEN** configuring summary for a layer with update_on_changes threshold
- **THEN** system SHALL track change count since last summary
- **AND** system SHALL trigger regeneration when changes >= threshold

#### Scenario: Configure summary depths
- **WHEN** configuring summary depths for a layer
- **THEN** system SHALL only generate summaries for configured depths
- **AND** system SHALL default to all three depths if not specified

### Requirement: Summary Retrieval
The system SHALL provide operations to retrieve layer summaries for context assembly.

#### Scenario: Get summary by layer and depth
- **WHEN** requesting summary for layer with specific depth
- **THEN** system SHALL return summary content and metadata
- **AND** system SHALL return null if summary not available

#### Scenario: Get all summaries for context
- **WHEN** assembling context across layers
- **THEN** system SHALL return summaries for all accessible layers
- **AND** system SHALL respect layer precedence (project > team > org > company)
- **AND** system SHALL include token counts for budget calculation

### Requirement: Context Vector Storage
The system SHALL store context vectors for semantic relevance matching during context assembly.

#### Scenario: Store context vector with summary
- **WHEN** generating summary for a layer
- **THEN** system SHALL generate semantic embedding for summary content
- **AND** system SHALL store embedding as context_vector
- **AND** system SHALL update vector when summary changes

#### Scenario: Query relevant layers by vector
- **WHEN** assembling context with query embedding
- **THEN** system SHALL compute similarity between query and layer context_vectors
- **AND** system SHALL return relevance scores per layer
- **AND** system SHALL enable adaptive context selection based on scores

### Requirement: Summary Staleness Detection
The system SHALL detect when summaries are stale and need regeneration.

#### Scenario: Detect stale summary
- **WHEN** checking summary freshness
- **AND** source content hash differs from summary source_hash
- **THEN** system SHALL mark summary as stale
- **AND** system SHALL return needs_regeneration=true

#### Scenario: Track summary age
- **WHEN** checking summary age
- **AND** age exceeds configured max_age for layer
- **THEN** system SHALL mark summary as stale regardless of content hash

### Requirement: Summary Observability
The system SHALL emit metrics for summary operations.

#### Scenario: Emit generation metrics
- **WHEN** summary generation completes
- **THEN** system SHALL emit histogram: memory.summary.generation_duration_ms with labels (layer, depth)
- **AND** system SHALL emit counter: memory.summary.generations with labels (layer, depth, trigger)

#### Scenario: Emit retrieval metrics
- **WHEN** summary retrieval completes
- **THEN** system SHALL emit histogram: memory.summary.retrieval_latency_ms
- **AND** system SHALL emit counter: memory.summary.retrievals with labels (layer, depth, cache_hit)
