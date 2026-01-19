//! Integration tests for Qdrant provider using testcontainers

use memory::providers::qdrant::QdrantProvider;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer, TenantContext};
use qdrant_client::{Qdrant, config::QdrantConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use testcontainers::{
    ContainerAsync, GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::sync::OnceCell;

struct QdrantFixture {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    url: String,
}

static QDRANT: OnceCell<Option<QdrantFixture>> = OnceCell::const_new();
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn get_qdrant_fixture() -> Option<&'static QdrantFixture> {
    QDRANT
        .get_or_init(|| async {
            match GenericImage::new("qdrant/qdrant", "latest")
                .with_exposed_port(ContainerPort::Tcp(6334))
                .with_wait_for(WaitFor::message_on_stdout(
                    "Qdrant is ready to accept connections",
                ))
                .start()
                .await
            {
                Ok(container) => {
                    let host = container.get_host().await.ok()?;
                    let port = container.get_host_port_ipv4(6334).await.ok()?;
                    let url = format!("http://{}:{}", host, port);
                    Some(QdrantFixture { container, url })
                }
                Err(_) => None,
            }
        })
        .await
        .as_ref()
}

fn unique_collection_name(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_{}", prefix, id)
}

fn test_ctx() -> TenantContext {
    TenantContext::default()
}

#[tokio::test]
async fn test_qdrant_full_lifecycle() {
    let Some(fixture) = get_qdrant_fixture().await else {
        eprintln!("Skipping Qdrant test: Docker not available");
        return;
    };

    let client =
        Qdrant::new(QdrantConfig::from_url(&fixture.url)).expect("Failed to create Qdrant client");

    let collection = unique_collection_name("lifecycle_test");
    let provider = QdrantProvider::new(client, collection, 128);

    provider
        .ensure_collection()
        .await
        .expect("Failed to create collection");

    let ctx = test_ctx();

    for i in 0..5 {
        let entry = MemoryEntry {
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
            id: format!("id_{}", i),
            content: format!("Content {}", i),
            embedding: Some(vec![i as f32 * 0.1; 128]),
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 1000 + i as i64,
            updated_at: 1000 + i as i64,
        };
        provider
            .add(ctx.clone(), entry)
            .await
            .expect("Failed to add entry");
    }

    let query = vec![0.25; 128];
    let search_results = provider
        .search(ctx.clone(), query, 10, HashMap::new())
        .await
        .expect("Search failed");
    assert!(search_results.len() >= 2);

    let first_id = &search_results[0].id;
    assert!(first_id == "id_2" || first_id == "id_3");

    let entry = provider
        .get(ctx.clone(), "id_0")
        .await
        .expect("Get failed")
        .expect("Entry not found");
    assert_eq!(entry.content, "Content 0");

    let mut entry_to_update = entry;
    entry_to_update.content = "Updated content".to_string();
    provider
        .update(ctx.clone(), entry_to_update)
        .await
        .expect("Update failed");

    let updated = provider
        .get(ctx.clone(), "id_0")
        .await
        .expect("Get failed")
        .expect("Entry not found");
    assert_eq!(updated.content, "Updated content");

    let (list, next_cursor) = provider
        .list(ctx.clone(), MemoryLayer::User, 2, None)
        .await
        .expect("List failed");
    assert_eq!(list.len(), 2);
    assert!(next_cursor.is_some());

    provider
        .delete(ctx.clone(), "id_0")
        .await
        .expect("Delete failed");
    let deleted = provider.get(ctx, "id_0").await.expect("Get failed");
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_qdrant_error_conditions() {
    let Some(fixture) = get_qdrant_fixture().await else {
        eprintln!("Skipping Qdrant test: Docker not available");
        return;
    };

    let client =
        Qdrant::new(QdrantConfig::from_url(&fixture.url)).expect("Failed to create Qdrant client");

    let collection = unique_collection_name("error_test");
    let provider = QdrantProvider::new(client, collection, 128);

    let entry_no_emb = MemoryEntry {
        summaries: std::collections::HashMap::new(),
        context_vector: None,
        importance_score: None,
        id: "no_emb".to_string(),
        content: "No embedding".to_string(),
        embedding: None,
        layer: MemoryLayer::User,
        metadata: HashMap::new(),
        created_at: 0,
        updated_at: 0,
    };
    let ctx = test_ctx();
    let result = provider.add(ctx, entry_no_emb).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("missing embedding")
    );

    provider.ensure_collection().await.unwrap();
    let ctx = test_ctx();
    let wrong_dim_query = vec![1.0; 64];
    let result = provider
        .search(ctx, wrong_dim_query, 10, HashMap::new())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_qdrant_complex_metadata() {
    let Some(fixture) = get_qdrant_fixture().await else {
        eprintln!("Skipping Qdrant test: Docker not available");
        return;
    };

    let client =
        Qdrant::new(QdrantConfig::from_url(&fixture.url)).expect("Failed to create Qdrant client");

    let collection = unique_collection_name("metadata_test");
    let provider = QdrantProvider::new(client, collection, 128);

    let mut metadata = HashMap::new();
    metadata.insert("tags".to_string(), serde_json::json!(["rust", "ai"]));
    metadata.insert("nested".to_string(), serde_json::json!({"key": "value"}));
    metadata.insert("priority".to_string(), serde_json::json!(5));

    let entry = MemoryEntry {
        summaries: std::collections::HashMap::new(),
        context_vector: None,
        importance_score: None,
        id: "meta_1".to_string(),
        content: "Metadata test".to_string(),
        embedding: Some(vec![0.1; 128]),
        layer: MemoryLayer::Session,
        metadata,
        created_at: 123456789,
        updated_at: 123456789,
    };

    let ctx = test_ctx();
    provider
        .add(ctx.clone(), entry.clone())
        .await
        .expect("Failed to add entry with metadata");

    let retrieved = provider
        .get(ctx, "meta_1")
        .await
        .expect("Get failed")
        .unwrap();
    assert_eq!(
        retrieved
            .metadata
            .get("priority")
            .unwrap()
            .as_i64()
            .unwrap(),
        5
    );
    assert_eq!(
        retrieved
            .metadata
            .get("tags")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        retrieved
            .metadata
            .get("nested")
            .unwrap()
            .as_object()
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap(),
        "value"
    );

    if let MemoryLayer::Session = retrieved.layer {
        assert!(true);
    } else {
        panic!("Layer was not preserved correctly");
    }
}

#[tokio::test]
async fn test_qdrant_collection_management() {
    let Some(fixture) = get_qdrant_fixture().await else {
        eprintln!("Skipping Qdrant test: Docker not available");
        return;
    };

    let client =
        Qdrant::new(QdrantConfig::from_url(&fixture.url)).expect("Failed to create Qdrant client");

    let collection = unique_collection_name("mgmt_test");
    let provider = QdrantProvider::new(client, collection, 384);

    provider
        .ensure_collection()
        .await
        .expect("First creation failed");
    provider
        .ensure_collection()
        .await
        .expect("Idempotent creation failed");
}
