## MODIFIED Requirements

### Requirement: NPM Plugin Package

The system SHALL provide an NPM package `@aeterna-org/opencode-plugin` that integrates with OpenCode using the official `@opencode-ai/plugin` SDK.

The plugin MUST:
- Export a default Plugin function conforming to OpenCode's Plugin type
- Register all supported Aeterna tools as OpenCode tools
- Implement lifecycle hooks for deep integration
- Support configuration via `opencode.jsonc`, environment variables, and `.aeterna/config.toml`
- Initialize a `LocalMemoryManager` on startup for local-first personal layer access
- Run a background `SyncEngine` for bidirectional memory synchronization when a server URL is configured

#### Scenario: Plugin installation
- **WHEN** a user adds `"plugin": ["@aeterna-org/opencode-plugin"]` to `opencode.jsonc`
- **THEN** OpenCode SHALL install and load the plugin automatically at startup
- **AND** the supported installation flow SHALL NOT require a separate manual `npm install -D` step for normal usage
- **AND** the plugin SHALL use the Bun-compatible SQLite runtime supported by OpenCode

#### Scenario: Plugin initialization with local store
- **WHEN** OpenCode starts with the Aeterna plugin configured
- **THEN** the plugin SHALL initialize the `LocalMemoryManager` with the configured database path
- **AND** the plugin SHALL start the `SyncEngine` background loop if a server URL is configured
- **AND** the plugin SHALL register all tools and hooks
- **AND** the plugin SHALL establish exactly one active session context for the OpenCode session

#### Scenario: Plugin initialization without server
- **WHEN** OpenCode starts with the Aeterna plugin configured
- **AND** no `AETERNA_SERVER_URL` is set
- **THEN** the plugin SHALL initialize the `LocalMemoryManager` for offline-only operation
- **AND** personal layer tools SHALL function normally
- **AND** shared layer tools SHALL return empty or cached results with an informative message rather than failing the session

#### Scenario: Plugin shutdown with pending sync
- **WHEN** OpenCode shuts down
- **AND** the `sync_queue` contains pending operations
- **THEN** the plugin SHALL attempt a final sync push (up to 5s timeout)
- **AND** the plugin SHALL close the SQLite database cleanly

#### Scenario: Plugin configuration via opencode.jsonc
- **WHEN** the user adds `"plugin": ["@aeterna-org/opencode-plugin"]` to `opencode.jsonc`
- **THEN** OpenCode SHALL load and initialize the Aeterna plugin
- **AND** all supported Aeterna tools SHALL be available to the AI

### Requirement: Tool Registration

The system SHALL register the supported Aeterna tools as OpenCode tools using the `tool()` helper from `@opencode-ai/plugin/tool`.

| Tool Name | Description |
|-----------|-------------|
| `aeterna_memory_add` | Add memory entry with content, layer, tags, importance |
| `aeterna_memory_search` | Search memories with semantic similarity |
| `aeterna_memory_get` | Retrieve specific memory by ID |
| `aeterna_memory_promote` | Promote memory to higher layer |
| `aeterna_knowledge_query` | Query knowledge repository |
| `aeterna_knowledge_propose` | Propose new knowledge item |
| `aeterna_graph_query` | Query graph relationships and traversals |
| `aeterna_graph_neighbors` | Find directly related memories |
| `aeterna_graph_path` | Find shortest path between memories |
| `aeterna_context_assemble` | Assemble hierarchical context |
| `aeterna_note_capture` | Capture trajectory events for distillation |
| `aeterna_hindsight_query` | Query learned error patterns |
| `aeterna_meta_loop_status` | Inspect meta-loop state |
| `aeterna_sync_status` | Check sync status |
| `aeterna_governance_status` | Check governance state |

#### Scenario: Tool schema definition
- **WHEN** OpenCode requests tool definitions
- **THEN** the plugin SHALL return Zod-based schemas for each tool
- **AND** each schema SHALL include descriptions for all parameters

#### Scenario: Memory add tool invocation
- **WHEN** the AI invokes `aeterna_memory_add` with content
- **THEN** the plugin SHALL create a memory entry in Aeterna
- **AND** the plugin SHALL return confirmation with memory ID and computed importance

#### Scenario: Knowledge query tool invocation
- **WHEN** the AI invokes `aeterna_knowledge_query` with a query
- **THEN** the plugin SHALL search the knowledge repository
- **AND** the plugin SHALL return formatted results with relevance scores

### Requirement: Tool Execute After Hook

The system SHALL implement the `tool.execute.after` hook to capture tool executions as working memory.

#### Scenario: Tool execution capture
- **WHEN** any tool execution completes
- **THEN** the plugin SHALL record the execution in Aeterna
- **AND** the plugin SHALL include tool name, arguments, output, and timestamp

#### Scenario: Significance detection
- **WHEN** a tool execution is captured
- **THEN** the plugin SHALL analyze the execution for significance
- **AND** significant executions SHALL be flagged for promotion

#### Scenario: Error pattern detection
- **WHEN** a tool execution follows a previous error
- **THEN** the plugin SHALL detect the error-resolution pattern
- **AND** the plugin SHALL flag the pattern as significant

#### Scenario: Captured arguments reflect executed tool call
- **WHEN** the plugin captures a completed tool execution
- **THEN** the stored execution record SHALL preserve the executed arguments for that tool call
- **AND** the plugin SHALL NOT replace captured arguments with an empty placeholder object

### Requirement: Event Hook

The system SHALL implement the `event` hook to handle session lifecycle events.

#### Scenario: Session end event
- **WHEN** an OpenCode session ends
- **THEN** the plugin SHALL call `client.sessionEnd()`
- **AND** the plugin SHALL promote significant memories to session layer
- **AND** the plugin SHALL generate a session summary

#### Scenario: Session start event
- **WHEN** a new OpenCode session starts
- **THEN** the plugin SHALL initialize session context
- **AND** the plugin SHALL subscribe to governance notifications

#### Scenario: Session startup does not create duplicate backend sessions
- **WHEN** the plugin starts and later receives the `session.start` event for the same OpenCode session
- **THEN** the plugin SHALL maintain a single active Aeterna session context for that OpenCode session
- **AND** the plugin SHALL NOT create duplicate backend sessions for the same logical start

### Requirement: Automatic Session Capture

The system SHALL automatically capture session context during OpenCode coding sessions without requiring explicit user action.

Capture events:
- Tool invocations and their results
- Chat messages with significant content
- File modifications (via tool execution context)

#### Scenario: Automatic working memory creation
- **WHEN** a tool execution completes
- **THEN** the plugin SHALL create a working memory entry
- **AND** the memory SHALL include execution context and outcome

#### Scenario: Session summary generation
- **WHEN** a session ends
- **THEN** the plugin SHALL analyze all captured memories
- **AND** the plugin SHALL generate a session summary memory
- **AND** significant memories SHALL be promoted to session layer

### Requirement: Significance Detection

The system SHALL automatically detect significant learnings that warrant promotion from working memory to session memory.

Significance criteria:
- **Error resolution**: Error followed by successful outcome
- **Repeated pattern**: Similar queries 3+ times
- **Novel approach**: Solution differs from existing knowledge
- **Explicit capture**: User explicitly called `aeterna_memory_add`

#### Scenario: Error resolution detection
- **WHEN** a successful outcome follows an error in the session
- **THEN** the plugin SHALL flag the resolution as significant
- **AND** auto-promote the error-solution pair

#### Scenario: Repeated pattern detection
- **WHEN** similar queries are detected 3+ times
- **THEN** the plugin SHALL consolidate into a single memory
- **AND** flag for promotion

#### Scenario: Repeated pattern history is recorded
- **WHEN** the plugin analyzes tool executions for repeated significant patterns
- **THEN** the execution history used for repeated-pattern detection SHALL be updated for each completed execution
- **AND** repeated-pattern significance SHALL be based on actual recorded session history rather than an empty in-memory baseline
