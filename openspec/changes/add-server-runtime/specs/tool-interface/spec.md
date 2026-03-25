## ADDED Requirements

### Requirement: MCP HTTP SSE Transport Binding
The MCP tool interface SHALL be accessible via HTTP Server-Sent Events (SSE) transport when the Aeterna server is running, in addition to the existing stdio transport.

#### Scenario: HTTP SSE Session Establishment
- **WHEN** a client sends `GET /mcp/sse` to the running server
- **THEN** the server SHALL open an SSE stream with `Content-Type: text/event-stream`
- **AND** send an `endpoint` event containing the URL for `POST /mcp/message`

#### Scenario: HTTP Tool Call
- **WHEN** a client sends a JSON-RPC `tools/call` request to `POST /mcp/message`
- **THEN** the McpServer dispatcher SHALL process the request
- **AND** return the tool result as a JSON-RPC response via the SSE stream or as an HTTP response body

#### Scenario: Stdio Transport Unchanged
- **WHEN** `aeterna mcp serve` is invoked (CLI command)
- **THEN** the stdio transport SHALL continue to work as before
- **AND** the HTTP transport SHALL have no effect on stdio behavior
