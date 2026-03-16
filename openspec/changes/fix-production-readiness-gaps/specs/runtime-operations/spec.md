## ADDED Requirements

### Requirement: Supported Runtime Entrypoints
The system SHALL ship only container and operational entrypoints that invoke supported commands.

#### Scenario: Default container start
- **WHEN** the published Aeterna container starts with its default command
- **THEN** it SHALL execute a supported runtime command or binary mode
- **AND** startup SHALL fail with a clear error if required configuration is missing
- **AND** the image SHALL NOT default to a nonexistent or unsupported command

#### Scenario: Migration command invocation
- **WHEN** deployment automation invokes the database migration command
- **THEN** it SHALL call the supported command path and flags exactly as implemented by the CLI
- **AND** migration automation SHALL fail clearly on invalid invocation

### Requirement: Honest Runtime Command Behavior
The system SHALL ensure user-facing CLI and service operations either execute real backend behavior or fail explicitly as unsupported.

#### Scenario: Real command execution
- **WHEN** a user invokes a runtime command for memory, sync, search, governance, or administration
- **THEN** the command SHALL execute real backend-backed logic for that operation
- **AND** the command SHALL return actual result data, status, or errors from the underlying system

#### Scenario: Unsupported command path
- **WHEN** a runtime command path is not implemented for the current mode
- **THEN** the command SHALL return an explicit unsupported error
- **AND** it SHALL NOT return simulated success, empty healthy state, or placeholder result payloads

### Requirement: Runtime Persistence and State Handling
The system SHALL persist runtime thread or session state for features that advertise persistence.

#### Scenario: Thread persistence enabled
- **WHEN** agent or A2A thread persistence is configured
- **THEN** thread retrieval, update, expiration, and recovery SHALL use a real persistence backend
- **AND** runtime restarts SHALL preserve persisted state according to configuration

#### Scenario: Persistence unavailable
- **WHEN** required persistence is unavailable
- **THEN** the affected runtime path SHALL report degraded or unavailable status
- **AND** the system SHALL NOT acknowledge persistence operations as successful when no state was stored

### Requirement: Runtime Health Semantics
The system SHALL expose health endpoints whose status matches the supported runtime mode and dependency state.

#### Scenario: Live and ready checks
- **WHEN** runtime health endpoints are called
- **THEN** liveness SHALL report process viability
- **AND** readiness SHALL report whether required downstream dependencies and runtime components are available for the configured mode

#### Scenario: Degraded runtime state
- **WHEN** a required backend, auth provider, or persistence layer is unavailable
- **THEN** the runtime SHALL report degraded or unready status
- **AND** operational output SHALL identify the failing component category
