# Spec Delta: Memory System

## ADDED Requirements

### Requirement: Cost-Optimized Embedding Generation

The system SHALL implement semantic caching to reduce embedding API costs by 60-80%.

#### Scenario: Exact Match Cache Hit
- **WHEN** user adds memory with content "Use PostgreSQL for persistence"
- **AND** system checks exact match cache
- **AND** cache contains entry with identical content
- **THEN** system returns cached embedding without API call
- **AND** cache hit metric incremented

#### Scenario: Semantic Similarity Cache Hit
- **WHEN** user adds memory with content "We use PostgreSQL for database"
- **AND** exact match cache misses
- **AND** system checks semantic similarity cache
- **AND** cache contains entry with 0.98+ similarity
- **THEN** system returns similar embedding without API call
- **AND** similarity cache hit metric incremented

#### Scenario: Cache Miss with New Embedding
- **WHEN** user adds memory with unique content
- **AND** both caches miss
- **THEN** system generates new embedding via API
- **AND** stores embedding in both caches
- **AND** cache miss metric incremented

### Requirement: Tiered Storage Management

The system SHALL automatically tier memories based on access patterns and age.

#### Scenario: Hot Tier Storage
- **WHEN** memory accessed within last 7 days
- **THEN** system stores in hot tier (Redis)
- **AND** retrieval latency < 10ms

#### Scenario: Warm Tier Migration
- **WHEN** memory age > 7 days AND < 90 days
- **AND** access frequency < 1/day
- **THEN** system migrates to warm tier (PostgreSQL)
- **AND** retrieval latency < 50ms

#### Scenario: Cold Tier Archival
- **WHEN** memory age > 90 days
- **AND** access frequency < 1/week
- **THEN** system migrates to cold tier (S3 + Parquet)
- **AND** retrieval latency < 500ms

## ADDED Requirements

### Requirement: Context Architect (CCA)

The system SHALL provide hierarchical context compression with adaptive level selection.

#### Scenario: High Token Budget with Detailed Context
- **WHEN** agent requests context with budget 4096 tokens
- **AND** relevant memories total 8000 tokens
- **THEN** system generates 3 summary levels per memory (sentence/paragraph/detailed)
- **AND** selects detailed level for high-relevance memories (>0.9)
- **AND** selects paragraph level for medium-relevance (0.7-0.9)
- **AND** selects sentence level for low-relevance (<0.7)
- **AND** assembled context fits within budget

#### Scenario: Low Token Budget with Sentence Context
- **WHEN** agent requests context with budget 512 tokens
- **AND** relevant memories total 2000 tokens
- **THEN** system uses primarily sentence-level summaries
- **AND** includes only highest-relevance memories
- **AND** assembled context fits within budget

### Requirement: Hindsight Learning (CCA)

The system SHALL capture error patterns and suggest resolutions based on historical data.

#### Scenario: First Error Occurrence
- **WHEN** agent encounters NullPointerException in JWT decoding
- **THEN** system captures error signature
- **AND** searches for similar historical errors
- **AND** finds no matches
- **AND** stores error as new pattern

#### Scenario: Recurring Error with Resolution
- **WHEN** agent encounters NullPointerException in JWT decoding (3rd occurrence)
- **THEN** system finds 2 previous occurrences
- **AND** identifies common pattern (missing null check after decode)
- **AND** suggests resolution: "Add null check after JWT decode, return 401 if null"
- **AND** promotes pattern to team layer for sharing

### Requirement: Note-Taking Agent (CCA)

The system SHALL capture significant trajectory events during agent execution.

#### Scenario: Significant Event Capture
- **WHEN** agent executes tool that modifies state
- **OR** agent encounters error
- **OR** agent receives user feedback
- **THEN** system captures event with context
- **AND** marks as significant if impact > threshold

#### Scenario: Trajectory Summarization
- **WHEN** agent session completes
- **AND** significant events > 0
- **THEN** system summarizes trajectory
- **AND** extracts learnings
- **AND** stores as procedural memory

## MODIFIED Requirements

### Requirement: Memory Retrieval Performance

The system SHALL provide predictable retrieval latency across all memory layers WITH cost optimization.

#### Scenario: Cached Embedding Search
- **WHEN** user searches with previously used query
- **AND** embedding cache contains query embedding
- **THEN** system retrieves cached embedding (< 5ms)
- **AND** performs vector search
- **AND** total latency < 100ms (vs 250ms without cache)
