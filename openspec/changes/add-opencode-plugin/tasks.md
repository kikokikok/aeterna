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
