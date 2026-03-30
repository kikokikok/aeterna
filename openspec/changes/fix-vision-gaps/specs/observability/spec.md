## ADDED Requirements

### Requirement: SLO Monitoring

The system SHALL monitor Service Level Objectives (SLOs) for all critical operations and alert when error budgets are at risk.

#### Scenario: Memory search latency SLO
- **WHEN** the P95 latency for `memory_search` exceeds 200ms over a 5-minute window
- **THEN** the system SHALL emit a warning alert
- **AND** the SLO dashboard SHALL show the degradation

#### Scenario: API availability SLO
- **WHEN** the API error rate exceeds 1% over a 15-minute window
- **THEN** the system SHALL emit a critical alert
- **AND** the error budget burn rate SHALL be tracked

### Requirement: Distributed Tracing Integration

The system SHALL propagate OpenTelemetry trace context across ALL crates, not just the observability crate.

#### Scenario: End-to-end request trace
- **WHEN** an MCP tool request is received
- **THEN** a trace SHALL span from the HTTP handler through memory/knowledge/governance layers to storage
- **AND** each crate SHALL emit spans with relevant attributes (layer, tenant_id, operation)

#### Scenario: Cross-service trace propagation
- **WHEN** Aeterna calls the OPAL fetcher for entity data
- **THEN** the trace context SHALL be propagated via HTTP headers
- **AND** the OPAL fetcher's spans SHALL appear as children of the Aeterna request trace
