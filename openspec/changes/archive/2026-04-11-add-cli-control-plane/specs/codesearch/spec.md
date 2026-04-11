## MODIFIED Requirements

### Requirement: Request Repository via CLI
The system MUST allow users to request repository indexing via a supported CLI command surface.

#### Scenario: Requesting a remote repository through the supported CLI path
- **WHEN** a user executes the supported `aeterna code-search` repository request command
- **THEN** the command SHALL route through a supported backend path for repository request handling
- **AND** the command SHALL create or request the repository indexing operation rather than failing with a reference to a removed legacy binary

#### Scenario: Unsupported code-search mode is explicit
- **WHEN** the configured deployment or build does not support the requested code-search operation
- **THEN** the CLI SHALL return an explicit unsupported error describing the supported migration path
- **AND** it SHALL NOT present a dead command shell that only references removed legacy components
