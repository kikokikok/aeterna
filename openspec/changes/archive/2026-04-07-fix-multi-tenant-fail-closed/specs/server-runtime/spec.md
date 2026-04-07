## MODIFIED Requirements

### Requirement: HTTP Router Composition
The server SHALL compose a unified Axum HTTP router by merging sub-routers from each service crate.

#### Scenario: Route Registration
- **WHEN** the server builds the HTTP router
- **THEN** the following route groups SHALL be registered:
  - `/health`, `/ready`, `/live` — health endpoints
  - `/api/v1/knowledge/*` — canonical knowledge API endpoints
  - `/openspec/v1/knowledge/*` — compatibility alias for knowledge API endpoints
  - `/api/v1/governance/*` — Governance Dashboard API endpoints
  - `/api/v1/auth/*` — plugin authentication and token lifecycle endpoints
  - `/mcp/*` — MCP HTTP/SSE transport endpoints
  - `/a2a/*` — A2A agent protocol endpoints
  - `/ws/*` — WebSocket sync endpoints
  - `/webhooks/*` — IDP webhook endpoints

#### Scenario: Unknown Route
- **WHEN** a request targets a path not matching any registered route
- **THEN** the server SHALL return HTTP 404 with a JSON error body

#### Scenario: Mounted route requires verified tenant context
- **WHEN** a mounted API or transport route performs tenant-scoped operations in a production-capable mode
- **THEN** the route SHALL derive or verify tenant context from authenticated identity or trusted boundary data before invoking downstream logic
- **AND** the route SHALL reject requests whose tenant context cannot be verified

### Requirement: MCP HTTP Transport
The server SHALL expose MCP tools via the MCP 2024-11-05 streamable HTTP transport (Server-Sent Events).

#### Scenario: MCP Session Initialization
- **WHEN** a client sends `GET /mcp/sse`
- **THEN** the server SHALL open an SSE stream
- **AND** send an `endpoint` event with the message URL

#### Scenario: MCP Tool Invocation via HTTP
- **WHEN** a client sends a JSON-RPC `tools/call` request to `POST /mcp/message`
- **THEN** the server SHALL route the request to the `McpServer` dispatcher
- **AND** return the tool result as a JSON-RPC response
- **AND** the response SHALL stream via the SSE connection if one is active

#### Scenario: MCP Tool List via HTTP
- **WHEN** a client sends a JSON-RPC `tools/list` request to `POST /mcp/message`
- **THEN** the server SHALL return all registered MCP tools with their schemas

#### Scenario: MCP tenant context must match authenticated caller
- **WHEN** a caller provides `tenantContext` in an MCP request payload
- **THEN** the server SHALL verify that the provided tenant scope is authorized for the authenticated caller
- **AND** the request SHALL be rejected if the payload tenant context does not match the authenticated tenant identity
