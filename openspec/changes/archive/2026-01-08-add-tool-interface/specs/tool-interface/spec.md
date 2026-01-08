## ADDED Requirements

### Requirement: MCP Server
The system SHALL implement a Model Context Protocol (MCP) compliant server for tool discovery and invocation.

#### Scenario: Tool discovery endpoint
- **WHEN** MCP client sends tools/list request
- **THEN** server SHALL return list of all 8 registered tools
- **AND** each tool SHALL include name, description, input schema

#### Scenario: Tool invocation endpoint
- **WHEN** MCP client sends tools/call request with tool name
- **THEN** server SHALL execute corresponding tool handler
- **AND** server SHALL return structured response matching spec
- **AND** server SHALL include success boolean in response

#### Scenario: Handle concurrent requests
- **WHEN** multiple MCP clients send requests simultaneously
- **THEN** server SHALL process all requests concurrently
- **AND** server SHALL handle > 100 concurrent connections

### Requirement: Tool Registration
The system SHALL provide a mechanism to register tools with JSON Schema definitions.

#### Scenario: Register memory tools
- **WHEN** system initializes
- **THEN** server SHALL register memory_add tool
- **AND** server SHALL register memory_search tool
- **AND** server SHALL register memory_delete tool
- **AND** each tool SHALL have JSON Schema for input validation

#### Scenario: Register knowledge tools
- **WHEN** system initializes
- **THEN** server SHALL register knowledge_query tool
- **AND** server SHALL register knowledge_check tool
- **AND** server SHALL register knowledge_show tool
- **AND** each tool SHALL have JSON Schema for input validation

#### Scenario: Register sync tools
- **WHEN** system initializes
- **THEN** server SHALL register sync_now tool
- **AND** server SHALL register sync_status tool
- **AND** each tool SHALL have JSON Schema for input validation

### Requirement: JSON Schema Validation
The system SHALL validate all tool inputs against JSON Schema before execution.

#### Scenario: Validate required fields
- **WHEN** tool request is missing required field
- **THEN** server SHALL return INVALID_INPUT error
- **AND** error SHALL indicate which field is missing

#### Scenario: Validate field types
- **WHEN** tool request has incorrect field type
- **THEN** server SHALL return INVALID_INPUT error
- **AND** error SHALL indicate expected and actual types

#### Scenario: Validate enum values
- **WHEN** tool request has invalid enum value
- **THEN** server SHALL return INVALID_INPUT error
- **AND** error SHALL list valid enum values

#### Scenario: Validate range constraints
- **WHEN** tool request has value outside valid range
- **THEN** server SHALL return INVALID_INPUT error
- **AND** error SHALL indicate min and max valid values

### Requirement: Memory Tools Implementation
The system SHALL implement three tools for memory operations.

#### Scenario: memory_add tool
- **WHEN** client calls memory_add with valid content
- **THEN** system SHALL call MemoryManager.add()
- **AND** system SHALL return memory entry with ID
- **AND** system SHALL use default layer='user' if not specified

#### Scenario: memory_search tool
- **WHEN** client calls memory_search with query and identifiers
- **THEN** system SHALL call MemoryManager.search()
- **AND** system SHALL return results with score, layer, content
- **AND** system SHALL apply default limit=10, threshold=0.7

#### Scenario: memory_delete tool
- **WHEN** client calls memory_delete with valid ID
- **THEN** system SHALL call MemoryManager.delete()
- **AND** system SHALL return success boolean

### Requirement: Knowledge Tools Implementation
The system SHALL implement three tools for knowledge operations.

#### Scenario: knowledge_query tool
- **WHEN** client calls knowledge_query with filters
- **THEN** system SHALL call KnowledgeManager.query()
- **AND** system SHALL return item summaries (not full content)
- **AND** system SHALL apply default status=['accepted']

#### Scenario: knowledge_check tool
- **WHEN** client calls knowledge_check with files and dependencies
- **THEN** system SHALL call KnowledgeManager.check_constraints()
- **AND** system SHALL aggregate violations by severity
- **AND** system SHALL return passed boolean and violations array

#### Scenario: knowledge_show tool
- **WHEN** client calls knowledge_show with ID and includeConstraints=true
- **THEN** system SHALL call KnowledgeManager.get()
- **AND** system SHALL return full item with constraints

### Requirement: Sync Tools Implementation
The system SHALL implement two tools for sync operations.

#### Scenario: sync_now tool
- **WHEN** client calls sync_now with force=false
- **THEN** system SHALL call SyncManager.full_sync()
- **AND** system SHALL use delta sync if not forced
- **AND** system SHALL return sync statistics

#### Scenario: sync_status tool
- **WHEN** client calls sync_status
- **THEN** system SHALL return lastSyncAt, lastKnowledgeCommit, stats
- **AND** system SHALL include items added, updated, deleted, unchanged

### Requirement: Tool Error Handling
The system SHALL return standardized errors with specific codes for all failure scenarios.

#### Scenario: Invalid input error
- **WHEN** tool input validation fails
- **THEN** system SHALL return INVALID_INPUT error
- **AND** error SHALL NOT be retryable
- **AND** error SHALL include validation details

#### Scenario: Not found error
- **WHEN** requested resource does not exist
- **THEN** system SHALL return NOT_FOUND error
- **AND** error SHALL NOT be retryable
- **AND** error SHALL include requested ID

#### Scenario: Provider error with retry
- **WHEN** underlying provider returns transient error
- **THEN** system SHALL return PROVIDER_ERROR
- **AND** error SHALL be marked as retryable
- **AND** system SHALL retry up to 3 times with exponential backoff

#### Scenario: Rate limited error
- **WHEN** provider returns rate limit error
- **THEN** system SHALL return RATE_LIMITED error
- **AND** error SHALL be marked as retryable
- **AND** system SHALL apply appropriate delay before retry

### Requirement: Ecosystem Adapter Interface
The system SHALL define trait for integrating with AI agent frameworks.

#### Scenario: Ecosystem adapter trait methods
- **WHEN** implementing ecosystem adapter
- **THEN** adapter SHALL implement get_memory_tools()
- **AND** adapter SHALL implement get_knowledge_tools()
- **AND** adapter SHALL implement get_sync_tools()
- **AND** adapter SHALL implement get_session_context()

#### Scenario: Context injection hooks
- **WHEN** ecosystem adapter needs to inject context
- **THEN** system SHALL provide onSessionStart hook
- **AND** system SHALL provide onSessionEnd hook
- **AND** system SHALL provide onMessage hook
- **AND** system SHALL provide onToolUse hook

### Requirement: OpenCode Integration
The system SHALL provide an adapter for seamless OpenCode integration.

#### Scenario: OpenCode JSON Schema format
- **WHEN** generating tool schemas for OpenCode
- **THEN** system SHALL use JSON Schema format
- **AND** system SHALL include tool names, descriptions, input schemas
- **AND** system SHALL be compatible with oh-my-opencode

#### Scenario: OpenCode tool handlers
- **WHEN** OpenCode invokes a tool
- **THEN** system SHALL call corresponding tool handler
- **AND** system SHALL translate responses to OpenCode format
- **AND** system SHALL handle errors appropriately

### Requirement: LangChain Integration
The system SHALL provide an adapter for LangChain framework compatibility.

#### Scenario: LangChain tool format
- **WHEN** generating tools for LangChain
- **THEN** system SHALL create DynamicStructuredTool instances
- **AND** system SHALL use Zod schemas for input validation
- **AND** system SHALL match LangChain tool interface

#### Scenario: LangChain context injection
- **WHEN** LangChain session starts
- **THEN** system SHALL inject active constraints into system prompt
- **AND** system SHALL rehydrate relevant memories

### Requirement: MCP Observability
The system SHALL emit metrics and logs for all MCP operations.

#### Scenario: Emit request metrics
- **WHEN** MCP request is received
- **THEN** system SHALL emit counter: mcp.requests.total
- **AND** system SHALL emit histogram: mcp.requests.duration
- **AND** system SHALL include tool name label

#### Scenario: Emit error metrics
- **WHEN** MCP request fails
- **THEN** system SHALL emit counter: mcp.errors.total
- **AND** system SHALL include error code label

#### Scenario: Emit tool invocation metrics
- **WHEN** tool is called
- **THEN** system SHALL emit counter: mcp.tool.invocations
- **AND** system SHALL include tool name label

### Requirement: Tool Response Format
The system SHALL return structured responses consistent with the specification.

#### Scenario: Success response
- **WHEN** tool operation succeeds
- **THEN** response SHALL include success=true
- **AND** response SHALL include data field with results

#### Scenario: Error response
- **WHEN** tool operation fails
- **THEN** response SHALL include success=false
- **AND** response SHALL include error field with code and message
- **AND** response SHALL include details field for additional context

### Requirement: MCP Performance
The system SHALL meet performance targets for tool responses.

#### Scenario: Tool response latency
- **WHEN** tool is invoked
- **THEN** response SHALL be returned in <200ms (P95)

#### Scenario: Concurrent request handling
- **WHEN** 100+ concurrent MCP requests are made
- **THEN** server SHALL handle all without significant degradation
- **AND** latency SHALL increase by <50%