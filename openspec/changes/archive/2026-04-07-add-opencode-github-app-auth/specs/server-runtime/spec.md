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

### Requirement: Plugin Bearer Token Validation
The server SHALL validate Aeterna-issued plugin bearer tokens on plugin-facing API routes before serving authenticated requests.

#### Scenario: Valid plugin token is accepted
- **WHEN** a plugin-facing API route receives a bearer token issued for authenticated plugin access
- **THEN** the server SHALL validate the token issuer, expiry, and required claims
- **AND** the request SHALL proceed with authenticated request context derived from the token claims

#### Scenario: Invalid plugin token is rejected
- **WHEN** a plugin-facing API route receives a missing, malformed, expired, or untrusted bearer token
- **THEN** the server SHALL reject the request with an unauthorized response
- **AND** the route SHALL NOT continue using default or anonymous identity context

### Requirement: Plugin Auth Endpoints
The server SHALL expose dedicated authentication endpoints for the OpenCode plugin to establish, refresh, and end authenticated plugin sessions.

#### Scenario: Plugin auth bootstrap
- **WHEN** the plugin completes the supported upstream sign-in flow and calls the auth bootstrap endpoint
- **THEN** the server SHALL validate the upstream identity result
- **AND** the server SHALL issue Aeterna plugin session credentials scoped for plugin API access

#### Scenario: Plugin token refresh
- **WHEN** the plugin calls the refresh endpoint with a valid refresh credential
- **THEN** the server SHALL issue a new access token for the same authenticated plugin session
- **AND** the server SHALL reject refresh attempts that are revoked, expired, or otherwise invalid
