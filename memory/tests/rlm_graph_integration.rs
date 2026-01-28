use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::GitRepository;
#[cfg(feature = "llm-integration")]
use memory::llm::mock::MockLlmService;
use memory::manager::MemoryManager;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::{MemoryEntry, MemoryLayer, TenantContext, TenantId, UserId};
use std::collections::HashMap;
use std::sync::Arc;
use storage::graph::GraphStore;
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore};

pub fn test_ctx() -> TenantContext {
    use std::str::FromStr;
    TenantContext::new(
        TenantId::from_str("test-tenant").unwrap(),
        UserId::from_str("test-user").unwrap()
    )
}

#[tokio::test]
async fn test_rlm_graph_reward_propagation() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let graph_store = Arc::new(
        DuckDbGraphStore::new(DuckDbGraphConfig::default())
            .expect("Failed to create DuckDB graph store")
    );

    let repo = Arc::new(GitRepository::new_mock());
    let governance = Arc::new(GovernanceEngine::new());
    let km = Arc::new(KnowledgeManager::new(repo, governance));

    let llm_service = memory::llm::mock::MockLlmService::new();

    let action1 = r#"{"SearchLayer": {"layer": "project", "query": "bridge"}}"#;
    let action2 = r#"{"DrillDown": {"memory_id": "mem1", "query": "target"}}"#;
    let action3 =
        r#"{"Aggregate": {"strategy": "Summary", "results": ["Found target via bridge"]}}"#;

    llm_service
        .set_responses(vec![
            action1.to_string(),
            action2.to_string(),
            action3.to_string(),
        ])
        .await;
    let llm = Arc::new(llm_service);

    let manager = MemoryManager::new()
        .with_embedding_service(Arc::new(
            memory::embedding::mock::MockEmbeddingService::new(1536)
        ))
        .with_config(config::MemoryConfig {
            rlm: config::RlmConfig {
                enabled: true,
                max_steps: 5,
                complexity_threshold: 0.1
            },
            ..Default::default()
        })
        .with_graph_store(graph_store.clone()
            as Arc<dyn GraphStore<Error = Box<dyn std::error::Error + Send + Sync>>>)
        .with_llm_service(llm)
        .with_knowledge_manager(km.clone());

    let ctx = test_ctx();
    println!("Starting RLM graph integration test");

    km.add(
        ctx.clone(),
        mk_core::types::KnowledgeEntry {
            path: "mem1".to_string(),
            content: "bridge content".to_string(),
            layer: mk_core::types::KnowledgeLayer::Project,
            kind: mk_core::types::KnowledgeType::Spec,
            status: mk_core::types::KnowledgeStatus::Accepted,
            summaries: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: 0
        },
        "add mem1"
    )
    .await?;

    graph_store
        .add_node(
            ctx.clone(),
            storage::graph::GraphNode {
                id: "mem1".to_string(),
                label: "Memory".to_string(),
                properties: serde_json::json!({}),
                tenant_id: ctx.tenant_id.to_string()
            }
        )
        .await?;

    graph_store
        .add_node(
            ctx.clone(),
            storage::graph::GraphNode {
                id: "mem2".to_string(),
                label: "target".to_string(),
                properties: serde_json::json!({}),
                tenant_id: ctx.tenant_id.to_string()
            }
        )
        .await?;

    graph_store
        .add_edge(
            ctx.clone(),
            storage::graph::GraphEdge {
                id: "edge1".to_string(),
                source_id: "mem1".to_string(),
                target_id: "mem2".to_string(),
                relation: "RELATES_TO".to_string(),
                properties: serde_json::json!({}),
                tenant_id: ctx.tenant_id.to_string()
            }
        )
        .await?;

    let provider = Arc::new(memory::providers::MockProvider::new());
    manager
        .register_provider(MemoryLayer::Project, provider.clone())
        .await;

    provider
        .add(
            ctx.clone(),
            MemoryEntry {
                id: "mem1".to_string(),
                content: "Bridge memory content".to_string(),
                layer: MemoryLayer::Project,
                importance_score: Some(0.5),
                ..Default::default()
            }
        )
        .await?;

    provider
        .add(
            ctx.clone(),
            MemoryEntry {
                id: "mem2".to_string(),
                content: "Target memory content".to_string(),
                layer: MemoryLayer::Project,
                importance_score: Some(0.5),
                ..Default::default()
            }
        )
        .await?;

    let results = manager
        .search(
            ctx.clone(),
            "compare the bridge and target relationships summarize",
            1,
            0.0,
            HashMap::new()
        )
        .await?;

    assert!(!results.is_empty());

    let m1 = manager
        .get_from_layer(ctx.clone(), MemoryLayer::Project, "mem1")
        .await?
        .unwrap();
    let m2 = manager
        .get_from_layer(ctx.clone(), MemoryLayer::Project, "mem2")
        .await?
        .unwrap();

    assert!(m1.importance_score.unwrap() > 0.5);
    assert!(m2.importance_score.unwrap() > 0.5);

    assert!(m1.metadata.contains_key("reward"));
    assert!(m2.metadata.contains_key("reward"));

    let trajectories = manager.get_trajectories(&ctx).await;
    let reward_events: Vec<_> = trajectories.iter().filter(|e| e.reward.is_some()).collect();

    assert!(
        reward_events.len() >= 2,
        "Should have at least 2 reward events in trajectory, found {}",
        reward_events.len()
    );

    let ids: Vec<_> = reward_events.iter().map(|e| e.entry_id.as_str()).collect();
    assert!(ids.contains(&"mem1"));
    assert!(ids.contains(&"mem2"));

    Ok(())
}
