#![cfg(feature = "mongodb")]

use super::factory::MongodbConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, DistanceMetric, HealthStatus, SearchQuery,
    SearchResult, UpsertResult, VectorBackend, VectorRecord
};
use async_trait::async_trait;
use mongodb::{
    Client, Collection,
    bson::{Bson, Document, doc},
    options::ClientOptions
};
use std::collections::HashMap;
use std::time::Instant;

pub struct MongodbBackend {
    client: Client,
    config: MongodbConfig
}

impl MongodbBackend {
    pub async fn new(config: MongodbConfig) -> Result<Self, BackendError> {
        let client_options = ClientOptions::parse(&config.connection_string)
            .await
            .map_err(|e| BackendError::ConnectionFailed(format!("MongoDB: {}", e)))?;

        let client = Client::with_options(client_options)
            .map_err(|e| BackendError::ConnectionFailed(format!("MongoDB: {}", e)))?;

        client
            .database(&config.database)
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| BackendError::ConnectionFailed(format!("MongoDB: {}", e)))?;

        Ok(Self { client, config })
    }

    fn collection(&self, tenant_id: &str) -> Collection<Document> {
        let collection_name = format!("{}_{}", self.config.collection, tenant_id);
        self.client
            .database(&self.config.database)
            .collection(&collection_name)
    }

    fn record_to_document(record: &VectorRecord) -> Document {
        let metadata_bson: Bson = serde_json::from_value::<Bson>(
            serde_json::to_value(&record.metadata).unwrap_or(serde_json::json!({}))
        )
        .unwrap_or(Bson::Document(Document::new()));

        let vector_bson: Vec<Bson> = record
            .vector
            .iter()
            .map(|f| Bson::Double(*f as f64))
            .collect();

        doc! {
            "_id": &record.id,
            "vector": vector_bson,
            "metadata": metadata_bson,
        }
    }

    fn document_to_record(doc: Document) -> Option<VectorRecord> {
        let id = doc.get_str("_id").ok()?.to_string();

        let vector: Vec<f32> = doc
            .get_array("vector")
            .ok()?
            .iter()
            .filter_map(|b| b.as_f64().map(|f| f as f32))
            .collect();

        let metadata: HashMap<String, serde_json::Value> = doc
            .get_document("metadata")
            .ok()
            .and_then(|d| {
                let json = serde_json::to_value(d).ok()?;
                serde_json::from_value(json).ok()
            })
            .unwrap_or_default();

        Some(VectorRecord {
            id,
            vector,
            metadata
        })
    }

    fn document_to_search_result(doc: Document, score: f32) -> Option<SearchResult> {
        let record = Self::document_to_record(doc)?;
        Some(SearchResult {
            id: record.id,
            score,
            vector: Some(record.vector),
            metadata: record.metadata
        })
    }
}

#[async_trait]
impl VectorBackend for MongodbBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();

        match self
            .client
            .database(&self.config.database)
            .run_command(doc! { "ping": 1 })
            .await
        {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("mongodb").with_latency(latency))
            }
            Err(e) => Ok(HealthStatus::unhealthy("mongodb", e.to_string()))
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            max_vector_dimensions: 4096,
            supports_metadata_filter: true,
            supports_hybrid_search: true,
            supports_batch_upsert: true,
            supports_namespaces: false,
            distance_metrics: vec![DistanceMetric::Cosine, DistanceMetric::Euclidean],
            max_batch_size: 100,
            supports_delete_by_filter: true
        }
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>
    ) -> Result<UpsertResult, BackendError> {
        let collection = self.collection(tenant_id);
        let count = vectors.len();
        let mut failed_ids = Vec::new();

        for record in vectors {
            let doc = Self::record_to_document(&record);
            let filter = doc! { "_id": &record.id };

            let result = collection.replace_one(filter, doc).upsert(true).await;

            if result.is_err() {
                failed_ids.push(record.id);
            }
        }

        Ok(UpsertResult {
            upserted_count: count - failed_ids.len(),
            failed_ids
        })
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery
    ) -> Result<Vec<SearchResult>, BackendError> {
        let collection = self.collection(tenant_id);

        let vector_bson: Vec<Bson> = query
            .vector
            .iter()
            .map(|f| Bson::Double(*f as f64))
            .collect();

        let mut pipeline = vec![doc! {
            "$vectorSearch": {
                "index": &self.config.index_name,
                "path": "vector",
                "queryVector": vector_bson,
                "numCandidates": (query.limit * 10) as i32,
                "limit": query.limit as i32,
            }
        }];

        if !query.filters.is_empty() {
            let mut filter_doc = Document::new();
            for (key, value) in &query.filters {
                let bson_value: Bson =
                    serde_json::from_value::<Bson>(value.clone()).unwrap_or(Bson::Null);
                filter_doc.insert(format!("metadata.{}", key), bson_value);
            }
            pipeline.push(doc! { "$match": filter_doc });
        }

        pipeline.push(doc! {
            "$project": {
                "_id": 1,
                "vector": 1,
                "metadata": 1,
                "score": { "$meta": "vectorSearchScore" }
            }
        });

        let mut cursor = collection
            .aggregate(pipeline)
            .await
            .map_err(|e| BackendError::Internal(format!("Search failed: {}", e)))?;

        let mut results = Vec::new();
        while cursor.advance().await.unwrap_or(false) {
            if let Ok(doc) = cursor.deserialize_current() {
                let score = doc.get_f64("score").unwrap_or(0.0) as f32;
                if let Some(result) = Self::document_to_search_result(doc, score) {
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>
    ) -> Result<DeleteResult, BackendError> {
        let collection = self.collection(tenant_id);

        let filter = doc! {
            "_id": { "$in": ids.clone() }
        };

        let result = collection
            .delete_many(filter)
            .await
            .map_err(|e| BackendError::Internal(format!("Delete failed: {}", e)))?;

        Ok(DeleteResult::new(result.deleted_count as usize))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let collection = self.collection(tenant_id);

        let filter = doc! { "_id": id };

        let result = collection
            .find_one(filter)
            .await
            .map_err(|e| BackendError::Internal(format!("Get failed: {}", e)))?;

        Ok(result.and_then(Self::document_to_record))
    }

    fn backend_name(&self) -> &'static str {
        "mongodb"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_to_document() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));

        let record = VectorRecord::new("test-id", vec![0.1, 0.2, 0.3], metadata);
        let doc = MongodbBackend::record_to_document(&record);

        assert_eq!(doc.get_str("_id").unwrap(), "test-id");
        assert!(doc.get_array("vector").is_ok());
        assert!(doc.get_document("metadata").is_ok());
    }
}
