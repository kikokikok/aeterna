// Integration tests for the pgvector backend.
//
// # Setup
//
// Requires PostgreSQL 14+ with the pgvector extension installed.
//
// 1. Install pgvector and create a database:
//    ```sh
//    # macOS (Homebrew)
//    brew install pgvector
//
//    # or via Docker
//    docker run -d -e POSTGRES_PASSWORD=postgres -p 5432:5432 ankane/pgvector
//    ```
//
// 2. Enable the extension in your database:
//    ```sql
//    CREATE EXTENSION IF NOT EXISTS vector;
//    ```
//
// 3. Export environment variables:
//    ```sh
//    export PGVECTOR_URL="postgres://postgres:postgres@localhost:5432/aeterna_test"
//    # Optional overrides:
//    export PGVECTOR_SCHEMA="public"
//    export PGVECTOR_TABLE="vectors"
//    ```
//
// 4. Run:
//    ```sh
//    cargo test -p memory --features pgvector --test backends_pgvector_test -- --ignored
//    ```

#[cfg(feature = "pgvector")]
mod pgvector_tests {
    use memory::backends::{
        BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend,
    };
    use std::collections::HashMap;

    fn pgvector_config() -> BackendConfig {
        BackendConfig {
            backend_type: VectorBackendType::Pgvector,
            embedding_dimension: 3,
            qdrant: None,
            pinecone: None,
            pgvector: Some(memory::backends::factory::PgvectorConfig {
                connection_string: std::env::var("PGVECTOR_URL")
                    .or_else(|_| std::env::var("DATABASE_URL"))
                    .unwrap_or_else(|_| {
                        "postgres://postgres:postgres@localhost:5432/aeterna_test".to_string()
                    }),
                schema: std::env::var("PGVECTOR_SCHEMA").unwrap_or_else(|_| "public".to_string()),
                table_name: std::env::var("PGVECTOR_TABLE")
                    .unwrap_or_else(|_| "vectors_test".to_string()),
            }),
            vertex_ai: None,
            databricks: None,
            weaviate: None,
            mongodb: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_health_check() {
        let backend = create_backend(pgvector_config()).await.unwrap();

        let status = backend.health_check().await.unwrap();
        assert!(status.healthy);
        assert_eq!(status.backend, "pgvector");
        assert!(status.latency_ms.is_some());
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_capabilities() {
        let backend = create_backend(pgvector_config()).await.unwrap();

        let caps = backend.capabilities().await;
        assert!(caps.supports_metadata_filter);
        assert!(caps.supports_batch_upsert);
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_upsert_and_get() {
        let backend = create_backend(pgvector_config()).await.unwrap();
        let tenant_id = "test-tenant-pgvector-1";

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("memory"));
        metadata.insert("layer".to_string(), serde_json::json!("project"));

        let record = VectorRecord::new("pg-vec-1", vec![0.1, 0.2, 0.3], metadata);

        let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
        assert_eq!(result.upserted_count, 1);
        assert!(result.failed_ids.is_empty());

        let retrieved = backend.get(tenant_id, "pg-vec-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "pg-vec-1");
        assert_eq!(
            retrieved.metadata.get("type"),
            Some(&serde_json::json!("memory"))
        );

        backend
            .delete(tenant_id, vec!["pg-vec-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_search() {
        let backend = create_backend(pgvector_config()).await.unwrap();
        let tenant_id = "test-tenant-pgvector-search";

        let records = vec![
            VectorRecord::new(
                "pg-search-1",
                vec![1.0, 0.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("a"))]),
            ),
            VectorRecord::new(
                "pg-search-2",
                vec![0.0, 1.0, 0.0],
                HashMap::from([("label".to_string(), serde_json::json!("b"))]),
            ),
            VectorRecord::new(
                "pg-search-3",
                vec![0.0, 0.0, 1.0],
                HashMap::from([("label".to_string(), serde_json::json!("c"))]),
            ),
        ];

        backend.upsert(tenant_id, records).await.unwrap();

        let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
        let results = backend.search(tenant_id, query).await.unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "pg-search-1");

        backend
            .delete(
                tenant_id,
                vec![
                    "pg-search-1".to_string(),
                    "pg-search-2".to_string(),
                    "pg-search-3".to_string(),
                ],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_upsert_idempotent() {
        let backend = create_backend(pgvector_config()).await.unwrap();
        let tenant_id = "test-tenant-pgvector-upsert";

        let record = VectorRecord::new("pg-upsert-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend
            .upsert(tenant_id, vec![record.clone()])
            .await
            .unwrap();

        let updated = VectorRecord::new(
            "pg-upsert-1",
            vec![0.0, 1.0, 0.0],
            HashMap::from([("updated".to_string(), serde_json::json!(true))]),
        );
        let result = backend.upsert(tenant_id, vec![updated]).await.unwrap();
        assert_eq!(result.upserted_count, 1);

        let retrieved = backend
            .get(tenant_id, "pg-upsert-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.vector, vec![0.0, 1.0, 0.0]);

        backend
            .delete(tenant_id, vec!["pg-upsert-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_tenant_isolation() {
        let backend = create_backend(pgvector_config()).await.unwrap();

        let record_a = VectorRecord::new("pg-iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
        let record_b = VectorRecord::new("pg-iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

        backend
            .upsert("pgvector-tenant-a", vec![record_a])
            .await
            .unwrap();
        backend
            .upsert("pgvector-tenant-b", vec![record_b])
            .await
            .unwrap();

        let retrieved_a = backend
            .get("pgvector-tenant-a", "pg-iso-1")
            .await
            .unwrap()
            .unwrap();
        let retrieved_b = backend
            .get("pgvector-tenant-b", "pg-iso-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved_a.vector, vec![1.0, 0.0, 0.0]);
        assert_eq!(retrieved_b.vector, vec![0.0, 1.0, 0.0]);

        assert!(
            backend
                .get("pgvector-tenant-c", "pg-iso-1")
                .await
                .unwrap()
                .is_none()
        );

        backend
            .delete("pgvector-tenant-a", vec!["pg-iso-1".to_string()])
            .await
            .unwrap();
        backend
            .delete("pgvector-tenant-b", vec!["pg-iso-1".to_string()])
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector extension - set PGVECTOR_URL"]
    async fn test_pgvector_delete() {
        let backend = create_backend(pgvector_config()).await.unwrap();
        let tenant_id = "test-tenant-pgvector-delete";

        let record = VectorRecord::new("pg-del-1", vec![1.0, 0.0, 0.0], HashMap::new());
        backend.upsert(tenant_id, vec![record]).await.unwrap();

        assert!(backend.get(tenant_id, "pg-del-1").await.unwrap().is_some());

        let result = backend
            .delete(tenant_id, vec!["pg-del-1".to_string()])
            .await
            .unwrap();
        assert_eq!(result.deleted_count, 1);

        assert!(backend.get(tenant_id, "pg-del-1").await.unwrap().is_none());
    }
}
