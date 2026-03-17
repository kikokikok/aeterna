# Environment Variables Reference

All environment variables used by aeterna services.

## Qdrant (Vector Store)

| Variable | Default | Description |
|----------|---------|-------------|
| `QDRANT_URL` | `http://localhost:6334` | Qdrant gRPC endpoint |
| `QDRANT_COLLECTION` | `aeterna_memories` | Collection name |

## Embedding API (OpenAI-compatible)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `EMBEDDING_API_BASE` | **Yes** | — | Base URL for embeddings endpoint (e.g. `http://localhost:11434/v1`) |
| `EMBEDDING_API_KEY` | No | `not-needed` | API key (most local servers accept any value) |
| `EMBEDDING_MODEL` | No | `text-embedding-nomic-embed-text-v1.5` | Embedding model name |
| `EMBEDDING_DIMENSION` | No | `768` | Embedding vector dimension |

## LLM API (OpenAI-compatible)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LLM_API_BASE` | **Yes** | — | Base URL for chat/completions endpoint (e.g. `http://localhost:11434/v1`) |
| `LLM_API_KEY` | No | `not-needed` | API key |
| `LLM_MODEL` | No | `qwen3.5-35b-a3b` | Reasoning model name |
| `REASONING_TIMEOUT_MS` | No | `180000` | Reasoning timeout in milliseconds |

## PostgreSQL

| Variable | Default | Description |
|----------|---------|-------------|
| `PG_HOST` | `localhost` | PostgreSQL host |
| `PG_PORT` | `5432` | PostgreSQL port |
| `PG_USER` | `aeterna` | Database user |
| `PG_PASSWORD` | `aeterna_dev` | Database password |
| `PG_DATABASE` | `aeterna` | Database name |
| `DATABASE_URL` | (derived) | Full connection string |

## Redis

| Variable | Default | Description |
|----------|---------|-------------|
| `RD_URL` | `redis://localhost:6379` | Redis connection URL |

## agent-a2a Service

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_A2A_BIND_ADDRESS` | `0.0.0.0` | Bind address |
| `AGENT_A2A_PORT` | `8080` | HTTP port |
| `AGENT_A2A_AUTH_ENABLED` | `false` | Enable API key auth |
| `AGENT_A2A_AUTH_API_KEY` | | API key (if auth enabled) |
| `JWT_SECRET` | | JWT signing secret |

## Aeterna Context

| Variable | Default | Description |
|----------|---------|-------------|
| `AETERNA_TENANT_ID` | | Tenant ID for multi-tenancy |
| `AETERNA_USER_ID` | | User ID for audit trail |
