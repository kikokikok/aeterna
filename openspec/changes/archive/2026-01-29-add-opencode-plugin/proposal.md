# Change: OpenCode Plugin Integration

## Why

OpenCode is becoming the standard AI-assisted coding tool for many developers. To maximize Aeterna's impact, we need deep integration that:
- Automatically captures session context and promotes valuable learnings to memory
- Provides real-time knowledge queries during coding sessions
- Enables governance workflows (proposal submission, approval notifications)
- Works seamlessly without requiring manual intervention from developers

## What Changes

- Add MCP (Model Context Protocol) server implementation for Aeterna
- Expose all 8 Aeterna tools via MCP for OpenCode consumption
- Add session lifecycle hooks for automatic memory capture
- Add knowledge query integration for context-aware suggestions
- Add governance notification stream for real-time alerts
- Create OpenCode configuration package for easy setup

## Impact

- Affected specs: `tool-interface` (extends existing MCP tools)
- New spec: `opencode-integration`
- Affected code: `adapters/` (new opencode adapter), `tools/` (MCP server wrapper)
- External dependencies: None (MCP is a protocol, not a library)
