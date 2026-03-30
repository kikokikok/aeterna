# observability Specification

## Purpose
TBD - created by archiving change add-production-improvements. Update Purpose after archive.
## Requirements
### Requirement: Distributed Tracing with Correlation

The system SHALL provide end-to-end tracing with correlation across all services using live runtime signals rather than placeholder metrics or synthetic health success.

#### Scenario: Trace Propagation
- **WHEN** user makes API request
- **THEN** system generates trace ID
- **AND** propagates trace ID through all service calls
- **AND** includes trace ID in all log entries
- **AND** includes trace ID in all metrics
- **AND** stores trace spans in Jaeger/Zipkin

### Requirement: Cost Tracking and Budgeting

The system SHALL track operational costs per tenant with budget enforcement.

#### Scenario: Embedding Cost Tracking
- **WHEN** tenant generates embeddings
- **THEN** system records:
  - Number of API calls
  - Total tokens processed
  - Cost per call
  - Cumulative monthly cost
- **AND** stores in cost tracking database

#### Scenario: Budget Alert
- **WHEN** tenant embedding costs exceed 80% of budget
- **THEN** system sends alert to tenant admin
- **AND** logs budget warning
- **AND** optionally rate-limits further embedding requests

#### Scenario: Cost Dashboard
- **WHEN** admin views cost dashboard
- **THEN** system displays:
  - Total costs (embeddings, storage, compute)
  - Per-tenant breakdown
  - Cost trends over time
  - Top 10 cost consumers
  - Projected monthly costs

### Requirement: Anomaly Detection

The system SHALL detect anomalies in key metrics and alert operators.

#### Scenario: Latency Spike Detection
- **WHEN** p95 search latency exceeds baseline by 2x
- **AND** condition persists for 5 minutes
- **THEN** system triggers alert
- **AND** includes context:
  - Affected service
  - Metric baseline vs current
  - Recent changes/deployments
  - Suggested investigations

#### Scenario: Error Rate Anomaly
- **WHEN** error rate increases from 0.1% to 5%
- **AND** condition persists for 2 minutes
- **THEN** system triggers critical alert
- **AND** auto-creates incident
- **AND** pages on-call engineer

### Requirement: SLO Monitoring

The system SHALL monitor and report on Service Level Objectives.

#### Scenario: Availability SLO
- **WHEN** calculating monthly availability
- **THEN** system measures:
  - Total uptime vs downtime
  - Planned maintenance windows (excluded)
  - Unplanned outages
- **AND** reports SLO compliance (target: 99.9%)
- **AND** calculates error budget remaining

#### Scenario: Latency SLO
- **WHEN** measuring search latency SLO
- **THEN** system tracks:
  - p50 latency (target: < 100ms)
  - p95 latency (target: < 200ms)
  - p99 latency (target: < 500ms)
- **AND** reports percentage of requests meeting SLO
- **AND** alerts when SLO breached

### Requirement: Comprehensive Dashboards

The system SHALL provide role-specific dashboards for different stakeholders.

#### Scenario: Operations Dashboard
- **WHEN** operations engineer opens dashboard
- **THEN** system displays:
  - Service health (all services)
  - Active alerts and incidents
  - Resource utilization (CPU, memory, disk)
  - Key metrics (QPS, latency, errors)
  - Recent deployments

#### Scenario: Cost Management Dashboard
- **WHEN** finance user opens cost dashboard
- **THEN** system displays:
  - Current month costs (total and by category)
  - Top cost drivers
  - Budget vs actual
  - Cost trends (last 12 months)
  - Forecasted costs

#### Scenario: Performance Dashboard
- **WHEN** developer opens performance dashboard
- **THEN** system displays:
  - Request latency distributions
  - Cache hit rates
  - Database query performance
  - Vector search performance
  - Memory promotion rates

#### Scenario: Server Runtime Metrics Availability
- **WHEN** the Aeterna server is running
- **THEN** a dedicated metrics listener on port 9090 SHALL serve Prometheus-format metrics at `GET /metrics`
- **AND** the metrics SHALL include HTTP request counters, latency histograms, status code distributions, and backend connection pool gauges
- **AND** the metrics listener SHALL be independent of the main application port to avoid interference with application traffic

### Requirement: S3 Operations Observability
The system SHALL emit OpenTelemetry spans and metrics for all operations directed to the Iceberg Catalog and underlying object storage.

#### Scenario: Telemetry Export
- **WHEN** an Iceberg commit is performed
- **THEN** a `storage.iceberg.commit` span must be generated
- **AND** latency and error rate metrics must be exported to Prometheus

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

