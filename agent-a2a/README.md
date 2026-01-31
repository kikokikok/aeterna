# Aeterna A2A Agent

A2A (Agent-to-Agent) protocol implementation using Radkit for the Aeterna platform.

## Overview

This crate provides an A2A-compliant agent server that exposes three skills:
- **Memory**: Manage ephemeral memories
- **Knowledge**: Query knowledge base
- **Governance**: Validate policies and check drift

## Quick Start

```bash
# Run the server
cargo run -p agent-a2a

# The server will start on port 8080 by default
```

## Configuration

Environment variables:
- `AGENT_A2A_BIND_ADDRESS`: Bind address (default: 0.0.0.0)
- `AGENT_A2A_PORT`: Port (default: 8080)
- `AGENT_A2A_AUTH_ENABLED`: Enable authentication (default: false)
- `AGENT_A2A_AUTH_API_KEY`: API key for authentication

## Endpoints

- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics
- `GET /.well-known/agent.json` - Agent Card (A2A discovery)
- `POST /tasks/send` - Send tasks to skills

## A2A Tool Schemas

### Memory Skill

**memory_add**
```json
{
  "content": "string",
  "layer": "optional string",
  "tags": ["optional", "array", "of", "strings"]
}
```

**memory_search**
```json
{
  "query": "string",
  "limit": "optional number",
  "layer": "optional string"
}
```

**memory_delete**
```json
{
  "id": "string"
}
```

### Knowledge Skill

**knowledge_query**
```json
{
  "query": "string",
  "doc_type": "optional string",
  "limit": "optional number"
}
```

**knowledge_show**
```json
{
  "id": "string"
}
```

**knowledge_check**
```json
{
  "content": "string",
  "doc_type": "string"
}
```

### Governance Skill

**governance_validate**
```json
{
  "policy": "string"
}
```

**governance_drift_check**
```json
{}
```

## Testing

```bash
cargo test -p agent-a2a
```
