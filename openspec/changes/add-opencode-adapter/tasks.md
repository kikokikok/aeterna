# Implementation Tasks

## 1. OpenCode Adapter Core
- [x] 1.1 Create adapters/opencode/src/lib.rs
- [x] 1.2 Implement OpenCodeAdapter struct
- [x] 1.3 Implement EcosystemAdapter trait
- [x] 1.4 Define OpenCode-specific configuration
- [x] 1.5 Write unit tests for adapter core

## 2. JSON Schema Generation
- [x] 2.1 Implement generate_schema_for_tool() function
- [x] 2.2 Generate schemas for all 8 tools
- [x] 2.3 Validate schemas against JSON Schema spec
- [x] 2.4 Add descriptions and examples
- [x] 2.5 Write unit tests for schema generation

## 3. Tool Handler Functions
- [x] 3.1 Implement memory_add handler
- [x] 3.2 Implement memory_search handler
- [x] 3.3 Implement memory_delete handler
- [x] 3.4 Implement knowledge_query handler
- [x] 3.5 Implement knowledge_check handler
- [x] 3.6 Implement knowledge_show handler
- [x] 3.7 Implement sync_now handler
- [x] 3.8 Implement sync_status handler
- [x] 3.9 Call appropriate managers
- [x] 3.10 Translate responses to OpenCode format
- [x] 3.11 Write unit tests for all handlers

## 4. OpenCode Plugin Manifest
- [x] 4.1 Create plugin-manifest.json
- [x] 4.2 Define plugin metadata
- [x] 4.3 Register all 8 tools
- [x] 4.4 Define dependencies
- [x] 4.5 Add tool descriptions
- [x] 4.6 Validate manifest against OpenCode spec

## 5. Context Injection
- [x] 5.1 Implement onSessionStart hook
- [x] 5.2 Load relevant memories on session start
- [x] 5.3 Load active constraints on session start
- [x] 5.4 Inject context into OpenCode
- [x] 5.5 Implement onSessionEnd hook
- [x] 5.6 Clean up session state on session end
- [x] 5.7 Write integration tests for context injection

## 6. OpenCode Integration
- [x] 6.1 Test with oh-my-opencode plugin system
- [x] 6.2 Verify tool discovery works
- [x] 6.3 Verify tool invocation works
- [x] 6.4 Verify error handling works
- [x] 6.5 Write end-to-end integration tests

## 7. Documentation
- [x] 7.1 Document OpenCode adapter usage
- [x] 7.2 Document installation process
- [x] 7.3 Document configuration options
- [x] 7.4 Provide examples in README
- [x] 7.5 Add inline code examples
- [x] 7.6 Update adapters/opencode README

## 8. Quality Assurance
- [x] 8.1 Write unit tests for all functions
- [x] 8.2 Write integration tests with OpenCode
- [x] 8.3 Test error handling
- [x] 8.4 Test tool parameter validation
- [x] 8.5 Ensure 85%+ test coverage
