## MODIFIED Requirements

### Requirement: Authenticated CLI Control Plane
The system SHALL provide the `aeterna` binary as the supported authenticated control plane for Aeterna users and operators.

#### Scenario: Connected operator command uses real backend behavior
- **WHEN** an operator runs a shipped admin, governance, organization, user, or team command against a configured Aeterna server and the backend path exists
- **THEN** the CLI SHALL execute the real backend-backed operation for that command
- **AND** the CLI SHALL return actual result data or backend errors rather than placeholder responses

#### Scenario: Unsupported operator path fails explicitly
- **WHEN** a shipped operator command path is not supported for the current deployment or integration mode
- **THEN** the CLI SHALL return an explicit unsupported error
- **AND** the CLI SHALL NOT return fabricated analysis, example rows, or placeholder healthy status on the live path
