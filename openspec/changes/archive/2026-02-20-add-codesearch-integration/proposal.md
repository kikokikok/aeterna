# Change: Code Search Integration for Semantic Code Navigation

## Why

Aeterna provides **memory and knowledge** for AI agents, but lacks **semantic code understanding**. Code Search (MIT-licensed, 1000+ stars) offers:
- Semantic code search via embeddings
- Call graph analysis (callers/callees tracing)
- Real-time file watching and indexing
- Native MCP server with structured tools

Integrating Code Search gives AI agents a complete picture: organizational knowledge (Aeterna) + codebase understanding (Code Search) through a unified interface.

## What Changes

### Core Integration
- Add Code Search as an MCP sidecar in the Helm chart
- Create unified MCP proxy that exposes Code Search tools alongside Aeterna tools
- Configure shared vector backend (Qdrant or PostgreSQL/pgvector)
- Add Code Search workspace management tied to Aeterna tenant hierarchy

### New MCP Tools (via proxy)
- `code_search` - Semantic code search (wraps `codesearch_search`)
- `code_trace_callers` - Find function callers (wraps `codesearch_trace_callers`)
- `code_trace_callees` - Find function callees (wraps `codesearch_trace_callees`)
- `code_graph` - Build call dependency graph (wraps `codesearch_trace_graph`)

### Helm Chart Additions
- Code Search sidecar container in Aeterna deployment
- Shared volume for Code Search index (or shared Qdrant collection)
- ConfigMap for Code Search configuration
- Init container for `codesearch init` on project paths

### CLI Integration
- `aeterna codesearch init` - Initialize Code Search for a project
- `aeterna codesearch search` - Direct semantic code search
- `aeterna codesearch trace` - Call graph analysis

## Impact

- Affected specs: New `codesearch-integration` capability
- Affected code:
  - `tools/` - MCP proxy for Code Search tools
  - `charts/aeterna/` - Sidecar deployment
  - `cli/` - Code Search subcommands
  - `config/` - Code Search configuration section

## Benefits

| Capability | Before | After |
|------------|--------|-------|
| Code search | Keyword grep only | Semantic "find auth logic" |
| Impact analysis | Manual | Automatic call graph |
| Refactoring safety | Hope-based | Trace all callers first |
| Context for agents | Memory + knowledge | + full codebase semantics |

## Non-Goals

- Forking or embedding Code Search source (it's Go, we're Rust)
- Replacing Code Search's indexing with Aeterna's
- Breaking Code Search's standalone operation

## Risks

| Risk | Mitigation |
|------|------------|
| Code Search version compatibility | Pin to specific version, test in CI |
| Shared backend conflicts | Separate collections/schemas with prefixes |
| Index sync lag | Real-time file watcher, or webhook triggers |
| Binary size (Go + Rust) | Sidecar pattern keeps binaries separate |
