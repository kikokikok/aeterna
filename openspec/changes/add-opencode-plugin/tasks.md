# Tasks: OpenCode Plugin Integration

## 1. NPM Package Setup

- [ ] 1.1 Create `packages/opencode-plugin` directory with TypeScript config
- [ ] 1.2 Initialize `package.json` for `@aeterna/opencode-plugin`
- [ ] 1.3 Add dependencies: `@opencode-ai/plugin`, `zod`, TypeScript tooling
- [ ] 1.4 Create `tsconfig.json` with ESM output and strict mode
- [ ] 1.5 Set up build script and entry point (`src/index.ts`)

## 2. Aeterna Client Library

- [ ] 2.1 Create `packages/aeterna-client` TypeScript library
- [ ] 2.2 Implement gRPC/HTTP client for Aeterna backend
- [ ] 2.3 Add methods: `sessionStart`, `sessionEnd`, `memoryAdd`, `memorySearch`
- [ ] 2.4 Add methods: `knowledgeQuery`, `knowledgePropose`, `syncStatus`
- [ ] 2.5 Add methods: `governanceStatus`, `checkProposalPermission`
- [ ] 2.6 Implement connection pooling and retry logic
- [ ] 2.7 Add context helpers: `getProjectContext`, `queryRelevantKnowledge`

## 3. Tool Implementations

- [ ] 3.1 Implement `aeterna_memory_add` tool with Zod schema
- [ ] 3.2 Implement `aeterna_memory_search` tool with Zod schema
- [ ] 3.3 Implement `aeterna_memory_get` tool with Zod schema
- [ ] 3.4 Implement `aeterna_memory_promote` tool with Zod schema
- [ ] 3.5 Implement `aeterna_knowledge_query` tool with Zod schema
- [ ] 3.6 Implement `aeterna_knowledge_propose` tool with Zod schema
- [ ] 3.7 Implement `aeterna_sync_status` tool with Zod schema
- [ ] 3.8 Implement `aeterna_governance_status` tool with Zod schema
- [ ] 3.9 Create tool result formatters for consistent output

## 4. Chat Hooks

- [ ] 4.1 Implement `chat.message` hook for knowledge injection
- [ ] 4.2 Query relevant knowledge based on user message content
- [ ] 4.3 Query session memories for additional context
- [ ] 4.4 Format and prepend `<aeterna_context>` block to message parts
- [ ] 4.5 Add configurable threshold and max items

## 5. System Prompt Hooks

- [ ] 5.1 Implement `experimental.chat.system.transform` hook
- [ ] 5.2 Inject project context (project, team, org)
- [ ] 5.3 Inject active governance policies
- [ ] 5.4 Inject recent learnings summary
- [ ] 5.5 Add tool usage guidance to system prompt

## 6. Tool Execution Hooks

- [ ] 6.1 Implement `tool.execute.before` hook for arg validation/enrichment
- [ ] 6.2 Implement `tool.execute.after` hook for execution capture
- [ ] 6.3 Capture tool metadata: tool name, session ID, call ID, output
- [ ] 6.4 Implement significance detection for promotion flagging
- [ ] 6.5 Add debouncing for high-frequency tool calls

## 7. Permission Hooks

- [ ] 7.1 Implement `permission.ask` hook for governance checks
- [ ] 7.2 Check proposal permissions before knowledge proposals
- [ ] 7.3 Validate against governance policies
- [ ] 7.4 Return deny status with reason when not permitted

## 8. Session Lifecycle

- [ ] 8.1 Implement `event` hook handler
- [ ] 8.2 Handle `session.end` event - call `client.sessionEnd()`
- [ ] 8.3 Handle session start in plugin initialization
- [ ] 8.4 Implement session state cleanup on end
- [ ] 8.5 Add graceful handling for unexpected disconnects

## 9. Plugin Entry Point

- [ ] 9.1 Create main plugin function in `src/index.ts`
- [ ] 9.2 Initialize AeternaClient from plugin input context
- [ ] 9.3 Register all 8 tools in hooks return object
- [ ] 9.4 Register all hook handlers (chat, tool, permission, event)
- [ ] 9.5 Export plugin as default export
- [ ] 9.6 Add configuration loading from environment and `.aeterna/config.toml`

## 10. MCP Server (Alternative Integration)

- [ ] 10.1 Create `packages/aeterna-mcp` package
- [ ] 10.2 Implement MCP server using `@modelcontextprotocol/sdk`
- [ ] 10.3 Register all 8 tools with JSON Schema definitions
- [ ] 10.4 Implement tool call handler
- [ ] 10.5 Expose resources: `aeterna://knowledge/project`, `aeterna://memory/session`
- [ ] 10.6 Add stdio transport for local mode
- [ ] 10.7 Add HTTP transport with auth for remote mode

## 11. Configuration & CLI

- [ ] 11.1 Create CLI command: `npx aeterna init --opencode`
- [ ] 11.2 Generate `opencode.jsonc` with plugin config
- [ ] 11.3 Generate `.aeterna/config.toml` with project settings
- [ ] 11.4 Add `.aeterna` to `.gitignore` if not present
- [ ] 11.5 Validate configuration on plugin load
- [ ] 11.6 Support environment variable overrides

## 12. Utility Functions

- [ ] 12.1 Create `formatKnowledgeContext()` for context blocks
- [ ] 12.2 Create `formatMemoryContext()` for memory display
- [ ] 12.3 Create `formatMemoryResults()` for search results
- [ ] 12.4 Create `formatKnowledgeResults()` for query results
- [ ] 12.5 Implement significance detection algorithm
- [ ] 12.6 Create context size estimator for token limits

## 13. Testing

- [ ] 13.1 Unit tests for Aeterna client methods
- [ ] 13.2 Unit tests for each tool implementation
- [ ] 13.3 Unit tests for hook handlers
- [ ] 13.4 Integration tests with mock OpenCode plugin context
- [ ] 13.5 Integration tests with mock Aeterna backend
- [ ] 13.6 End-to-end tests for full plugin flow

## 14. Documentation

- [ ] 14.1 OpenCode plugin installation guide
- [ ] 14.2 Configuration reference (TOML and environment)
- [ ] 14.3 Tool usage examples
- [ ] 14.4 MCP server setup guide (alternative integration)
- [ ] 14.5 Troubleshooting guide

---

## 15. Production Gap Requirements

### 15.1 Plugin SDK Version Stability (OC-C1) - CRITICAL
- [ ] 15.1.1 Pin `@opencode-ai/plugin` to exact version in `package.json` (e.g., `"@opencode-ai/plugin": "0.1.0"`)
- [ ] 15.1.2 Create `src/sdk-abstraction/index.ts` module
- [ ] 15.1.3 Define `OpenCodeAdapter` interface abstracting SDK operations
- [ ] 15.1.4 Implement `OpenCodeSDKAdapter` class implementing the interface
- [ ] 15.1.5 Add SDK version detection and logging on initialization
- [ ] 15.1.6 Create compatibility test suite in `tests/sdk-compatibility.test.ts`
- [ ] 15.1.7 Document SDK version in CHANGELOG.md when updated

### 15.2 Credential Security (OC-C2) - CRITICAL
- [ ] 15.2.1 Create `src/security/credential-masker.ts` module
- [ ] 15.2.2 Implement log masking for `AETERNA_TOKEN` and sensitive env vars
- [ ] 15.2.3 Add mask format: `[REDACTED:...last4chars]`
- [ ] 15.2.4 Create `src/security/secure-storage.ts` for keychain integration
- [ ] 15.2.5 Implement macOS Keychain, Windows Credential Manager, Linux Secret Service support
- [ ] 15.2.6 Add token refresh mechanism without service interruption
- [ ] 15.2.7 Implement token rotation event handling
- [ ] 15.2.8 Write security tests for credential masking

### 15.3 Experimental Hook Fallback (OC-H1) - HIGH
- [ ] 15.3.1 Add `experimental_hooks` feature flags section to config
- [ ] 15.3.2 Implement `HookRegistry` with availability detection
- [ ] 15.3.3 Create fallback implementations for each experimental hook
- [ ] 15.3.4 Implement `system.transform` fallback (inject via chat.message)
- [ ] 15.3.5 Add runtime hook availability checking
- [ ] 15.3.6 Implement warning system for experimental hook usage
- [ ] 15.3.7 Write tests for fallback scenarios

### 15.4 Session Capture Performance (OC-H2) - HIGH
- [ ] 15.4.1 Refactor `tool.execute.after` handler to use async queue
- [ ] 15.4.2 Implement capture queue with configurable batch size
- [ ] 15.4.3 Add sampling when rate exceeds threshold (default: 10/sec)
- [ ] 15.4.4 Implement debouncing for similar tool executions (default: 500ms)
- [ ] 15.4.5 Add `capture.async_enabled`, `capture.sample_rate`, `capture.debounce_ms` config options
- [ ] 15.4.6 Implement capture latency metrics
- [ ] 15.4.7 Write performance benchmark tests

### 15.5 Knowledge Query Performance (OC-H3) - HIGH
- [ ] 15.5.1 Implement session-start pre-fetch for project/team knowledge
- [ ] 15.5.2 Create `src/cache/knowledge-cache.ts` module
- [ ] 15.5.3 Implement LRU cache with TTL (default: 60s)
- [ ] 15.5.4 Add timeout handling with fallback to cache (default: 200ms)
- [ ] 15.5.5 Implement background refresh for frequently accessed knowledge
- [ ] 15.5.6 Add cache hit/miss metrics
- [ ] 15.5.7 Write tests for cache and timeout scenarios

### 15.6 Session State Persistence (OC-H4) - HIGH
- [ ] 15.6.1 Create `src/storage/session-storage.ts` interface
- [ ] 15.6.2 Implement `RedisSessionStorage` class
- [ ] 15.6.3 Implement `LocalFileSessionStorage` fallback class
- [ ] 15.6.4 Add storage mode auto-detection (Redis available → use Redis)
- [ ] 15.6.5 Implement session TTL enforcement (default: 24 hours)
- [ ] 15.6.6 Add session cleanup job for expired sessions
- [ ] 15.6.7 Write integration tests for both storage backends

### 15.7 MCP Server Health Management (OC-H5) - HIGH
- [ ] 15.7.1 Add `/health` endpoint to MCP server
- [ ] 15.7.2 Implement health check logic (backend connectivity, memory usage)
- [ ] 15.7.3 Create `src/mcp/supervisor.ts` with restart logic
- [ ] 15.7.4 Implement exponential backoff for restarts (initial: 1s, max: 30s, 3 retries)
- [ ] 15.7.5 Add crash event metrics and alerting hooks
- [ ] 15.7.6 Implement graceful shutdown with in-flight request draining
- [ ] 15.7.7 Write tests for crash recovery scenarios

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 5 | NPM Package Setup |
| 2 | 7 | Aeterna Client Library |
| 3 | 9 | Tool Implementations |
| 4 | 5 | Chat Hooks |
| 5 | 5 | System Prompt Hooks |
| 6 | 5 | Tool Execution Hooks |
| 7 | 4 | Permission Hooks |
| 8 | 5 | Session Lifecycle |
| 9 | 6 | Plugin Entry Point |
| 10 | 7 | MCP Server |
| 11 | 6 | Configuration & CLI |
| 12 | 6 | Utility Functions |
| 13 | 6 | Testing |
| 14 | 5 | Documentation |
| 15 | 49 | Production Gap Requirements (OC-C1 to OC-H5) |
| **Total** | **130** | |

**Estimated effort**: 5-6 weeks with 80% test coverage target

---

## Production Gap Tracking

| Gap ID | Priority | Requirement | Tasks |
|--------|----------|-------------|-------|
| OC-C1 | Critical | Plugin SDK Version Stability | 15.1.1-15.1.7 |
| OC-C2 | Critical | Credential Security | 15.2.1-15.2.8 |
| OC-H1 | High | Experimental Hook Fallback | 15.3.1-15.3.7 |
| OC-H2 | High | Session Capture Performance | 15.4.1-15.4.7 |
| OC-H3 | High | Knowledge Query Performance | 15.5.1-15.5.7 |
| OC-H4 | High | Session State Persistence | 15.6.1-15.6.7 |
| OC-H5 | High | MCP Server Health Management | 15.7.1-15.7.7 |
