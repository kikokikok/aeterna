## MODIFIED Requirements

### Requirement: Distributed Tracing with Correlation

The system SHALL provide end-to-end tracing with correlation across all services using live runtime signals rather than placeholder metrics or synthetic health success.

#### Scenario: Trace Propagation
- **WHEN** user makes API request
- **THEN** system generates trace ID
- **AND** propagates trace ID through all service calls
- **AND** includes trace ID in all log entries
- **AND** includes trace ID in all metrics
- **AND** stores trace spans in Jaeger/Zipkin

## ADDED Requirements

### Requirement: Service Health and Metrics Integrity
The system SHALL expose health and metrics outputs that reflect actual process and dependency state.

#### Scenario: Dependency-aware readiness
- **WHEN** a readiness or health endpoint reports success for a runtime service
- **THEN** the service SHALL have verified required dependencies for that mode of operation
- **AND** placeholder success responses SHALL NOT be returned when backend connectivity or required runtime state is unavailable

#### Scenario: Real metrics emission
- **WHEN** metrics endpoints are scraped
- **THEN** counters, gauges, and histograms SHALL be backed by live runtime instrumentation
- **AND** static placeholder metric payloads SHALL NOT be used for production-capable services

#### Scenario: Crash and restart visibility
- **WHEN** a runtime integration process crashes or is restarted
- **THEN** the system SHALL emit metrics and logs describing the crash and recovery attempt
- **AND** health output SHALL reflect the degraded state until recovery is complete
