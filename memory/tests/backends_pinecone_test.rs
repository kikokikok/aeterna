// Integration tests for the Pinecone vector backend.
//
// # Setup
//
// These tests require a live Pinecone account. Before running:
//
// 1. Create a Pinecone index:
//    - Dimensions: 3 (for these tests)
//    - Metric: cosine
//    - Environment: your preferred cloud region (e.g. `us-east-1-aws`)
//
// 2. Export environment variables:
//    ```sh
//    export PINECONE_API_KEY="<your-api-key>"
//    export PINECONE_ENVIRONMENT="us-east-1-aws"   # or your region
//    export PINECONE_INDEX_NAME="aeterna-test"       # the index created above
//    ```
//
// 3. Run with the `pinecone` feature and `--ignored` flag:
//    ```sh
//    cargo test -p memory --features pinecone --test backends_pinecone_test -- --ignored
//    ```
//
// # Notes
// - Pinecone has eventual-consistency semantics; the tests include short delays where needed.
// - Namespaces are used for tenant isolation; cleanup is performed at the end of each test.

#[cfg(feature = "pinecone")]
mod pinecone_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn pinecone_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::Pinecone,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: Some(memory::backends::factory::PineconeConfig {
                api_key: std::env::var("PINECONE_API_KEY").unwrap_or_default(),
                environment: std::env::var("PINECONE_ENVIRONMENT")
                    .unwrap_or_else(|_| "us-east-1-aws".to_string()),
                index_name: std::env::var("PINECONE_INDEX_NAME")
                    .unwrap_or_else(|_| "aeterna-test".to_string()),
            }),
            pgvector: None,
            vertex_ai: None,
            databricks: None,
            weaviate: None,
            mongodb: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_health_check() {
        let backend = create_backend(pinecone_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "pinecone");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_capabilities() {
        let backend = create_backend(pinecone_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_metadata_filter);
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_upsert_and_get() {
        let backend = create_backend(pinecone_config()).await.unwrap();
        let tenant_id = "test-tenant-pinecone-1";

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("memory"));
        metadata.insert("layer".to_string(), serde_json::json!("project"));

        let record = VectorRecord::new("pine-vec-1", vec![0.1, 0.2, 0.3], metadata);

        let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
        assert_eq!(result.upserted_count, 1);
        assert!(result.failed_ids.is_empty());

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let retrieved = backend.get(tenant_id, "pine-vec-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "pine-vec-1");
        assert_eq!(
            retrieved.metadata.get("type"),
            Some(&serde_json::json!("memory"))
        );

        backend
            .delete(tenant_id, vec!["pine-vec-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_search() {
        let backend = create_backend(pinecone_config()).await.unwrap();
        let tenant_id = "test-tenant-pinecone-search";

        let records = vec![
            VectorRecord::new(
                "pine-search-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "pine-search-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
            VectorRecord::new(
                "pine-search-3",
                vec![0.0, 0.0, 1.0],
                HashMap::from([("label".to_string(), serde_json::json!("c"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
        let results = backend.search(tenant_id, query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "pine-search-1");

        backend
            .delete(
                tenant_id,
                vec![
                    "pine-search-1".to_string(),
                    "pine-search-2".to_string(),
                    "pine-search-3".to_string(),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_tenant_isolation() {
        let backend = create_backend(pinecone_config()).await.unwrap();

        let record_a = VectorRecord::new("pine-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("pine-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("pinecone-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("pinecone-tenant-b", vec![record_b])
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let retrieved_a = backend
            .get("pinecone-tenant-a", "pine-iso-1")
            .await
            .unwrap()
            .unwrap();
        let retrieved_b = backend
            .get("pinecone-tenant-b", "pine-iso-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_a.id, "pine-iso-1");
        assert_eq!(retrieved_b.id, "pine-iso-1");

        assert!(
            backend
                .get("pinecone-tenant-c", "pine-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("pinecone-tenant-a", vec!["pine-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("pinecone-tenant-b", vec!["pine-iso-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Pinecone account - set PINECONE_API_KEY, PINECONE_ENVIRONMENT, PINECONE_INDEX_NAME"]
    async fn test_pinecone_delete() {
        let backend = create_backend(pinecone_config()).await.unwrap();
        let tenant_id = "test-tenant-pinecone-delete";

        let record = VectorRecord::new("pine-del-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend.upsert(tenant_id, vec![record]).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let result = backend
            .delete(tenant_id, vec!["pine-del-1".to_string()])
            .await
            .unwrap();
        assert_eq!(result.deleted_count, 1);

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        assert!(
            backend
                .get(tenant_id, "pine-del-1")
                .await
                .unwrap()
                .is_none()
        );
    }
}
