use super::factory::QdrantConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, CreateCollectionBuilder, Distance, Filter, GetPointsBuilder, PointId,
        PointStruct, SearchPointsBuilder, Value as QdrantValue, VectorParams, VectorsConfig,
        point_id::PointIdOptions, vectors_config::Config
    }
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub struct QdrantBackend {
    client: Arc<Qdrant>,
    config: QdrantConfig,
    embedding_dimension: usize
}

impl QdrantBackend {
    pub async fn new(
        config: QdrantConfig,
        embedding_dimension: usize
    ) -> Result<Self, BackendError> {
        let mut builder = Qdrant::from_url(&config.url);

        if let Some(ref api_key) = config.api_key {
            builder = builder.api_key(api_key.clone());
        }

        let client = builder
            .build()
            .map_err(|e| BackendError::ConnectionFailed(format!("{}: {}", config.url, e)))?;

        Ok(Self {
            client: Arc::new(client),
            config,
            embedding_dimension
        })
    }

    fn collection_name(&self, tenant_id: &str) -> String {
        format!("{}_{}", self.config.collection_prefix, tenant_id)
    }

    async fn ensure_collection(&self, tenant_id: &str) -> Result<(), BackendError> {
        let collection_name = self.collection_name(tenant_id);

        let collections =
            self.client.list_collections().await.map_err(|e| {
                BackendError::Internal(format!("Failed to list collections: {}", e))
            })?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !exists {
            let request =
                CreateCollectionBuilder::new(&collection_name).vectors_config(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: self.embedding_dimension as u64,
                        distance: Distance::Cosine.into(),
                        ..Default::default()
                    }))
                });

            self.client.create_collection(request).await.map_err(|e| {
                BackendError::Internal(format!("Failed to create collection: {}", e))
            })?;
        }

        Ok(())
    }

    fn record_to_point(&self, record: &VectorRecord) -> PointStruct {
        let mut payload: HashMap<String, QdrantValue> = HashMap::new();

        for (key, value) in &record.metadata {
            if let Some(s) = value.as_str() {
                payload.insert(key.clone(), s.to_string().into());
            } else if let Some(n) = value.as_i64() {
                payload.insert(key.clone(), n.into());
            } else if let Some(f) = value.as_f64() {
                payload.insert(key.clone(), f.into());
            } else if let Some(b) = value.as_bool() {
                payload.insert(key.clone(), b.into());
            } else {
                payload.insert(key.clone(), value.to_string().into());
            }
        }

        PointStruct {
            id: Some(PointId::from(record.id.clone())),
            vectors: Some(record.vector.clone().into()),
            payload
        }
    }

    fn point_to_record(
        &self,
        id: PointId,
        payload: HashMap<String, QdrantValue>,
        vectors: Option<qdrant_client::qdrant::VectorsOutput>,
        score: Option<f32>
    ) -> VectorRecord {
        let id_str = match id.point_id_options {
            Some(PointIdOptions::Uuid(u)) => u,
            Some(PointIdOptions::Num(n)) => n.to_string(),
            None => String::new()
        };

        let mut metadata: HashMap<String, serde_json::Value> = HashMap::new();
        for (key, value) in payload {
            let json_value: serde_json::Value = value.into();
            metadata.insert(key, json_value);
        }

        if let Some(s) = score {
            metadata.insert("_score".to_string(), serde_json::json!(s));
        }

        let vector = vectors
            .and_then(|v| v.get_vector())
            .and_then(|vec| match vec {
                qdrant_client::qdrant::vector_output::Vector::Dense(dense) => Some(dense.data),
                _ => None
            })
            .unwrap_or_default();

        VectorRecord {
            id: id_str,
            vector,
            metadata
        }
    }
}

#[async_trait]
impl VectorBackend for QdrantBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();

        match self.client.list_collections().await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("qdrant").with_latency(latency))
            }
            Err(e) => Ok(HealthStatus::unhealthy("qdrant", e.to_string()))
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::qdrant()
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        self.ensure_collection(tenant_id).await?;

        let collection_name = self.collection_name(tenant_id);
        let points: Vec<PointStruct> = vectors.iter().map(|r| self.record_to_point(r)).collect();
        let count = points.len();

        use qdrant_client::qdrant::UpsertPointsBuilder;
        let request = UpsertPointsBuilder::new(&collection_name, points);

        self.client
            .upsert_points(request)
            .await
            .map_err(|e| BackendError::Internal(format!("Upsert failed: {}", e)))?;

        Ok(UpsertResult::success(count))
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        self.ensure_collection(tenant_id).await?;

        let collection_name = self.collection_name(tenant_id);

        let mut conditions: Vec<Condition> = Vec::new();
        for (key, value) in &query.filters {
            if let Some(s) = value.as_str() {
                conditions.push(Condition::matches(key.as_str(), s.to_string()));
            }
        }

        let mut request =
            SearchPointsBuilder::new(&collection_name, query.vector.clone(), query.limit as u64)
                .with_payload(query.include_metadata)
                .with_vectors(query.include_vectors);

        if !conditions.is_empty() {
            request = request.filter(Filter::all(conditions));
        }

        if let Some(threshold) = query.score_threshold {
            request = request.score_threshold(threshold);
        }

        let result = self
            .client
            .search_points(request)
            .await
            .map_err(|e| BackendError::Internal(format!("Search failed: {}", e)))?;

        Ok(result
            .result
            .into_iter()
            .filter_map(|p| {
                let id = p.id.clone()?;
                let record = self.point_to_record(id, p.payload, p.vectors, Some(p.score));
                Some(SearchResult {
                    id: record.id,
                    score: p.score,
                    vector: if query.include_vectors {
                        Some(record.vector)
                    } else {
                        None
                    },
                    metadata: record.metadata
                })
            })
            .collect())
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        self.ensure_collection(tenant_id).await?;

        let collection_name = self.collection_name(tenant_id);
        let point_ids: Vec<PointId> = ids.into_iter().map(PointId::from).collect();
        let count = point_ids.len();

        use qdrant_client::qdrant::DeletePointsBuilder;
        let request = DeletePointsBuilder::new(&collection_name).points(point_ids);

        self.client
            .delete_points(request)
            .await
            .map_err(|e| BackendError::Internal(format!("Delete failed: {}", e)))?;

        Ok(DeleteResult::new(count))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        self.ensure_collection(tenant_id).await?;

        let collection_name = self.collection_name(tenant_id);

        let request = GetPointsBuilder::new(&collection_name, vec![PointId::from(id.to_string())])
            .with_payload(true)
            .with_vectors(true);

        let result = self
            .client
            .get_points(request)
            .await
            .map_err(|e| BackendError::Internal(format!("Get failed: {}", e)))?;

        Ok(result.result.into_iter().next().and_then(|p| {
            let id = p.id?;
            Some(self.point_to_record(id, p.payload, p.vectors, None))
        }))
    }

    fn backend_name(&self) -> &'static str {
        "qdrant"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_name() {
        let config = QdrantConfig {
            url: "http://localhost:6334".to_string(),
            api_key: None,
            collection_prefix: "aeterna".to_string()
        };

        let backend = QdrantBackend {
            client: Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()),
            config,
            embedding_dimension: 1536
        };

        assert_eq!(backend.collection_name("tenant-123"), "aeterna_tenant-123");
    }

    #[test]
    fn test_record_to_point() {
        let config = QdrantConfig::default();
        let backend = QdrantBackend {
            client: Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()),
            config,
            embedding_dimension: 3
        };

        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));
        metadata.insert("count".to_string(), serde_json::json!(42));

        let record = VectorRecord::new("test-id", vec![0.1, 0.2, 0.3], metadata);
        let point = backend.record_to_point(&record);

        assert!(point.id.is_some());
        assert!(point.vectors.is_some());
        assert!(point.payload.contains_key("key"));
        assert!(point.payload.contains_key("count"));
    }
}
