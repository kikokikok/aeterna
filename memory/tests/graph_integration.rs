use memory::llm::mock::MockLlmService;
use memory::manager::MemoryManager;
use mk_core::types::{MemoryEntry, MemoryLayer, TenantContext};
use std::collections::HashMap;
use std::sync::Arc;
use storage::postgres::PostgresBackend;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
#[ignore = "requires Docker with PostgreSQL"]
async fn test_graph_based_reasoning() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let container = Postgres::default()
        .with_db_name("graph_test")
        .with_user("test")
        .with_password("test")
        .start()
        .await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let url = format!("postgres://test:test@localhost:{}/graph_test", port);

    let backend = PostgresBackend::new(&url).await?;
    backend.initialize_schema().await?;
    let graph_store = Arc::new(backend);

    let mut llm_service = MockLlmService::new();
    llm_service
        .set_response(
            r#"{
        "entities": [
            { "name": "Rust", "label": "Language", "properties": {} },
            { "name": "Borrow Checker", "label": "Feature", "properties": {} }
        ],
        "relations": [
            { "source": "Rust", "target": "Borrow Checker", "relation": "has", "properties": {} }
        ]
    }"#,
        )
        .await;
    let llm = Arc::new(llm_service);

    let manager = MemoryManager::new()
        .with_graph_store(graph_store)
        .with_llm_service(llm);

    let ctx = TenantContext::default();

    let entry = MemoryEntry {
        id: "mem_1".to_string(),
        content: "Rust has a borrow checker.".to_string(),
        embedding: None,
        layer: MemoryLayer::User,
        summaries: HashMap::new(),
        context_vector: None,
        importance_score: None,
        metadata: HashMap::new(),
        created_at: 0,
        updated_at: 0,
    };

    manager
        .register_provider(
            MemoryLayer::User,
            Arc::new(memory::providers::MockProvider::new()),
        )
        .await;
    manager
        .add_to_layer(ctx.clone(), MemoryLayer::User, entry)
        .await?;

    let nodes = manager.search_graph(ctx.clone(), "Rust", 1).await?;
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, "Rust");

    let neighbors = manager.get_graph_neighbors(ctx.clone(), "Rust").await?;
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].1.id, "Borrow Checker");
    assert_eq!(neighbors[0].0.relation, "has");

    Ok(())
}
