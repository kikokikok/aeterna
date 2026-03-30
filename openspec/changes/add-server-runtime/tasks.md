## 1. Workspace and Dependencies

- [x] 1.1 Add `axum`, `tower`, `tower-http` to workspace shared dependencies in root `Cargo.toml`
- [x] 1.2 Add `axum`, `tower`, `tower-http`, `tokio` (with `signal` feature), `hyper` dependencies to `cli/Cargo.toml`
- [x] 1.3 Add workspace crate dependencies to `cli/Cargo.toml`: `knowledge`, `tools`, `sync`, `storage`, `config`, `observability`, `agent-a2a`, `idp-sync`, `mk_core`

## 2. Application State and Bootstrap

- [x] 2.1 Create `cli/src/server/mod.rs` module with `AppState` struct holding all service dependencies as `Arc<dyn Trait>`
- [x] 2.2 Create `cli/src/server/bootstrap.rs` with ordered initialization sequence: config → tracing → postgres → vector store → redis → memory → knowledge → governance → mcp → sync
- [x] 2.3 Implement fail-fast validation for required env vars (`AETERNA_POSTGRESQL_HOST`, `AETERNA_POSTGRESQL_DATABASE`, `AETERNA_REDIS_HOST`)
- [x] 2.4 Implement feature-gated subsystem initialization for `AETERNA_FEATURE_CCA`, `AETERNA_FEATURE_RADKIT`, `AETERNA_FEATURE_RLM`, `AETERNA_FEATURE_REFLECTIVE`
- [x] 2.5 Write tests for `AppState` construction with various feature flag combinations

## 3. Health Endpoints

- [x] 3.1 Create `cli/src/server/health.rs` with `/health`, `/ready`, `/live` route handlers
- [x] 3.2 Implement liveness check (stateless, returns 200 immediately)
- [x] 3.3 Implement readiness check (verifies Postgres, vector store, Redis connectivity; returns 200 or 503 with backend status JSON)
- [x] 3.4 Write tests for health endpoints: healthy state, degraded state (one backend down), all backends down

## 4. Metrics Endpoint

- [x] 4.1 Create `cli/src/server/metrics.rs` with Prometheus exporter setup using `metrics-exporter-prometheus`
- [x] 4.2 Implement dedicated metrics listener binding on configurable port (default 9090) with single `GET /metrics` route
- [x] 4.3 Register HTTP request metrics (counter, latency histogram, status codes) via tower middleware
- [x] 4.4 Write tests for metrics endpoint rendering

## 5. Router Composition

- [x] 5.1 Create `cli/src/server/router.rs` that builds the unified Axum router by merging sub-routers
- [x] 5.2 Mount health routes at `/health`, `/ready`, `/live`
- [x] 5.3 Mount Governance Dashboard API from `knowledge::api` at `/api/v1/` — refactor `knowledge/src/api.rs` to export `pub fn router(state) -> Router`
- [x] 5.4 Mount knowledge API endpoints at `/api/v1/knowledge` (with `/openspec/v1/knowledge` compatibility alias) (discovery, query, create, update, delete, batch, stream, metadata)
- [x] 5.5 Mount MCP HTTP/SSE transport at `/mcp/` (session init, message handler)
- [x] 5.6 Mount A2A routes from `agent-a2a::create_router()` at `/a2a/`
- [x] 5.7 Mount WebSocket sync from `sync::websocket::WsServer` at `/ws/`
- [x] 5.8 Mount IDP webhook routes from `idp-sync` at `/webhooks/`
- [x] 5.9 Add tower middleware stack: request tracing, compression, CORS, request ID
- [x] 5.10 Add 404 fallback handler returning JSON error body

## 6. MCP HTTP/SSE Transport

- [x] 6.1 Create `cli/src/server/mcp_transport.rs` implementing MCP 2024-11-05 streamable HTTP transport
- [x] 6.2 Implement `GET /mcp/sse` — SSE stream with `endpoint` event
- [x] 6.3 Implement `POST /mcp/message` — JSON-RPC request routing to `McpServer` dispatcher
- [x] 6.4 Write tests for MCP session lifecycle: connect, list tools, call tool, disconnect

## 7. OpenSpec Compliance Endpoints

- [x] 7.1 Create `cli/src/server/knowledge_api.rs` implementing the 8 knowledge API endpoints with OpenSpec compatibility alias
- [x] 7.2 Implement `GET /api/v1/knowledge` (discovery; keep `/openspec/v1/knowledge` compatibility alias)
- [x] 7.3 Implement `POST /api/v1/knowledge/query` (search with relevance)
- [x] 7.4 Implement `POST /api/v1/knowledge/create` (create knowledge item)
- [x] 7.5 Implement `PUT /api/v1/knowledge/{id}` (update)
- [x] 7.6 Implement `DELETE /api/v1/knowledge/{id}` (delete)
- [x] 7.7 Implement `POST /api/v1/knowledge/batch` (batch operations)
- [x] 7.8 Implement `GET /api/v1/knowledge/stream` (SSE stream of updates)
- [x] 7.9 Implement `GET /api/v1/knowledge/{id}/metadata` (metadata)
- [x] 7.10 Write tests for each endpoint with mock knowledge repository

## 8. Knowledge API Refactoring

- [x] 8.1 Refactor `knowledge/src/api.rs` — add `pub fn router(state: AppState) -> Router` that mounts all 11 existing utoipa-annotated handlers onto an Axum router
- [x] 8.2 Ensure existing function signatures remain public and unchanged (backward compatible)
- [x] 8.3 Write tests for the knowledge API router with mock state

## 9. Serve Command Rewrite

- [x] 9.1 Rewrite `cli/src/commands/serve.rs` `run()` to be `async` — replace the stub `anyhow::bail!` with real server startup
- [x] 9.2 Call bootstrap to create `AppState`
- [x] 9.3 Build unified router and metrics router
- [x] 9.4 Bind main HTTP listener on `args.bind:args.port`
- [x] 9.5 Bind metrics listener on `args.bind:args.metrics_port`
- [x] 9.6 Implement graceful shutdown via `tokio::signal` (SIGTERM + SIGINT) with shared watch channel
- [x] 9.7 Wire both listeners with graceful shutdown support
- [x] 9.8 Update existing `serve.rs` tests to test the new behavior (startup validation, signal handling)

## 10. Dockerfile and Container

- [x] 10.1 Update `Dockerfile` HEALTHCHECK from CLI probe to `curl http://localhost:8080/health`
- [x] 10.2 Add `curl` to Dockerfile runtime dependencies (or use a Rust-based health probe)
- [x] 10.3 Verify `cargo build --release --package aeterna` still compiles cleanly with new dependencies

## 11. Integration Tests

- [x] 11.1 Write integration test: server starts and `/health` returns 200
- [x] 11.2 Write integration test: server starts and `/metrics` on port 9090 returns Prometheus format
- [x] 11.3 Write integration test: `/ready` returns 503 when backend is not configured
- [x] 11.4 Write integration test: MCP tool list via HTTP returns all registered tools
- [x] 11.5 Write integration test: A2A agent card at `/a2a/.well-known/agent.json` returns valid JSON
- [x] 11.6 Write integration test: graceful shutdown completes within timeout
- [ ] 11.7 Ensure test coverage meets 80% minimum threshold for new code
