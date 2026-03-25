## MODIFIED Requirements

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
