// Integration tests for the Weaviate vector backend.
//
// # Setup
//
// Requires a running Weaviate instance (local or cloud).
//
// 1. Start a local Weaviate instance:
//    ```sh
//    docker run -d \
//      -e QUERY_DEFAULTS_LIMIT=25 \
//      -e AUTHENTICATION_ANONYMOUS_ACCESS_ENABLED=true \
//      -e PERSISTENCE_DATA_PATH=/var/lib/weaviate \
//      -e DEFAULT_VECTORIZER_MODULE=none \
//      -p 8080:8080 \
//      semitechnologies/weaviate:latest
//    ```
//
//    Or use Weaviate Cloud Services (WCS): https://console.weaviate.cloud
//
// 2. Export environment variables:
//    ```sh
//    export WEAVIATE_URL="http://localhost:8080"
//    export WEAVIATE_API_KEY=""          # leave empty for anonymous local
//    export WEAVIATE_CLASS="AeternaTest" # class name to use for tests
//    ```
//
// 3. Run:
//    ```sh
//    cargo test -p memory --features weaviate --test backends_weaviate_test -- --ignored
//    ```

#[cfg(feature = "weaviate")]
mod weaviate_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn weaviate_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::Weaviate,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: None,
            pgvector: None,
            vertex_ai: None,
            databricks: None,
            weaviate: Some(memory::backends::factory::WeaviateConfig {
                url: std::env::var("WEAVIATE_URL")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
                api_key: std::env::var("WEAVIATE_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty()),
                class_name: std::env::var("WEAVIATE_CLASS")
                    .unwrap_or_else(|_| "AeternaTest".to_string()),
            }),
            mongodb: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_health_check() {
        let backend = create_backend(weaviate_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "weaviate");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_capabilities() {
        let backend = create_backend(weaviate_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_hybrid_search);
        assert!(caps.supports_metadata_filter);
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_upsert_and_get() {
        let backend = create_backend(weaviate_config()).await.unwrap();
        let tenant_id = "test-tenant-weaviate-1";

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("memory"));
        metadata.insert("layer".to_string(), serde_json::json!("project"));

        let record = VectorRecord::new("wv-vec-1", vec![0.1, 0.2, 0.3], metadata);

        let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
        assert_eq!(result.upserted_count, 1);
        assert!(result.failed_ids.is_empty());

        let retrieved = backend.get(tenant_id, "wv-vec-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "wv-vec-1");
        assert_eq!(
            retrieved.metadata.get("type"),
            Some(&serde_json::json!("memory"))
        );

        backend
            .delete(tenant_id, vec!["wv-vec-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_search() {
        let backend = create_backend(weaviate_config()).await.unwrap();
        let tenant_id = "test-tenant-weaviate-search";

        let records = vec![
            VectorRecord::new(
                "wv-search-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "wv-search-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
            VectorRecord::new(
                "wv-search-3",
                vec![0.0, 0.0, 1.0],
                HashMap::from([("label".to_string(), serde_json::json!("c"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
        let results = backend.search(tenant_id, query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "wv-search-1");

        backend
            .delete(
                tenant_id,
                vec![
                    "wv-search-1".to_string(),
                    "wv-search-2".to_string(),
                    "wv-search-3".to_string(),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_tenant_isolation() {
        let backend = create_backend(weaviate_config()).await.unwrap();

        let record_a = VectorRecord::new("wv-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("wv-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("weaviate-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("weaviate-tenant-b", vec![record_b])
            .await
            .unwrap();

        let retrieved_a = backend
            .get("weaviate-tenant-a", "wv-iso-1")
            .await
            .unwrap()
            .unwrap();
        let retrieved_b = backend
            .get("weaviate-tenant-b", "wv-iso-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_a.vector, vec![1.0, 0.0, 0.0]);
        assert_eq!(retrieved_b.vector, vec![0.0, 1.0, 0.0]);

        assert!(
            backend
                .get("weaviate-tenant-c", "wv-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("weaviate-tenant-a", vec!["wv-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("weaviate-tenant-b", vec!["wv-iso-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Weaviate instance - set WEAVIATE_URL (and optionally WEAVIATE_API_KEY, WEAVIATE_CLASS)"]
    async fn test_weaviate_delete() {
        let backend = create_backend(weaviate_config()).await.unwrap();
        let tenant_id = "test-tenant-weaviate-delete";

        let record = VectorRecord::new("wv-del-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend.upsert(tenant_id, vec![record]).await.unwrap();

        assert!(backend.get(tenant_id, "wv-del-1").await.unwrap().is_some());

        let result = backend
            .delete(tenant_id, vec!["wv-del-1".to_string()])
            .await
            .unwrap();
        assert_eq!(result.deleted_count, 1);

        assert!(backend.get(tenant_id, "wv-del-1").await.unwrap().is_none());
    }
}
