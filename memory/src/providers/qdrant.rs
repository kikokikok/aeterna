use async_trait::async_trait;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer};
use qdrant_client::{
    Qdrant,
    qdrant::{
        Distance, PointId, PointStruct, ScoredPoint, Value as QdrantValue, VectorParams,
        VectorsConfig, point_id::PointIdOptions, vectors_config::Config
    }
};
use serde_json::{Value, json};

use std::collections::HashMap;
use std::sync::Arc;

pub struct QdrantProvider {
    client: Arc<Qdrant>,
    collection_name: String,
    embedding_dimension: usize
}

impl QdrantProvider {
    pub fn new(client: Qdrant, collection_name: String, embedding_dimension: usize) -> Self {
        Self {
            client: Arc::new(client),
            collection_name,
            embedding_dimension
        }
    }

    pub async fn ensure_collection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let collections_list = self.client.list_collections().await?;
        let collection_exists = collections_list
            .collections
            .iter()
            .any(|c| c.name == self.collection_name);

        if !collection_exists {
            use qdrant_client::qdrant::CreateCollectionBuilder;
            let request = CreateCollectionBuilder::new(self.collection_name.clone())
                .vectors_config(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: self.embedding_dimension as u64,
                        distance: Distance::Cosine.into(),
                        ..Default::default()
                    }))
                });

            self.client.create_collection(request).await?;
        }

        Ok(())
    }

    fn entry_to_point(
        &self,
        entry: &MemoryEntry
    ) -> Result<PointStruct, Box<dyn std::error::Error + Send + Sync>> {
        let embedding = entry.embedding.as_ref().ok_or("Entry missing embedding")?;

        let mut payload: HashMap<String, QdrantValue> = HashMap::from([
            ("id".to_string(), entry.id.clone().into()),
            ("content".to_string(), entry.content.clone().into()),
            (
                "layer".to_string(),
                serde_json::to_string(&entry.layer)?.into()
            ),
            ("created_at".to_string(), entry.created_at.into()),
            ("updated_at".to_string(), entry.updated_at.into())
        ]);

        payload.insert(
            "metadata".to_string(),
            serde_json::to_string(&entry.metadata)?.into()
        );

        Ok(PointStruct {
            id: Some(PointId::from(entry.id.clone())),
            vectors: Some(embedding.clone().into()),
            payload
        })
    }

    fn point_to_entry(
        &self,
        point: ScoredPoint
    ) -> Result<MemoryEntry, Box<dyn std::error::Error + Send + Sync>> {
        let payload = point.payload;

        let metadata_str = payload
            .get("metadata")
            .and_then(|v| {
                let v: Value = v.clone().into();
                if v.is_string() {
                    v.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .ok_or("Missing metadata in payload")?;

        let mut metadata: HashMap<String, Value> = serde_json::from_str(&metadata_str)?;
        metadata.insert("score".to_string(), json!(point.score));

        let vector = match point.vectors {
            Some(v) => v
                .get_vector()
                .and_then(|vec| match vec {
                    qdrant_client::qdrant::vector_output::Vector::Dense(dense) => Some(dense.data),
                    _ => None
                })
                .ok_or("Unsupported or missing vector format")?,
            None => return Err("Point missing vector".into())
        };

        let layer_str = payload
            .get("layer")
            .and_then(|v| {
                let v: Value = v.clone().into();
                if v.is_string() {
                    v.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .ok_or("Missing layer")?;
        let layer: MemoryLayer = serde_json::from_str(&layer_str)?;

        let id = payload
            .get("id")
            .and_then(|v| {
                let v: Value = v.clone().into();
                v.as_str().map(|s| s.to_string())
            })
            .ok_or("Missing id")?;

        let content = payload
            .get("content")
            .and_then(|v| {
                let v: Value = v.clone().into();
                v.as_str().map(|s| s.to_string())
            })
            .ok_or("Missing content")?;

        let created_at = payload
            .get("created_at")
            .and_then(|v| {
                let v: Value = v.clone().into();
                v.as_i64()
            })
            .ok_or("Missing created_at")?;

        let updated_at = payload
            .get("updated_at")
            .and_then(|v| {
                let v: Value = v.clone().into();
                v.as_i64()
            })
            .ok_or("Missing updated_at")?;

        Ok(MemoryEntry {
            id,
            content,
            embedding: Some(vector),
            layer,
            metadata,
            created_at,
            updated_at
        })
    }
}

#[async_trait]
impl MemoryProviderAdapter for QdrantProvider {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn add(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: MemoryEntry
    ) -> Result<String, Self::Error> {
        self.ensure_collection().await?;
        let mut entry = entry;

        entry
            .metadata
            .insert("tenant_id".to_string(), json!(ctx.tenant_id.as_str()));
        entry
            .metadata
            .insert("user_id".to_string(), json!(ctx.user_id.as_str()));

        if let Some(agent_id) = &ctx.agent_id {
            entry
                .metadata
                .insert("agent_id".to_string(), json!(agent_id));
        }

        let point = self.entry_to_point(&entry)?;
        use qdrant_client::qdrant::UpsertPointsBuilder;
        let request = UpsertPointsBuilder::new(self.collection_name.clone(), vec![point]);
        self.client.upsert_points(request).await?;
        Ok(entry.id)
    }

    async fn search(
        &self,
        ctx: mk_core::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        _filters: HashMap<String, Value>
    ) -> Result<Vec<MemoryEntry>, Self::Error> {
        self.ensure_collection().await?;
        use qdrant_client::qdrant::{Condition, Filter, SearchPointsBuilder};

        let filter = Filter::all(vec![Condition::matches(
            "tenant_id",
            ctx.tenant_id.as_str().to_string()
        )]);

        let request =
            SearchPointsBuilder::new(self.collection_name.clone(), query_vector, limit as u64)
                .with_payload(true)
                .with_vectors(true)
                .filter(filter);

        let result = self.client.search_points(request).await?;
        result
            .result
            .into_iter()
            .map(|p| self.point_to_entry(p))
            .collect()
    }

    async fn get(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str
    ) -> Result<Option<MemoryEntry>, Self::Error> {
        self.ensure_collection().await?;
        use qdrant_client::qdrant::GetPointsBuilder;

        tracing::debug!(tenant_id = %ctx.tenant_id, "Qdrant get point");

        let request = GetPointsBuilder::new(
            self.collection_name.clone(),
            vec![PointId::from(id.to_string())]
        )
        .with_payload(true)
        .with_vectors(true);

        let result = self.client.get_points(request).await?;
        if let Some(point) = result.result.into_iter().next() {
            let entry = self.point_to_entry(ScoredPoint {
                id: point.id,
                version: 0,
                score: 1.0,
                payload: point.payload,
                vectors: point.vectors,
                order_value: None,
                shard_key: None
            })?;

            if entry.metadata.get("tenant_id").and_then(|t| t.as_str())
                != Some(ctx.tenant_id.as_str())
            {
                return Ok(None);
            }

            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn update(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: MemoryEntry
    ) -> Result<(), Self::Error> {
        self.add(ctx, entry).await?;
        Ok(())
    }

    async fn delete(
        &self,
        ctx: mk_core::types::TenantContext,
        id: &str
    ) -> Result<(), Self::Error> {
        self.ensure_collection().await?;

        if self.get(ctx.clone(), id).await?.is_none() {
            return Ok(());
        }

        tracing::debug!(tenant_id = %ctx.tenant_id, "Qdrant delete point");

        use qdrant_client::qdrant::DeletePointsBuilder;
        let request = DeletePointsBuilder::new(self.collection_name.clone())
            .points(vec![PointId::from(id.to_string())]);
        self.client.delete_points(request).await?;
        Ok(())
    }

    async fn list(
        &self,
        ctx: mk_core::types::TenantContext,
        _layer: MemoryLayer,
        limit: usize,
        cursor: Option<String>
    ) -> Result<(Vec<MemoryEntry>, Option<String>), Self::Error> {
        self.ensure_collection().await?;

        tracing::debug!(tenant_id = %ctx.tenant_id, "Qdrant list points");

        use qdrant_client::qdrant::{Condition, Filter};
        let filter = Filter::all(vec![Condition::matches(
            "tenant_id",
            ctx.tenant_id.as_str().to_string()
        )]);

        let scroll_request = qdrant_client::qdrant::ScrollPoints {
            collection_name: self.collection_name.clone(),
            limit: Some(limit as u32),
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            offset: cursor.map(|c| PointId::from(c)),
            filter: Some(filter),
            ..Default::default()
        };

        let result = self.client.scroll(scroll_request).await?;
        let entries: Result<Vec<MemoryEntry>, _> = result
            .result
            .into_iter()
            .map(|p| {
                self.point_to_entry(ScoredPoint {
                    id: p.id,
                    version: 0,
                    score: 1.0,
                    payload: p.payload,
                    vectors: p.vectors,
                    order_value: None,
                    shard_key: None
                })
            })
            .collect();

        let next_cursor = result.next_page_offset.map(|id| match id.point_id_options {
            Some(PointIdOptions::Uuid(u)) => u,
            Some(PointIdOptions::Num(n)) => n.to_string(),
            None => String::new()
        });
        Ok((entries?, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::MemoryLayer;

    use qdrant_client::qdrant::vectors_output::VectorsOptions;
    use qdrant_client::qdrant::{VectorOutput, VectorsOutput};

    fn setup_provider() -> QdrantProvider {
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        QdrantProvider::new(client, "test_collection".to_string(), 3)
    }

    #[test]
    fn test_point_to_entry_conversion() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert(
            "metadata".to_string(),
            serde_json::to_string(&HashMap::<String, Value>::new())
                .unwrap()
                .into()
        );
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let entry = provider.point_to_entry(point).unwrap();

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.content, "test content");
        assert_eq!(entry.layer, MemoryLayer::Agent);
        assert_eq!(entry.created_at, 1000);
        assert_eq!(entry.updated_at, 2000);
        assert_eq!(entry.embedding, Some(vec![0.1, 0.2, 0.3]));
    }

    #[test]
    fn test_point_to_entry_with_metadata() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert(
            "metadata".to_string(),
            serde_json::to_string(&HashMap::from([("key".to_string(), json!("value"))]))
                .unwrap()
                .into()
        );
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let entry = provider.point_to_entry(point).unwrap();

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.content, "test content");
        assert_eq!(entry.embedding.unwrap(), vec![0.1, 0.2, 0.3]);
        assert_eq!(entry.layer, MemoryLayer::Agent);
        assert_eq!(entry.metadata.get("key").unwrap(), &json!("value"));
        // Use approx comparison for floating point
        let score_value = entry.metadata.get("score").unwrap().as_f64().unwrap();
        assert!((score_value - 0.95).abs() < 0.0001);
        assert_eq!(entry.created_at, 1000);
        assert_eq!(entry.updated_at, 2000);
    }

    #[test]
    fn test_entry_to_point_missing_embedding() {
        let provider = setup_provider();
        let entry = MemoryEntry {
            id: "test-id".to_string(),
            content: "test content".to_string(),
            embedding: None,
            layer: MemoryLayer::Agent,
            metadata: HashMap::new(),
            created_at: 1000,
            updated_at: 2000
        };

        let result = provider.entry_to_point(&entry);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Entry missing embedding");
    }

    #[test]
    fn test_point_to_entry_missing_payload_fields() {
        let provider = setup_provider();
        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload: HashMap::new(),
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
    }

    #[test]
    fn test_point_to_entry_invalid_layer() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert("layer".to_string(), "\"InvalidLayer\"".to_string().into());
        payload.insert("metadata".to_string(), "{}".to_string().into());
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
    }

    #[test]
    fn test_point_to_entry_missing_metadata() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Missing metadata in payload"
        );
    }

    #[test]
    fn test_point_to_entry_unsupported_vector() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert("metadata".to_string(), "{}".to_string().into());
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: None
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unsupported or missing vector format"
        );
    }

    #[test]
    fn test_point_to_entry_invalid_metadata_json() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert("metadata".to_string(), "invalid-json".to_string().into());
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: Some(VectorsOutput {
                vectors_options: Some(VectorsOptions::Vector(VectorOutput {
                    vector: Some(qdrant_client::qdrant::vector_output::Vector::Dense(
                        qdrant_client::qdrant::DenseVector {
                            data: vec![0.1, 0.2, 0.3]
                        }
                    )),
                    ..Default::default()
                }))
            }),
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
    }

    #[test]
    fn test_point_to_entry_missing_vector() {
        let provider = setup_provider();
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), "test-id".to_string().into());
        payload.insert("content".to_string(), "test content".to_string().into());
        payload.insert(
            "layer".to_string(),
            serde_json::to_string(&MemoryLayer::Agent).unwrap().into()
        );
        payload.insert("metadata".to_string(), "{}".to_string().into());
        payload.insert("created_at".to_string(), 1000.into());
        payload.insert("updated_at".to_string(), 2000.into());

        let point = ScoredPoint {
            id: Some(PointId::from("test-id".to_string())),
            payload,
            vectors: None,
            score: 0.95,
            version: 1,
            order_value: None,
            shard_key: None
        };

        let result = provider.point_to_entry(point);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Point missing vector");
    }
}
