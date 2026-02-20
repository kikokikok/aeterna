// Integration tests for the MongoDB Atlas Vector Search backend.
//
// # Setup
//
// Requires a MongoDB Atlas cluster (M10+ or serverless) with a vector search index.
//
// 1. Create an Atlas cluster and enable Vector Search.
//
// 2. Create a vector search index on your collection:
//    - Index name: `vector_index` (or set MONGODB_VECTOR_INDEX)
//    - Path: `vector`
//    - Dimensions: 3 (for these tests)
//    - Similarity: cosine
//    See: https://www.mongodb.com/docs/atlas/atlas-vector-search/create-index/
//
// 3. Export environment variables:
//    ```sh
//    export MONGODB_URI="mongodb+srv://user:password@cluster.mongodb.net"
//    export MONGODB_DATABASE="aeterna_test"
//    export MONGODB_COLLECTION="vectors_test"
//    export MONGODB_VECTOR_INDEX="vector_index"
//    ```
//
// 4. Run:
//    ```sh
//    cargo test -p memory --features mongodb --test backends_mongodb_test -- --ignored
//    ```

#[cfg(feature = "mongodb")]
mod mongodb_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn mongodb_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::Mongodb,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: None,
            pgvector: None,
            vertex_ai: None,
            databricks: None,
            weaviate: None,
            mongodb: Some(memory::backends::factory::MongodbConfig {
                connection_string: std::env::var("MONGODB_URI").unwrap_or_default(),
                database: std::env::var("MONGODB_DATABASE")
                    .unwrap_or_else(|_| "aeterna_test".to_string()),
                collection: std::env::var("MONGODB_COLLECTION")
                    .unwrap_or_else(|_| "vectors_test".to_string()),
                index_name: std::env::var("MONGODB_VECTOR_INDEX")
                    .unwrap_or_else(|_| "vector_index".to_string()),
            }),
        }
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_health_check() {
        let backend = create_backend(mongodb_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "mongodb");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_capabilities() {
        let backend = create_backend(mongodb_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_metadata_filter);
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_upsert_and_get() {
        let backend = create_backend(mongodb_config()).await.unwrap();
        let tenant_id = "test-tenant-mongo-1";

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("memory"));
        metadata.insert("layer".to_string(), serde_json::json!("project"));

        let record = VectorRecord::new("mg-vec-1", vec![0.1, 0.2, 0.3], metadata);

        let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
        assert_eq!(result.upserted_count, 1);
        assert!(result.failed_ids.is_empty());

        let retrieved = backend.get(tenant_id, "mg-vec-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "mg-vec-1");
        assert_eq!(
            retrieved.metadata.get("type"),
            Some(&serde_json::json!("memory"))
        );

        backend
            .delete(tenant_id, vec!["mg-vec-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_search() {
        let backend = create_backend(mongodb_config()).await.unwrap();
        let tenant_id = "test-tenant-mongo-search";

        let records = vec![
            VectorRecord::new(
                "mg-search-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "mg-search-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
            VectorRecord::new(
                "mg-search-3",
                vec![0.0, 0.0, 1.0],
                HashMap::from([("label".to_string(), serde_json::json!("c"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
        let results = backend.search(tenant_id, query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "mg-search-1");

        backend
            .delete(
                tenant_id,
                vec![
                    "mg-search-1".to_string(),
                    "mg-search-2".to_string(),
                    "mg-search-3".to_string(),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_tenant_isolation() {
        let backend = create_backend(mongodb_config()).await.unwrap();

        let record_a = VectorRecord::new("mg-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("mg-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("mongo-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("mongo-tenant-b", vec![record_b])
            .await
            .unwrap();

        let retrieved_a = backend
            .get("mongo-tenant-a", "mg-iso-1")
            .await
            .unwrap()
            .unwrap();
        let retrieved_b = backend
            .get("mongo-tenant-b", "mg-iso-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_a.vector, vec![1.0, 0.0, 0.0]);
        assert_eq!(retrieved_b.vector, vec![0.0, 1.0, 0.0]);

        assert!(
            backend
                .get("mongo-tenant-c", "mg-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("mongo-tenant-a", vec!["mg-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("mongo-tenant-b", vec!["mg-iso-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_delete() {
        let backend = create_backend(mongodb_config()).await.unwrap();
        let tenant_id = "test-tenant-mongo-delete";

        let record = VectorRecord::new("mg-del-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend.upsert(tenant_id, vec![record]).await.unwrap();

        assert!(backend.get(tenant_id, "mg-del-1").await.unwrap().is_some());

        let result = backend
            .delete(tenant_id, vec!["mg-del-1".to_string()])
            .await
            .unwrap();
        assert_eq!(result.deleted_count, 1);

        assert!(backend.get(tenant_id, "mg-del-1").await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore = "requires MongoDB Atlas cluster - set MONGODB_URI, MONGODB_DATABASE, MONGODB_COLLECTION, MONGODB_VECTOR_INDEX"]
    async fn test_mongodb_search_with_filter() {
        let backend = create_backend(mongodb_config()).await.unwrap();
        let tenant_id = "test-tenant-mongo-filter";

        let records = vec![
            VectorRecord::new(
                "mg-filter-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("category".to_string(), serde_json::json!("important"))]),
            ),
            VectorRecord::new(
                "mg-filter-2",
                vec![0.9, 0.1, 0.0],
                HashMap::from([("category".to_string(), serde_json::json!("normal"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        let query = SearchQuery::new(vec![1.0, 0.0, 0.0])
            .with_limit(10)
            .with_filter("category", serde_json::json!("important"));

        let results = backend.search(tenant_id, query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "mg-filter-1");

        backend
            .delete(
                tenant_id,
                vec!["mg-filter-1".to_string(), "mg-filter-2".to_string()],
            )
            .await
            .unwrap();
    }
}
