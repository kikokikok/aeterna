## ADDED Requirements

### Requirement: Error Signature Schema
The system SHALL capture error signatures with structured metadata for pattern matching and retrieval.

#### Scenario: Capture error with signature
- **WHEN** an error occurs during agent execution
- **THEN** system SHALL extract error_type, message_pattern, and stack_patterns
- **AND** system SHALL identify context_patterns (file types, frameworks, dependencies)
- **AND** system SHALL generate semantic embedding for the error signature
- **AND** system SHALL store signature with unique identifier

#### Scenario: Normalize error messages
- **WHEN** capturing error message
- **THEN** system SHALL normalize variable parts (paths, line numbers, UUIDs) to patterns
- **AND** system SHALL preserve error structure for regex matching
- **AND** system SHALL maintain original message in metadata

### Requirement: Resolution Tracking
The system SHALL track successful resolutions linked to error signatures with success metrics.

#### Scenario: Link resolution to error
- **WHEN** an error is successfully resolved
- **THEN** system SHALL create Resolution record linked to error_signature_id
- **AND** system SHALL capture description, code changes, and context
- **AND** system SHALL set initial success_rate to 1.0 and application_count to 1

#### Scenario: Update resolution metrics
- **WHEN** resolution is applied to similar error
- **AND** application succeeds
- **THEN** system SHALL increment application_count
- **AND** system SHALL recalculate success_rate as successes/applications
- **AND** system SHALL update last_success_at timestamp

#### Scenario: Track resolution failure
- **WHEN** resolution is applied but fails
- **THEN** system SHALL increment application_count without incrementing successes
- **AND** system SHALL recalculate success_rate
- **AND** system SHALL log failure context for analysis

### Requirement: Hindsight Note Generation
The system SHALL generate hindsight notes as a specialized knowledge type containing error-resolution pairs.

#### Scenario: Generate hindsight note
- **WHEN** error signature has at least one resolution with success_rate > 0.5
- **THEN** system SHALL generate Markdown content with:
- **AND** ## Error Pattern: Description of the error signature
- **AND** ## Resolution: Step-by-step fix instructions
- **AND** ## Code Examples: Relevant code changes
- **AND** ## Tags: Error type, framework, affected files

#### Scenario: Store hindsight as knowledge item
- **WHEN** hindsight note is generated
- **THEN** system SHALL create knowledge item with type='hindsight'
- **AND** system SHALL set layer based on error scope (session → project default)
- **AND** system SHALL include error_signature and resolutions in metadata

### Requirement: Error Pattern Matching
The system SHALL match incoming errors against stored error signatures using semantic and pattern matching.

#### Scenario: Match by semantic similarity
- **WHEN** new error occurs
- **THEN** system SHALL generate embedding for error
- **AND** system SHALL search error signatures with cosine similarity > 0.85
- **AND** system SHALL return matching signatures ranked by similarity

#### Scenario: Match by regex pattern
- **WHEN** semantic matching returns insufficient results
- **THEN** system SHALL attempt regex matching on message_pattern
- **AND** system SHALL attempt regex matching on stack_patterns
- **AND** system SHALL combine results with semantic matches

#### Scenario: Filter by context
- **WHEN** matches are found
- **THEN** system SHALL filter by context_patterns (file type, framework)
- **AND** system SHALL boost scores for matching context
- **AND** system SHALL return top-k filtered results

### Requirement: Resolution Suggestion
The system SHALL suggest resolutions for matched error signatures ranked by success rate and relevance.

#### Scenario: Suggest resolutions for error
- **WHEN** error signature matches are found
- **THEN** system SHALL retrieve resolutions for matched signatures
- **AND** system SHALL rank by: success_rate * context_match_score * recency_score
- **AND** system SHALL return top-3 suggestions with confidence scores

#### Scenario: No matching resolutions
- **WHEN** no resolution matches found
- **THEN** system SHALL return empty suggestions
- **AND** system SHALL log miss for future learning
- **AND** system SHALL NOT suggest unrelated resolutions

### Requirement: Auto-Promotion with Governance
The system SHALL auto-promote high-confidence hindsight notes to higher layers respecting governance rules.

#### Scenario: Promote after success threshold
- **WHEN** hindsight note has resolution with:
- **AND** application_count >= configured threshold (default: 5)
- **AND** success_rate >= 0.8
- **THEN** system SHALL propose promotion to next layer
- **AND** system SHALL NOT auto-promote; require layer approval

#### Scenario: Respect layer governance
- **WHEN** promotion is proposed
- **THEN** system SHALL check target layer's governance policies
- **AND** system SHALL require approval from layer maintainers
- **AND** system SHALL set promoted note status to 'proposed' until approved

#### Scenario: Track promotion chain
- **WHEN** hindsight note is promoted
- **THEN** system SHALL maintain reference to original note
- **AND** system SHALL track promotion path (session → project → team → org)
- **AND** system SHALL aggregate metrics across all instances

### Requirement: Error Deduplication
The system SHALL detect and deduplicate similar error signatures to prevent bloat.

#### Scenario: Detect duplicate error
- **WHEN** new error signature has >0.95 similarity with existing
- **THEN** system SHALL skip creating new signature
- **AND** system SHALL link new occurrence to existing signature
- **AND** system SHALL increment occurrence_count on existing signature

#### Scenario: Merge similar signatures
- **WHEN** two signatures have >0.9 similarity
- **AND** both have resolutions with >0.7 success_rate
- **THEN** system MAY suggest merge to maintainer
- **AND** system SHALL NOT auto-merge without approval

### Requirement: Observability
The system SHALL emit metrics and logs for hindsight learning operations.

#### Scenario: Emit capture metrics
- **WHEN** error signature is captured
- **THEN** system SHALL emit counter: hindsight.capture.total with labels (error_type, layer)
- **AND** system SHALL emit histogram: hindsight.capture.signature_size_bytes

#### Scenario: Emit matching metrics
- **WHEN** error matching completes
- **THEN** system SHALL emit histogram: hindsight.match.latency_ms
- **AND** system SHALL emit counter: hindsight.match.total with labels (match_type, found)
- **AND** system SHALL emit histogram: hindsight.match.results_count

#### Scenario: Emit resolution metrics
- **WHEN** resolution is applied
- **THEN** system SHALL emit counter: hindsight.resolution.applications with labels (success, layer)
- **AND** system SHALL emit histogram: hindsight.resolution.success_rate distribution

#### Scenario: Emit promotion metrics
- **WHEN** promotion is proposed or completed
- **THEN** system SHALL emit counter: hindsight.promotion.total with labels (from_layer, to_layer, status)

### Requirement: Hindsight Note Deduplication
The system SHALL prevent storage of duplicate hindsight notes capturing the same error pattern.

#### Scenario: Detect duplicate hindsight note
- **WHEN** generating hindsight note for error signature
- **THEN** system SHALL check for existing notes with same error_signature_id
- **AND** system SHALL check for semantic similarity > 0.9 with existing notes
- **AND** system SHALL skip creation if duplicate detected

#### Scenario: Merge resolutions for duplicate
- **WHEN** duplicate error signature detected with new resolution
- **THEN** system SHALL add resolution to existing signature
- **AND** system SHALL NOT create new hindsight note
- **AND** system SHALL update existing note with merged resolutions

#### Scenario: Storage bloat monitoring
- **WHEN** hindsight note count exceeds threshold per layer (configurable, default: 1000)
- **THEN** system SHALL alert on potential bloat
- **AND** system SHALL suggest cleanup of low-value notes (low success_rate, old, unused)
- **AND** system SHALL emit metric: hindsight.storage.count with labels (layer)
