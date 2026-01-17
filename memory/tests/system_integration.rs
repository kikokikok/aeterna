//! System Integration Test for Aeterna Memory System
//!
//! Coordinates PostgreSQL, Redis, and Qdrant using testcontainers
//! to verify the full memory lifecycle across different storage layers.

use memory::manager::MemoryManager;
use memory::providers::qdrant::QdrantProvider;
use mk_core::types::{MemoryEntry, MemoryLayer, TenantContext};
use qdrant_client::{Qdrant, config::QdrantConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use storage::postgres::PostgresBackend;
use storage::redis::RedisStorage;
use testcontainers::{
    ContainerAsync, GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner
};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;

struct PostgresFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    url: String
}

struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String
}

struct QdrantFixture {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    url: String
}

static POSTGRES: OnceCell<Option<PostgresFixture>> = OnceCell::const_new();
static REDIS: OnceCell<Option<RedisFixture>> = OnceCell::const_new();
static QDRANT: OnceCell<Option<QdrantFixture>> = OnceCell::const_new();
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn get_postgres_fixture() -> Option<&'static PostgresFixture> {
    POSTGRES
        .get_or_init(|| async {
            let container = match Postgres::default()
                .with_db_name("aeterna_test")
                .with_user("aeterna")
                .with_password("aeterna")
                .start()
                .await
            {
                Ok(c) => c,
                Err(_) => return None
            };
            let port = match container.get_host_port_ipv4(5432).await {
                Ok(p) => p,
                Err(_) => return None
            };
            let url = format!("postgres://aeterna:aeterna@localhost:{}/aeterna_test", port);
            Some(PostgresFixture { container, url })
        })
        .await
        .as_ref()
}

async fn get_redis_fixture() -> Option<&'static RedisFixture> {
    REDIS
        .get_or_init(|| async {
            let container = match Redis::default().start().await {
                Ok(c) => c,
                Err(_) => return None
            };
            let port = match container.get_host_port_ipv4(6379).await {
                Ok(p) => p,
                Err(_) => return None
            };
            let url = format!("redis://localhost:{}", port);
            Some(RedisFixture { container, url })
        })
        .await
        .as_ref()
}

async fn get_qdrant_fixture() -> Option<&'static QdrantFixture> {
    QDRANT
        .get_or_init(|| async {
            let container = match GenericImage::new("qdrant/qdrant", "latest")
                .with_exposed_port(ContainerPort::Tcp(6334))
                .with_wait_for(WaitFor::message_on_stdout(
                    "Qdrant is ready to accept connections"
                ))
                .start()
                .await
            {
                Ok(c) => c,
                Err(_) => return None
            };
            let port = match container.get_host_port_ipv4(6334).await {
                Ok(p) => p,
                Err(_) => return None
            };
            let url = format!("http://localhost:{}", port);
            Some(QdrantFixture { container, url })
        })
        .await
        .as_ref()
}

fn unique_id(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_{}", prefix, id)
}

fn test_ctx() -> TenantContext {
    TenantContext::default()
}

#[tokio::test]
async fn test_system_wide_memory_flow() -> Result<(), Box<dyn std::error::Error>> {
    let (Some(pg_fixture), Some(redis_fixture), Some(qdrant_fixture)) = (
        get_postgres_fixture().await,
        get_redis_fixture().await,
        get_qdrant_fixture().await
    ) else {
        eprintln!("Skipping system test: Docker not available");
        return Ok(());
    };

    let pg_backend = PostgresBackend::new(&pg_fixture.url).await?;
    pg_backend.initialize_schema().await?;

    let _redis_storage = RedisStorage::new(&redis_fixture.url).await?;

    let user_collection = unique_id("system_test");
    let qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_fixture.url))?;
    let qdrant_provider = QdrantProvider::new(qdrant_client, user_collection.clone(), 128);
    qdrant_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    let manager = MemoryManager::new();
    manager
        .register_provider(MemoryLayer::User, Arc::new(qdrant_provider))
        .await;

    let msg_id = unique_id("system_msg");
    let entry = MemoryEntry {
        summaries: std::collections::HashMap::new(),
        context_vector: None,
        importance_score: None,
        id: msg_id.clone(),
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
        .get_from_layer(ctx.clone(), MemoryLayer::User, &msg_id)
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
    assert_eq!(search_results[0].id, msg_id);

    let session_collection = unique_id("session_test");
    let session_qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_fixture.url))?;
    let session_provider =
        QdrantProvider::new(session_qdrant_client, session_collection.clone(), 128);
    session_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    let project_collection = unique_id("project_test");
    let project_qdrant_client = Qdrant::new(QdrantConfig::from_url(&qdrant_fixture.url))?;
    let project_provider =
        QdrantProvider::new(project_qdrant_client, project_collection.clone(), 128);
    project_provider
        .ensure_collection()
        .await
        .map_err(|e| e.to_string())?;

    manager
        .register_provider(MemoryLayer::Session, Arc::new(session_provider))
        .await;
    manager
        .register_provider(MemoryLayer::Project, Arc::new(project_provider))
        .await;

    let session_msg_id = unique_id("session_important");
    let session_entry = MemoryEntry {
        summaries: std::collections::HashMap::new(),
        context_vector: None,
        importance_score: None,
        id: session_msg_id.clone(),
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

    manager
        .add_to_layer(ctx.clone(), MemoryLayer::Session, session_entry)
        .await
        .map_err(|e| e.to_string())?;

    let promoted_ids = manager
        .promote_important_memories(ctx.clone(), MemoryLayer::Session)
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(promoted_ids.len(), 1);
    assert!(promoted_ids[0].contains(&format!("{}_promoted", session_msg_id)));

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
