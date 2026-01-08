# MCP Tools Interface

8 MCP tools for memory-knowledge system.

## Core Tools

- **Memory Tools**: `memory_add`, `memory_search`, `memory_delete`
- **Knowledge Tools**: `knowledge_query`, `knowledge_show`, `knowledge_check`
- **Sync Tools**: `sync_now`, `sync_status`

## Quick Start

```rust
use tools::server::McpServer;
use memory::manager::MemoryManager;
use sync::bridge::SyncManager;
use knowledge::repository::GitRepository;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let memory_manager = Arc::new(MemoryManager::new());
    let repo = Arc::new(GitRepository::new("/path/to/repo")?);
    let sync_manager = Arc::new(SyncManager::new(
        memory_manager.clone(),
        repo.clone(),
        Arc::new(/* persister */),
    ).await?);

    let server = Arc::new(McpServer::new(memory_manager, sync_manager, repo));
    
    // List available tools
    let tools = server.list_tools();
    println!("Available tools: {:?}", tools);
    
    Ok(())
}
```

## Tool Examples

### Memory Add
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "memory_add",
    "arguments": {
      "content": "User prefers dark mode",
      "layer": "user",
      "tags": ["preference", "ui"]
    }
  }
}
```

### Knowledge Query
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "knowledge_query",
    "arguments": {
      "query": "authentication patterns",
      "layers": ["project", "team"],
      "types": ["spec", "decision"]
    }
  }
}
```

## Error Handling

Tools return standardized error codes:
- `-32602`: Invalid input parameters
- `-32601`: Tool not found
- `-32000`: Internal server error
- `-32001`: Request timeout

## Ecosystem Adapters

Use `adapters` crate for OpenCode and LangChain integration:
```rust
use adapters::langchain::LangChainAdapter;
use adapters::ecosystem::OpenCodeAdapter;

let langchain_adapter = LangChainAdapter::new(server.clone());
let opencode_adapter = OpenCodeAdapter::new(server);
```
