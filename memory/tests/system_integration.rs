//! System Integration Test for Aeterna Memory System
//!
//! Coordinates PostgreSQL, Redis, and Qdrant using testcontainers
//! to verify the full memory lifecycle across different storage layers.

use memory::manager::MemoryManager;
use memory::providers::qdrant::QdrantProvider;
use mk_core::types::{MemoryEntry, MemoryLayer, TenantContext};
use qdrant_client::{Qdrant, config::QdrantConfig};
use std::collections::HashMap;
use storage::postgres::PostgresBackend;
use storage::redis::RedisStorage;
use testcontainers::{
    ContainerAsync, GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner
};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

fn test_ctx() -> TenantContext {
    TenantContext::default()
}

async fn setup_postgres() -> Result<(ContainerAsync<Postgres>, String), Box<dyn std::error::Error>>
{
    let container = Postgres::default()
        .with_db_name("aeterna_test")
        .with_user("aeterna")
        .with_password("aeterna")
        .start()
        .await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let url = format!("postgres://aeterna:aeterna@localhost:{}/aeterna_test", port);
    Ok((container, url))
}

async fn setup_redis() -> Result<(ContainerAsync<Redis>, String), Box<dyn std::error::Error>> {
    let container = Redis::default().start().await?;
    let port = container.get_host_port_ipv4(6379).await?;
    let url = format!("redis://localhost:{}", port);
    Ok((container, url))
}

async fn setup_qdrant() -> Result<(ContainerAsync<GenericImage>, String), Box<dyn std::error::Error>>
{
    let container = GenericImage::new("qdrant/qdrant", "latest")
        .with_exposed_port(ContainerPort::Tcp(6334))
        .with_wait_for(WaitFor::message_on_stdout(
            "Qdrant is ready to accept connections"
        ))
        .start()
        .await?;
    let port = container.get_host_port_ipv4(6334).await?;
    let url = format!("http://localhost:{}", port);
    Ok((container, url))
}

#[tokio::test]
async fn test_system_wide_memory_flow() -> Result<(), Box<dyn std::error::Error>> {
    let postgres_setup = setup_postgres().await;
    let redis_setup = setup_redis().await;
    let qdrant_setup = setup_qdrant().await;

    let (_pg_container, pg_url) = match postgres_setup {
        Ok(res) => res,
        Err(_) => {
            eprintln!("Skipping system test: Docker not available");
            return Ok(());
        }
    };
    let (_redis_container, redis_url) = redis_setup?;
    let (_qdrant_container, qdrant_url) = qdrant_setup?;

    let pg_backend = PostgresBackend::new(&pg_url).await?;
    pg_backend.initialize_schema().await?;

    let _redis_storage = RedisStorage::new(&redis_url).await?;

    let qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_url))?;
    let qdrant_provider = QdrantProvider::new(qdrant_client, "system_test".to_string(), 128);
    qdrant_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    let manager = MemoryManager::new();
    manager
        .register_provider(MemoryLayer::User, Box::new(qdrant_provider))
        .await;

    let entry = MemoryEntry {
        id: "system_msg_1".to_string(),
        content: "System integration test content".to_string(),
        embedding: Some(vec![0.1; 128]),
        layer: MemoryLayer::User,
        metadata: HashMap::new(),
        created_at: 1736400000,
        updated_at: 1736400000
    };

    let ctx = test_ctx();

    manager
        .add_to_layer(ctx.clone(), MemoryLayer::User, entry.clone())
        .await
        .map_err(|e| e.to_string())?;

    let retrieved = manager
        .get_from_layer(ctx.clone(), MemoryLayer::User, "system_msg_1")
        .await
        .map_err(|e| e.to_string())?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.content, entry.content);

    let search_results = manager
        .search_hierarchical(ctx.clone(), vec![0.1; 128], 1, HashMap::new())
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].id, "system_msg_1");

    let session_entry = MemoryEntry {
        id: "session_important".to_string(),
        content: "Important session content for promotion".to_string(),
        embedding: Some(vec![0.2; 128]),
        layer: MemoryLayer::Session,
        metadata: {
            let mut m = HashMap::new();
            m.insert("score".to_string(), serde_json::json!(1.0));
            m.insert("access_count".to_string(), serde_json::json!(10));
            m.insert(
                "last_accessed_at".to_string(),
                serde_json::json!(chrono::Utc::now().timestamp())
            );
            m
        },
        created_at: 1736400000,
        updated_at: 1736400000
    };

    let session_qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_url))?;
    let session_provider =
        QdrantProvider::new(session_qdrant_client, "session_test".to_string(), 128);
    session_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    let project_qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_url))?;
    let project_provider =
        QdrantProvider::new(project_qdrant_client, "project_test".to_string(), 128);
    project_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    manager
        .register_provider(MemoryLayer::Session, Box::new(session_provider))
        .await;
    manager
        .register_provider(MemoryLayer::Project, Box::new(project_provider))
        .await;

    manager
        .add_to_layer(ctx.clone(), MemoryLayer::Session, session_entry)
        .await
        .map_err(|e| e.to_string())?;

    let promoted_ids = manager
        .promote_important_memories(ctx.clone(), MemoryLayer::Session)
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(promoted_ids.len(), 1);
    assert!(promoted_ids[0].contains("session_important_promoted"));

    let promoted_entry = manager
        .get_from_layer(ctx, MemoryLayer::Project, &promoted_ids[0])
        .await
        .map_err(|e| e.to_string())?;
    assert!(promoted_entry.is_some());
    assert_eq!(
        promoted_entry.unwrap().content,
        "Important session content for promotion"
    );

    Ok(())
}
