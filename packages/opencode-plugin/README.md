# @kiko-aeterna/opencode-plugin

OpenCode plugin for Aeterna memory and knowledge integration with CCA and RLM support.

## Features

- **Memory Management**: Add, search, retrieve, and promote memories across 7 layers
- **Knowledge Repository**: Query and propose knowledge items with governance
- **Graph Layer**: Explore memory relationships and find paths
- **CCA Capabilities**:
  - Context Architect: Assemble hierarchical context with token budgeting
  - Note-Taking Agent: Capture trajectory events for distillation
  - Hindsight Learning: Query error patterns and resolutions
  - Meta-Agent: Build-test-improve loop status
- **Governance**: Check sync status and governance compliance
- **Automatic Capture**: Tool executions captured as working memory
- **Knowledge Injection**: Relevant knowledge auto-injected into chat context

## Installation

```bash
npm install -D @kiko-aeterna/opencode-plugin
```

## Configuration

Add the plugin to your `opencode.jsonc`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",

  "plugin": ["@kiko-aeterna/opencode-plugin"]
}
```

### Environment Variables

| Variable | Description | Default |
|-----------|-------------|---------|
| `AETERNA_SERVER_URL` | Aeterna server URL | `http://localhost:8080` |
| `AETERNA_TOKEN` | API authentication token | (required for API calls) |
| `AETERNA_TEAM` | Team context for multi-tenant hierarchy | (optional) |
| `AETERNA_ORG` | Organization context for multi-tenant hierarchy | (optional) |
| `AETERNA_USER_ID` | User ID for personalization | (optional) |

### Aeterna Configuration

The plugin also supports a `.aeterna/config.toml` file for project-specific settings:

```toml
[capture]
enabled = true
sensitivity = "medium"  # low, medium, high
auto_promote = true
sample_rate = 1.0
debounce_ms = 500

[knowledge]
injection_enabled = true
max_items = 3
threshold = 0.75
cache_ttl_seconds = 60
timeout_ms = 200

[governance]
notifications = true
drift_alerts = true

[session]
storage_ttl_hours = 24
use_redis = false

[experimental]
system_prompt_hook = true
permission_hook = true
```

## Usage

### Memory Tools

- `aeterna_memory_add` - Capture learnings, solutions, or important context
- `aeterna_memory_search` - Find semantically similar memories
- `aeterna_memory_get` - Retrieve a specific memory by ID
- `aeterna_memory_promote` - Promote memory to higher layers

### Knowledge Tools

- `aeterna_knowledge_query` - Search knowledge repository by scope and type
- `aeterna_knowledge_propose` - Propose new knowledge (requires approval)

### Graph Tools

- `aeterna_graph_query` - Query memory relationships and traversals
- `aeterna_graph_neighbors` - Find memories directly related to a memory
- `aeterna_graph_path` - Find shortest path between two memories

### CCA Tools

- `aeterna_context_assemble` - Assemble hierarchical context from memory layers
- `aeterna_note_capture` - Capture trajectory events for note distillation
- `aeterna_hindsight_query` - Find error patterns and resolutions
- `aeterna_meta_loop_status` - Get meta-agent build-test-improve loop status

### Governance Tools

- `aeterna_sync_status` - Check sync status between memory and knowledge
- `aeterna_governance_status` - Check governance state and compliance

## Hooks

The plugin implements several OpenCode hooks for deep integration:

1. **Chat Message Hook** - Injects relevant knowledge and memories into user messages
2. **System Prompt Hook** - Adds project context, active policies, and Aeterna guidance to system prompt
3. **Tool Execute Before Hook** - Enriches Aeterna tool arguments with session context
4. **Tool Execute After Hook** - Captures tool executions as working memory, flags significant patterns for promotion
5. **Permission Hook** - Validates knowledge proposal permissions
6. **Session Event Hook** - Manages session lifecycle and cleanup

## How It Works

1. **Session Start**: When an OpenCode session starts, the plugin:
   - Initializes the Aeterna client
   - Starts a session context on the backend
   - Prefetches frequently accessed knowledge
   - Subscribes to governance events

2. **Automatic Knowledge Injection**:
   - When you send a message, the plugin automatically:
     - Queries relevant knowledge based on message content
     - Searches session memories for recent context
     - Injects the combined context into the chat

3. **Tool Execution Capture**:
   - Every tool execution is automatically captured
   - Significant patterns (error resolution, repeated usage) are flagged
   - Captured memories are available for future sessions

4. **Session End**: When session ends, the plugin:
   - Generates a session summary
   - Flushes all pending captures to the backend
   - Promotes significant memories to broader layers
   - Cleans up subscriptions and caches

## Development

```bash
# Install dependencies
npm install

# Build the plugin
npm run build

# Watch mode for development
npm run dev

# Type check
npm run typecheck
```

## Integration with Rust Backend

The plugin communicates with Aeterna's Rust backend via HTTP API:

- **Memory**: `/api/v1/memories/*` operations
- **Knowledge**: `/api/v1/knowledge/*` operations
- **Graph**: `/api/v1/graph/*` operations
- **CCA**: `/api/v1/cca/*` operations
- **Governance**: `/api/v1/governance/*` operations
- **Session**: `/api/v1/sessions/*` operations

All requests include proper authentication and tenant context headers.

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please read the main Aeterna repository for guidelines.

## Support

- [Aeterna Documentation](https://github.com/kikokikok/aeterna)
- [OpenCode Documentation](https://opencode.ai/docs)
- Report issues in [Aeterna Repository](https://github.com/kikokikok/aeterna/issues)

## Compatibility

- Requires `@opencode-ai/plugin` version 1.1.36 or higher
- Node.js version 18.0.0 or higher
- Requires Aeterna backend server running on HTTP

## Changelog

### 0.1.0

- Initial release with memory, knowledge, graph, CCA, and governance tools
- Updated to `@opencode-ai/plugin` v1.1.36 API (Zod v4, new hook signatures)
