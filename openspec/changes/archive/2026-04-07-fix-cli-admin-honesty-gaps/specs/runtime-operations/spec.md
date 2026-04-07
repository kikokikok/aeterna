## MODIFIED Requirements

### Requirement: Honest Runtime Command Behavior
The system SHALL ensure user-facing CLI and service operations either execute real backend behavior or fail explicitly as unsupported.

#### Scenario: Real command execution
- **WHEN** a user invokes a runtime command for memory, sync, search, governance, administration, organization, team, or user management and the backend path exists
- **THEN** the command SHALL execute real backend-backed logic for that operation
- **AND** the command SHALL return actual result data, status, or errors from the underlying system

#### Scenario: Unsupported command path
- **WHEN** a runtime command path is not implemented for the current mode
- **THEN** the command SHALL return an explicit unsupported error
- **AND** it SHALL NOT return simulated success, example result rows, empty healthy state, or placeholder result payloads
