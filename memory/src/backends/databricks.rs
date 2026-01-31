#![cfg(feature = "databricks")]

use super::factory::DatabricksConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, DistanceMetric, HealthStatus, SearchQuery,
    SearchResult, UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

pub struct DatabricksBackend {
    client: Client,
    config: DatabricksConfig
}

#[derive(Debug, Serialize)]
struct UpsertRequest {
    index_name: String,
    inputs_json: String
}

#[derive(Debug, Serialize)]
struct VectorInput {
    id: String,
    vector: Vec<f32>,
    #[serde(flatten)]
    metadata: HashMap<String, serde_json::Value>
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    index_name: String,
    query_vector: Vec<f32>,
    columns: Vec<String>,
    num_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters_json: Option<String>
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    result: Option<QueryResult>
}

#[derive(Debug, Deserialize)]
struct QueryResult {
    data_array: Option<Vec<Vec<serde_json::Value>>>,
    #[allow(dead_code)]
    row_count: Option<usize>
}

#[derive(Debug, Serialize)]
struct DeleteRequest {
    index_name: String,
    primary_keys: String
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    num_deleted: Option<usize>,
    #[allow(dead_code)]
    status: Option<String>
}

impl DatabricksBackend {
    pub async fn new(config: DatabricksConfig) -> Result<Self, BackendError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        let backend = Self { client, config };
        backend.verify_connection().await?;

        Ok(backend)
    }

    async fn verify_connection(&self) -> Result<(), BackendError> {
        let url = format!(
            "{}/api/2.0/vector-search/endpoints",
            self.config.workspace_url
        );

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.config.token)
            .send()
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            return Err(BackendError::AuthenticationFailed(
                "Invalid Databricks token".into()
            ));
        }

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::ConnectionFailed(format!(
                "Failed to connect to Databricks: {}",
                body
            )));
        }

        Ok(())
    }

    fn index_name(&self, tenant_id: &str) -> String {
        format!(
            "{}.{}.vectors_{}",
            self.config.catalog, self.config.schema, tenant_id
        )
    }

    fn base_url(&self) -> &str {
        &self.config.workspace_url
    }
}

#[async_trait]
impl VectorBackend for DatabricksBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();
        let url = format!("{}/api/2.0/vector-search/endpoints", self.base_url());

        match self
            .client
            .get(&url)
            .bearer_auth(&self.config.token)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("databricks").with_latency(latency))
            }
            Ok(resp) => Ok(HealthStatus::unhealthy(
                "databricks",
                format!("HTTP {}", resp.status())
            )),
            Err(e) => Ok(HealthStatus::unhealthy("databricks", e.to_string()))
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 4096,
            supports_metadata_filter: true,
            supports_hybrid_search: true,
            supports_batch_upsert: true,
            supports_namespaces: true,
            distance_metrics: vec![DistanceMetric::Cosine, DistanceMetric::Euclidean],
            max_batch_size: 1000,
            supports_delete_by_filter: true
        }
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        let url = format!(
            "{}/api/2.0/vector-search/indexes/{}/upsert-data",
            self.base_url(),
            urlencoding::encode(&self.index_name(tenant_id))
        );
        let count = vectors.len();

        let inputs: Vec<VectorInput> = vectors
            .into_iter()
            .map(|r| VectorInput {
                id: r.id,
                vector: r.vector,
                metadata: r.metadata
            })
            .collect();

        let inputs_json = serde_json::to_string(&inputs)
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let request = UpsertRequest {
            index_name: self.index_name(tenant_id),
            inputs_json
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Upsert request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                return Err(BackendError::RateLimited {
                    retry_after_ms: 5000
                });
            }

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
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/api/2.0/vector-search/indexes/{}/query",
            self.base_url(),
            urlencoding::encode(&self.index_name(tenant_id))
        );

        let filters_json = if !query.filters.is_empty() {
            Some(
                serde_json::to_string(&query.filters)
                    .map_err(|e| BackendError::Serialization(e.to_string()))?
            )
        } else {
            None
        };

        let request = QueryRequest {
            index_name: self.index_name(tenant_id),
            query_vector: query.vector,
            columns: vec!["id".to_string(), "vector".to_string()],
            num_results: query.limit,
            filters_json
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Search request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                return Err(BackendError::RateLimited {
                    retry_after_ms: 5000
                });
            }

            return Err(BackendError::Internal(format!(
                "Search failed with {}: {}",
                status, body
            )));
        }

        let response: QueryResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let data = response
            .result
            .and_then(|r| r.data_array)
            .unwrap_or_default();

        let results: Vec<SearchResult> = data
            .into_iter()
            .filter_map(|row| {
                if row.len() < 3 {
                    return None;
                }

                let id = row.first()?.as_str()?.to_string();
                let score = row.get(1)?.as_f64()? as f32;

                if let Some(threshold) = query.score_threshold {
                    if score < threshold {
                        return None;
                    }
                }

                let vector = if query.include_vectors {
                    row.get(2).and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect()
                    })
                } else {
                    None
                };

                let mut metadata = HashMap::new();
                for (i, val) in row.iter().enumerate().skip(3) {
                    metadata.insert(format!("col_{}", i), val.clone());
                }

                Some(SearchResult {
                    id,
                    score,
                    vector,
                    metadata
                })
            })
            .collect();

        Ok(results)
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        let url = format!(
            "{}/api/2.0/vector-search/indexes/{}/delete-data",
            self.base_url(),
            urlencoding::encode(&self.index_name(tenant_id))
        );
        let count = ids.len();

        let primary_keys =
            serde_json::to_string(&ids).map_err(|e| BackendError::Serialization(e.to_string()))?;

        let request = DeleteRequest {
            index_name: self.index_name(tenant_id),
            primary_keys
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Delete request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                return Err(BackendError::RateLimited {
                    retry_after_ms: 5000
                });
            }

            return Err(BackendError::Internal(format!(
                "Delete failed with {}: {}",
                status, body
            )));
        }

        let response: DeleteResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        Ok(DeleteResult::new(response.num_deleted.unwrap_or(count)))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let url = format!(
            "{}/api/2.0/vector-search/indexes/{}/scan",
            self.base_url(),
            urlencoding::encode(&self.index_name(tenant_id))
        );

        let filter = serde_json::json!({ "id": id });
        let filters_json = serde_json::to_string(&filter)
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let request = serde_json::json!({
            "index_name": self.index_name(tenant_id),
            "num_results": 1,
            "filters_json": filters_json,
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.token)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Get request failed: {}", e)))?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!(
                "Get failed with {}: {}",
                status, body
            )));
        }

        let response: QueryResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let data = response
            .result
            .and_then(|r| r.data_array)
            .unwrap_or_default();

        if let Some(row) = data.into_iter().next() {
            if row.len() >= 3 {
                let id = row
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let vector = row
                    .get(2)
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_f64().map(|f| f as f32))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut metadata = HashMap::new();
                for (i, val) in row.iter().enumerate().skip(3) {
                    metadata.insert(format!("col_{}", i), val.clone());
                }

                return Ok(Some(VectorRecord {
                    id,
                    vector,
                    metadata
                }));
            }
        }

        Ok(None)
    }

    fn backend_name(&self) -> &'static str {
        "databricks"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_name_generation() {
        let config = DatabricksConfig {
            workspace_url: "https://test.cloud.databricks.com".into(),
            token: "test-token".into(),
            catalog: "main".into(),
            schema: "aeterna".into()
        };

        let client = Client::new();
        let backend = DatabricksBackend { client, config };

        assert_eq!(
            backend.index_name("tenant-123"),
            "main.aeterna.vectors_tenant-123"
        );
    }

    #[test]
    fn test_upsert_request_serialization() {
        let input = VectorInput {
            id: "test-id".to_string(),
            vector: vec![0.1, 0.2, 0.3],
            metadata: HashMap::new()
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("test-id"));
        assert!(json.contains("vector"));
    }

    #[test]
    fn test_query_request_serialization() {
        let request = QueryRequest {
            index_name: "catalog.schema.index".to_string(),
            query_vector: vec![0.1, 0.2],
            columns: vec!["id".to_string()],
            num_results: 10,
            filters_json: None
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("query_vector"));
        assert!(json.contains("num_results"));
        assert!(!json.contains("filters_json"));
    }
}
