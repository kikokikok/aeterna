## MODIFIED Requirements

### Requirement: MCP Server Alternative

The system SHALL provide an MCP server as an alternative integration method for remote or hybrid deployments.

The MCP server MUST:
- Support stdio transport for local use
- Support HTTP transport for remote use
- Expose all 8 Aeterna tools via MCP protocol
- Expose knowledge and memory resources

#### Scenario: MCP stdio transport
- **WHEN** MCP server is configured with local type
- **THEN** OpenCode SHALL spawn the `aeterna-mcp` process
- **AND** communicate via stdin/stdout using JSON-RPC

#### Scenario: MCP HTTP transport
- **WHEN** MCP server is configured with remote type
- **THEN** OpenCode SHALL connect to the configured URL
- **AND** authenticate using Bearer token

#### Scenario: MCP tool invocation
- **WHEN** OpenCode invokes an Aeterna tool via MCP
- **THEN** the MCP server SHALL translate to internal operation
- **AND** return results in MCP-compliant format

#### Scenario: MCP HTTP Transport via Server Runtime
- **WHEN** the Aeterna server is running with `aeterna serve`
- **THEN** the MCP HTTP transport SHALL be served on the main HTTP port at `/mcp/*`
- **AND** the transport SHALL follow the MCP 2024-11-05 streamable HTTP spec (SSE)
- **AND** the MCP server process management (health, restart) is handled by the server runtime rather than a standalone supervisor

### Requirement: MCP Server Health Management (OC-H5)
The system SHALL implement robust MCP server process management.

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

#### Scenario: Embedded MCP in Server Runtime
- **WHEN** the Aeterna server is running with `aeterna serve`
- **THEN** the MCP server health is implicitly managed by the server process lifecycle
- **AND** MCP health is reflected in the `/ready` endpoint backend checks
- **AND** standalone supervisor pattern applies only to the `aeterna mcp serve` (stdio) command, not the embedded HTTP transport
