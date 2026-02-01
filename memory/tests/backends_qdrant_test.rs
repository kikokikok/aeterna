use memory::backends::{
    BackendConfig, SearchQuery, VectorBackend, VectorBackendType, VectorRecord, create_backend
};
use std::collections::HashMap;

fn qdrant_config() -> BackendConfig {
    BackendConfig {
        backend_type: VectorBackendType::Qdrant,
        embedding_dimension: 3,
        qdrant: Some(memory::backends::factory::QdrantConfig {
            url: std::env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6334".to_string()),
            api_key: std::env::var("QDRANT_API_KEY").ok(),
            collection_prefix: "test_backends".to_string()
        }),
        pinecone: None,
        pgvector: None,
        vertex_ai: None,
        databricks: None,
        weaviate: None,
        mongodb: None
    }
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_health_check() {
    let backend = create_backend(qdrant_config()).await.unwrap();

    let status = backend.health_check().await.unwrap();
    assert!(status.healthy);
    assert_eq!(status.backend, "qdrant");
    assert!(status.latency_ms.is_some());
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_capabilities() {
    let backend = create_backend(qdrant_config()).await.unwrap();

    let caps = backend.capabilities().await;
    assert!(caps.supports_metadata_filter);
    assert!(caps.supports_hybrid_search);
    assert!(caps.supports_batch_upsert);
    assert_eq!(caps.max_batch_size, 1000);
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_upsert_and_get() {
    let backend = create_backend(qdrant_config()).await.unwrap();
    let tenant_id = "test-tenant-1";

    let mut metadata = HashMap::new();
    metadata.insert("type".to_string(), serde_json::json!("memory"));
    metadata.insert("layer".to_string(), serde_json::json!("project"));

    let record = VectorRecord::new("vec-1", vec![0.1, 0.2, 0.3], metadata);

    let result = backend.upsert(tenant_id, vec![record]).await.unwrap();
    assert_eq!(result.upserted_count, 1);
    assert!(result.failed_ids.is_empty());

    let retrieved = backend.get(tenant_id, "vec-1").await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, "vec-1");
    assert_eq!(retrieved.vector, vec![0.1, 0.2, 0.3]);
    assert_eq!(
        retrieved.metadata.get("type"),
        Some(&serde_json::json!("memory"))
    );

    backend
        .delete(tenant_id, vec!["vec-1".to_string()])
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_search() {
    let backend = create_backend(qdrant_config()).await.unwrap();
    let tenant_id = "test-tenant-search";

    let records = vec![
        VectorRecord::new(
            "search-1",
            vec![1.0, 0.0, 0.0],
            HashMap::from([("label".to_string(), serde_json::json!("a"))])
        ),
        VectorRecord::new(
            "search-2",
            vec![0.0, 1.0, 0.0],
            HashMap::from([("label".to_string(), serde_json::json!("b"))])
        ),
        VectorRecord::new(
            "search-3",
            vec![0.0, 0.0, 1.0],
            HashMap::from([("label".to_string(), serde_json::json!("c"))])
        ),
    ];

    backend.upsert(tenant_id, records).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let query = SearchQuery::new(vec![1.0, 0.1, 0.0]).with_limit(2);
    let results = backend.search(tenant_id, query).await.unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].id, "search-1");

    backend
        .delete(
            tenant_id,
            vec![
                "search-1".to_string(),
                "search-2".to_string(),
                "search-3".to_string(),
            ]
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_search_with_filter() {
    let backend = create_backend(qdrant_config()).await.unwrap();
    let tenant_id = "test-tenant-filter";

    let records = vec![
        VectorRecord::new(
            "filter-1",
            vec![1.0, 0.0, 0.0],
            HashMap::from([("category".to_string(), serde_json::json!("important"))])
        ),
        VectorRecord::new(
            "filter-2",
            vec![0.9, 0.1, 0.0],
            HashMap::from([("category".to_string(), serde_json::json!("normal"))])
        ),
    ];

    backend.upsert(tenant_id, records).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let query = SearchQuery::new(vec![1.0, 0.0, 0.0])
        .with_limit(10)
        .with_filter("category", serde_json::json!("important"));

    let results = backend.search(tenant_id, query).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "filter-1");

    backend
        .delete(
            tenant_id,
            vec!["filter-1".to_string(), "filter-2".to_string()]
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_tenant_isolation() {
    let backend = create_backend(qdrant_config()).await.unwrap();

    let record_a = VectorRecord::new("iso-1", vec![1.0, 0.0, 0.0], HashMap::new());
    let record_b = VectorRecord::new("iso-1", vec![0.0, 1.0, 0.0], HashMap::new());

    backend.upsert("tenant-a", vec![record_a]).await.unwrap();
    backend.upsert("tenant-b", vec![record_b]).await.unwrap();

    let retrieved_a = backend.get("tenant-a", "iso-1").await.unwrap().unwrap();
    let retrieved_b = backend.get("tenant-b", "iso-1").await.unwrap().unwrap();

    assert_eq!(retrieved_a.vector, vec![1.0, 0.0, 0.0]);
    assert_eq!(retrieved_b.vector, vec![0.0, 1.0, 0.0]);

    assert!(
        backend
            .get("tenant-a", "nonexistent")
            .await
            .unwrap()
            .is_none()
    );
    assert!(backend.get("tenant-c", "iso-1").await.unwrap().is_none());

    backend
        .delete("tenant-a", vec!["iso-1".to_string()])
        .await
        .unwrap();
    backend
        .delete("tenant-b", vec!["iso-1".to_string()])
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires running Qdrant instance"]
async fn test_qdrant_backend_delete() {
    let backend = create_backend(qdrant_config()).await.unwrap();
    let tenant_id = "test-tenant-delete";

    let record = VectorRecord::new("del-1", vec![1.0, 0.0, 0.0], HashMap::new());
    backend.upsert(tenant_id, vec![record]).await.unwrap();

    assert!(backend.get(tenant_id, "del-1").await.unwrap().is_some());

    let result = backend
        .delete(tenant_id, vec!["del-1".to_string()])
        .await
        .unwrap();
    assert_eq!(result.deleted_count, 1);

    assert!(backend.get(tenant_id, "del-1").await.unwrap().is_none());
}
