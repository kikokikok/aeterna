use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;
use tools::knowledge::{KnowledgeGetTool, KnowledgeQueryTool};
use tools::tools::Tool;

#[tokio::test]
async fn test_knowledge_tools() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a GitRepository and tools
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let memory_manager = Arc::new(MemoryManager::new());

    let query_tool = KnowledgeQueryTool::new(memory_manager, repo.clone());
    let show_tool = KnowledgeGetTool::new(repo.clone());

    // AND some existing knowledge
    let entry = KnowledgeEntry {
        path: "architecture/core.md".to_string(),
        content: "# Core Architecture\nHierarchical memory system.".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);

    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx, entry, "initial docs").await?;

    // WHEN querying knowledge
    let query_resp = query_tool
        .call(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "query": "Architecture",
            "layers": ["project"]
        }))
        .await?;

    // THEN it should find the entry
    assert!(query_resp["success"].as_bool().unwrap());
    assert!(query_resp["results"]["keyword"].as_array().unwrap().len() >= 1);
    assert_eq!(
        query_resp["results"]["keyword"][0]["path"],
        "architecture/core.md"
    );

    // WHEN showing specific knowledge
    let show_resp = show_tool
        .call(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "layer": "project",
            "path": "architecture/core.md"
        }))
        .await?;

    // THEN it should return the full content
    assert!(show_resp["success"].as_bool().unwrap());
    assert_eq!(
        show_resp["entry"]["content"],
        "# Core Architecture\nHierarchical memory system."
    );

    Ok(())
}
