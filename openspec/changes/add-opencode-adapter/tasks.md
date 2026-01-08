# Implementation Tasks

## 1. OpenCode Adapter Core
- [ ] 1.1 Create adapters/opencode/src/lib.rs
- [ ] 1.2 Implement OpenCodeAdapter struct
- [ ] 1.3 Implement EcosystemAdapter trait
- [ ] 1.4 Define OpenCode-specific configuration
- [ ] 1.5 Write unit tests for adapter core

## 2. JSON Schema Generation
- [ ] 2.1 Implement generate_schema_for_tool() function
- [ ] 2.2 Generate schemas for all 8 tools
- [ ] 2.3 Validate schemas against JSON Schema spec
- [ ] 2.4 Add descriptions and examples
- [ ] 2.5 Write unit tests for schema generation

## 3. Tool Handler Functions
- [ ] 3.1 Implement memory_add handler
- [ ] 3.2 Implement memory_search handler
- [ ] 3.3 Implement memory_delete handler
- [ ] 3.4 Implement knowledge_query handler
- [ ] 3.5 Implement knowledge_check handler
- [ ] 3.6 Implement knowledge_show handler
- [ ] 3.7 Implement sync_now handler
- [ ] 3.8 Implement sync_status handler
- [ ] 3.9 Call appropriate managers
- [ ] 3.10 Translate responses to OpenCode format
- [ ] 3.11 Write unit tests for all handlers

## 4. OpenCode Plugin Manifest
- [ ] 4.1 Create plugin-manifest.json
- [ ] 4.2 Define plugin metadata
- [ ] 4.3 Register all 8 tools
- [ ] 4.4 Define dependencies
- [ ] 4.5 Add tool descriptions
- [ ] 4.6 Validate manifest against OpenCode spec

## 5. Context Injection
- [ ] 5.1 Implement onSessionStart hook
- [ ] 5.2 Load relevant memories on session start
- [ ] 5.3 Load active constraints on session start
- [ ] 5.4 Inject context into OpenCode
- [ ] 5.5 Implement onSessionEnd hook
- [ ] 5.6 Clean up session state on session end
- [ ] 5.7 Write integration tests for context injection

## 6. OpenCode Integration
- [ ] 6.1 Test with oh-my-opencode plugin system
- [ ] 6.2 Verify tool discovery works
- [ ] 6.3 Verify tool invocation works
- [ ] 6.4 Verify error handling works
- [ ] 6.5 Write end-to-end integration tests

## 7. Documentation
- [ ] 7.1 Document OpenCode adapter usage
- [ ] 7.2 Document installation process
- [ ] 7.3 Document configuration options
- [ ] 7.4 Provide examples in README
- [ ] 7.5 Add inline code examples
- [ ] 7.6 Update adapters/opencode README

## 8. Quality Assurance
- [ ] 8.1 Write unit tests for all functions
- [ ] 8.2 Write integration tests with OpenCode
- [ ] 8.3 Test error handling
- [ ] 8.4 Test tool parameter validation
- [ ] 8.5 Ensure 85%+ test coverage
