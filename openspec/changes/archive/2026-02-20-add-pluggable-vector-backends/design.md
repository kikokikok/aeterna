## Context

The memory system currently uses Qdrant as the default vector database. Enterprise customers require flexibility to use their existing cloud infrastructure and managed services. This design introduces a pluggable backend architecture supporting multiple vector databases.

### Stakeholders
- Platform teams deploying on GCP (Vertex AI)
- Data engineering teams using Databricks
- Teams requiring simpler deployments (pgvector)
- Organizations with existing MongoDB infrastructure

## Goals / Non-Goals

### Goals
- Provide uniform API across all vector backends
- Support managed services (Vertex AI, Databricks, Pinecone, Weaviate, MongoDB Atlas)
- Support self-hosted options (Qdrant, pgvector)
- Enable backend selection via configuration
- Maintain performance parity with current Qdrant implementation
- Preserve multi-tenant isolation across all backends

### Non-Goals
- Data migration tooling between backends
- Multi-backend queries (federation)
- Backend-specific query optimizations
- Custom embedding models per backend

## Decisions

### Decision 1: Backend Trait Abstraction

All backends implement a common `VectorBackend` trait:

```rust
#[async_trait]
pub trait VectorBackend: Send + Sync {
    async fn health_check(&self) -> Result<HealthStatus, BackendError>;
    async fn capabilities(&self) -> BackendCapabilities;
    
    async fn upsert(&self, tenant_id: &str, vectors: Vec<VectorRecord>) -> Result<UpsertResult, BackendError>;
    async fn search(&self, tenant_id: &str, query: SearchQuery) -> Result<Vec<SearchResult>, BackendError>;
    async fn delete(&self, tenant_id: &str, ids: Vec<String>) -> Result<DeleteResult, BackendError>;
    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError>;
}
```

**Rationale**: Uniform interface enables backend swapping without code changes. The trait captures common vector DB operations while allowing backends to advertise their specific capabilities.

### Decision 2: Backend Selection via Configuration

Backend is selected via `VECTOR_BACKEND` environment variable or config file:

```yaml
vector:
  backend: vertex_ai  # qdrant | vertex_ai | databricks | pinecone | weaviate | mongodb | pgvector
  
  # Backend-specific configuration
  vertex_ai:
    project_id: my-gcp-project
    location: us-central1
    index_endpoint: projects/.../indexEndpoints/...
    
  databricks:
    workspace_url: https://xxx.cloud.databricks.com
    token: ${DATABRICKS_TOKEN}
    
  pinecone:
    api_key: ${PINECONE_API_KEY}
    environment: us-east-1
    
  qdrant:
    url: http://localhost:6334
    api_key: ${QDRANT_API_KEY}
    
  pgvector:
    connection_string: ${PGVECTOR_URL}
```

**Rationale**: Environment-based configuration enables different backends per deployment environment without code changes.

### Decision 3: Tenant Isolation Strategy per Backend

| Backend | Isolation Method |
|---------|------------------|
| Qdrant | Collection per tenant OR payload filter |
| Vertex AI | Index per tenant OR metadata filter |
| Databricks | Unity Catalog + Delta table per tenant |
| Pinecone | Namespace per tenant |
| Weaviate | Tenant key filter |
| MongoDB Atlas | Database per tenant OR collection filter |
| pgvector | Schema per tenant OR row-level filter |

**Rationale**: Each backend has different native multi-tenancy support. We use the most efficient isolation method for each while maintaining consistent tenant isolation semantics.

### Decision 4: Capability-Based Feature Degradation

Backends advertise capabilities; features degrade gracefully:

```rust
pub struct BackendCapabilities {
    pub max_vector_dimensions: usize,
    pub supports_metadata_filter: bool,
    pub supports_hybrid_search: bool,
    pub supports_batch_upsert: bool,
    pub supports_namespaces: bool,
    pub distance_metrics: Vec<DistanceMetric>,
}
```

**Rationale**: Not all backends support all features (e.g., hybrid search). The system adapts behavior based on advertised capabilities.

### Decision 5: REST API for Non-Rust SDKs

For backends without Rust SDKs (Vertex AI, Databricks, Weaviate), use REST/gRPC clients:

| Backend | Client Strategy |
|---------|----------------|
| Vertex AI | Google Cloud REST API via `reqwest` + gRPC via `tonic` |
| Databricks | REST API via `reqwest` |
| Pinecone | Official Rust SDK (`pinecone-sdk`, alpha) |
| Weaviate | REST + GraphQL via `reqwest` |
| MongoDB | Official Rust driver (`mongodb`) |
| pgvector | PostgreSQL driver (`sqlx` or `tokio-postgres`) |

**Rationale**: REST clients provide universal compatibility while official SDKs (where available) offer better ergonomics.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Pinecone Rust SDK is alpha/unstable | Implement REST fallback; pin SDK version |
| Vertex AI/Databricks have no Rust SDKs | Well-typed REST client wrappers with comprehensive error handling |
| Performance variance across backends | Document latency characteristics; provide benchmarking tools |
| Backend-specific bugs | Comprehensive integration tests per backend |
| Cost variance across backends | Document pricing; provide cost estimation tooling |

## Migration Plan

1. **Phase 1**: Implement `VectorBackend` trait; refactor Qdrant as reference implementation
2. **Phase 2**: Add Pinecone backend (has Rust SDK)
3. **Phase 3**: Add pgvector backend (simpler, self-hosted)
4. **Phase 4**: Add Vertex AI backend (GCP managed)
5. **Phase 5**: Add Databricks backend (Lakehouse integration)
6. **Phase 6**: Add Weaviate and MongoDB backends

### Rollback
- Backend selection is configuration-only
- No data migration required for rollback
- Previous backend remains available

## Open Questions

1. Should we support backend-specific query hints for optimization?
2. How to handle embedding dimension mismatches across backends?
3. Should we provide a backend migration CLI tool?
