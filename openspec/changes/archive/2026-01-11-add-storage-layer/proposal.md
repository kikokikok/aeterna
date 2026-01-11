# Change: Implement Storage Layer

## Why
The Storage Layer provides concrete implementations for all storage backends (PostgreSQL, Qdrant, Redis). This is the foundation that all other components depend on for data persistence.

## What Changes

### PostgreSQL Implementation
- Implement schema for episodic, procedural, user, organization layers
- Use `sqlx` for compile-time query validation
- Implement connection pooling with `deadpool`
- Support pgvector for similarity search
- Implement health checks and metrics
- Support migrations for schema updates

### Qdrant Implementation
- Implement vector storage for semantic and archival layers
- Use `qdrant-client` crate
- Create collections per memory layer
- Implement vector similarity search
- Implement metadata filtering
- Implement batch operations for efficiency
- Implement health checks and metrics

### Redis Implementation
- Implement working memory (in-memory, microseconds)
- Implement session memory (TTL-based, milliseconds)
- Use `redis` crate with connection pooling
- Implement caching for embeddings and metadata
- Implement health checks and metrics

### Storage Abstraction
- Define `StorageBackend` trait for all implementations
- Implement factory pattern for backend selection
- Implement multi-backend configuration
- Support fallback between backends

## Impact

### Affected Specs
- `storage` - Complete implementation
- All other specs depend on storage implementations

### Affected Code
- New `storage/` crate with 3 implementations
- `memory/` crate uses storage backends
- `knowledge/` crate uses PostgreSQL for manifest
- `sync/` crate uses Redis for caching

### Dependencies
- `sqlx` 0.7 + `postgres` for PostgreSQL
- `qdrant-client` 1.7+ for Qdrant
- `redis` 0.24+ for Redis
- `deadpool` 0.9+ for connection pooling

## Breaking Changes
None - this implements storage interfaces defined in Phase 1
