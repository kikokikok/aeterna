#![cfg(feature = "vertex-ai")]

use super::factory::VertexAiConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, DistanceMetric, HealthStatus, SearchQuery,
    SearchResult, UpsertResult, VectorBackend, VectorRecord,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

pub struct VertexAiBackend {
    client: Client,
    config: VertexAiConfig,
    access_token: tokio::sync::RwLock<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

#[derive(Debug, Serialize)]
struct UpsertRequest {
    datapoints: Vec<Datapoint>,
}

#[derive(Debug, Serialize)]
struct Datapoint {
    datapoint_id: String,
    feature_vector: Vec<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    restricts: Vec<Restrict>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Restrict {
    namespace: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allow_list: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RemoveRequest {
    datapoint_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FindNeighborsRequest {
    deployed_index_id: String,
    queries: Vec<QueryRequest>,
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    datapoint: QueryDatapoint,
    neighbor_count: usize,
}

#[derive(Debug, Serialize)]
struct QueryDatapoint {
    datapoint_id: String,
    feature_vector: Vec<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    restricts: Vec<Restrict>,
}

#[derive(Debug, Deserialize)]
struct FindNeighborsResponse {
    nearest_neighbors: Option<Vec<NearestNeighborResult>>,
}

#[derive(Debug, Deserialize)]
struct NearestNeighborResult {
    neighbors: Option<Vec<Neighbor>>,
}

#[derive(Debug, Deserialize)]
struct Neighbor {
    datapoint: Option<NeighborDatapoint>,
    distance: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct NeighborDatapoint {
    datapoint_id: String,
    feature_vector: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct MetadataTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
}

impl VertexAiBackend {
    pub async fn new(config: VertexAiConfig) -> Result<Self, BackendError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        let backend = Self {
            client,
            config,
            access_token: tokio::sync::RwLock::new(None),
        };

        backend.refresh_token().await?;

        Ok(backend)
    }

    async fn refresh_token(&self) -> Result<String, BackendError> {
        {
            let cached = self.access_token.read().await;
            if let Some(ref token) = *cached {
                if token.expires_at > std::time::Instant::now() {
                    return Ok(token.token.clone());
                }
            }
        }

        let token = self.fetch_access_token().await?;

        {
            let mut cached = self.access_token.write().await;
            *cached = Some(CachedToken {
                token: token.clone(),
                expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3500),
            });
        }

        Ok(token)
    }

    async fn fetch_access_token(&self) -> Result<String, BackendError> {
        let metadata_url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

        let resp = self
            .client
            .get(metadata_url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let token_resp: MetadataTokenResponse = r
                    .json()
                    .await
                    .map_err(|e| BackendError::AuthenticationFailed(e.to_string()))?;
                Ok(token_resp.access_token)
            }
            Ok(r) => Err(BackendError::AuthenticationFailed(format!(
                "Metadata server returned {}",
                r.status()
            ))),
            Err(_) => {
                if let Ok(token) = std::env::var("GOOGLE_ACCESS_TOKEN") {
                    return Ok(token);
                }
                Err(BackendError::AuthenticationFailed(
                    "Not running on GCP and GOOGLE_ACCESS_TOKEN not set".into(),
                ))
            }
        }
    }

    fn index_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/indexes/{}",
            self.config.location,
            self.config.project_id,
            self.config.location,
            self.config.index_endpoint
        )
    }

    fn endpoint_url(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/indexEndpoints/{}",
            self.config.location,
            self.config.project_id,
            self.config.location,
            self.config.index_endpoint
        )
    }

    fn tenant_restrict(tenant_id: &str) -> Restrict {
        Restrict {
            namespace: "tenant".to_string(),
            allow_list: vec![tenant_id.to_string()],
        }
    }
}

#[async_trait]
impl VectorBackend for VertexAiBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();

        match self.refresh_token().await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("vertex_ai").with_latency(latency))
            }
            Err(e) => Ok(HealthStatus::unhealthy("vertex_ai", e.to_string())),
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 2048,
            supports_metadata_filter: true,
            supports_hybrid_search: false,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![
                DistanceMetric::Cosine,
                DistanceMetric::Euclidean,
                DistanceMetric::DotProduct,
            ],
            max_batch_size: 100,
            supports_delete_by_filter: false,
        }
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>,
    ) -> Result<UpsertResult, BackendError> {
        let token = self.refresh_token().await?;
        let url = format!("{}:upsertDatapoints", self.index_url());
        let count = vectors.len();

        let datapoints: Vec<Datapoint> = vectors
            .into_iter()
            .map(|r| Datapoint {
                datapoint_id: r.id,
                feature_vector: r.vector,
                restricts: vec![Self::tenant_restrict(tenant_id)],
            })
            .collect();

        let request = UpsertRequest { datapoints };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Upsert request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Upsert failed with {}: {}",
                status, body
            )));
        }

        Ok(UpsertResult::success(count))
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let token = self.refresh_token().await?;
        let url = format!("{}:findNeighbors", self.endpoint_url());

        let request = FindNeighborsRequest {
            deployed_index_id: self.config.deployed_index_id.clone(),
            queries: vec![QueryRequest {
                datapoint: QueryDatapoint {
                    datapoint_id: uuid::Uuid::new_v4().to_string(),
                    feature_vector: query.vector,
                    restricts: vec![Self::tenant_restrict(tenant_id)],
                },
                neighbor_count: query.limit,
            }],
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Search request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Search failed with {}: {}",
                status, body
            )));
        }

        let response: FindNeighborsResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let results = response
            .nearest_neighbors
            .and_then(|nn| nn.into_iter().next())
            .and_then(|r| r.neighbors)
            .unwrap_or_default();

        Ok(results
            .into_iter()
            .filter_map(|n| {
                let dp = n.datapoint?;
                let score = n.distance.map(|d| 1.0 - d as f32).unwrap_or(0.0);

                if let Some(threshold) = query.score_threshold {
                    if score < threshold {
                        return None;
                    }
                }

                Some(SearchResult {
                    id: dp.datapoint_id,
                    score,
                    vector: if query.include_vectors {
                        dp.feature_vector
                    } else {
                        None
                    },
                    metadata: HashMap::new(),
                })
            })
            .collect())
    }

    async fn delete(
        &self,
        _tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError> {
        let token = self.refresh_token().await?;
        let url = format!("{}:removeDatapoints", self.index_url());
        let count = ids.len();

        let request = RemoveRequest { datapoint_ids: ids };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Delete request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Delete failed with {}: {}",
                status, body
            )));
        }

        Ok(DeleteResult::new(count))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let query = SearchQuery::new(vec![0.0; 128])
            .with_limit(100)
            .with_filter("datapoint_id", serde_json::json!(id));

        let results = self.search(tenant_id, query).await?;

        Ok(results
            .into_iter()
            .find(|r| r.id == id)
            .map(|r| VectorRecord {
                id: r.id,
                vector: r.vector.unwrap_or_default(),
                metadata: r.metadata,
            }))
    }

    fn backend_name(&self) -> &'static str {
        "vertex_ai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_restrict() {
        let restrict = VertexAiBackend::tenant_restrict("tenant-123");
        assert_eq!(restrict.namespace, "tenant");
        assert_eq!(restrict.allow_list, vec!["tenant-123"]);
    }

    #[test]
    fn test_datapoint_serialization() {
        let dp = Datapoint {
            datapoint_id: "test-id".to_string(),
            feature_vector: vec![0.1, 0.2, 0.3],
            restricts: vec![VertexAiBackend::tenant_restrict("tenant-1")],
        };

        let json = serde_json::to_string(&dp).unwrap();
        assert!(json.contains("test-id"));
        assert!(json.contains("feature_vector"));
    }
}
