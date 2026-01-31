//! Pluggable vector database backends for the Aeterna memory system.
//!
//! This module provides a unified interface for multiple vector database
//! backends, enabling organizations to choose the backend that best fits their
//! infrastructure.
//!
//! # Backend Comparison
//!
//! | Backend | Status | Multi-tenancy | Hybrid Search | Max Dimensions | Best For |
//! |---------|--------|---------------|---------------|----------------|----------|
//! | Qdrant | ✅ Ready | Collection/filter | ✅ | 65536 | Self-hosted, full control |
//! | Pinecone | ✅ Ready | Namespace | ❌ | 20000 | Serverless, minimal ops |
//! | pgvector | ✅ Ready | Schema/filter | ❌ | 2000 | Existing PostgreSQL |
//! | Weaviate | ✅ Ready | Tenant key | ✅ | 65536 | GraphQL, hybrid search |
//! | MongoDB Atlas | ✅ Ready | Collection/filter | ✅ | 4096 | Existing MongoDB |
//! | Vertex AI | ⏳ Planned | Index/filter | ❌ | 2048 | GCP native |
//! | Databricks | ⏳ Planned | Unity Catalog | ❌ | 4096 | Data lakehouse |
//!
//! # Backend Selection Guide
//!
//! Choose your backend based on these criteria:
//!
//! ## Self-Hosted / Full Control
//! - **Qdrant**: Best for teams wanting full control, supports hybrid search
//! - **pgvector**: Best if you already have PostgreSQL infrastructure
//!
//! ## Managed / Serverless
//! - **Pinecone**: Simplest setup, pay-per-query, good for prototypes
//! - **MongoDB Atlas**: Good if you already use MongoDB
//! - **Weaviate Cloud**: Managed Weaviate with GraphQL API
//!
//! ## Cloud Provider Native
//! - **Vertex AI**: Best for GCP-centric organizations (planned)
//! - **Databricks**: Best for data lakehouse architectures (planned)
//!
//! # Feature Flags
//!
//! Enable backends via Cargo features:
//!
//! ```toml
//! [dependencies]
//! memory = { version = "0.1", features = ["pinecone", "pgvector", "weaviate", "mongodb"] }
//! ```
//!
//! Available features:
//! - `pinecone` - Pinecone serverless backend
//! - `pgvector` - PostgreSQL with pgvector extension
//! - `weaviate` - Weaviate vector database
//! - `mongodb` - MongoDB Atlas Vector Search
//! - `vertex-ai` - Google Vertex AI (planned)
//! - `databricks` - Databricks Vector Search (planned)
//!
//! # Environment Configuration
//!
//! Set `VECTOR_BACKEND` to select the backend:
//!
//! ```bash
//! # Qdrant (default)
//! export VECTOR_BACKEND=qdrant
//! export QDRANT_URL=http://localhost:6334
//!
//! # Pinecone
//! export VECTOR_BACKEND=pinecone
//! export PINECONE_API_KEY=your-api-key
//! export PINECONE_ENVIRONMENT=us-east-1-aws
//! export PINECONE_INDEX_NAME=aeterna-memories
//!
//! # pgvector
//! export VECTOR_BACKEND=pgvector
//! export PGVECTOR_URL=postgres://user:pass@localhost/aeterna
//!
//! # Weaviate
//! export VECTOR_BACKEND=weaviate
//! export WEAVIATE_URL=http://localhost:8080
//!
//! # MongoDB Atlas
//! export VECTOR_BACKEND=mongodb
//! export MONGODB_URI=mongodb+srv://user:pass@cluster.mongodb.net
//! export MONGODB_DATABASE=aeterna
//! ```
//!
//! # Observability
//!
//! Wrap any backend with instrumentation for metrics and circuit breaker:
//!
//! ```rust,ignore
//! use memory::backends::{create_backend, wrap_with_instrumentation, BackendConfig};
//!
//! let config = BackendConfig::from_env()?;
//! let backend = create_backend(config).await?;
//! let instrumented = wrap_with_instrumentation(backend);
//!
//! // Now all operations emit metrics:
//! // - vector_backend_operation_duration_seconds
//! // - vector_backend_operations_total
//! // - vector_backend_errors_total
//! // - vector_backend_circuit_breaker_rejected_total
//! ```
//!
//! # Troubleshooting
//!
//! ## Common Errors
//!
//! ### ConnectionFailed
//! - **Qdrant**: Check `QDRANT_URL` is reachable, default port is 6334
//! - **pgvector**: Verify `PGVECTOR_URL` connection string and pgvector
//!   extension installed
//! - **Weaviate**: Ensure `WEAVIATE_URL` points to running instance (default
//!   port 8080)
//! - **MongoDB**: Verify `MONGODB_URI` and network access to Atlas cluster
//!
//! ### AuthenticationFailed
//! - **Pinecone**: Verify `PINECONE_API_KEY` is valid and not expired
//! - **Vertex AI**: Check GCP credentials (metadata server or
//!   `GOOGLE_ACCESS_TOKEN`)
//! - **Databricks**: Verify `DATABRICKS_TOKEN` has correct permissions
//!
//! ### RateLimited
//! - All backends may rate limit. Use `BackendError::retry_after_ms()` for
//!   backoff
//! - Pinecone: Check pod quotas in Pinecone console
//! - Databricks: Monitor workspace API limits
//!
//! ### CircuitOpen
//! - Circuit breaker opened after repeated failures
//! - Default: opens after 5 failures, resets after 30 seconds
//! - Configure via `InstrumentedBackend::with_circuit_breaker(threshold,
//!   timeout_secs)`
//!
//! ## Backend-Specific Issues
//!
//! ### Qdrant
//! - Collection not found: Will be auto-created on first upsert
//! - Dimension mismatch: Ensure `embedding_dimension` matches your vectors
//!
//! ### pgvector
//! - Extension not installed: Run `CREATE EXTENSION vector;`
//! - Index not created: Backend auto-creates HNSW index on first use
//! - Slow queries: Check `EXPLAIN ANALYZE` and index settings
//!
//! ### Pinecone
//! - Namespace limits: Free tier has namespace restrictions
//! - Index not ready: Wait for index to be ready after creation
//!
//! ### Weaviate
//! - Class not found: Auto-created with multi-tenancy enabled
//! - Tenant not found: Auto-created on first upsert
//!
//! ### MongoDB Atlas
//! - Vector index not found: Create via Atlas UI or API before use
//! - `$vectorSearch` errors: Ensure index name matches `MONGODB_VECTOR_INDEX`
//!
//! ### Vertex AI
//! - Not running on GCP: Set `GOOGLE_ACCESS_TOKEN` manually
//! - Index endpoint not deployed: Deploy index to endpoint first
//! - Restricts not working: Ensure namespace filter matches tenant pattern
//!
//! ### Databricks
//! - Index not found: Create DIRECT_ACCESS index via Databricks UI/API
//! - Unity Catalog errors: Ensure catalog/schema exist and user has access

pub mod error;
pub mod factory;
pub mod observability;
pub mod qdrant;
pub mod types;

#[cfg(feature = "pinecone")]
pub mod pinecone;

#[cfg(feature = "pgvector")]
pub mod pgvector;

#[cfg(feature = "vertex-ai")]
pub mod vertex_ai;

#[cfg(feature = "databricks")]
pub mod databricks;

#[cfg(feature = "weaviate")]
pub mod weaviate;

#[cfg(feature = "mongodb")]
pub mod mongodb;

pub use error::BackendError;
pub use factory::{BackendConfig, VectorBackendType, create_backend};
pub use observability::{CircuitBreaker, InstrumentedBackend, wrap_with_instrumentation};
pub use types::{
    BackendCapabilities, DeleteResult, DistanceMetric, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorRecord
};

use async_trait::async_trait;

/// Unified trait for all vector database backends.
///
/// This trait provides a common interface for vector operations across
/// different storage backends, enabling seamless backend switching via
/// configuration.
///
/// # Tenant Isolation
///
/// All operations are scoped to a tenant via the `tenant_id` parameter.
/// Each backend implements isolation differently:
/// - Qdrant: Collection per tenant or payload filter
/// - Pinecone: Namespace per tenant
/// - pgvector: Schema per tenant or row-level filter
/// - Vertex AI: Index per tenant or metadata filter
/// - Databricks: Unity Catalog + Delta table per tenant
/// - Weaviate: Tenant key filter
/// - MongoDB Atlas: Database per tenant or collection filter
///
/// # Example
///
/// ```rust,ignore
/// use memory::backends::{VectorBackend, create_backend, BackendConfig};
///
/// let config = BackendConfig::from_env()?;
/// let backend = create_backend(config).await?;
///
/// // Check health
/// let status = backend.health_check().await?;
///
/// // Upsert vectors
/// let records = vec![VectorRecord::new("id-1", vec![0.1, 0.2, 0.3], metadata)];
/// backend.upsert("tenant-123", records).await?;
///
/// // Search
/// let query = SearchQuery::new(vec![0.1, 0.2, 0.3]).with_limit(10);
/// let results = backend.search("tenant-123", query).await?;
/// ```
#[async_trait]
pub trait VectorBackend: Send + Sync {
    /// Performs a health check on the backend.
    ///
    /// Returns the current health status and any diagnostic information.
    async fn health_check(&self) -> Result<HealthStatus, BackendError>;

    /// Returns the capabilities advertised by this backend.
    ///
    /// Use this to adapt behavior based on backend features (e.g., hybrid
    /// search).
    async fn capabilities(&self) -> BackendCapabilities;

    /// Upserts (insert or update) vectors into the backend.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `vectors` - Vector records to upsert
    ///
    /// # Returns
    /// Result containing the number of vectors upserted and any IDs that failed
    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError>;

    /// Performs semantic vector search.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `query` - Search parameters including vector, filters, and limits
    ///
    /// # Returns
    /// Ranked search results with scores
    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError>;

    /// Deletes vectors by their IDs.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `ids` - Vector IDs to delete
    ///
    /// # Returns
    /// Result containing the number of vectors deleted
    async fn delete(&self, tenant_id: &str, ids: Vec<String>)
    -> Result<DeleteResult, BackendError>;

    /// Retrieves a single vector by ID.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `id` - The vector ID to retrieve
    ///
    /// # Returns
    /// The vector record if found, None otherwise
    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError>;

    /// Returns the name of this backend implementation.
    fn backend_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_error_display() {
        let err = BackendError::ConnectionFailed("localhost:6334".to_string());
        assert!(err.to_string().contains("localhost:6334"));
    }
}
