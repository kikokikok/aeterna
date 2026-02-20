# Code Search Integration Design

## Overview

Aeterna integrates with Code Search to provide semantic code search, call graph analysis, and dependency tracing for AI agents. Code Search runs as a sidecar container in Kubernetes and communicates with Aeterna via MCP (Model Context Protocol) over stdio.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Aeterna Pod                     │
│                                                  │
│  ┌──────────────┐     stdio      ┌────────────┐ │
│  │   Aeterna    │◄──────────────►│ Code Search│ │
│  │  (tools/)    │  JSON-RPC/MCP  │  Sidecar   │ │
│  │              │                │            │ │
│  │ CodeSearch   │                │  codesearch│ │
│  │   Client     │                │  mcp serve │ │
│  └──────┬───────┘                └─────┬──────┘ │
│         │                              │         │
│         │ :8080 HTTP                   │ :9090   │
│         │ :9090 metrics                │ MCP     │
└─────────┼──────────────────────────────┼─────────┘
          │                              │
    ┌─────▼──────┐                ┌──────▼──────┐
    │  Kubernetes │                │   Qdrant /  │
    │   Service   │                │  PostgreSQL │
    └────────────┘                └─────────────┘
```

## Communication Pattern

### Spawn-per-call stdio

Each MCP tool invocation spawns a fresh `codesearch mcp serve` process:

1. `CodeSearchClient` builds a JSON-RPC request
2. Spawns `codesearch mcp serve` via `tokio::process::Command`
3. Writes the JSON-RPC request to stdin, then closes stdin
4. Reads the complete JSON-RPC response from stdout
5. Parses the response and extracts the result

This pattern was chosen over persistent connections because:
- Code Search's MCP server is designed for single-request invocations
- No connection state to manage or leak
- Process isolation prevents memory leaks from accumulating
- Simple error recovery (process either succeeds or fails)

### JSON-RPC Protocol

Request format:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "codesearch_search",
    "arguments": { "query": "authentication middleware" }
  }
}
```

Response format:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      { "type": "text", "text": "{\"results\": [...]}" }
    ]
  }
}
```

### Response Parsing Priority

1. `response.result.content[0].text` — standard MCP text content (parsed as JSON if valid)
2. `response.result` — raw result object
3. `response.error` — error propagation

## Resilience

### Circuit Breaker

The client tracks consecutive failures. After 5 failures, the circuit opens and all subsequent calls are rejected immediately with "circuit breaker open". A successful call resets the counter.

### Timeout

Each call is wrapped in a configurable timeout (default 30s). Timeouts count as failures for circuit breaker purposes.

### Mock Fallback

When `CodeSearchConfig.use_mock` is true:
- All calls use mock responses (no binary spawned)
- If binary spawn fails with "not found" errors, falls back to mock
- Useful for development, testing, and environments without Code Search

## Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `binary_path` | `String` | `"codesearch"` | Path to Code Search binary |
| `workspace` | `String` | `"default"` | Tenant isolation namespace |
| `timeout_secs` | `u64` | `30` | Per-call timeout |
| `debug` | `bool` | `false` | Debug logging to stderr |
| `mcp_args` | `Vec<String>` | `["mcp", "serve"]` | Arguments passed to binary |
| `use_mock` | `bool` | `false` | Use mock responses |

## Kubernetes Deployment

### Helm Service

The Kubernetes Service exposes an MCP port (9090) conditionally when Code Search is enabled:

```yaml
ports:
  - port: 8080
    name: http
  - port: 9090
    name: metrics
  # Conditional:
  - port: 9090
    name: mcp        # when codesearch.enabled=true
```

### Sidecar Container

Code Search runs as an init + sidecar container pair:
- **Init container**: Clones repositories and builds initial index
- **Sidecar container**: Runs `codesearch mcp serve` for on-demand queries

Shared volumes mount workspace data between Aeterna and Code Search containers.

## MCP Tools

| Tool | Description |
|------|-------------|
| `codesearch_search` | Semantic code search with natural language queries |
| `codesearch_trace_callers` | Find all callers of a symbol |
| `codesearch_trace_callees` | Find all callees of a symbol |
| `codesearch_trace_graph` | Build dependency graph for a symbol |
| `codesearch_index_status` | Check indexing status for a project |
| `codesearch_repo_request` | Request indexing of a new repository |

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Binary not found | Graceful error (or mock fallback if `use_mock=true`) |
| Process exits non-zero | Error with exit status |
| Malformed JSON response | Parse error |
| JSON-RPC error field | Error propagated from Code Search |
| Timeout | Circuit breaker incremented, timeout error returned |
| Circuit breaker open | Immediate rejection, no spawn attempted |

## Future Considerations

- **Persistent connection**: If spawn overhead becomes significant, switch to a long-lived process with multiplexed requests
- **Connection pooling**: Pre-spawned process pool for high-throughput scenarios
- **Health checks**: Periodic liveness probes to the sidecar for proactive circuit breaker management
- **Metrics**: Expose call latency, error rates, and circuit breaker state to Prometheus
