## MODIFIED Requirements

### Requirement: Runtime Health Semantics
The system SHALL expose health endpoints whose status matches the supported runtime mode and dependency state.

#### Scenario: Live and ready checks
- **WHEN** runtime health endpoints are called
- **THEN** liveness SHALL report process viability
- **AND** readiness SHALL report whether required downstream dependencies and runtime components are available for the configured mode

#### Scenario: Degraded runtime state
- **WHEN** a required backend, auth provider, persistence layer, vector store, or session backing store is unavailable
- **THEN** the runtime SHALL report degraded or unready status
- **AND** operational output SHALL identify the failing component category
