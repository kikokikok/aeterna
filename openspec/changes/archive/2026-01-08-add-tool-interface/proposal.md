# Change: Implement Tool Interface

## Why
The Tool Interface exposes Memory and Knowledge functionality to AI agents via Model Context Protocol (MCP). This is the primary integration point for all agent frameworks (LangChain, AutoGen, CrewAI, OpenCode).

## What Changes

### MCP Server
- Implement MCP-compliant server using standard JSON-RPC protocol
- Use well-maintained crate: `tokio` for async runtime
- Implement tool registration and discovery
- Implement JSON Schema validation

### Tools Implementation
- **Memory Tools**: memory_add, memory_search, memory_delete
- **Knowledge Tools**: knowledge_query, knowledge_check, knowledge_show
- **Sync Tools**: sync_now, sync_status

### Error Handling
- Standard error format with success boolean
- 7 error codes: INVALID_INPUT, NOT_FOUND, PROVIDER_ERROR, RATE_LIMITED, UNAUTHORIZED, TIMEOUT, CONFLICT
- Retryable flags on each error

### Ecosystem Adapters
- OpenCode adapter: JSON Schema + handler functions
- LangChain adapter: Zod schemas + DynamicStructuredTool
- Context injection hooks (onSessionStart, onSessionEnd, onMessage, onToolUse)

## Impact

### Affected Specs
- `tool-interface` - Complete implementation
- `adapter-layer` - Create ecosystem adapters

### Affected Code
- New `tools` crate with MCP server
- New `adapters/opencode` crate
- New `adapters/langchain` crate

### Dependencies
- `serde` and `serde_json` for JSON handling
- `schemars` for JSON Schema generation
- `tokio` for async runtime

## Breaking Changes
None - this integrates with existing systems
