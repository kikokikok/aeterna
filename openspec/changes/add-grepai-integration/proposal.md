# Change: GrepAI Integration for Semantic Code Navigation

## Why

Aeterna provides **memory and knowledge** for AI agents, but lacks **semantic code understanding**. GrepAI (MIT-licensed, 1000+ stars) offers:
- Semantic code search via embeddings
- Call graph analysis (callers/callees tracing)
- Real-time file watching and indexing
- Native MCP server with structured tools

Integrating GrepAI gives AI agents a complete picture: organizational knowledge (Aeterna) + codebase understanding (GrepAI) through a unified interface.

## What Changes

### Core Integration
- Add GrepAI as an MCP sidecar in the Helm chart
- Create unified MCP proxy that exposes GrepAI tools alongside Aeterna tools
- Configure shared vector backend (Qdrant or PostgreSQL/pgvector)
- Add GrepAI workspace management tied to Aeterna tenant hierarchy

### New MCP Tools (via proxy)
- `code_search` - Semantic code search (wraps `grepai_search`)
- `code_trace_callers` - Find function callers (wraps `grepai_trace_callers`)
- `code_trace_callees` - Find function callees (wraps `grepai_trace_callees`)
- `code_graph` - Build call dependency graph (wraps `grepai_trace_graph`)

### Helm Chart Additions
- GrepAI sidecar container in Aeterna deployment
- Shared volume for GrepAI index (or shared Qdrant collection)
- ConfigMap for GrepAI configuration
- Init container for `grepai init` on project paths

### CLI Integration
- `aeterna grepai init` - Initialize GrepAI for a project
- `aeterna grepai search` - Direct semantic code search
- `aeterna grepai trace` - Call graph analysis

## Impact

- Affected specs: New `grepai-integration` capability
- Affected code:
  - `tools/` - MCP proxy for GrepAI tools
  - `charts/aeterna/` - Sidecar deployment
  - `cli/` - GrepAI subcommands
  - `config/` - GrepAI configuration section

## Benefits

| Capability | Before | After |
|------------|--------|-------|
| Code search | Keyword grep only | Semantic "find auth logic" |
| Impact analysis | Manual | Automatic call graph |
| Refactoring safety | Hope-based | Trace all callers first |
| Context for agents | Memory + knowledge | + full codebase semantics |

## Non-Goals

- Forking or embedding GrepAI source (it's Go, we're Rust)
- Replacing GrepAI's indexing with Aeterna's
- Breaking GrepAI's standalone operation

## Risks

| Risk | Mitigation |
|------|------------|
| GrepAI version compatibility | Pin to specific version, test in CI |
| Shared backend conflicts | Separate collections/schemas with prefixes |
| Index sync lag | Real-time file watcher, or webhook triggers |
| Binary size (Go + Rust) | Sidecar pattern keeps binaries separate |
