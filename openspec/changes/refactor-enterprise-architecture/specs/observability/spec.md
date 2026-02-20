## ADDED Requirements
### Requirement: S3 Operations Observability
The system SHALL emit OpenTelemetry spans and metrics for all operations directed to the Iceberg Catalog and underlying object storage.

#### Scenario: Telemetry Export
- **WHEN** an Iceberg commit is performed
- **THEN** a `storage.iceberg.commit` span must be generated
- **AND** latency and error rate metrics must be exported to Prometheus