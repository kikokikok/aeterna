//! Pluggable vector database backends for the Aeterna memory system.
//!
//! This module provides a unified interface for multiple vector database backends,
//! enabling organizations to choose the backend that best fits their infrastructure.
//!
//! # Supported Backends
//!
//! | Backend | Status | Multi-tenancy | Hybrid Search |
//! |---------|--------|---------------|---------------|
//! | Qdrant | ✅ Ready | Collection/filter | ✅ |
//! | Pinecone | 🚧 WIP | Namespace | ❌ |
//! | pgvector | 🚧 WIP | Schema/filter | ❌ |
//! | Vertex AI | 🚧 WIP | Index/filter | ❌ |
//! | Databricks | 🚧 WIP | Unity Catalog | ❌ |
//! | Weaviate | 🚧 WIP | Tenant key | ✅ |
//! | MongoDB Atlas | 🚧 WIP | Database/filter | ✅ |

pub mod error;
pub mod factory;
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
pub use factory::{create_backend, BackendConfig, VectorBackendType};
pub use types::{
    BackendCapabilities, DeleteResult, DistanceMetric, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorRecord,
};

use async_trait::async_trait;

/// Unified trait for all vector database backends.
///
/// This trait provides a common interface for vector operations across different
/// storage backends, enabling seamless backend switching via configuration.
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
    /// Use this to adapt behavior based on backend features (e.g., hybrid search).
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
        vectors: Vec<VectorRecord>,
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
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError>;

    /// Deletes vectors by their IDs.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `ids` - Vector IDs to delete
    ///
    /// # Returns
    /// Result containing the number of vectors deleted
    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError>;

    /// Retrieves a single vector by ID.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant scope for this operation
    /// * `id` - The vector ID to retrieve
    ///
    /// # Returns
    /// The vector record if found, None otherwise
    async fn get(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<VectorRecord>, BackendError>;

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
