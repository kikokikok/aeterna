# Design: OpenCode Plugin Integration

## Context

OpenCode is a Go/TypeScript TUI/CLI for AI-assisted coding with a rich plugin ecosystem:

- **NPM Plugins**: Full TypeScript plugins via `@opencode-ai/plugin` SDK with hooks
- **Custom Tools**: TypeScript files in `.opencode/tool/*.ts` auto-discovered
- **MCP Servers**: Model Context Protocol integrations for external services
- **Agents/Commands**: Markdown-based AI behavior customization

Target integration: Deep Aeterna integration using OpenCode's native plugin architecture.

## Goals / Non-Goals

### Goals
- Create `@aeterna/opencode-plugin` NPM package using official SDK
- Expose Aeterna tools as OpenCode tools (not just MCP)
- Use OpenCode's native hook system for deep integration
- Automatic session capture via `tool.execute.after` hooks
- Knowledge injection via `chat.message` and system prompt hooks
- Zero-config setup with sensible defaults

### Non-Goals
- Offline support (OpenCode requires LLM providers anyway)
- Custom UI components (use OpenCode's native UI)
- Replacing OpenCode's built-in features

## Decisions

### 1. Dual Integration Strategy

We'll provide **two integration methods** to maximize flexibility:

| Method | Use Case | Complexity |
|--------|----------|------------|
| **NPM Plugin** | Full integration with hooks | Install package |
| **MCP Server** | External/remote deployments | Configure URL |

Both share the same Aeterna backend, but the NPM plugin provides deeper hooks.

### 2. NPM Plugin Architecture

Using OpenCode's official `@opencode-ai/plugin` SDK:

```typescript
// packages/opencode-plugin/src/index.ts
import type { Plugin, Hooks, PluginInput } from "@opencode-ai/plugin"
import { tool } from "@opencode-ai/plugin/tool"
import { AeternaClient } from "@aeterna/client"

const aeterna: Plugin = async (input: PluginInput): Promise<Hooks> => {
  const client = new AeternaClient({
    project: input.project.name,
    directory: input.directory,
    serverUrl: process.env.AETERNA_SERVER_URL,
  })

  await client.sessionStart()

  return {
    // Register Aeterna tools
    tool: {
      aeterna_memory_add: memoryAddTool(client),
      aeterna_memory_search: memorySearchTool(client),
      aeterna_memory_get: memoryGetTool(client),
      aeterna_memory_promote: memoryPromoteTool(client),
      aeterna_knowledge_query: knowledgeQueryTool(client),
      aeterna_knowledge_propose: knowledgeProposeTool(client),
      aeterna_sync_status: syncStatusTool(client),
      aeterna_governance_status: governanceStatusTool(client),
    },

    // Hook: Inject knowledge into chat messages
    "chat.message": async (input, output) => {
      const knowledge = await client.queryRelevantKnowledge(output.message.content)
      if (knowledge.length > 0) {
        output.parts.unshift({
          type: "text",
          text: formatKnowledgeContext(knowledge),
        })
      }
    },

    // Hook: Modify system prompt with project context
    "experimental.chat.system.transform": async (input, output) => {
      const context = await client.getProjectContext()
      output.system.push(formatSystemContext(context))
    },

    // Hook: Capture tool executions as memory
    "tool.execute.after": async (input, output) => {
      await client.captureToolExecution({
        tool: input.tool,
        sessionId: input.sessionID,
        callId: input.callID,
        output: output.output,
        metadata: output.metadata,
      })
    },

    // Hook: Pre-validate tool arguments
    "tool.execute.before": async (input, output) => {
      if (input.tool.startsWith("aeterna_")) {
        await client.enrichToolArgs(input.tool, output.args)
      }
    },

    // Hook: Handle governance permissions
    "permission.ask": async (input, output) => {
      if (input.tool?.startsWith("aeterna_knowledge_propose")) {
        const canPropose = await client.checkProposalPermission()
        if (!canPropose) {
          output.status = "deny"
        }
      }
    },

    // Hook: React to events (session end, etc.)
    event: async ({ event }) => {
      if (event.type === "session.end") {
        await client.sessionEnd()
      }
    },
  }
}

export default aeterna
```

### 3. Tool Implementations

Using OpenCode's `tool()` helper with Zod schemas:

```typescript
// packages/opencode-plugin/src/tools/memory.ts
import { tool } from "@opencode-ai/plugin/tool"
import { z } from "zod"

export const memoryAddTool = (client: AeternaClient) => tool({
  description: "Add a memory entry to Aeterna. Use this to capture learnings, solutions, or important context.",
  args: {
    content: z.string().describe("The content to remember"),
    layer: z.enum(["working", "session", "episodic"]).optional()
      .describe("Memory layer (default: working)"),
    tags: z.array(z.string()).optional()
      .describe("Tags for categorization"),
    importance: z.number().min(0).max(1).optional()
      .describe("Importance score 0-1 (default: auto-calculated)"),
  },
  async execute(args, context) {
    const result = await client.memoryAdd({
      content: args.content,
      layer: args.layer ?? "working",
      tags: args.tags,
      importance: args.importance,
      sessionId: context.sessionID,
    })
    return `Memory added: ${result.id} (layer: ${result.layer}, importance: ${result.importance})`
  },
})

export const memorySearchTool = (client: AeternaClient) => tool({
  description: "Search memories for relevant context. Returns semantically similar memories.",
  args: {
    query: z.string().describe("Search query"),
    layers: z.array(z.enum(["working", "session", "episodic"])).optional()
      .describe("Layers to search (default: all)"),
    limit: z.number().min(1).max(20).optional()
      .describe("Max results (default: 5)"),
    threshold: z.number().min(0).max(1).optional()
      .describe("Similarity threshold (default: 0.7)"),
  },
  async execute(args, context) {
    const results = await client.memorySearch({
      query: args.query,
      layers: args.layers,
      limit: args.limit ?? 5,
      threshold: args.threshold ?? 0.7,
      sessionId: context.sessionID,
    })
    return formatMemoryResults(results)
  },
})

export const knowledgeQueryTool = (client: AeternaClient) => tool({
  description: "Query the knowledge repository for project/team/org knowledge.",
  args: {
    query: z.string().describe("Search query"),
    scope: z.enum(["project", "team", "org", "company"]).optional()
      .describe("Knowledge scope (default: project)"),
    types: z.array(z.enum(["adr", "pattern", "policy", "reference"])).optional()
      .describe("Knowledge types to include"),
    limit: z.number().min(1).max(10).optional()
      .describe("Max results (default: 3)"),
  },
  async execute(args, context) {
    const results = await client.knowledgeQuery({
      query: args.query,
      scope: args.scope ?? "project",
      types: args.types,
      limit: args.limit ?? 3,
    })
    return formatKnowledgeResults(results)
  },
})
```

### 4. Hook Implementations

**Chat Message Hook** - Inject relevant knowledge:

```typescript
"chat.message": async (input, output) => {
  // Extract the user's message content
  const userMessage = output.message.content
  
  // Query relevant knowledge based on message
  const knowledge = await client.queryRelevantKnowledge(userMessage, {
    limit: 3,
    threshold: 0.75,
  })
  
  // Query relevant memories from session
  const memories = await client.searchSessionMemories(userMessage, {
    limit: 5,
  })
  
  if (knowledge.length > 0 || memories.length > 0) {
    // Prepend context to the message parts
    output.parts.unshift({
      type: "text",
      text: `<aeterna_context>
${formatKnowledgeContext(knowledge)}
${formatMemoryContext(memories)}
</aeterna_context>`,
    })
  }
}
```

**System Transform Hook** - Add project context to system prompt:

```typescript
"experimental.chat.system.transform": async (input, output) => {
  const context = await client.getProjectContext()
  
  output.system.push(`
## Aeterna Project Context

Project: ${context.project.name}
Team: ${context.team?.name ?? "N/A"}
Organization: ${context.org?.name ?? "N/A"}

### Active Policies
${context.policies.map(p => `- ${p.name}: ${p.summary}`).join("\n")}

### Recent Learnings
${context.recentMemories.map(m => `- ${m.summary}`).join("\n")}

When you discover useful patterns or solutions, use aeterna_memory_add to capture them.
When you need context, use aeterna_memory_search or aeterna_knowledge_query.
`)
}
```

**Tool Execute After Hook** - Capture significant interactions:

```typescript
"tool.execute.after": async (input, output) => {
  // Capture all tool executions for context
  await client.captureToolExecution({
    tool: input.tool,
    sessionId: input.sessionID,
    callId: input.callID,
    title: output.title,
    output: output.output,
    metadata: output.metadata,
    timestamp: Date.now(),
  })
  
  // Detect significant patterns
  if (await client.detectSignificance(input, output)) {
    await client.flagForPromotion(input.sessionID, input.callID)
  }
}
```

### 5. MCP Server (Alternative Integration)

For remote/hybrid deployments, we also provide an MCP server:

```typescript
// packages/aeterna-mcp/src/index.ts
import { Server } from "@modelcontextprotocol/sdk/server/index.js"
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js"

const server = new Server({
  name: "aeterna",
  version: "1.0.0",
}, {
  capabilities: {
    tools: {},
    resources: {},
  },
})

// Register tools
server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    { name: "aeterna_memory_add", description: "...", inputSchema: {...} },
    { name: "aeterna_memory_search", description: "...", inputSchema: {...} },
    // ... all 8 tools
  ],
}))

// Handle tool calls
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  const result = await handleToolCall(name, args)
  return { content: [{ type: "text", text: result }] }
})

// Expose resources
server.setRequestHandler(ListResourcesRequestSchema, async () => ({
  resources: [
    { uri: "aeterna://knowledge/project", name: "Project Knowledge" },
    { uri: "aeterna://memory/session", name: "Session Memory" },
  ],
}))

const transport = new StdioServerTransport()
await server.connect(transport)
```

### 6. Configuration

**OpenCode config** (`opencode.jsonc`):

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  
  // Option 1: NPM Plugin (recommended for full integration)
  "plugin": ["@aeterna/opencode-plugin"],
  
  // Option 2: MCP Server (for remote deployments)
  "mcp": {
    "aeterna": {
      "type": "local",
      "command": ["aeterna-mcp", "--mode", "stdio"],
      "env": {
        "AETERNA_PROJECT": "${project.name}"
      }
    }
  },
  
  // Or remote MCP
  "mcp": {
    "aeterna": {
      "type": "remote",
      "url": "https://aeterna.company.com/mcp",
      "headers": {
        "Authorization": "Bearer ${AETERNA_TOKEN}"
      }
    }
  }
}
```

**Aeterna config** (`.aeterna/config.toml`):

```toml
[project]
name = "my-project"
team = "backend"
org = "engineering"

[capture]
enabled = true
sensitivity = "medium"  # low, medium, high
auto_promote = true

[knowledge]
injection_enabled = true
max_items = 3
threshold = 0.75

[governance]
notifications = true
drift_alerts = true
```

### 7. Setup Experience

**One-command setup**:

```bash
# Install plugin
npm install -D @aeterna/opencode-plugin

# Initialize configuration
npx aeterna init --opencode

# This creates:
# - opencode.jsonc with plugin config
# - .aeterna/config.toml with project settings
# - Adds .aeterna to .gitignore
```

**Package structure**:

```
@aeterna/opencode-plugin/
├── package.json
├── src/
│   ├── index.ts        # Plugin entry point
│   ├── client.ts       # Aeterna client wrapper
│   ├── tools/
│   │   ├── memory.ts   # Memory tools
│   │   ├── knowledge.ts # Knowledge tools
│   │   └── governance.ts # Governance tools
│   ├── hooks/
│   │   ├── chat.ts     # Chat hooks
│   │   ├── tool.ts     # Tool execution hooks
│   │   └── session.ts  # Session lifecycle
│   └── utils/
│       ├── format.ts   # Output formatting
│       └── detect.ts   # Significance detection
└── README.md
```

### Alternatives Considered

1. **MCP-only approach**: Rejected - misses native hook integration for deeper features
2. **Custom tool files only**: Rejected - no lifecycle hooks, limited capabilities
3. **Fork OpenCode**: Rejected - high maintenance, upstream drift

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Plugin SDK changes | Pin to stable version, abstract SDK layer |
| Hook API experimental | Wrap experimental hooks with feature flags |
| Session capture overhead | Async capture, debouncing, configurable |
| Knowledge query latency | Local caching, pre-fetch on session start |

## Migration Plan

1. **Phase 1**: NPM plugin with basic tools
2. **Phase 2**: Chat hooks for knowledge injection
3. **Phase 3**: Tool execution hooks for capture
4. **Phase 4**: Governance integration
5. **Phase 5**: MCP server for remote deployments

## Open Questions

- [ ] Plugin package name: `@aeterna/opencode-plugin` or `aeterna-opencode`?
- [ ] TypeScript client: bundled or separate `@aeterna/client` package?
- [ ] Experimental hooks: use immediately or wait for stable?
- [ ] Session persistence: Redis or local file?
