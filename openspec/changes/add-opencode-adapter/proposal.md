# Change: Create OpenCode Adapter

## Why
The OpenCode adapter provides direct integration with the oh-my-opencode ecosystem, enabling seamless use of Memory-Knowledge tools within OpenCode's plugin system.

## What Changes

### OpenCode Adapter Implementation
- Create adapters/opencode crate
- Implement `EcosystemAdapter` trait
- Generate JSON Schema for all 8 tools
- Implement tool handler functions
- Create OpenCode plugin manifest
- Implement context injection (onSessionStart, onSessionEnd)

### Tool Registration
- Register all 8 MCP tools with OpenCode
- Provide tool descriptions for auto-completion
- Implement tool parameter validation
- Return structured responses matching MCP format

### Context Injection
- Implement memory rehydration on session start
- Implement constraint monitoring for active session
- Implement sync triggers based on session events
- Inject relevant knowledge into system prompt

### Plugin Integration
- Create plugin manifest with dependencies
- Implement plugin lifecycle hooks
- Provide configuration interface
- Support oh-my-opencode plugin system

## Impact

### Affected Specs
- `tool-interface` - OpenCode adapter integration

### Affected Code
- New `adapters/opencode/` crate
- `tools/` crate exports to OpenCode format
- Existing `adapters/opencode/` README enhanced with implementation

### Dependencies
- `serde_json` for JSON Schema generation
- No external dependencies beyond core crates

## Breaking Changes
None - this is a new adapter
