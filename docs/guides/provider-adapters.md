# Provider Adapters Guide

## Overview

Aeterna uses a **pluggable backend architecture** built around the `VectorBackend` trait. Every vector database — self-hosted or managed — implements the same trait, allowing you to switch providers by changing a single environment variable.

This guide explains how the adapter system works and how to add a new provider.

## Runtime Model Provider Adapters

Vector backends are only one side of the provider story. Aeterna also supports runtime-selected server-side model providers for:

- text generation via `mk_core::traits::LlmService`
- embeddings via `mk_core::traits::EmbeddingService`

These services are constructed through dedicated runtime factories:

- `memory/src/llm/factory.rs`
- `memory/src/embedding/factory.rs`

The server startup path wires those factories from deployment configuration in `cli/src/commands/serve.rs`.

### Supported Runtime LLM Providers

`AETERNA_LLM_PROVIDER` currently supports:

- `openai`
- `google`
- `bedrock`
- `none`

Provider construction is fail-closed. Unsupported providers or incomplete provider-specific configuration return an error instead of falling back implicitly.

### OpenAI Runtime Configuration

```bash
export AETERNA_LLM_PROVIDER=openai
export OPENAI_API_KEY=your-api-key
export AETERNA_OPENAI_MODEL=gpt-4.1-mini
export AETERNA_OPENAI_EMBEDDING_MODEL=text-embedding-3-small
# Optional: route to an OpenAI-compatible endpoint instead of api.openai.com.
# Used by the e2e suite (ollama, GitHub Models, recorded fixtures) and by
# self-hosted users running a local OpenAI-compat gateway.
# export AETERNA_OPENAI_BASE_URL=http://localhost:11434/v1
```

> **`AETERNA_OPENAI_BASE_URL`** is honored by both the LLM and embedding
> services. Empty string is treated as unset. Per-tenant DB-stored
> providers always use the public OpenAI endpoint; only env-based config
> reads this variable.

### Google Cloud Runtime Configuration

```bash
export AETERNA_LLM_PROVIDER=google
export AETERNA_GOOGLE_PROJECT_ID=my-project
export AETERNA_GOOGLE_LOCATION=global
export AETERNA_GOOGLE_MODEL=gemini-2.5-flash
export AETERNA_GOOGLE_EMBEDDING_MODEL=text-embedding-005
```

Authentication behavior:

1. Use `GOOGLE_ACCESS_TOKEN` when explicitly provided
2. Otherwise use ADC via `GOOGLE_APPLICATION_CREDENTIALS`
3. Otherwise use ambient ADC available in Google Cloud runtimes

The Google adapters target Vertex AI / Gemini server-side APIs, not browser-user federation.

### AWS Bedrock Runtime Configuration

```bash
export AETERNA_LLM_PROVIDER=bedrock
export AETERNA_BEDROCK_REGION=us-east-1
export AETERNA_BEDROCK_MODEL=anthropic.claude-3-5-sonnet-20241022-v2:0
export AETERNA_BEDROCK_EMBEDDING_MODEL=amazon.titan-embed-text-v2:0
```

Authentication uses the AWS SDK credential chain. In Kubernetes this normally means IAM roles for service accounts or another workload identity path rather than long-lived static credentials.

### Adapter Design Notes

- provider-specific request and response shapes stay inside adapter modules
- factories normalize provider selection and missing-config validation
- `MemoryManager` continues using injected trait implementations rather than provider-specific logic
- `none` is an explicit mode for deployments that do not want server-side provider construction

### Testing Expectations for Runtime Providers

Provider adapters should include:

- factory tests for provider parsing and fail-closed validation
- request/response adaptation tests for each provider module
- setup/deployment validation for emitted runtime environment

The current Google and Bedrock implementations are covered in:

- `memory/src/llm/google.rs`
- `memory/src/embedding/google.rs`
- `memory/src/llm/bedrock.rs`
- `memory/src/embedding/bedrock.rs`
- `cli/src/commands/setup/`
- `charts/aeterna/`

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
| Weaviate | `weaviate.rs` | `weaviate` | Multi-tenancy class | GraphQL + hybrid search |
| MongoDB Atlas | `mongodb.rs` | `mongodb` | Collection filter | `$vectorSearch` aggregation |
| Vertex AI | `vertex_ai.rs` | `vertex-ai` | Metadata restricts | GCP native |
| Databricks | `databricks.rs` | `databricks` | Unity Catalog schema | Data lakehouse |
