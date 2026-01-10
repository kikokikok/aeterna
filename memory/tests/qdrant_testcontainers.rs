//! Integration tests for Qdrant provider using testcontainers

use memory::providers::qdrant::QdrantProvider;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer};
use qdrant_client::{Qdrant, config::QdrantConfig};
use std::collections::HashMap;
use testcontainers::{
    GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner
};

#[tokio::test]
async fn test_qdrant_full_lifecycle() {
    let container = match GenericImage::new("qdrant/qdrant", "latest")
        .with_exposed_port(ContainerPort::Tcp(6334))
        .with_wait_for(WaitFor::message_on_stdout(
            "Qdrant is ready to accept connections"
        ))
        .start()
        .await
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Qdrant test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6334).await.unwrap();
    let connection_url = format!("http://{}:{}", host, port);

    let client = Qdrant::new(QdrantConfig::from_url(&connection_url))
        .expect("Failed to create Qdrant client");

    let provider = QdrantProvider::new(client, "lifecycle_test".to_string(), 128);

    provider
        .ensure_collection()
        .await
        .expect("Failed to create collection");

    for i in 0..5 {
        let entry = MemoryEntry {
            id: format!("id_{}", i),
            content: format!("Content {}", i),
            embedding: Some(vec![i as f32 * 0.1; 128]),
            layer: MemoryLayer::User,
            metadata: HashMap::new(),
            created_at: 1000 + i as i64,
            updated_at: 1000 + i as i64
        };
        provider.add(entry).await.expect("Failed to add entry");
    }

    let query = vec![0.25; 128];
    let search_results = provider
        .search(query, 10, HashMap::new())
        .await
        .expect("Search failed");
    assert!(search_results.len() >= 2);

    let first_id = &search_results[0].id;
    assert!(first_id == "id_2" || first_id == "id_3");

    let entry = provider
        .get("id_0")
        .await
        .expect("Get failed")
        .expect("Entry not found");
    assert_eq!(entry.content, "Content 0");

    let mut entry_to_update = entry;
    entry_to_update.content = "Updated content".to_string();
    provider
        .update(entry_to_update)
        .await
        .expect("Update failed");

    let updated = provider
        .get("id_0")
        .await
        .expect("Get failed")
        .expect("Entry not found");
    assert_eq!(updated.content, "Updated content");

    let (list, next_cursor) = provider
        .list(MemoryLayer::User, 2, None)
        .await
        .expect("List failed");
    assert_eq!(list.len(), 2);
    assert!(next_cursor.is_some());

    provider.delete("id_0").await.expect("Delete failed");
    let deleted = provider.get("id_0").await.expect("Get failed");
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_qdrant_error_conditions() {
    let container = match GenericImage::new("qdrant/qdrant", "latest")
        .with_exposed_port(ContainerPort::Tcp(6334))
        .with_wait_for(WaitFor::message_on_stdout(
            "Qdrant is ready to accept connections"
        ))
        .start()
        .await
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Qdrant test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6334).await.unwrap();
    let connection_url = format!("http://{}:{}", host, port);

    let client = Qdrant::new(QdrantConfig::from_url(&connection_url))
        .expect("Failed to create Qdrant client");

    let provider = QdrantProvider::new(client, "error_test".to_string(), 128);

    let entry_no_emb = MemoryEntry {
        id: "no_emb".to_string(),
        content: "No embedding".to_string(),
        embedding: None,
        layer: MemoryLayer::User,
        metadata: HashMap::new(),
        created_at: 0,
        updated_at: 0
    };
    let result = provider.add(entry_no_emb).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("missing embedding")
    );

    provider.ensure_collection().await.unwrap();
    let wrong_dim_query = vec![1.0; 64];
    let result = provider.search(wrong_dim_query, 10, HashMap::new()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_qdrant_complex_metadata() {
    let container = match GenericImage::new("qdrant/qdrant", "latest")
        .with_exposed_port(ContainerPort::Tcp(6334))
        .with_wait_for(WaitFor::message_on_stdout(
            "Qdrant is ready to accept connections"
        ))
        .start()
        .await
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Qdrant test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6334).await.unwrap();
    let connection_url = format!("http://{}:{}", host, port);

    let client = Qdrant::new(QdrantConfig::from_url(&connection_url))
        .expect("Failed to create Qdrant client");

    let provider = QdrantProvider::new(client, "metadata_test".to_string(), 128);

    let mut metadata = HashMap::new();
    metadata.insert("tags".to_string(), serde_json::json!(["rust", "ai"]));
    metadata.insert("nested".to_string(), serde_json::json!({"key": "value"}));
    metadata.insert("priority".to_string(), serde_json::json!(5));

    let entry = MemoryEntry {
        id: "meta_1".to_string(),
        content: "Metadata test".to_string(),
        embedding: Some(vec![0.1; 128]),
        layer: MemoryLayer::Session,
        metadata,
        created_at: 123456789,
        updated_at: 123456789
    };

    provider
        .add(entry.clone())
        .await
        .expect("Failed to add entry with metadata");

    let retrieved = provider.get("meta_1").await.expect("Get failed").unwrap();
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
    let container = match GenericImage::new("qdrant/qdrant", "latest")
        .with_exposed_port(ContainerPort::Tcp(6334))
        .with_wait_for(WaitFor::message_on_stdout(
            "Qdrant is ready to accept connections"
        ))
        .start()
        .await
    {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Qdrant test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(6334).await.unwrap();
    let connection_url = format!("http://{}:{}", host, port);

    let client = Qdrant::new(QdrantConfig::from_url(&connection_url))
        .expect("Failed to create Qdrant client");

    let provider = QdrantProvider::new(client, "mgmt_test".to_string(), 384);

    provider
        .ensure_collection()
        .await
        .expect("First creation failed");
    provider
        .ensure_collection()
        .await
        .expect("Idempotent creation failed");
}
