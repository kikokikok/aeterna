# Implementation Tasks

## 1. MCP Server Setup
- [x] 1.1 Create server.rs in tools/ crate
- [x] 1.2 Define MCPServer struct
- [x] 1.3 Implement tool registry (Map<tool_name, handler>)
- [x] 1.4 Implement JSON-RPC message handling
- [x] 1.5 Implement request/response parsing
- [x] 1.6 Implement async request handler with tokio
- [x] 1.7 Implement tool discovery endpoint
- [x] 1.8 Write unit tests for MCP server

## 2. Tool Registration
- [x] 2.1 Define ToolDefinition struct
- [x] 2.2 Define ToolHandler trait
- [x] 2.3 Implement register_tool() method
- [x] 2.4 Implement list_tools() method
- [x] 2.5 Implement get_tool() method
- [x] 2.6 Write unit tests for tool registration

## 3. JSON Schema Generation
- [x] 3.1 Use schemars crate for schema generation
- [x] 3.2 Implement generate_schema<T>() function
- [x] 3.3 Add custom schema derivations
- [x] 3.4 Validate schemas against JSON Schema spec
- [x] 3.5 Write unit tests for schema generation

## 4. Memory Tools - Implementation
- [x] 4.1 Implement memory_add tool
- [x] 4.2 Define AddMemoryInput struct with validation
- [x] 4.3 Call MemoryManager.add()
- [x] 4.4 Handle errors and translate to MCP response
- [x] 4.5 Implement memory_search tool
- [x] 4.6 Define SearchMemoryInput struct with validation
- [x] 4.7 Call MemoryManager.search()
- [x] 4.8 Handle layer filtering and threshold
- [x] 4.9 Implement memory_delete tool
- [x] 4.10 Define DeleteMemoryInput struct with validation
- [x] 4.11 Call MemoryManager.delete()
- [x] 4.12 Write unit tests for all memory tools

## 5. Knowledge Tools - Implementation
- [x] 5.1 Implement knowledge_query tool
- [x] 5.2 Define QueryKnowledgeInput struct with validation
- [x] 5.3 Call KnowledgeManager.query()
- [x] 5.4 Handle filtering (type, layer, status, tags)
- [x] 5.5 Implement knowledge_check tool
- [x] 5.6 Define CheckConstraintsInput struct with validation
- [x] 5.7 Call KnowledgeManager.check_constraints()
- [x] 5.8 Aggregate violations by severity
- [x] 5.9 Implement knowledge_show tool
- [x] 5.10 Define GetKnowledgeInput struct with validation
- [x] 5.11 Call KnowledgeManager.get()
- [x] 5.12 Handle includeConstraints and includeHistory flags
- [x] 5.13 Write unit tests for all knowledge tools

## 6. Sync Tools - Implementation
- [x] 6.1 Implement sync_now tool
- [x] 6.2 Define SyncNowInput struct with validation
- [x] 6.3 Call SyncManager.full_sync()
- [x] 6.4 Handle force and types/layers filtering
- [x] 6.5 Implement sync_status tool
- [x] 6.6 Define no input required
- [x] 6.7 Call SyncManager.get_status()
- [x] 6.8 Return lastSyncAt, lastCommit, stats
- [x] 6.9 Write unit tests for all sync tools

## 7. Error Handling
- [x] 7.1 Implement ToolError enum
- [x] 7.2 Define all 7 error codes: INVALID_INPUT, NOT_FOUND, PROVIDER_ERROR, RATE_LIMITED, UNAUTHORIZED, TIMEOUT, CONFLICT
- [x] 7.3 Implement MCPErrorResponse struct
- [x] 7.4 Implement error translation from domain errors
- [x] 7.5 Set retryable flags on each error type
- [x] 7.6 Add detailed error context in details field
- [x] 7.7 Write unit tests for error handling

## 8. Input Validation
- [x] 8.1 Use validator crate for struct validation
- [x] 8.2 Derive Validate trait on all input structs
- [x] 8.3 Implement custom validators for complex types
- [x] 8.4 Validate required fields
- [x] 8.5 Validate enums and ranges
- [x] 8.6 Return INVALID_INPUT on validation failure
- [x] 8.7 Write unit tests for validation

## 9. Response Formatting
- [x] 9.1 Define MCPResponse<T> struct
- [x] 9.2 Implement success boolean field
- [x] 9.3 Implement data field (generic)
- [x] 9.4 Implement error field (ToolError)
- [x] 9.5 Implement metadata field
- [x] 9.6 Write unit tests for response formatting

## 10. OpenCode Adapter
- [x] 10.1 Create adapters/opencode/src/lib.rs
- [x] 10.2 Implement OpenCodeAdapter struct
- [x] 10.3 Implement EcosystemAdapter trait
- [x] 10.4 Implement get_memory_tools() returning JSON Schema
- [x] 10.5 Implement get_knowledge_tools() returning JSON Schema
- [x] 10.6 Implement get_session_context() for rehydration
- [x] 10.7 Implement tool handler functions
- [x] 10.8 Create OpenCode plugin manifest
- [x] 10.9 Write integration tests with OpenCode

## 11. LangChain Adapter
- [x] 11.1 Create adapters/langchain/src/lib.rs
- [x] 11.2 Implement LangChainAdapter struct
- [x] 11.3 Implement EcosystemAdapter trait
- [x] 11.4 Convert tool definitions to LangChain format
- [x] 11.5 Use Zod for schema validation
- [x] 11.6 Create DynamicStructuredTool instances
- [x] 11.7 Implement context injection hooks
- [x] 11.8 Write integration tests with LangChain

## 12. Context Injection Hooks
- [x] 12.1 Define ContextHooks struct
- [x] 12.2 Implement onSessionStart hook
- [x] 12.3 Implement onSessionEnd hook
- [x] 12.4 Implement onMessage hook
- [x] 12.5 Implement onToolUse hook
- [x] 12.6 Implement get_active_constraints() helper
- [x] 12.7 Write unit tests for all hooks

## 13. Async Runtime
- [x] 13.1 Set up tokio runtime in tools/ crate
- [x] 13.2 Configure tokio with multi-threaded scheduler
- [x] 13.3 Implement graceful shutdown
- [x] 13.4 Handle concurrent requests
- [x] 13.5 Add request timeout handling
- [x] 13.6 Write unit tests for async behavior

## 14. Observability
- [x] 14.1 Integrate OpenTelemetry for MCP operations
- [x] 14.2 Add Prometheus metrics
- [x] 14.3 Emit metrics: mcp.requests.total, mcp.requests.duration
- [x] 14.4 Emit metrics: mcp.tool.invocations (by tool name)
- [x] 14.5 Emit metrics: mcp.errors.total (by error code)
- [x] 14.6 Add structured logging with tracing spans
- [x] 14.7 Configure metric histograms

## 15. Integration Tests
- [x] 15.1 Create MCP server integration test suite
- [x] 15.2 Test tools/list endpoint
- [x] 15.3 Test all 8 tools with valid inputs
- [x] 15.4 Test all 8 tools with invalid inputs
- [x] 15.5 Test error responses
- [x] 15.6 Test concurrent requests
- [x] 15.7 Test OpenCode adapter integration
- [x] 15.8 Test LangChain adapter integration
- [x] 15.9 Test context injection hooks
- [x] 15.10 Ensure 85%+ test coverage

## 16. Documentation
- [x] 16.1 Document MCP server public API
- [x] 16.2 Document tool registration API
- [x] 16.3 Document JSON Schema generation
- [x] 16.4 Document OpenCode adapter usage
- [x] 16.5 Document LangChain adapter usage
- [x] 16.6 Document context injection hooks
- [x] 16.7 Add inline examples for all tools
- [x] 16.8 Write MCP protocol documentation
- [x] 16.9 Update crate README
