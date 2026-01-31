#![cfg(feature = "databricks")]

//! Databricks Vector Search backend implementation.
//!
//! This module is a placeholder for Databricks Vector Search integration.
//! Full implementation will be added in a future update.
//!
//! # Requirements
//!
//! - Databricks workspace with Vector Search enabled
//! - Unity Catalog configured
//! - Personal Access Token or Service Principal
//!
//! # Environment Variables
//!
//! - `DATABRICKS_HOST`: Workspace URL (e.g., https://xxx.cloud.databricks.com)
//! - `DATABRICKS_TOKEN`: Personal Access Token
//! - `DATABRICKS_CATALOG`: Unity Catalog name (default: main)
//! - `DATABRICKS_SCHEMA`: Schema name (default: aeterna)

use super::factory::DatabricksConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, DistanceMetric, HealthStatus, SearchQuery,
    SearchResult, UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;

/// Databricks Vector Search backend.
///
/// **Status**: Not yet implemented. This is a placeholder.
pub struct DatabricksBackend {
    #[allow(dead_code)]
    config: DatabricksConfig
}

impl DatabricksBackend {
    /// Creates a new Databricks backend instance.
    ///
    /// # Errors
    ///
    /// Returns an error as this backend is not yet implemented.
    pub async fn new(_config: DatabricksConfig) -> Result<Self, BackendError> {
        // TODO: Implement Databricks client initialization
        // - REST API authentication
        // - Unity Catalog verification
        // - Vector Search index validation
        Err(BackendError::Configuration(
            "Databricks backend is not yet implemented. See \
             openspec/changes/add-pluggable-vector-backends/tasks.md for progress."
                .into()
        ))
    }
}

#[async_trait]
impl VectorBackend for DatabricksBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        Err(BackendError::Configuration("Not implemented".into()))
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 4096,
            supports_metadata_filter: true,
            supports_hybrid_search: false,
            supports_batch_upsert: true,
            supports_namespaces: true, // Unity Catalog provides namespace isolation
            distance_metrics: vec![DistanceMetric::Cosine, DistanceMetric::Euclidean],
            max_batch_size: 1000,
            supports_delete_by_filter: true
        }
    }

    async fn upsert(
        &self,
        _tenant_id: &str,
        _vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        Err(BackendError::Configuration("Not implemented".into()))
    }

    async fn search(
        &self,
        _tenant_id: &str,
        _query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        Err(BackendError::Configuration("Not implemented".into()))
    }

    async fn delete(
        &self,
        _tenant_id: &str,
        _ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        Err(BackendError::Configuration("Not implemented".into()))
    }

    async fn get(&self, _tenant_id: &str, _id: &str) -> Result<Option<VectorRecord>, BackendError> {
        Err(BackendError::Configuration("Not implemented".into()))
    }

    fn backend_name(&self) -> &'static str {
        "databricks"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_databricks_not_implemented() {
        let config = DatabricksConfig {
            workspace_url: "https://test.cloud.databricks.com".into(),
            token: "test-token".into(),
            catalog: "main".into(),
            schema: "aeterna".into()
        };

        let result = DatabricksBackend::new(config).await;
        assert!(result.is_err());
    }
}
