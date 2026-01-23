## ADDED Requirements

### Requirement: Complexity-Based Query Routing
The system SHALL automatically route memory search queries based on computed complexity, using RLM executor for complex queries and standard vector search for simple queries.

#### Scenario: Simple query routed to standard search
- **WHEN** a memory search query has complexity score below routing threshold (default: 0.3)
- **THEN** system SHALL route to standard vector search
- **AND** system SHALL NOT invoke RLM executor
- **AND** system SHALL return results with standard latency

#### Scenario: Complex query routed to RLM executor
- **WHEN** a memory search query has complexity score at or above routing threshold
- **THEN** system SHALL route to RLM executor internally
- **AND** system SHALL execute decomposition strategy
- **AND** system SHALL return unified results (user sees results, not decomposition)

#### Scenario: RLM failure falls back to standard search
- **WHEN** RLM executor fails (timeout, error, or depth limit)
- **THEN** system SHALL fall back to standard vector search
- **AND** system SHALL log failure for observability
- **AND** system SHALL return best-effort results

### Requirement: Complexity Scoring
The system SHALL compute a complexity score for memory search queries to determine routing.

#### Scenario: Compute complexity from query signals
- **WHEN** a query is received for memory search
- **THEN** system SHALL analyze query for complexity signals:
- **AND** multi-layer signals (mentions teams, orgs, projects) contribute weight 0.3
- **AND** aggregation signals ("across", "all", "summarize") contribute weight 0.25
- **AND** comparison signals ("compare", "vs", "difference") contribute weight 0.25
- **AND** query length and structure contribute weight 0.2
- **AND** final score SHALL be clamped to [0.0, 1.0]

#### Scenario: Configurable routing threshold
- **WHEN** system is configured with custom routing threshold
- **THEN** system SHALL use configured threshold for routing decisions
- **AND** threshold SHALL be configurable per-tenant

### Requirement: Decomposition Strategy Execution
The system SHALL support internal decomposition strategies for complex memory queries.

#### Scenario: SearchLayer strategy execution
- **WHEN** RLM executor selects SearchLayer action with layer and query
- **THEN** system SHALL perform semantic search in specified layer
- **AND** system SHALL return matching memories with scores
- **AND** system SHALL record action in internal trajectory

#### Scenario: DrillDown strategy execution
- **WHEN** RLM executor selects DrillDown action from parent to child layer
- **THEN** system SHALL identify child entities matching filter
- **AND** system SHALL narrow scope to matched entities
- **AND** system SHALL record action in internal trajectory

#### Scenario: RecursiveCall strategy execution
- **WHEN** RLM executor selects RecursiveCall action with sub-query
- **AND** current depth is less than max_recursion_depth (default: 3)
- **THEN** system SHALL invoke sub-LM with sub-query and context
- **AND** system SHALL track tokens used
- **AND** system SHALL record action in internal trajectory

#### Scenario: RecursiveCall depth limit enforced
- **WHEN** RecursiveCall would exceed max_recursion_depth
- **THEN** system SHALL reject the action
- **AND** system SHALL return best results found so far
- **AND** system SHALL NOT invoke sub-LM

#### Scenario: Aggregate strategy execution
- **WHEN** RLM executor selects Aggregate action with strategy
- **THEN** system SHALL combine results using specified strategy (combine, compare, summarize)
- **AND** system SHALL return unified result set

### Requirement: Internal Trajectory Recording
The system SHALL record decomposition trajectories internally for training purposes, without exposing this to users.

#### Scenario: Record trajectory during RLM execution
- **WHEN** RLM executor processes a query
- **THEN** system SHALL create internal trajectory record
- **AND** system SHALL record each action with timestamp, duration, and token counts
- **AND** system SHALL record outcome (result count, success/failure)

#### Scenario: Trajectory not exposed to users
- **WHEN** memory search returns results
- **THEN** response SHALL NOT include trajectory details
- **AND** response SHALL NOT indicate whether RLM was used
- **AND** user experience SHALL be identical regardless of routing

### Requirement: Decomposition Training
The system SHALL train decomposition strategies from usage patterns without user involvement.

#### Scenario: Compute reward from outcome
- **WHEN** trajectory is completed with outcome
- **THEN** system SHALL compute reward based on:
- **AND** success component (was query answered?)
- **AND** efficiency component (token cost penalty)
- **AND** reward SHALL be clamped to [-1.0, 1.0]

#### Scenario: Update policy weights
- **WHEN** sufficient trajectories are collected (minimum batch: 20)
- **THEN** system SHALL compute returns and advantages
- **AND** system SHALL update action weights using policy gradient
- **AND** system SHALL persist weights to database

#### Scenario: Training outcome signals
- **WHEN** search result is subsequently used in context assembly
- **THEN** system SHALL record positive training signal
- **WHEN** user refines query after search
- **THEN** system SHALL record partial success signal
- **WHEN** search result is ignored
- **THEN** system SHALL record negative training signal

### Requirement: Trainer State Persistence
The system SHALL persist decomposition trainer state for continuity.

#### Scenario: Persist trainer state
- **WHEN** training step completes
- **THEN** system SHALL save action weights, baseline, and statistics to PostgreSQL
- **AND** system SHALL use tenant-scoped storage for isolation

#### Scenario: Restore trainer state on startup
- **WHEN** system initializes
- **THEN** system SHALL load persisted trainer state if available
- **AND** system SHALL resume training from saved state

### Requirement: RLM Observability
The system SHALL emit metrics for RLM infrastructure without exposing details to users.

#### Scenario: Routing decision metrics
- **WHEN** query routing decision is made
- **THEN** system SHALL emit counter: `memory.rlm.routing.decision` with label (standard, rlm)
- **AND** system SHALL emit histogram: `memory.rlm.complexity_score`

#### Scenario: Execution metrics
- **WHEN** RLM execution completes
- **THEN** system SHALL emit histogram: `memory.rlm.execution.duration_ms`
- **AND** system SHALL emit histogram: `memory.rlm.execution.depth`
- **AND** system SHALL emit histogram: `memory.rlm.execution.tokens`

#### Scenario: Training metrics
- **WHEN** training step completes
- **THEN** system SHALL emit histogram: `memory.rlm.training.reward`
- **AND** system SHALL emit gauge: `memory.rlm.training.exploration_rate`

## MODIFIED Requirements

### Requirement: Memory Search Operation
The system SHALL provide semantic search across multiple memory layers with configurable parameters, tenant isolation, and automatic complexity-based routing to optimize retrieval for both simple and complex queries.

#### Scenario: Search across all accessible layers with tenant context
- **WHEN** searching memories with query, layer identifiers, and valid TenantContext
- **THEN** system SHALL validate TenantContext authorization
- **AND** system SHALL compute query complexity score
- **AND** system SHALL route to appropriate executor (standard or RLM) based on complexity
- **AND** system SHALL enforce tenant isolation (no cross-tenant results)
- **AND** system SHALL merge results by layer precedence
- **AND** system SHALL return results sorted by precedence then score

#### Scenario: Search with layer filter
- **WHEN** searching memories with specific layers parameter and TenantContext
- **THEN** system SHALL only search in specified layers within the tenant
- **AND** system SHALL skip other layers

#### Scenario: Search with threshold parameter
- **WHEN** searching memories with custom threshold and TenantContext
- **THEN** system SHALL only return results with score >= threshold
- **AND** system SHALL use threshold 0.7 if not specified
