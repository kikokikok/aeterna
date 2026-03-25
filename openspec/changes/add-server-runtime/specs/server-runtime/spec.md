## ADDED Requirements

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
The server SHALL compose a unified Axum HTTP router by merging sub-routers from each service crate.

#### Scenario: Route Registration
- **WHEN** the server builds the HTTP router
- **THEN** the following route groups SHALL be registered:
  - `/health`, `/ready`, `/live` — health endpoints
  - `/openspec/v1/*` — OpenSpec compliance endpoints
  - `/api/v1/*` — Governance Dashboard API endpoints
  - `/mcp/*` — MCP HTTP/SSE transport endpoints
  - `/a2a/*` — A2A agent protocol endpoints
  - `/ws/*` — WebSocket sync endpoints
  - `/webhooks/*` — IDP webhook endpoints

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

### Requirement: OpenSpec Compliance Endpoints
The server SHALL implement the OpenSpec v1 protocol endpoints as defined in `project.md`.

#### Scenario: Discovery Endpoint
- **WHEN** `GET /openspec/v1/knowledge` is called
- **THEN** the server SHALL return a list of available knowledge items

#### Scenario: Query Endpoint
- **WHEN** `POST /openspec/v1/knowledge/query` is called with a search body
- **THEN** the server SHALL return matching knowledge items with relevance scores

#### Scenario: Create Endpoint
- **WHEN** `POST /openspec/v1/knowledge/create` is called with a knowledge item body
- **THEN** the server SHALL create the item and return the created resource with ID

#### Scenario: Streaming Endpoint
- **WHEN** `GET /openspec/v1/knowledge/stream` is called
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
