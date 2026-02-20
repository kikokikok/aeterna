// Integration tests for the Vertex AI Vector Search backend.
//
// # Setup
//
// Requires a GCP project with Vertex AI Vector Search configured.
//
// 1. Create a Vector Search index and deploy it to an endpoint:
//    - Dimensions: 3 (for these tests)
//    - Distance measure: DOT_PRODUCT_DISTANCE or COSINE_DISTANCE
//    - See: https://cloud.google.com/vertex-ai/docs/vector-search/quickstart
//
// 2. Grant the service account the `Vertex AI User` IAM role.
//
// 3. Export environment variables:
//    ```sh
//    export GCP_PROJECT_ID="my-gcp-project"
//    export VERTEX_AI_LOCATION="us-central1"          # region of your index endpoint
//    export VERTEX_AI_INDEX_ENDPOINT="projects/PROJECT_NUMBER/locations/REGION/indexEndpoints/ENDPOINT_ID"
//    export VERTEX_AI_DEPLOYED_INDEX_ID="my_deployed_index"
//    # Authentication: use ADC or set GOOGLE_APPLICATION_CREDENTIALS
//    export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
//    ```
//
// 4. Run:
//    ```sh
//    cargo test -p memory --features vertex-ai --test backends_vertex_ai_test -- --ignored
//    ```

#[cfg(feature = "vertex-ai")]
mod vertex_ai_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn vertex_ai_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::VertexAi,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: None,
            pgvector: None,
            vertex_ai: Some(memory::backends::factory::VertexAiConfig {
                project_id: std::env::var("GCP_PROJECT_ID").unwrap_or_default(),
                location: std::env::var("VERTEX_AI_LOCATION")
                    .unwrap_or_else(|_| "us-central1".to_string()),
                index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT").unwrap_or_default(),
                deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID").unwrap_or_default(),
            }),
            databricks: None,
            weaviate: None,
            mongodb: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID"]
    async fn test_vertex_ai_health_check() {
        let backend = create_backend(vertex_ai_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "vertex_ai");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID"]
    async fn test_vertex_ai_capabilities() {
        let backend = create_backend(vertex_ai_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID"]
    async fn test_vertex_ai_upsert_and_search() {
        let backend = create_backend(vertex_ai_config()).await.unwrap();
        let tenant_id = "test-tenant-vertex-1";

        let records = vec![
            VectorRecord::new(
                "vt-vec-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "vt-vec-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
        ];

        let result = backend.upsert(tenant_id, records).await.unwrap();
        assert_eq!(result.upserted_count, 2);
        assert!(result.failed_ids.is_empty());

        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(1);
        let results = backend.search(tenant_id, query).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "vt-vec-1");

        backend
            .delete(
                tenant_id,
                vec!["vt-vec-1".to_string(), "vt-vec-2".to_string()],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID"]
    async fn test_vertex_ai_batch_upsert() {
        let backend = create_backend(vertex_ai_config()).await.unwrap();
        let tenant_id = "test-tenant-vertex-batch";

        let records: Vec<VectorRecord> = (0..10)
            .map(|i| {
                let mut vec = vec![0.0f32; 3];
                vec[i % 3] = 1.0;
                VectorRecord::new(
                    format!("vt-batch-{}", i),
                    vec,
                    HashMap::from([("index".to_string(), serde_json::json!(i))]),
                )
            })
            .collect();

        let result = backend.upsert(tenant_id, records).await.unwrap();
        assert_eq!(result.upserted_count, 10);

        let ids: Vec<String> = (0..10).map(|i| format!("vt-batch-{}", i)).collect();
        backend.delete(tenant_id, ids).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires GCP project with Vertex AI - set GCP_PROJECT_ID, VERTEX_AI_LOCATION, VERTEX_AI_INDEX_ENDPOINT, VERTEX_AI_DEPLOYED_INDEX_ID"]
    async fn test_vertex_ai_tenant_isolation() {
        let backend = create_backend(vertex_ai_config()).await.unwrap();

        let record_a = VectorRecord::new("vt-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("vt-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("vertex-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("vertex-tenant-b", vec![record_b])
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        assert!(
            backend
                .get("vertex-tenant-c", "vt-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("vertex-tenant-a", vec!["vt-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("vertex-tenant-b", vec!["vt-iso-1".to_string()])
            .await
            .unwrap();
    }
}
