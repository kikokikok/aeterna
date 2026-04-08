# server-runtime Specification

## Purpose
TBD - created by archiving change add-server-runtime. Update Purpose after archive.
## Requirements
### Requirement: Server Process Lifecycle
The system SHALL provide a unified server process (`aeterna serve`) that composes all service libraries into a running binary, manages lifecycle (startup, readiness, graceful shutdown), and binds HTTP + metrics listeners.

#### Scenario: Server Startup
- **WHEN** `aeterna serve` is executed with valid configuration
- **THEN** the server SHALL bind an HTTP listener on the configured address (default `0.0.0.0:8080`)
- **AND** bind a metrics listener on the configured address (default `0.0.0.0:9090`)
- **AND** log `"Aeterna server ready"` when all listeners are bound

#### Scenario: Server Startup with Missing Required Infrastructure
- **WHEN** `aeterna serve` is executed without `AETERNA_POSTGRESQL_HOST`
- **THEN** the server SHALL exit with a non-zero code
- **AND** print an actionable error message listing the missing variables

#### Scenario: Server Startup with Missing Config Directory
- **WHEN** `aeterna serve` is executed and the config path does not exist
- **THEN** the server SHALL exit with a non-zero code
- **AND** suggest running `aeterna setup`

### Requirement: Service Composition
The server SHALL bootstrap and compose all core service dependencies into a shared application state accessible by all route handlers.

#### Scenario: Full Service Bootstrap
- **WHEN** the server starts with all required environment variables
- **THEN** the server SHALL initialize `MemoryManager` with embedding and LLM services
- **AND** initialize `KnowledgeRepository` connected to PostgreSQL
- **AND** initialize `StorageBackend` connected to the configured vector backend
- **AND** initialize `GovernanceEngine` with policy evaluation support
- **AND** initialize `McpServer` with all 36+ registered tools
- **AND** initialize `SyncManager` for memory-knowledge synchronization
- **AND** make all services available to route handlers via shared state

#### Scenario: Feature-Gated Subsystem Initialization
- **WHEN** `AETERNA_FEATURE_CCA=false`
- **THEN** the server SHALL skip CCA capability initialization (Context Architect, Note-Taking, Hindsight, Meta-Agent)
- **AND** log `"CCA capabilities disabled"` at info level
- **AND** MCP tools for CCA SHALL return a descriptive "feature disabled" error

#### Scenario: Optional LLM Service
- **WHEN** `AETERNA_LLM_PROVIDER=none`
- **THEN** the server SHALL start without LLM and embedding services
- **AND** `ReflectiveReasoner` SHALL be `None`
- **AND** features requiring LLM (semantic search enrichment, reflective reasoning) SHALL gracefully degrade

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

### Requirement: Health Endpoints
The server SHALL expose health, readiness, and liveness endpoints for Kubernetes probe integration.

#### Scenario: Liveness Check
- **WHEN** `GET /health` is called
- **THEN** the server SHALL return HTTP 200 with `{"status": "ok"}`
- **AND** the response SHALL NOT depend on backend connectivity
- **AND** the response time SHALL be under 10ms

#### Scenario: Readiness Check Success
- **WHEN** `GET /ready` is called and all required backends (PostgreSQL, vector store, Redis) are connected
- **THEN** the server SHALL return HTTP 200 with `{"status": "ready", "backends": {"postgres": "ok", "vector": "ok", "redis": "ok"}}`

#### Scenario: Readiness Check Failure
- **WHEN** `GET /ready` is called and PostgreSQL is unreachable
- **THEN** the server SHALL return HTTP 503 with `{"status": "not_ready", "backends": {"postgres": "error", "vector": "ok", "redis": "ok"}}`

### Requirement: Graceful Shutdown
The server SHALL implement coordinated graceful shutdown on receiving termination signals.

#### Scenario: SIGTERM Received
- **WHEN** the server process receives SIGTERM
- **THEN** the server SHALL stop accepting new connections
- **AND** drain all in-flight HTTP requests (up to 30s timeout)
- **AND** close WebSocket connections with close frames
- **AND** release database connection pool resources
- **AND** flush pending metrics
- **AND** exit with code 0

#### Scenario: SIGINT Received During Development
- **WHEN** the server process receives SIGINT (Ctrl+C)
- **THEN** the server SHALL perform the same graceful shutdown as SIGTERM

### Requirement: Metrics Endpoint
The server SHALL expose a Prometheus-compatible metrics endpoint on a dedicated port.

#### Scenario: Metrics Scrape
- **WHEN** `GET /metrics` is called on the metrics port (default 9090)
- **THEN** the server SHALL return HTTP 200 with Prometheus text format
- **AND** include process metrics (uptime, memory, CPU)
- **AND** include HTTP request metrics (total requests, latency histogram, status codes)
- **AND** include backend connection pool metrics

#### Scenario: Metrics Port Independence
- **WHEN** the main HTTP port (8080) is under heavy load
- **THEN** the metrics port (9090) SHALL remain responsive independently
- **AND** metrics scraping SHALL NOT compete with application traffic

### Requirement: Configuration from Environment
The server SHALL read all configuration from environment variables matching the Helm configmap schema.

#### Scenario: Standard Configuration
- **WHEN** the server starts
- **THEN** it SHALL read the following environment variables:
  - `AETERNA_DEPLOYMENT_MODE` (default: `kubernetes`)
  - `AETERNA_LOG_LEVEL` (default: `info`)
  - `AETERNA_LOG_FORMAT` (default: `json`)
  - `AETERNA_VECTOR_BACKEND` (default: `qdrant`)
  - `AETERNA_POSTGRESQL_HOST`, `AETERNA_POSTGRESQL_PORT`, `AETERNA_POSTGRESQL_DATABASE`
  - `AETERNA_POSTGRESQL_USERNAME`, `AETERNA_POSTGRESQL_PASSWORD`
  - `AETERNA_REDIS_HOST`, `AETERNA_REDIS_PORT`
  - `AETERNA_QDRANT_HOST`, `AETERNA_QDRANT_PORT`
  - `AETERNA_LLM_PROVIDER` and provider-specific variables
  - `AETERNA_FEATURE_CCA`, `AETERNA_FEATURE_RADKIT`, `AETERNA_FEATURE_RLM`, `AETERNA_FEATURE_REFLECTIVE`
  - `GOOGLE_APPLICATION_CREDENTIALS` (for Vertex AI)

#### Scenario: CLI Argument Override
- **WHEN** `aeterna serve --port 3000 --metrics-port 3001`
- **THEN** the server SHALL use port 3000 for HTTP and 3001 for metrics
- **AND** CLI arguments SHALL take precedence over environment variables

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

### Requirement: A2A Route Embedding
The server SHALL embed A2A agent protocol routes from the `agent-a2a` crate.

#### Scenario: Agent Card Discovery
- **WHEN** a client sends `GET /a2a/.well-known/agent.json`
- **THEN** the server SHALL return the agent card with capabilities, skills, and endpoint URL

#### Scenario: A2A Health
- **WHEN** a client sends `GET /a2a/health`
- **THEN** the server SHALL return `{"status": "ok"}`

### Requirement: Governance Dashboard API
The server SHALL mount the Governance Dashboard API endpoints from the `knowledge` crate.

#### Scenario: Dashboard Endpoint Availability
- **WHEN** the server is running
- **THEN** all 11 Governance Dashboard API endpoints SHALL be accessible under `/api/v1/`
- **AND** each endpoint SHALL use the existing utoipa-annotated handler functions

### Requirement: Knowledge API Endpoints
The server SHALL implement the knowledge API endpoints as defined in `project.md`, with `/api/v1/knowledge/*` as canonical routes and `/openspec/v1/knowledge/*` as compatibility aliases.

#### Scenario: Discovery Endpoint
- **WHEN** `GET /api/v1/knowledge` is called
- **THEN** the server SHALL return a list of available knowledge items

#### Scenario: Query Endpoint
- **WHEN** `POST /api/v1/knowledge/query` is called with a search body
- **THEN** the server SHALL return matching knowledge items with relevance scores

#### Scenario: Create Endpoint
- **WHEN** `POST /api/v1/knowledge/create` is called with a knowledge item body
- **THEN** the server SHALL create the item and return the created resource with ID

#### Scenario: Streaming Endpoint
- **WHEN** `GET /api/v1/knowledge/stream` is called
- **THEN** the server SHALL return an SSE stream of knowledge updates

### Requirement: WebSocket Sync Integration
The server SHALL mount the WebSocket sync server for real-time memory-knowledge push notifications.

#### Scenario: WebSocket Upgrade
- **WHEN** a client sends a WebSocket upgrade request to `/ws/sync`
- **THEN** the server SHALL upgrade the connection using the `WsServer` from the `sync` crate
- **AND** authenticate the connection using the provided token
- **AND** subscribe the client to the requested room

#### Scenario: Real-Time Push
- **WHEN** a memory or knowledge update occurs
- **THEN** connected WebSocket clients in the relevant room SHALL receive a push notification

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

### Requirement: Promotion Lifecycle API
The server SHALL provide first-class lifecycle APIs for knowledge promotion workflows.

#### Scenario: Preview promotion split
- **WHEN** a client requests a promotion preview
- **THEN** the server SHALL return suggested shared content and residual content
- **AND** the response SHALL include a suggested residual semantic role

#### Scenario: Create promotion request
- **WHEN** a client submits a promotion request
- **THEN** the server SHALL persist the request
- **AND** the server SHALL return a stable promotion request identifier

#### Scenario: Approve promotion request
- **WHEN** an authorized reviewer approves a promotion request
- **THEN** the server SHALL apply the reviewed decision
- **AND** the server SHALL create or update the resulting items and relations atomically or with safe compensation

### Requirement: Additive Backward Compatibility
The server SHALL preserve existing knowledge CRUD and governance routes during rollout.

#### Scenario: Legacy client continues to operate
- **WHEN** an older client continues to use the existing knowledge CRUD endpoints
- **THEN** the server SHALL continue to support those endpoints
- **AND** promotion-specific lifecycle behavior SHALL remain accessible through additive endpoints

### Requirement: Promotion Event Emission
The server SHALL emit promotion lifecycle events for audit and real-time monitoring.

#### Scenario: Emit event on promotion apply
- **WHEN** a promotion request is successfully applied
- **THEN** the server SHALL emit a `KnowledgePromotionApplied` event
- **AND** the event SHALL include source item ID, resulting item IDs, target layer, and request ID
