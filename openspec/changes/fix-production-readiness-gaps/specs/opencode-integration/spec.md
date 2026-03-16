## MODIFIED Requirements

### Requirement: MCP Server Health Management (OC-H5)
The system SHALL implement robust MCP server process management with runtime behavior aligned to actual backend connectivity and supported integration paths.

#### Scenario: Health Check Implementation
- **WHEN** MCP server is running
- **THEN** it MUST expose health check endpoint
- **AND** health check MUST verify backend connectivity

#### Scenario: Supervisor Pattern
- **WHEN** MCP server process crashes
- **THEN** the system MUST implement automatic restart
- **AND** restart MUST use exponential backoff (max 3 retries, then alert)

#### Scenario: Crash Recovery
- **WHEN** MCP server recovers from crash
- **THEN** it MUST restore in-flight request state if possible
- **AND** emit metrics for crash events

#### Scenario: Unsupported integration path
- **WHEN** a documented plugin or MCP integration mode is not fully implemented for the current build or deployment mode
- **THEN** the system SHALL fail explicitly during initialization
- **AND** the system SHALL NOT present the mode as healthy or ready
- **AND** documentation and examples SHALL identify the supported integration modes
