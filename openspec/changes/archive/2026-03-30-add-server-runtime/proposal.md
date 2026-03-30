## Why

The `aeterna serve` command is an intentional stub that bails with `"The Aeterna HTTP API server is not yet integrated into this binary."` (introduced by the `fix-production-readiness-gaps` change). All core service libraries — MCP tools, A2A agent, Governance Dashboard API, WebSocket sync, IDP webhooks, OPAL data fetcher — are fully implemented as library crates but **no binary composes them into a running server process**. The Helm chart, Dockerfile, and Kubernetes probes all expect a working `serve` command on ports 8080 (HTTP) and 9090 (metrics). Without this, Aeterna cannot be deployed.

## What Changes

- **Replace the `serve` stub** in `cli/src/commands/serve.rs` with a real async server that bootstraps all core services and binds listeners
- **Create a unified Axum router** composing: health/readiness/liveness endpoints, Governance Dashboard API (11 endpoints from `knowledge/src/api.rs`), OpenSpec compliance endpoints (8 from `project.md`), MCP HTTP/SSE transport (wrapping `McpServer` from `tools/src/server.rs`), A2A agent routes (from `agent-a2a/src/lib.rs`)
- **Bind a separate metrics listener** on port 9090 serving Prometheus `/metrics` via `metrics-exporter-prometheus`
- **Wire the full service dependency graph**: `MemoryManager`, `ReflectiveReasoner`, `McpServer`, `KnowledgeRepository`, `StorageBackend`, `GovernanceEngine`, `AuthorizationService`, `SyncManager`, `EventPublisher`, `DuckDbGraphStore`
- **Integrate WebSocket server** from `sync/src/websocket.rs` for real-time push (mounted on the main listener)
- **Implement graceful shutdown** coordinating all listeners via `tokio::signal` and a shared shutdown channel
- **Feature-gate optional subsystems** (CCA, Radkit, RLM, Reflective) based on `AETERNA_FEATURE_*` env vars already defined in the configmap
- **Update Dockerfile HEALTHCHECK** to use HTTP `GET /health` instead of CLI `admin health`

## Capabilities

### New Capabilities
- `server-runtime`: The unified server process that composes all service libraries into a running binary, manages lifecycle (startup, readiness, graceful shutdown), and binds HTTP + metrics listeners.

### Modified Capabilities
- `deployment`: Add requirement for the server binary to satisfy Kubernetes probe contracts (GET /health on 8080 with startup/readiness/liveness timing).
- `observability`: Add requirement for a dedicated metrics listener on port 9090 exposing Prometheus-format `/metrics`.
- `opencode-integration`: The MCP HTTP transport requirement (OC-H5, MCP Server Health Management) is now fulfilled by the server runtime rather than a standalone process.
- `tool-interface`: The MCP tools are now exposed via HTTP SSE transport on the main server, not just stdio.

## Impact

- **Affected code**:
  - `cli/src/commands/serve.rs` — Complete rewrite (stub → real server)
  - `cli/Cargo.toml` — New dependencies: `axum`, `tower`, `tower-http`, `hyper`
  - `knowledge/src/api.rs` — Refactor free functions into an Axum Router constructor
  - `Dockerfile` — Update HEALTHCHECK to HTTP probe
  - `Cargo.toml` (workspace) — Add `axum`, `tower`, `tower-http` to shared deps
- **Affected APIs**: All HTTP endpoints become live (previously zero)
- **Affected dependencies**: `axum 0.8+`, `tower`, `tower-http` (CORS, compression, tracing), `hyper` — all already used by `agent-a2a` and `opal-fetcher`
- **Affected systems**: Helm chart deployment (`aeterna.enabled=true`), CI/CD pipeline (new integration tests), Dockerfile (healthcheck change)
