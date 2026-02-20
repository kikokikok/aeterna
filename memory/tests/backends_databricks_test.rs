// Integration tests for the Databricks Vector Search backend.
//
// # Setup
//
// Requires a Databricks workspace with Vector Search enabled and Unity Catalog.
//
// 1. Create a Vector Search index in Databricks:
//    - Navigate to Machine Learning > Vector Search
//    - Create an index with Direct Vector Access (for these tests)
//    - Dimensions: 3
//    - Similarity metric: COSINE
//
// 2. Export environment variables:
//    ```sh
//    export DATABRICKS_HOST="https://my-workspace.azuredatabricks.net"
//    export DATABRICKS_TOKEN="dapiXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
//    export DATABRICKS_CATALOG="main"       # Unity Catalog catalog name
//    export DATABRICKS_SCHEMA="aeterna"     # schema within the catalog
//    ```
//
// 3. Run:
//    ```sh
//    cargo test -p memory --features databricks --test backends_databricks_test -- --ignored
//    ```

#[cfg(feature = "databricks")]
mod databricks_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn databricks_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::Databricks,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: None,
            pgvector: None,
            vertex_ai: None,
            databricks: Some(memory::backends::factory::DatabricksConfig {
                workspace_url: std::env::var("DATABRICKS_HOST").unwrap_or_default(),
                token: std::env::var("DATABRICKS_TOKEN").unwrap_or_default(),
                catalog: std::env::var("DATABRICKS_CATALOG").unwrap_or_else(|_| "main".to_string()),
                schema: std::env::var("DATABRICKS_SCHEMA")
                    .unwrap_or_else(|_| "aeterna".to_string()),
            }),
            weaviate: None,
            mongodb: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_health_check() {
        let backend = create_backend(databricks_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "databricks");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_capabilities() {
        let backend = create_backend(databricks_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_metadata_filter);
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_upsert_and_get() {
        let backend = create_backend(databricks_config()).await.unwrap();
        let tenant_id = "test-tenant-databricks-1";

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("memory"));

        let record = VectorRecord::new("db-vec-1", vec![0.1, 0.2, 0.3], metadata);

        let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
        assert_eq!(result.upserted_count, 1);
        assert!(result.failed_ids.is_empty());

        let retrieved = backend.get(tenant_id, "db-vec-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "db-vec-1");
        assert_eq!(
            retrieved.metadata.get("type"),
            Some(&serde_json::json!("memory"))
        );

        backend
            .delete(tenant_id, vec!["db-vec-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_search() {
        let backend = create_backend(databricks_config()).await.unwrap();
        let tenant_id = "test-tenant-databricks-search";

        let records = vec![
            VectorRecord::new(
                "db-search-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "db-search-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
            VectorRecord::new(
                "db-search-3",
                vec![0.0, 0.0, 1.0],
                HashMap::from([("label".to_string(), serde_json::json!("c"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
        let results = backend.search(tenant_id, query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "db-search-1");

        backend
            .delete(
                tenant_id,
                vec![
                    "db-search-1".to_string(),
                    "db-search-2".to_string(),
                    "db-search-3".to_string(),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_tenant_isolation() {
        let backend = create_backend(databricks_config()).await.unwrap();

        let record_a = VectorRecord::new("db-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("db-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("databricks-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("databricks-tenant-b", vec![record_b])
            .await
            .unwrap();

        let retrieved_a = backend
            .get("databricks-tenant-a", "db-iso-1")
            .await
            .unwrap()
            .unwrap();
        let retrieved_b = backend
            .get("databricks-tenant-b", "db-iso-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_a.vector, vec![1.0, 0.0, 0.0]);
        assert_eq!(retrieved_b.vector, vec![0.0, 1.0, 0.0]);

        assert!(
            backend
                .get("databricks-tenant-c", "db-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("databricks-tenant-a", vec!["db-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("databricks-tenant-b", vec!["db-iso-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires Databricks workspace - set DATABRICKS_HOST, DATABRICKS_TOKEN, DATABRICKS_CATALOG, DATABRICKS_SCHEMA"]
    async fn test_databricks_delete() {
        let backend = create_backend(databricks_config()).await.unwrap();
        let tenant_id = "test-tenant-databricks-delete";

        let record = VectorRecord::new("db-del-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend.upsert(tenant_id, vec![record]).await.unwrap();

        let result = backend
            .delete(tenant_id, vec!["db-del-1".to_string()])
            .await
            .unwrap();
        assert_eq!(result.deleted_count, 1);

        assert!(backend.get(tenant_id, "db-del-1").await.unwrap().is_none());
    }
}
