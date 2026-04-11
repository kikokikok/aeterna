## MODIFIED Requirements

### Requirement: Honest Runtime Command Behavior
The system SHALL ensure user-facing CLI and service operations either execute real backend behavior or fail explicitly as unsupported.

#### Scenario: Real command execution
- **WHEN** a user invokes a runtime command for memory, sync, search, governance, administration, or identity management
- **THEN** the command SHALL execute real backend-backed logic for that operation
- **AND** the command SHALL return actual result data, status, or errors from the underlying system

#### Scenario: Unsupported command path
- **WHEN** a runtime command path is not implemented for the current mode
- **THEN** the command SHALL return an explicit unsupported error
- **AND** it SHALL NOT return simulated success, empty healthy state, or placeholder result payloads

#### Scenario: Connectivity failure is explicit
- **WHEN** a backend-facing CLI command cannot reach the configured server or cannot authenticate against it
- **THEN** the CLI SHALL return an explicit connectivity or authentication failure
- **AND** the command SHALL identify the active target profile or server URL involved in the failure
- **AND** the command SHALL NOT silently fall back to a fake or local-only success path

## ADDED Requirements

### Requirement: CLI Backend Connectivity Layer
The system SHALL provide a shared CLI connectivity layer for authenticated backend-facing commands.

#### Scenario: Shared client resolves target and credentials
- **WHEN** a backend-facing CLI command is executed
- **THEN** the CLI SHALL resolve the effective target profile, server URL, and stored credentials before issuing backend requests
- **AND** all backend-facing command groups SHALL use the same shared connectivity layer rather than ad hoc per-command wiring

#### Scenario: Offline-capable command reports degraded state honestly
- **WHEN** a CLI command uses an offline/deferred execution path because the server is unreachable
- **THEN** the command SHALL report the operation as queued, deferred, or degraded
- **AND** the command SHALL NOT report the operation as already committed on the remote server
