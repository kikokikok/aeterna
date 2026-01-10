## MODIFIED Requirements
### Requirement: PII Redaction
The system SHALL redact personally identifiable information (PII) from memory content before it is promoted to persistent layers.

#### Scenario: Redact sensitive info during promotion
- **WHEN** a memory is being evaluated for promotion
- **THEN** the system SHALL apply centralized PII redaction (email, phone) to the content
- **AND** record telemetry for the redaction event.

### Requirement: Sensitivity Check
The system SHALL prevent promotion of memories marked as sensitive or private.

#### Scenario: Block promotion of sensitive memory
- **WHEN** a memory is marked as `sensitive: true` or `private: true` in metadata
- **THEN** the system SHALL NOT promote this memory to higher layers
- **AND** record telemetry for the promotion block.
