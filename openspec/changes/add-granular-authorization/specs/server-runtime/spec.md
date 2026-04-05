## ADDED Requirements

### Requirement: Global Authentication Tower Layer
The server SHALL enforce authentication via a global Axum tower layer applied to all protected route groups, ensuring consistent authentication before any handler executes.

#### Scenario: Protected routes require authentication
- **WHEN** a request arrives at any route under `/api/v1/*` or `/mcp/*`
- **AND** authentication is enabled
- **AND** the route is not in the auth-exempt list (`/api/v1/auth/plugin/*`)
- **THEN** the authentication layer SHALL validate the bearer token before the request reaches any handler
- **AND** the layer SHALL inject the authenticated `TenantContext` as a request extension

#### Scenario: Disabled auth passes through
- **WHEN** authentication is disabled (`pluginAuth.enabled: false`)
- **THEN** the authentication layer SHALL pass all requests through without token validation
- **AND** the system SHALL use legacy header-based identity extraction for backward compatibility

#### Scenario: Health routes excluded from auth
- **WHEN** a request arrives at `/health`, `/live`, or `/ready`
- **THEN** the authentication layer SHALL NOT require a bearer token regardless of authentication configuration

### Requirement: MCP Route Authentication
The server SHALL require authentication on MCP transport routes (`/mcp/sse`, `/mcp/message`) when authentication is enabled, closing the current zero-auth gap.

#### Scenario: MCP SSE connection requires auth
- **WHEN** a client connects to `/mcp/sse`
- **AND** authentication is enabled
- **THEN** the server SHALL validate the bearer token before establishing the SSE stream
- **AND** the server SHALL reject unauthenticated connections with 401 Unauthorized

#### Scenario: MCP message requires auth
- **WHEN** a client sends a JSON-RPC request to `POST /mcp/message`
- **AND** authentication is enabled
- **THEN** the server SHALL validate the bearer token and derive tenant context before dispatching the tool call

### Requirement: Admin Sync GitHub Authentication
The server SHALL require PlatformAdmin authentication on `POST /api/v1/admin/sync/github`, closing the current unprotected endpoint gap.

#### Scenario: Admin sync requires PlatformAdmin
- **WHEN** a request arrives at `POST /api/v1/admin/sync/github`
- **AND** authentication is enabled
- **THEN** the server SHALL require a valid bearer token with PlatformAdmin role
- **AND** the server SHALL reject requests from non-PlatformAdmin users with 403 Forbidden

## MODIFIED Requirements

### Requirement: HTTP Router Composition
The server SHALL compose a unified Axum HTTP router by merging sub-routers from each service crate, with a global authentication tower layer applied to all protected route groups.

#### Scenario: Route Registration
- **WHEN** the server builds the HTTP router
- **THEN** the following route groups SHALL be registered:
  - `/health`, `/ready`, `/live` — health endpoints (auth-exempt)
  - `/api/v1/auth/plugin/*` — auth bootstrap endpoints (auth-exempt)
  - `/api/v1/knowledge/*` — canonical knowledge API endpoints (auth-protected)
  - `/openspec/v1/knowledge/*` — compatibility alias for knowledge API endpoints (auth-protected)
  - `/api/v1/governance/*` — Governance Dashboard API endpoints (auth-protected)
  - `/api/v1/admin/*` — Admin control plane endpoints (auth-protected, role-enforced)
  - `/mcp/*` — MCP HTTP/SSE transport endpoints (auth-protected)
  - `/a2a/*` — A2A agent protocol endpoints (auth-protected)
  - `/ws/*` — WebSocket sync endpoints (auth-protected)
  - `/webhooks/*` — IDP webhook endpoints (HMAC-authenticated)

#### Scenario: Unknown Route
- **WHEN** a request targets a path not matching any registered route
- **THEN** the server SHALL return HTTP 404 with a JSON error body

### Requirement: MCP HTTP Transport
The server SHALL expose MCP tools via the MCP 2024-11-05 streamable HTTP transport (Server-Sent Events), with authentication enforcement when auth is enabled.

#### Scenario: MCP Session Initialization
- **WHEN** a client sends `GET /mcp/sse`
- **AND** the client presents a valid bearer token (when auth is enabled)
- **THEN** the server SHALL open an SSE stream
- **AND** send an `endpoint` event with the message URL

#### Scenario: MCP Tool Invocation via HTTP
- **WHEN** a client sends a JSON-RPC `tools/call` request to `POST /mcp/message`
- **AND** the client presents a valid bearer token (when auth is enabled)
- **THEN** the server SHALL route the request to the `McpServer` dispatcher
- **AND** the dispatcher SHALL evaluate the tool's corresponding Cedar action before execution
- **AND** return the tool result as a JSON-RPC response

#### Scenario: MCP Tool List via HTTP
- **WHEN** a client sends a JSON-RPC `tools/list` request to `POST /mcp/message`
- **THEN** the server SHALL return all registered MCP tools with their schemas
