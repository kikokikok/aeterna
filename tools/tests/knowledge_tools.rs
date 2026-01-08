use knowledge::repository::GitRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeType};
use std::sync::Arc;
use tempfile::tempdir;
use tools::knowledge::{KnowledgeQueryTool, KnowledgeShowTool};
use tools::tools::Tool;
use serde_json::json;

#[tokio::test]
async fn test_knowledge_tools() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a GitRepository and tools
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    
    let query_tool = KnowledgeQueryTool::new(repo.clone());
    let show_tool = KnowledgeShowTool::new(repo.clone());

    // AND some existing knowledge
    let entry = KnowledgeEntry {
        path: "architecture/core.md".to_string(),
        content: "# Core Architecture\nHierarchical memory system.".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: std::collections::HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), entry, "initial docs").await?;

    // WHEN querying knowledge
    let query_resp = query_tool.call(json!({
        "layer": "project",
        "prefix": "architecture"
    })).await?;

    // THEN it should find the entry
    assert!(query_resp["success"].as_bool().unwrap());
    assert_eq!(query_resp["totalCount"], 1);
    assert_eq!(query_resp["results"][0]["path"], "architecture/core.md");

    // WHEN showing specific knowledge
    let show_resp = show_tool.call(json!({
        "layer": "project",
        "path": "architecture/core.md"
    })).await?;

    // THEN it should return the full content
    assert!(show_resp["success"].as_bool().unwrap());
    assert_eq!(show_resp["entry"]["content"], "# Core Architecture\nHierarchical memory system.");

    Ok(())
}
