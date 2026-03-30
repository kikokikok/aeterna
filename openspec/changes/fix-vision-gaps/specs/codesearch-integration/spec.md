## MODIFIED Requirements

### Requirement: Code Search Integration Architecture

Code search is NOT a core Aeterna subsystem. It is a developer navigation tool that SHALL be integrated as an external skill/plugin via pluggable MCP backends (e.g., JetBrains Code Intelligence MCP, VS Code extensions). Aeterna's agent discovers available code intelligence MCP tools at runtime and invokes them.

#### Scenario: Agent discovers code intelligence tools
- **WHEN** an Aeterna agent starts in an environment with a code intelligence MCP backend available (e.g., JetBrains Code Intelligence MCP plugin running)
- **THEN** the agent SHALL discover the available tools via MCP tool listing
- **AND** use them for code navigation, symbol search, call graph traversal

#### Scenario: No code intelligence backend available
- **WHEN** an Aeterna agent starts without any code intelligence MCP backend running
- **THEN** code search tools SHALL be unavailable
- **AND** the agent SHALL inform the user that code intelligence requires an IDE plugin or compatible MCP backend

#### Scenario: Pluggable backend trait
- **WHEN** a new code intelligence MCP backend is developed (e.g., VS Code extension, Neovim LSP bridge)
- **THEN** the system SHALL support it without code changes to Aeterna core
- **AND** the agent discovers the backend's tools via standard MCP protocol

## REMOVED Requirements

### Requirement: Code Search Indexing Pipeline
**Reason**: Code search indexing (tree-sitter parsing, embedding generation, vector storage) is NOT an Aeterna responsibility. This belongs in IDE-native tools (JetBrains Code Intelligence, VS Code extensions) that expose their capabilities via MCP.
**Migration**: Remove `tools/src/codesearch/client.rs` sidecar binary spawning pattern. Keep `storage/src/repo_manager.rs` for the remote repository approval workflow (separate concern).

### Requirement: Code Search MCP Tool Wiring
**Reason**: Replaced by the pluggable MCP backend architecture. Aeterna agents discover and invoke code intelligence tools from whatever MCP backend is available, rather than implementing fixed tool stubs.
**Migration**: The 6 hardcoded MCP tool stubs (`codesearch_search`, `codesearch_trace_callers`, etc.) in `tools/src/codesearch/tools.rs` should be removed. The agent dynamically discovers tools from the connected MCP backend.
