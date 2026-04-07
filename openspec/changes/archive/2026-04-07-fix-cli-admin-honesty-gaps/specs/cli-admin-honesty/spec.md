## ADDED Requirements

### Requirement: Honest Live Admin Command Semantics
The CLI SHALL ensure that live admin/operator command paths return real persisted results, explicit unsupported failures, or actual validation/authz errors.

#### Scenario: Live admin command with backend support
- **WHEN** an operator runs a shipped admin command whose backend path is supported
- **THEN** the CLI SHALL execute the real backend-backed operation
- **AND** it SHALL return actual persisted result data or the backend's actual error response

#### Scenario: Live admin command without backend support
- **WHEN** an operator runs a shipped admin command whose backend path is not yet supported
- **THEN** the CLI SHALL return an explicit unsupported error with failing exit semantics
- **AND** it SHALL NOT print fabricated success output or example result rows as if they were real data

### Requirement: Honest Local Context Update Flows
The CLI SHALL execute real local state changes for locally supported context-selection flows.

#### Scenario: Local context selection updates persisted config
- **WHEN** a user runs a locally supported context-selection command such as choosing a default org or team
- **THEN** the CLI SHALL update the documented local config or context file for that selection
- **AND** it SHALL NOT stop at printing what it would have written when no server call is required
