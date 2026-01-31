#![cfg(feature = "weaviate")]

use super::factory::WeaviateConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, DistanceMetric, HealthStatus, SearchQuery,
    SearchResult, UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

pub struct WeaviateBackend {
    client: Client,
    config: WeaviateConfig
}

#[derive(Debug, Serialize)]
struct BatchRequest {
    objects: Vec<WeaviateObject>
}

#[derive(Debug, Serialize, Deserialize)]
struct WeaviateObject {
    class: String,
    id: String,
    vector: Vec<f32>,
    properties: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant: Option<String>
}

#[derive(Debug, Serialize)]
struct GraphQLQuery {
    query: String
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    errors: Option<Vec<GraphQLError>>
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    #[serde(rename = "Get")]
    get: Option<HashMap<String, Vec<GraphQLObject>>>
}

#[derive(Debug, Deserialize)]
struct GraphQLObject {
    _additional: GraphQLAdditional,
    #[serde(flatten)]
    properties: HashMap<String, serde_json::Value>
}

#[derive(Debug, Deserialize)]
struct GraphQLAdditional {
    id: String,
    distance: Option<f32>,
    certainty: Option<f32>,
    vector: Option<Vec<f32>>
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReadyResponse {
    status: String
}

impl WeaviateBackend {
    pub async fn new(config: WeaviateConfig) -> Result<Self, BackendError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(ref api_key) = config.api_key {
            headers.insert(
                "Authorization",
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| BackendError::Configuration(e.to_string()))?
            );
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        let backend = Self { client, config };
        backend.ensure_class().await?;

        Ok(backend)
    }

    async fn ensure_class(&self) -> Result<(), BackendError> {
        let url = format!("{}/v1/schema/{}", self.config.url, self.config.class_name);

        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) if r.status().as_u16() == 404 => {
                let class_schema = serde_json::json!({
                    "class": self.config.class_name,
                    "vectorizer": "none",
                    "multiTenancyConfig": {
                        "enabled": true
                    },
                    "properties": [
                        {
                            "name": "content",
                            "dataType": ["text"]
                        },
                        {
                            "name": "metadata_json",
                            "dataType": ["text"]
                        }
                    ]
                });

                let create_url = format!("{}/v1/schema", self.config.url);
                let create_resp = self
                    .client
                    .post(&create_url)
                    .json(&class_schema)
                    .send()
                    .await
                    .map_err(|e| {
                        BackendError::Internal(format!("Failed to create class: {}", e))
                    })?;

                if !create_resp.status().is_success() {
                    let body = create_resp.text().await.unwrap_or_default();
                    return Err(BackendError::Internal(format!(
                        "Failed to create class: {}",
                        body
                    )));
                }
                Ok(())
            }
            Ok(r) => {
                let body = r.text().await.unwrap_or_default();
                Err(BackendError::Internal(format!(
                    "Failed to check class: {}",
                    body
                )))
            }
            Err(e) => Err(BackendError::ConnectionFailed(e.to_string()))
        }
    }

    async fn ensure_tenant(&self, tenant_id: &str) -> Result<(), BackendError> {
        let url = format!(
            "{}/v1/schema/{}/tenants",
            self.config.url, self.config.class_name
        );

        let tenant_data = serde_json::json!([{
            "name": tenant_id
        }]);

        let _ = self.client.post(&url).json(&tenant_data).send().await;

        Ok(())
    }
}

#[async_trait]
impl VectorBackend for WeaviateBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();
        let url = format!("{}/v1/.well-known/ready", self.config.url);

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("weaviate").with_latency(latency))
            }
            Ok(resp) => Ok(HealthStatus::unhealthy(
                "weaviate",
                format!("HTTP {}", resp.status())
            )),
            Err(e) => Ok(HealthStatus::unhealthy("weaviate", e.to_string()))
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 65536,
            supports_metadata_filter: true,
            supports_hybrid_search: true,
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

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        self.ensure_tenant(tenant_id).await?;

        let url = format!("{}/v1/batch/objects", self.config.url);
        let count = vectors.len();

        let objects: Vec<WeaviateObject> = vectors
            .into_iter()
            .map(|r| {
                let mut properties = HashMap::new();
                properties.insert(
                    "metadata_json".to_string(),
                    serde_json::json!(serde_json::to_string(&r.metadata).unwrap_or_default())
                );

                WeaviateObject {
                    class: self.config.class_name.clone(),
                    id: r.id,
                    vector: r.vector,
                    properties,
                    tenant: Some(tenant_id.to_string())
                }
            })
            .collect();

        let request = BatchRequest { objects };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Upsert failed: {}", e)))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!("Upsert failed: {}", body)));
        }

        Ok(UpsertResult::success(count))
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!("{}/v1/graphql", self.config.url);

        let vector_str = query
            .vector
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let additional = if query.include_vectors {
            "id distance vector"
        } else {
            "id distance"
        };

        let graphql_query = format!(
            r#"{{
                Get {{
                    {class}(
                        nearVector: {{ vector: [{vector}] }}
                        limit: {limit}
                        tenant: "{tenant}"
                    ) {{
                        metadata_json
                        _additional {{ {additional} }}
                    }}
                }}
            }}"#,
            class = self.config.class_name,
            vector = vector_str,
            limit = query.limit,
            tenant = tenant_id,
            additional = additional,
        );

        let request = GraphQLQuery {
            query: graphql_query
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Search failed: {}", e)))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!("Search failed: {}", body)));
        }

        let response: GraphQLResponse = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                return Err(BackendError::Internal(
                    errors
                        .iter()
                        .map(|e| e.message.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        let results = response
            .data
            .and_then(|d| d.get)
            .and_then(|mut g| g.remove(&self.config.class_name))
            .unwrap_or_default();

        Ok(results
            .into_iter()
            .map(|obj| {
                let metadata: HashMap<String, serde_json::Value> = obj
                    .properties
                    .get("metadata_json")
                    .and_then(|v| v.as_str())
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();

                let score = obj
                    ._additional
                    .distance
                    .map(|d| 1.0 - d)
                    .or(obj._additional.certainty)
                    .unwrap_or(0.0);

                SearchResult {
                    id: obj._additional.id,
                    score,
                    vector: obj._additional.vector,
                    metadata
                }
            })
            .collect())
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        let count = ids.len();

        for id in ids {
            let url = format!(
                "{}/v1/objects/{}/{}?tenant={}",
                self.config.url, self.config.class_name, id, tenant_id
            );

            let _ = self.client.delete(&url).send().await;
        }

        Ok(DeleteResult::new(count))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let url = format!(
            "{}/v1/objects/{}/{}?tenant={}&include=vector",
            self.config.url, self.config.class_name, id, tenant_id
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("Get failed: {}", e)))?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(BackendError::Internal(format!("Get failed: {}", body)));
        }

        let obj: WeaviateObject = resp
            .json()
            .await
            .map_err(|e| BackendError::Serialization(e.to_string()))?;

        let metadata: HashMap<String, serde_json::Value> = obj
            .properties
            .get("metadata_json")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        Ok(Some(VectorRecord {
            id: obj.id,
            vector: obj.vector,
            metadata
        }))
    }

    fn backend_name(&self) -> &'static str {
        "weaviate"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weaviate_config_default() {
        let config = WeaviateConfig::default();
        assert_eq!(config.url, "http://localhost:8080");
        assert_eq!(config.class_name, "AeternaMemory");
    }
}
