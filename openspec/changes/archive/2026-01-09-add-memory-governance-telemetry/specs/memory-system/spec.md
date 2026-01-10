## ADDED Requirements

### Requirement: PII Redaction
The system SHALL redact personally identifiable information (PII) from memory content before it is promoted to persistent layers.

#### Scenario: Redact email from content
- **WHEN** a memory is being evaluated for promotion
- **AND** the content contains an email address (e.g., "user@example.com")
- **THEN** the system SHALL replace the email with `[REDACTED]`

### Requirement: Sensitivity Check
The system SHALL prevent promotion of memories marked as sensitive or private.

#### Scenario: Block promotion of sensitive memory
- **WHEN** a memory is marked as `sensitive: true` or `private: true` in metadata
- **THEN** the system SHALL NOT promote this memory to higher layers, regardless of its importance score.

### Requirement: Performance Telemetry
The system SHALL track and emit metrics for key memory operations.

#### Scenario: Track search latency
- **WHEN** a semantic search is performed
- **THEN** the system SHALL record the operation latency and emit it to the configured metrics provider.
