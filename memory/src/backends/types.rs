use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: HashMap<String, serde_json::Value>
}

impl VectorRecord {
    pub fn new(
        id: impl Into<String>,
        vector: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>
    ) -> Self {
        Self {
            id: id.into(),
            vector,
            metadata
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub score_threshold: Option<f32>,
    pub filters: HashMap<String, serde_json::Value>,
    pub include_vectors: bool,
    pub include_metadata: bool
}

impl SearchQuery {
    pub fn new(vector: Vec<f32>) -> Self {
        Self {
            vector,
            limit: 10,
            score_threshold: None,
            filters: HashMap::new(),
            include_vectors: false,
            include_metadata: true
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }

    pub fn with_filter(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.filters.insert(key.into(), value);
        self
    }

    pub fn with_vectors(mut self, include: bool) -> Self {
        self.include_vectors = include;
        self
    }

    pub fn with_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub vector: Option<Vec<f32>>,
    pub metadata: HashMap<String, serde_json::Value>
}

#[derive(Debug, Clone, Default)]
pub struct UpsertResult {
    pub upserted_count: usize,
    pub failed_ids: Vec<String>
}

impl UpsertResult {
    pub fn success(count: usize) -> Self {
        Self {
            upserted_count: count,
            failed_ids: Vec::new()
        }
    }

    pub fn with_failures(mut self, ids: Vec<String>) -> Self {
        self.failed_ids = ids;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeleteResult {
    pub deleted_count: usize
}

impl DeleteResult {
    pub fn new(count: usize) -> Self {
        Self {
            deleted_count: count
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct
}

impl Default for DistanceMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    pub max_vector_dimensions: usize,
    pub supports_metadata_filter: bool,
    pub supports_hybrid_search: bool,
    pub supports_batch_upsert: bool,
    pub supports_namespaces: bool,
    pub distance_metrics: Vec<DistanceMetric>,
    pub max_batch_size: usize,
    pub supports_delete_by_filter: bool
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self {
            max_vector_dimensions: 4096,
            supports_metadata_filter: true,
            supports_hybrid_search: false,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![DistanceMetric::Cosine],
            max_batch_size: 100,
            supports_delete_by_filter: false
        }
    }
}

impl BackendCapabilities {
    pub fn qdrant() -> Self {
        Self {
            max_vector_dimensions: 65536,
            supports_metadata_filter: true,
            supports_hybrid_search: true,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![
                DistanceMetric::Cosine,
                DistanceMetric::Euclidean,
                DistanceMetric::DotProduct,
            ],
            max_batch_size: 1000,
            supports_delete_by_filter: true
        }
    }

    pub fn pinecone() -> Self {
        Self {
            max_vector_dimensions: 20000,
            supports_metadata_filter: true,
            supports_hybrid_search: false,
            supports_batch_upsert: true,
            supports_namespaces: true,
            distance_metrics: vec![
                DistanceMetric::Cosine,
                DistanceMetric::Euclidean,
                DistanceMetric::DotProduct,
            ],
            max_batch_size: 100,
            supports_delete_by_filter: true
        }
    }

    pub fn pgvector() -> Self {
        Self {
            max_vector_dimensions: 2000,
            supports_metadata_filter: true,
            supports_hybrid_search: false,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![
                DistanceMetric::Cosine,
                DistanceMetric::Euclidean,
                DistanceMetric::DotProduct,
            ],
            max_batch_size: 1000,
            supports_delete_by_filter: true
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
    pub backend: String
}

impl HealthStatus {
    pub fn healthy(backend: &str) -> Self {
        Self {
            healthy: true,
            latency_ms: None,
            message: None,
            backend: backend.to_string()
        }
    }

    pub fn unhealthy(backend: &str, message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            latency_ms: None,
            message: Some(message.into()),
            backend: backend.to_string()
        }
    }

    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = Some(ms);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_record_builder() {
        let record = VectorRecord::new("test-id", vec![0.1, 0.2], HashMap::new())
            .with_metadata("key", serde_json::json!("value"));

        assert_eq!(record.id, "test-id");
        assert_eq!(
            record.metadata.get("key"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new(vec![0.1, 0.2])
            .with_limit(20)
            .with_score_threshold(0.8)
            .with_filter("type", serde_json::json!("memory"));

        assert_eq!(query.limit, 20);
        assert_eq!(query.score_threshold, Some(0.8));
        assert!(query.filters.contains_key("type"));
    }

    #[test]
    fn test_health_status() {
        let healthy = HealthStatus::healthy("qdrant").with_latency(5);
        assert!(healthy.healthy);
        assert_eq!(healthy.latency_ms, Some(5));

        let unhealthy = HealthStatus::unhealthy("qdrant", "Connection refused");
        assert!(!unhealthy.healthy);
        assert!(unhealthy.message.is_some());
    }

    #[test]
    fn test_backend_capabilities_presets() {
        let qdrant = BackendCapabilities::qdrant();
        assert!(qdrant.supports_hybrid_search);
        assert_eq!(qdrant.max_batch_size, 1000);

        let pinecone = BackendCapabilities::pinecone();
        assert!(pinecone.supports_namespaces);
        assert!(!pinecone.supports_hybrid_search);

        let pgvector = BackendCapabilities::pgvector();
        assert_eq!(pgvector.max_vector_dimensions, 2000);
    }
}
