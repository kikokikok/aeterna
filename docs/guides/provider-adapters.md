# Provider Adapters Guide

## Overview

Aeterna uses a **pluggable backend architecture** built around the `VectorBackend` trait. Every vector database — self-hosted or managed — implements the same trait, allowing you to switch providers by changing a single environment variable.

This guide explains how the adapter system works and how to add a new provider.

## Architecture

```
                  ┌──────────────────────────────────┐
                  │         MemoryManager            │
                  └──────────────┬───────────────────┘
                                 │
                                 ▼
                  ┌──────────────────────────────────┐
                  │   InstrumentedBackend (wrapper)   │
                  │   • Metrics emission              │
                  │   • Circuit breaker               │
                  │   • Latency tracking              │
                  └──────────────┬───────────────────┘
                                 │
                                 ▼
                  ┌──────────────────────────────────┐
                  │   dyn VectorBackend (trait obj)   │
                  └──────────┬───────────┬───────────┘
                  ┌──────────┘           └──────────┐
                  ▼                                  ▼
        ┌─────────────────┐                ┌─────────────────┐
        │  QdrantBackend  │                │ PineconeBackend │  ...
        └─────────────────┘                └─────────────────┘
```

All backends are created through the `create_backend()` factory function in `memory/src/backends/factory.rs`, which reads configuration and returns an `Arc<dyn VectorBackend>`. The `InstrumentedBackend` wrapper (in `observability.rs`) adds metrics and a circuit breaker around any backend transparently.

## The `VectorBackend` Trait

Located at `memory/src/backends/mod.rs`:

```rust
#[async_trait]
pub trait VectorBackend: Send + Sync {
    /// Health check — returns status and optional latency.
    async fn health_check(&self) -> Result<HealthStatus, BackendError>;

    /// Advertised capabilities (max dimensions, hybrid search, etc.).
    async fn capabilities(&self) -> BackendCapabilities;

    /// Insert or update vectors, scoped to a tenant.
    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>,
    ) -> Result<UpsertResult, BackendError>;

    /// Semantic search, scoped to a tenant.
    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError>;

    /// Delete vectors by ID, scoped to a tenant.
    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError>;

    /// Retrieve a single vector by ID.
    async fn get(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<VectorRecord>, BackendError>;

    /// Human-readable backend name (e.g., "qdrant", "pinecone").
    fn backend_name(&self) -> &'static str;
}
```

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `VectorRecord` | `types.rs` | ID + embedding vector + metadata hashmap |
| `SearchQuery` | `types.rs` | Vector + limit + score threshold + filters |
| `SearchResult` | `types.rs` | ID + score + optional vector + metadata |
| `UpsertResult` | `types.rs` | Count of upserted records + any failed IDs |
| `DeleteResult` | `types.rs` | Count of deleted records |
| `HealthStatus` | `types.rs` | Healthy/unhealthy flag + latency + message |
| `BackendCapabilities` | `types.rs` | Feature flags (hybrid search, namespaces, max dims, etc.) |
| `BackendError` | `error.rs` | Typed errors with `is_retryable()` and `retry_after_ms()` |

### Tenant Isolation Contract

Every mutating and query method takes `tenant_id: &str` as its first argument. The backend implementation is responsible for enforcing isolation. Strategies vary by provider:

| Backend | Isolation Strategy |
|---------|-------------------|
| Qdrant | Collection per tenant (`{prefix}_{tenant_id}`) |
| Pinecone | Namespace per tenant |
| pgvector | Row-level filter on `tenant_id` column |
| Weaviate | Tenant key in multi-tenancy class |
| MongoDB | Collection-level or filter-level isolation |
| Vertex AI | Metadata restricts filter |
| Databricks | Unity Catalog schema per tenant |

## Step-by-Step: Adding a New Provider

This walkthrough uses a hypothetical "Milvus" backend as an example.

### Step 1: Add the Feature Flag

Edit `memory/Cargo.toml`:

```toml
[features]
default = []
pinecone = ["dep:reqwest"]
pgvector = ["dep:sqlx"]
weaviate = ["dep:reqwest"]
mongodb = ["dep:mongodb"]
milvus = ["dep:tonic"]  # ← add your feature
```

Add the dependency itself under `[dependencies]` with `optional = true`:

```toml
[dependencies]
tonic = { version = "0.12", optional = true }
```

### Step 2: Create the Backend Module

Create `memory/src/backends/milvus.rs`:

```rust
use super::factory::MilvusConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, HealthStatus,
    SearchQuery, SearchResult, UpsertResult, VectorBackend, VectorRecord,
};
use async_trait::async_trait;

pub struct MilvusBackend {
    // Client handle, config, etc.
    config: MilvusConfig,
}

impl MilvusBackend {
    pub async fn new(config: MilvusConfig) -> Result<Self, BackendError> {
        // Initialize client connection
        Ok(Self { config })
    }
}

#[async_trait]
impl VectorBackend for MilvusBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        // Ping the Milvus server
        Ok(HealthStatus::healthy("milvus"))
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 32768,
            supports_metadata_filter: true,
            supports_hybrid_search: true,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![
                DistanceMetric::Cosine,
                DistanceMetric::Euclidean,
            ],
            max_batch_size: 1000,
            supports_delete_by_filter: true,
        }
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>,
    ) -> Result<UpsertResult, BackendError> {
        // Ensure collection exists for tenant
        // Map VectorRecord → Milvus insert request
        // Return UpsertResult::success(count)
        todo!()
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // Build Milvus search request from SearchQuery
        // Apply tenant isolation filter
        // Map Milvus results → Vec<SearchResult>
        todo!()
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError> {
        // Delete by IDs within tenant scope
        todo!()
    }

    async fn get(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<VectorRecord>, BackendError> {
        // Fetch single record by ID within tenant scope
        todo!()
    }

    fn backend_name(&self) -> &'static str {
        "milvus"
    }
}
```

### Step 3: Add Configuration

In `memory/src/backends/factory.rs`, add the config struct and update `BackendConfig`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MilvusConfig {
    pub endpoint: String,
    pub token: Option<String>,
    pub database: String,
}

impl MilvusConfig {
    pub fn from_env() -> Result<Self, BackendError> {
        Ok(Self {
            endpoint: std::env::var("MILVUS_ENDPOINT")
                .map_err(|_| BackendError::Configuration(
                    "MILVUS_ENDPOINT not set".into()
                ))?,
            token: std::env::var("MILVUS_TOKEN").ok(),
            database: std::env::var("MILVUS_DATABASE")
                .unwrap_or_else(|_| "aeterna".to_string()),
        })
    }
}
```

Add the variant to `VectorBackendType`:

```rust
pub enum VectorBackendType {
    // ... existing variants ...
    Milvus,
}
```

Update `FromStr`, `Display`, and the `create_backend()` match arm following the same pattern as existing backends.

### Step 4: Register the Module

In `memory/src/backends/mod.rs`, add the conditional module declaration:

```rust
#[cfg(feature = "milvus")]
pub mod milvus;
```

### Step 5: Write an Integration Test Stub

Create `memory/tests/milvus_integration_test.rs`:

```rust
//! Integration tests for Milvus backend.
//! Requires a running Milvus instance.
//! Run with: cargo test --features milvus -- --ignored

#[cfg(feature = "milvus")]
mod milvus_tests {
    use memory::backends::*;

    #[tokio::test]
    #[ignore] // Requires live Milvus instance
    async fn test_milvus_health_check() {
        let config = BackendConfig::from_env().unwrap();
        let backend = create_backend(config).await.unwrap();
        let health = backend.health_check().await.unwrap();
        assert!(health.healthy);
        assert_eq!(health.backend, "milvus");
    }

    #[tokio::test]
    #[ignore]
    async fn test_milvus_upsert_and_search() {
        let config = BackendConfig::from_env().unwrap();
        let backend = create_backend(config).await.unwrap();

        let records = vec![VectorRecord::new(
            "test-1",
            vec![0.1; 1536],
            Default::default(),
        )];

        let result = backend.upsert("test-tenant", records).await.unwrap();
        assert_eq!(result.upserted_count, 1);

        let query = SearchQuery::new(vec![0.1; 1536]).with_limit(5);
        let results = backend.search("test-tenant", query).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_milvus_tenant_isolation() {
        let config = BackendConfig::from_env().unwrap();
        let backend = create_backend(config).await.unwrap();

        // Upsert to tenant-a
        let records = vec![VectorRecord::new(
            "isolated-1",
            vec![0.5; 1536],
            Default::default(),
        )];
        backend.upsert("tenant-a", records).await.unwrap();

        // Search in tenant-b should not find it
        let query = SearchQuery::new(vec![0.5; 1536]).with_limit(10);
        let results = backend.search("tenant-b", query).await.unwrap();
        let found = results.iter().any(|r| r.id == "isolated-1");
        assert!(!found, "tenant-b must not see tenant-a records");
    }
}
```

### Step 6: Update Documentation

Add the new backend to the comparison table in `memory/src/backends/mod.rs` doc comments and update `docs/guides/managed-services-evaluation.md` if the provider has a managed offering.

## New Provider Checklist

Use this checklist when adding any new vector backend:

- [ ] **Feature flag** added in `memory/Cargo.toml` with optional dependency
- [ ] **Config struct** created in `factory.rs` with `from_env()` method
- [ ] **`VectorBackendType` variant** added with `FromStr`, `Display`, and `create_backend()` match arm
- [ ] **Backend module** created (e.g., `milvus.rs`) implementing all 7 `VectorBackend` methods
- [ ] **Tenant isolation** enforced in all methods (upsert, search, delete, get)
- [ ] **`BackendCapabilities`** returned accurately (max dims, hybrid search support, etc.)
- [ ] **Error mapping** from provider SDK errors to `BackendError` variants
- [ ] **Module registered** in `mod.rs` behind `#[cfg(feature = "...")]`
- [ ] **Integration test stub** created (marked `#[ignore]` for CI)
- [ ] **Tenant isolation test** verifying cross-tenant data cannot leak
- [ ] **Health check test** verifying connectivity reporting
- [ ] **Doc comments** added to the backend struct and module

## Switching Providers at Runtime

Aeterna selects the backend at startup based on the `VECTOR_BACKEND` environment variable:

```bash
# Switch from self-hosted Qdrant to Qdrant Cloud
export VECTOR_BACKEND=qdrant
export QDRANT_URL=https://xyz-abc.us-east4-0.gcp.cloud.qdrant.io:6333
export QDRANT_API_KEY=your-cloud-api-key

# Switch from Qdrant to Pinecone
export VECTOR_BACKEND=pinecone
export PINECONE_API_KEY=your-api-key
export PINECONE_ENVIRONMENT=us-east-1-aws
export PINECONE_INDEX_NAME=aeterna-memories
```

The `InstrumentedBackend` wrapper is applied automatically by `wrap_with_instrumentation()`, so all backends get metrics, circuit breaker protection, and latency tracking without additional configuration.

## Observability for All Backends

Every backend, regardless of provider, emits the following Prometheus metrics through the `InstrumentedBackend` wrapper:

| Metric | Type | Labels |
|--------|------|--------|
| `vector_backend_operation_duration_seconds` | Histogram | `backend`, `operation`, `tenant_id` |
| `vector_backend_operations_total` | Counter | `backend`, `operation`, `status` |
| `vector_backend_errors_total` | Counter | `backend`, `error_type` |
| `vector_backend_circuit_breaker_rejected_total` | Counter | `backend` |

The circuit breaker defaults to opening after 5 consecutive failures and resetting after 30 seconds. Configure with:

```rust
let instrumented = InstrumentedBackend::new(backend)
    .with_circuit_breaker(10, 60); // 10 failures, 60s reset
```

## Existing Backends Reference

| Backend | File | Feature Flag | Isolation | Notes |
|---------|------|-------------|-----------|-------|
| Qdrant | `qdrant.rs` | *(always compiled)* | Collection per tenant | Default backend |
| Pinecone | `pinecone.rs` | `pinecone` | Namespace per tenant | Serverless |
| pgvector | `pgvector.rs` | `pgvector` | Row-level filter | Requires PostgreSQL 16+ |
| Weaviate | `weaviate.rs` | `weaviate` | Multi-tenancy class | GraphQL + hybrid search |
| MongoDB Atlas | `mongodb.rs` | `mongodb` | Collection filter | `$vectorSearch` aggregation |
| Vertex AI | `vertex_ai.rs` | `vertex-ai` | Metadata restricts | GCP native |
| Databricks | `databricks.rs` | `databricks` | Unity Catalog schema | Data lakehouse |
