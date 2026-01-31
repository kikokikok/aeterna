# Change: Add Pluggable Vector Database Backends

## Why

The current memory system uses Qdrant as the sole vector database backend. To support enterprise deployments and diverse cloud environments, the system needs pluggable vector DB backends including:

1. **Google Vertex AI Vector Search** - For organizations using GCP ecosystem
2. **Databricks Mosaic AI Vector Search** - For data teams using Databricks Lakehouse
3. **Additional managed options** - Pinecone, Weaviate, MongoDB Atlas, pgvector for simpler deployments

This enables organizations to choose the vector DB that best fits their infrastructure, compliance requirements, and cost constraints.

## What Changes

- Add `VectorBackend` trait abstraction for all vector database implementations
- **Add Vertex AI Vector Search backend** (Google Cloud managed service)
- **Add Databricks Vector Search backend** (Lakehouse-integrated)
- **Add Pinecone backend** (production-grade RAG, has Rust SDK)
- **Add Weaviate backend** (hybrid BM25 + vector search)
- **Add MongoDB Atlas Vector Search backend** (unified operational + vector)
- **Add pgvector backend** (self-hosted PostgreSQL extension)
- Add backend configuration via environment variables and config files
- Add backend health checks and capability detection
- Add backend-specific observability metrics
- Update `project.md` to document new storage options

## Impact

- **Affected specs**: `memory-system`
- **Affected code**: 
  - `memory/src/backends/` - New backend implementations
  - `memory/src/config.rs` - Backend configuration
  - `memory/src/provider.rs` - Provider trait extension
- **New dependencies**:
  - Google Cloud SDK (REST/gRPC client)
  - Databricks REST client
  - `pinecone-sdk` (Rust, alpha)
  - `qdrant-client` (existing, reference implementation)
  - Weaviate REST/GraphQL client
  - MongoDB driver
  - PostgreSQL driver with pgvector extension support

## Non-Goals

- Migrating existing data between backends (out of scope, manual process)
- Multi-backend federation (single backend per deployment)
- Custom embedding models per backend (use centralized embedding generation)
