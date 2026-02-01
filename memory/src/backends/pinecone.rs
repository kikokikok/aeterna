#![cfg(feature = "pinecone")]

use super::factory::PineconeConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

pub struct PineconeBackend {
    client: Client,
    config: PineconeConfig,
    host: String
}

#[derive(Debug, Serialize)]
struct UpsertRequest {
    vectors: Vec<PineconeVector>,
    namespace: String
}

#[derive(Debug, Serialize)]
struct PineconeVector {
    id: String,
    values: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, serde_json::Value>>
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    vector: Vec<f32>,
    #[serde(rename = "topK")]
    top_k: usize,
    namespace: String,
    #[serde(rename = "includeMetadata")]
    include_metadata: bool,
    #[serde(rename = "includeValues")]
    include_values: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<HashMap<String, serde_json::Value>>
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    matches: Vec<QueryMatch>
}

#[derive(Debug, Deserialize)]
struct QueryMatch {
    id: String,
    score: f32,
    values: Option<Vec<f32>>,
    metadata: Option<HashMap<String, serde_json::Value>>
}

#[derive(Debug, Serialize)]
struct DeleteRequest {
    ids: Vec<String>,
    namespace: String
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct FetchRequest {
    ids: Vec<String>,
    namespace: String
}

#[derive(Debug, Deserialize)]
struct FetchResponse {
    vectors: HashMap<String, FetchedVector>
}

#[derive(Debug, Deserialize)]
struct FetchedVector {
    id: String,
    values: Vec<f32>,
    metadata: Option<HashMap<String, serde_json::Value>>
}

#[derive(Debug, Deserialize)]
struct DescribeIndexResponse {
    database: IndexDatabase
}

#[derive(Debug, Deserialize)]
struct IndexDatabase {
    #[allow(dead_code)]
    name: String,
    host: String
}

impl PineconeBackend {
    pub async fn new(config: PineconeConfig) -> Result<Self, BackendError> {
        let client = Client::new();

        let controller_url = format!("https://api.pinecone.io/indexes/{}", config.index_name);

        let resp = client
            .get(&controller_url)
            .header("Api-Key", &config.api_key)
            .send()
            .await
            .map_err(|e| BackendError::ConnectionFailed(format!("Pinecone: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::AuthenticationFailed(format!(
                "Pinecone API error {}: {}",
                status, body
            )));
        }

        let index_info: DescribeIndexResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        Ok(Self {
            client,
            host: format!("https://{}", index_info.database.host),
            config
        })
    }

    fn namespace(&self, tenant_id: &str) -> String {
        tenant_id.to_string()
    }
}

#[async_trait]
impl VectorBackend for PineconeBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();

        let url = format!("{}/describe_index_stats", self.host);

        match self
            .client
            .post(&url)
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("pinecone").with_latency(latency))
            }
            Ok(resp) => Ok(HealthStatus::unhealthy(
                "pinecone",
                format!("HTTP {}", resp.status())
            )),
            Err(e) => Ok(HealthStatus::unhealthy("pinecone", e.to_string()))
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::pinecone()
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        let url = format!("{}/vectors/upsert", self.host);
        let count = vectors.len();

        let request = UpsertRequest {
            vectors: vectors
                .into_iter()
                .map(|r| PineconeVector {
                    id: r.id,
                    values: r.vector,
                    metadata: if r.metadata.is_empty() {
                        None
                    } else {
                        Some(r.metadata)
                    }
                })
                .collect(),
            namespace: self.namespace(tenant_id)
        };

        let resp = self
            .client
            .post(&url)
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Upsert failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                return Err(BackendError::RateLimited {
                    retry_after_ms: 1000
                });
            }

            return Err(BackendError::Internal(format!(
                "Upsert failed with status {}: {}",
                status, body
            )));
        }

        Ok(UpsertResult::success(count))
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!("{}/query", self.host);

        let request = QueryRequest {
            vector: query.vector,
            top_k: query.limit,
            namespace: self.namespace(tenant_id),
            include_metadata: query.include_metadata,
            include_values: query.include_vectors,
            filter: if query.filters.is_empty() {
                None
            } else {
                Some(query.filters)
            }
        };

        let resp = self
            .client
            .post(&url)
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Search failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Search failed with status {}: {}",
                status, body
            )));
        }

        let result: QueryResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        Ok(result
            .matches
            .into_iter()
            .map(|m| SearchResult {
                id: m.id,
                score: m.score,
                vector: m.values,
                metadata: m.metadata.unwrap_or_default()
            })
            .collect())
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        let url = format!("{}/vectors/delete", self.host);
        let count = ids.len();

        let request = DeleteRequest {
            ids,
            namespace: self.namespace(tenant_id)
        };

        let resp = self
            .client
            .post(&url)
            .header("Api-Key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Delete failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Delete failed with status {}: {}",
                status, body
            )));
        }

        Ok(DeleteResult::new(count))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let url = format!(
            "{}/vectors/fetch?ids={}&namespace={}",
            self.host,
            urlencoding::encode(id),
            urlencoding::encode(&self.namespace(tenant_id))
        );

        let resp = self
            .client
            .get(&url)
            .header("Api-Key", &self.config.api_key)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Fetch failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            if status.as_u16() == 404 {
                return Ok(None);
            }
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Fetch failed with status {}: {}",
                status, body
            )));
        }

        let result: FetchResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        Ok(result.vectors.get(id).map(|v| VectorRecord {
            id: v.id.clone(),
            vector: v.values.clone(),
            metadata: v.metadata.clone().unwrap_or_default()
        }))
    }

    fn backend_name(&self) -> &'static str {
        "pinecone"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace() {
        let config = PineconeConfig {
            api_key: "test".to_string(),
            environment: "test".to_string(),
            index_name: "test".to_string()
        };

        let backend = PineconeBackend {
            client: Client::new(),
            config,
            host: "https://test.svc.pinecone.io".to_string()
        };

        assert_eq!(backend.namespace("tenant-123"), "tenant-123");
    }
}
