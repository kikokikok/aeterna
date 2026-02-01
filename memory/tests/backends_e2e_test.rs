use memory::backends::{
    BackendConfig, BackendError, SearchQuery, VectorBackend, VectorBackendType, VectorRecord,
    create_backend
};
use std::collections::HashMap;
use std::sync::Arc;

fn make_test_records(prefix: &str, count: usize, dim: usize) -> Vec<VectorRecord> {
    (0..count)
        .map(|i| {
            let mut metadata = HashMap::new();
            metadata.insert("index".to_string(), serde_json::json!(i));
            metadata.insert("prefix".to_string(), serde_json::json!(prefix));

            let vector: Vec<f32> = (0..dim).map(|j| ((i + j) as f32) / 10.0).collect();

            VectorRecord::new(format!("{}-{}", prefix, i), vector, metadata)
        })
        .collect()
}

async fn run_backend_test_suite(backend: Arc<dyn VectorBackend>, tenant_id: &str, dim: usize) {
    let backend_name = backend.backend_name();

    let health = backend.health_check().await.unwrap();
    assert!(health.healthy, "{} health check failed", backend_name);

    let caps = backend.capabilities().await;
    assert!(caps.max_vector_dimensions > 0);

    let records = make_test_records("e2e", 5, dim);
    let upsert_result = backend.upsert(tenant_id, records.clone()).await.unwrap();
    assert_eq!(
        upsert_result.upserted_count, 5,
        "{} upsert count mismatch",
        backend_name
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let query_vector: Vec<f32> = (0..dim).map(|j| (j as f32) / 10.0).collect();
    let query = SearchQuery::new(query_vector).with_limit(3);
    let results = backend.search(tenant_id, query).await.unwrap();
    assert!(
        !results.is_empty(),
        "{} search returned no results",
        backend_name
    );
    assert!(
        results.len() <= 3,
        "{} search limit not respected",
        backend_name
    );

    let retrieved = backend.get(tenant_id, "e2e-0").await.unwrap();
    assert!(retrieved.is_some(), "{} get returned None", backend_name);
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "e2e-0");

    let delete_result = backend
        .delete(
            tenant_id,
            vec![
                "e2e-0".to_string(),
                "e2e-1".to_string(),
                "e2e-2".to_string(),
                "e2e-3".to_string(),
                "e2e-4".to_string(),
            ]
        )
        .await
        .unwrap();
    assert!(
        delete_result.deleted_count > 0,
        "{} delete count is 0",
        backend_name
    );

    let after_delete = backend.get(tenant_id, "e2e-0").await.unwrap();
    assert!(
        after_delete.is_none(),
        "{} record still exists after delete",
        backend_name
    );
}

async fn run_tenant_isolation_test(backend: Arc<dyn VectorBackend>, dim: usize) {
    let backend_name = backend.backend_name();

    let record_a = VectorRecord::new(
        "shared-id",
        (0..dim).map(|i| i as f32 * 0.1).collect(),
        HashMap::from([("tenant".to_string(), serde_json::json!("a"))])
    );
    let record_b = VectorRecord::new(
        "shared-id",
        (0..dim).map(|i| i as f32 * 0.2).collect(),
        HashMap::from([("tenant".to_string(), serde_json::json!("b"))])
    );

    backend.upsert("tenant-a", vec![record_a]).await.unwrap();
    backend.upsert("tenant-b", vec![record_b]).await.unwrap();

    let get_a = backend.get("tenant-a", "shared-id").await.unwrap();
    let get_b = backend.get("tenant-b", "shared-id").await.unwrap();

    assert!(
        get_a.is_some(),
        "{} tenant-a record not found",
        backend_name
    );
    assert!(
        get_b.is_some(),
        "{} tenant-b record not found",
        backend_name
    );

    let vec_a = get_a.unwrap().vector;
    let vec_b = get_b.unwrap().vector;
    assert_ne!(
        vec_a, vec_b,
        "{} tenant isolation failed - vectors match",
        backend_name
    );

    let get_c = backend.get("tenant-c", "shared-id").await.unwrap();
    assert!(
        get_c.is_none(),
        "{} tenant-c found record from other tenant",
        backend_name
    );

    backend
        .delete("tenant-a", vec!["shared-id".to_string()])
        .await
        .unwrap();
    backend
        .delete("tenant-b", vec!["shared-id".to_string()])
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Qdrant,
        embedding_dimension: 128,
        qdrant: Some(memory::backends::factory::QdrantConfig {
            url: std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into()),
            api_key: std::env::var("QDRANT_API_KEY").ok(),
            collection_prefix: "e2e_test".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-qdrant", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires running PostgreSQL with pgvector"]
async fn test_pgvector_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Pgvector,
        embedding_dimension: 128,
        pgvector: Some(memory::backends::factory::PgvectorConfig {
            connection_string: std::env::var("PGVECTOR_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/aeterna_test".into()),
            schema: "public".into(),
            table_name: "e2e_vectors".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-pgvector", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires Pinecone API key"]
async fn test_pinecone_e2e() {
    let api_key = std::env::var("PINECONE_API_KEY").expect("PINECONE_API_KEY required");
    let environment = std::env::var("PINECONE_ENVIRONMENT").expect("PINECONE_ENVIRONMENT required");

    let config = BackendConfig {
        backend_type: VectorBackendType::Pinecone,
        embedding_dimension: 128,
        pinecone: Some(memory::backends::factory::PineconeConfig {
            api_key,
            environment,
            index_name: "e2e-test".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-pinecone", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires running Weaviate instance"]
async fn test_weaviate_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Weaviate,
        embedding_dimension: 128,
        weaviate: Some(memory::backends::factory::WeaviateConfig {
            url: std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://localhost:8080".into()),
            api_key: std::env::var("WEAVIATE_API_KEY").ok(),
            class_name: "E2ETest".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-weaviate", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires MongoDB Atlas with vector search"]
async fn test_mongodb_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Mongodb,
        embedding_dimension: 128,
        mongodb: Some(memory::backends::factory::MongodbConfig {
            connection_string: std::env::var("MONGODB_URI").expect("MONGODB_URI required"),
            database: "e2e_test".into(),
            collection: "vectors".into(),
            index_name: "vector_index".into()
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-mongodb", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires GCP project with Vertex AI"]
async fn test_vertex_ai_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::VertexAi,
        embedding_dimension: 128,
        vertex_ai: Some(memory::backends::factory::VertexAiConfig {
            project_id: std::env::var("GCP_PROJECT_ID").expect("GCP_PROJECT_ID required"),
            location: std::env::var("VERTEX_AI_LOCATION").unwrap_or_else(|_| "us-central1".into()),
            index_endpoint: std::env::var("VERTEX_AI_INDEX_ENDPOINT")
                .expect("VERTEX_AI_INDEX_ENDPOINT required"),
            deployed_index_id: std::env::var("VERTEX_AI_DEPLOYED_INDEX_ID")
                .expect("VERTEX_AI_DEPLOYED_INDEX_ID required")
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-vertex", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
#[ignore = "requires Databricks workspace"]
async fn test_databricks_e2e() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Databricks,
        embedding_dimension: 128,
        databricks: Some(memory::backends::factory::DatabricksConfig {
            workspace_url: std::env::var("DATABRICKS_HOST").expect("DATABRICKS_HOST required"),
            token: std::env::var("DATABRICKS_TOKEN").expect("DATABRICKS_TOKEN required"),
            catalog: std::env::var("DATABRICKS_CATALOG").unwrap_or_else(|_| "main".into()),
            schema: std::env::var("DATABRICKS_SCHEMA").unwrap_or_else(|_| "e2e_test".into())
        }),
        ..Default::default()
    };

    let backend = create_backend(config).await.unwrap();
    run_backend_test_suite(backend.clone(), "e2e-databricks", 128).await;
    run_tenant_isolation_test(backend, 128).await;
}

#[tokio::test]
async fn test_backend_switching_config() {
    for backend_type in [
        VectorBackendType::Qdrant,
        VectorBackendType::Pinecone,
        VectorBackendType::Pgvector,
        VectorBackendType::Weaviate,
        VectorBackendType::Mongodb,
        VectorBackendType::VertexAi,
        VectorBackendType::Databricks
    ] {
        let type_str = backend_type.to_string();
        let parsed: VectorBackendType = type_str.parse().unwrap();
        assert_eq!(parsed, backend_type);
    }

    let config = BackendConfig::default();
    assert_eq!(config.backend_type, VectorBackendType::Qdrant);
    assert!(config.qdrant.is_some());
}

#[tokio::test]
async fn test_missing_config_errors() {
    let config = BackendConfig {
        backend_type: VectorBackendType::Pinecone,
        embedding_dimension: 128,
        pinecone: None,
        ..Default::default()
    };

    let result = create_backend(config).await;
    assert!(result.is_err());

    if let Err(BackendError::Configuration(msg)) = result {
        assert!(msg.contains("config missing") || msg.contains("not enabled"));
    }
}
