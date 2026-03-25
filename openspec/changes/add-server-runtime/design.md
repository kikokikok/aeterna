## Context

The `aeterna` binary (built from `cli/`) is the single container entrypoint. The `CMD ["serve"]` in the Dockerfile dispatches to `cli/src/commands/serve.rs`, which currently bails with a stub message. Meanwhile, seven service library crates implement all business logic:

| Crate | Key Type | Status |
|-------|----------|--------|
| `tools` | `McpServer` — 36 MCP tools, JSON-RPC dispatcher | ✅ Implemented, no transport bound |
| `agent-a2a` | `create_router()` — Axum router for A2A protocol | ✅ Working standalone binary |
| `knowledge` | `GovernanceDashboardApi` — 11 utoipa endpoints | ✅ Annotated, not mounted on Router |
| `sync` | `WsServer` — tungstenite WebSocket push | ✅ Implemented, needs port binding |
| `idp-sync` | Webhook handler — Okta/Azure AD | ✅ Implemented as library |
| `opal-fetcher` | Fetcher server — OPAL data source | ✅ Standalone binary |
| `memory` | `MemoryManager` + `ReflectiveReasoner` | ✅ Core domain |

The Helm chart expects port 8080 (HTTP), port 9090 (metrics), and K8s probes targeting `GET /health` on 8080. The configmap injects ~30 environment variables. The Dockerfile exposes 8080 and 9090.

**Stakeholders**: DevOps (Helm deployment), product (server must run to unblock all features), security (authorization integration with OPAL/Cedar).

## Goals / Non-Goals

**Goals:**
- Wire the `serve` command into a real async Axum server composing all service libraries
- Serve HTTP on configurable port (default 8080) with unified router
- Serve Prometheus metrics on configurable port (default 9090)
- Implement health/readiness/liveness endpoints satisfying K8s probe contracts
- Bootstrap the full service dependency graph from env vars
- Feature-gate optional subsystems (CCA, Radkit, RLM, Reflective) via `AETERNA_FEATURE_*` env vars
- Implement graceful shutdown across all listeners
- Mount MCP tools via HTTP SSE transport (MCP 2024-11-05 streamable HTTP spec)
- Support both `aeterna serve` (production) and `cargo run -p aeterna -- serve` (development)

**Non-Goals:**
- gRPC transport — investigation found zero proto files; all existing surfaces use Axum HTTP or WebSocket. Not adding gRPC now.
- Standalone `services/memory` binary — it's an empty scaffold. The unified server approach is preferred.
- CLI-embedded web UI — out of scope.
- Authentication/authorization in the server itself — delegated to OPAL/Cedar sidecar and ingress (oauth2-proxy).
- Replacing the `opal-fetcher` standalone binary — it runs as a separate pod with its own lifecycle.
- Replacing the `agent-a2a` standalone binary — A2A routes are embedded in the unified server, but the standalone binary remains for decoupled deployments.

## Decisions

### D1: Single Unified Binary (not microservices)

**Decision**: Compose all services into a single `aeterna serve` process rather than separate per-service binaries.

**Rationale**: 
- Matches the Helm chart design (one Deployment, one container, ports 8080+9090)
- Matches the Dockerfile (single binary, `CMD ["serve"]`)
- Reduces operational complexity (1 pod type vs 5+)
- Service crates remain independent libraries — decoupled deployment is still possible later

**Alternatives considered**:
- Per-service binaries (rejected: contradicts existing Helm/Docker design, multiplies ops burden)
- Sidecar pattern (rejected: adds latency, complexity; services share state in-process)

### D2: Axum 0.8 as the HTTP Framework

**Decision**: Use Axum 0.8 (already a workspace dependency via `agent-a2a` and `opal-fetcher`).

**Rationale**:
- Already used by 2 crates — zero new framework dependencies
- Native tower middleware support (compression, CORS, tracing)
- utoipa integration for OpenAPI docs (already used by `knowledge/src/api.rs`)
- Excellent async performance with Tokio

**Alternatives considered**:
- Actix-web (rejected: different ecosystem, would be a second framework)
- Rocket (rejected: sync-first design, limited middleware)
- Raw hyper (rejected: too low-level, Axum already wraps it)

### D3: Router Composition via `axum::Router::merge` and `nest`

**Decision**: Each service crate exports a `fn router(state: AppState) -> Router` function. The serve command merges them:

```rust
let app = Router::new()
    .merge(health::router())                    // GET /health, /ready, /live
    .nest("/openspec/v1", openspec::router(state.clone()))  // OpenSpec compliance
    .nest("/api/v1", knowledge_api::router(state.clone()))  // Governance Dashboard
    .nest("/mcp", mcp_http::router(state.clone()))          // MCP SSE transport
    .nest("/a2a", a2a::router(state.clone()))               // A2A agent protocol
    .nest("/ws", ws::router(state.clone()))                  // WebSocket sync
    .nest("/webhooks", idp::router(state.clone()))           // IDP webhooks
    .layer(/* tower middleware stack */);
```

**Rationale**: Clean separation of concerns. Each crate owns its routes. Merge is additive and order-independent.

### D4: Shared `AppState` for Dependency Injection

**Decision**: A single `AppState` struct holds all bootstrapped services as `Arc<dyn Trait>`:

```rust
#[derive(Clone)]
pub struct AppState {
    pub memory_manager: Arc<MemoryManager>,
    pub knowledge_repo: Arc<dyn KnowledgeRepository>,
    pub storage_backend: Arc<dyn StorageBackend>,
    pub governance_engine: Arc<dyn GovernanceEngine>,
    pub authorization: Arc<dyn AuthorizationService>,
    pub sync_manager: Arc<dyn SyncManager>,
    pub mcp_server: Arc<McpServer>,
    pub reasoner: Option<Arc<dyn ReflectiveReasoner>>,
    pub graph_store: Option<Arc<DuckDbGraphStore>>,
    pub event_publisher: Option<Arc<dyn EventPublisher>>,
    pub config: Arc<AeternaConfig>,
}
```

**Rationale**: Axum's `State` extractor requires `Clone`. `Arc` is the standard Rust pattern for shared ownership of services across async tasks.

### D5: Two Listeners (HTTP + Metrics)

**Decision**: Bind two separate TCP listeners:
- `0.0.0.0:8080` — Main application router (all API endpoints + health + WebSocket)
- `0.0.0.0:9090` — Metrics-only router (`GET /metrics` Prometheus scrape endpoint)

**Rationale**:
- Helm chart service.yaml defines both ports
- Prometheus ServiceMonitor scrapes 9090 — isolating metrics prevents accidental auth/rate-limiting issues
- Standard cloud-native practice (application port vs admin port)

**Alternatives considered**:
- Single port with path routing (rejected: metrics should not require app-level auth)
- Third port for WebSocket (rejected: unnecessary; WS upgrades work fine on 8080)

### D6: Health Endpoint Contract

**Decision**: Three endpoints on port 8080:

| Endpoint | K8s Probe | Logic |
|----------|-----------|-------|
| `GET /health` | startup + liveness | Returns 200 if process is alive and listeners bound. No dependency checks. |
| `GET /ready` | readiness | Returns 200 only when ALL required backends are connected (Postgres, Qdrant, Redis). Returns 503 with JSON detail otherwise. |
| `GET /live` | (alias) | Same as `/health` |

**Rationale**: 
- K8s startup probe uses `/health` — must succeed quickly (no DB round-trip)
- Readiness probe uses `/ready` — must verify backend connectivity
- Liveness probe uses `/health` — must not false-positive on transient DB failures

### D7: MCP HTTP Transport via SSE

**Decision**: Expose MCP tools via Server-Sent Events (SSE) on `POST /mcp/message` + `GET /mcp/sse`, following the MCP 2024-11-05 streamable HTTP transport spec. The existing `McpServer` is transport-agnostic — it accepts JSON-RPC requests and returns responses. The HTTP layer wraps it.

**Rationale**:
- `opencode-integration/spec.md` requires "HTTP transport for remote use"
- SSE is the MCP standard for HTTP transport
- stdio transport remains available via `aeterna mcp serve` command (unchanged)

### D8: Bootstrap Sequence

**Decision**: Ordered initialization with fail-fast:

```
1. Parse CLI args + load config from env
2. Initialize tracing (level from AETERNA_LOG_LEVEL)
3. Connect PostgreSQL (required — fail if unreachable)
4. Connect Qdrant (required if AETERNA_VECTOR_BACKEND=qdrant)
5. Connect Redis/Dragonfly (required — fail if unreachable)
6. Create MemoryManager with embedding + LLM services
7. Create KnowledgeRepository + GovernanceEngine
8. Create McpServer with all service dependencies
9. Feature-gate: CCA, Radkit, RLM, Reflective (log skip if disabled)
10. Build Axum routers
11. Bind HTTP listener on :8080
12. Bind metrics listener on :9090
13. Log "Aeterna server ready" — readiness probe will now pass
14. Await shutdown signal (SIGTERM/SIGINT)
15. Graceful shutdown: stop accepting, drain in-flight, close connections
```

**Rationale**: Fail-fast on required infrastructure prevents zombie pods. Feature gates allow partial deployment during development.

### D9: Graceful Shutdown

**Decision**: Use `tokio::signal::ctrl_c()` and `tokio::signal::unix::signal(SignalKind::terminate())` to trigger coordinated shutdown:
- Set a shared `AtomicBool` or broadcast `tokio::sync::watch` channel
- Axum `graceful_shutdown` support via `axum::serve::Serve::with_graceful_shutdown`
- WebSocket server sends close frames
- Database connection pools drain

**Rationale**: K8s sends SIGTERM with 30s grace period. Must drain in-flight requests to avoid 502s during rolling updates.

## Risks / Trade-offs

- **Risk**: Large `serve.rs` becomes a monolith → **Mitigation**: Each service crate exports its own `router()` function. `serve.rs` only does bootstrap + compose. Create a `server` module in `cli/src/server/` if it grows beyond 500 lines.
- **Risk**: Startup time exceeds K8s budget (160s) due to slow backend connections → **Mitigation**: Connect backends with timeout (10s each). Use connection pooling. Health endpoint responds immediately (no backend check). Startup probe is generous (10s delay, 30 failures × 5s = 160s).
- **Risk**: `knowledge/src/api.rs` refactoring breaks existing code → **Mitigation**: Functions remain public with same signatures. Only add a new `pub fn router(state: AppState) -> Router` that wires them. Existing callers (if any) unaffected.
- **Risk**: MCP SSE transport implementation complexity → **Mitigation**: The `rmcp` crate (already referenced in tools) provides SSE transport helpers. If unavailable, implement minimal SSE wrapper (~100 LOC).
- **Risk**: Memory usage increases with all services in one process → **Mitigation**: Monitor with metrics. Individual services have low base footprint. Redis/Qdrant clients use connection pooling, not persistent connections per request.

## Migration Plan

1. **Phase 1 — Implement**: Replace stub in `serve.rs`, add router composition, bootstrap sequence
2. **Phase 2 — Test locally**: `cargo run -p aeterna -- serve` with Docker Compose backends
3. **Phase 3 — Update Dockerfile**: Change HEALTHCHECK to `curl http://localhost:8080/health`
4. **Phase 4 — Build image**: `docker build -t ghcr.io/kikokikok/aeterna:latest .`
5. **Phase 5 — Deploy**: `helm upgrade aeterna ... --set aeterna.enabled=true`
6. **Phase 6 — Verify**: K8s probes pass, `/health` returns 200, `/metrics` scraped, MCP tools accessible

**Rollback**: Set `aeterna.enabled=false` in Helm values. Infrastructure (CNPG, Dragonfly, Qdrant, OPAL) is independent and unaffected.

## Open Questions

- **Q1**: Should the WebSocket sync server share port 8080 or get its own port? → **Decision**: Share 8080. WebSocket upgrade on `/ws` path. Simplifies Helm service definition.
- **Q2**: Should IDP webhooks be embedded or remain a candidate for a separate pod? → **Tentative**: Embed for now (single binary approach). Can be extracted later if webhook volume justifies it.
- **Q3**: OpenAPI docs — should `utoipa` Swagger UI be served in production? → **Decision**: Yes, at `GET /api/docs` behind a feature flag (`AETERNA_FEATURE_SWAGGER=true`, default false in production).
