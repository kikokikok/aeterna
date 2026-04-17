use knowledge::repository::GitRepository;
use knowledge::{governance::GovernanceEngine, manager::KnowledgeManager};
use memory::manager::MemoryManager;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, Role};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;
use tools::knowledge::{KnowledgeGetTool, KnowledgePromotionPreviewTool, KnowledgeQueryTool};
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
        summaries: std::collections::HashMap::new(),
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
    assert!(
        !query_resp["results"]["keyword"]
            .as_array()
            .unwrap()
            .is_empty()
    );
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

#[tokio::test]
async fn test_knowledge_promotion_preview_tool()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));
    let tool = KnowledgePromotionPreviewTool::new(manager);

    let entry = KnowledgeEntry {
        path: "patterns/retry.md".to_string(),
        content: "Shared retry guidance\n\nLocal details: billing specific tuning".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Pattern,
        metadata: std::collections::HashMap::new(),
        summaries: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: Some("sha-1".to_string()),
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);

    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx.clone(), entry, "seed").await?;

    let preview = tool
        .call(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "sourceItemId": "patterns/retry.md",
            "targetLayer": "team",
            "mode": "partial"
        }))
        .await?;

    assert!(preview["success"].as_bool().unwrap());
    assert_eq!(preview["preview"]["sourceItemId"], "patterns/retry.md");
    assert_eq!(preview["preview"]["targetLayer"], "team");

    Ok(())
}

// ── Task 12.4 — MCP tool tests for promotion lifecycle tools ─────────────────

use tools::knowledge::{
    KnowledgeApproveTool, KnowledgeLinkTool, KnowledgePromoteTool, KnowledgeRejectTool,
    KnowledgeReviewPendingTool,
};

#[tokio::test]
async fn test_knowledge_promote_tool_creates_request()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));

    // Seed an accepted entry at project layer
    let entry = KnowledgeEntry {
        path: "patterns/promote-me.md".to_string(),
        content: "Canonical retry guidance for team promotion".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Pattern,
        metadata: std::collections::HashMap::new(),
        summaries: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);
    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx, entry, "seed promote").await?;

    // Do NOT pass sourceVersion — the tool will read the real git commit hash from
    // the stored entry and use it, so the stale check at create_promotion_request passes.
    let tool = KnowledgePromoteTool::new(manager.clone());
    let result = tool
        .call(serde_json::json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "sourceItemId": "patterns/promote-me.md",
            "targetLayer": "team",
            "mode": "full",
            "sharedContent": "Canonical retry guidance for team promotion"
        }))
        .await?;

    assert!(
        result["success"].as_bool().unwrap_or(false),
        "promote tool must succeed: {result}"
    );
    assert!(
        result["promotionRequest"]["id"].as_str().is_some(),
        "promote result must include a request id"
    );
    assert_eq!(result["promotionRequest"]["status"], "pendingReview");

    Ok(())
}

#[tokio::test]
async fn test_knowledge_review_pending_tool_lists_requests()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));

    let entry = KnowledgeEntry {
        path: "patterns/pending-list.md".to_string(),
        content: "pending list content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Pattern,
        metadata: std::collections::HashMap::new(),
        summaries: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);
    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx, entry, "seed pending").await?;

    // Create a promotion request via the promote tool first (no sourceVersion — use real hash)
    let promote_tool = KnowledgePromoteTool::new(manager.clone());
    let promote_result = promote_tool
        .call(serde_json::json!({
            "tenantContext": { "tenant_id": "t1", "user_id": "u1" },
            "sourceItemId": "patterns/pending-list.md",
            "targetLayer": "team",
            "mode": "full",
            "sharedContent": "pending list content"
        }))
        .await?;
    assert!(
        promote_result["success"].as_bool().unwrap_or(false),
        "promote must succeed: {promote_result}"
    );

    let review_tool = KnowledgeReviewPendingTool::new(manager.clone());
    let result = review_tool
        .call(serde_json::json!({
            "tenantContext": { "tenant_id": "t1", "user_id": "u1" }
        }))
        .await?;

    assert!(
        result["success"].as_bool().unwrap_or(false),
        "review_pending tool must succeed: {result}"
    );
    let requests = result["requests"]
        .as_array()
        .expect("must have requests array");
    assert!(
        !requests.is_empty(),
        "review_pending must return at least the promotion we just created"
    );

    Ok(())
}

#[tokio::test]
async fn test_knowledge_approve_tool_approves_pending_request()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));

    let entry = KnowledgeEntry {
        path: "patterns/approve-me.md".to_string(),
        content: "approvable content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Pattern,
        metadata: std::collections::HashMap::new(),
        summaries: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);
    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx, entry, "seed approve").await?;

    let promote_tool = KnowledgePromoteTool::new(manager.clone());
    let promote_result = promote_tool
        .call(serde_json::json!({
            "tenantContext": { "tenant_id": "t1", "user_id": "u1" },
            "sourceItemId": "patterns/approve-me.md",
            "targetLayer": "team",
            "mode": "full",
            "sharedContent": "approvable content"
        }))
        .await?;
    assert!(
        promote_result["success"].as_bool().unwrap_or(false),
        "promote must succeed: {promote_result}"
    );
    let request_id = promote_result["promotionRequest"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let reviewer_ctx = mk_core::types::TenantContext::new(
        mk_core::types::TenantId::new("t1".into()).unwrap(),
        mk_core::types::UserId::new("reviewer-1".into()).unwrap(),
    )
    .with_role(Role::TechLead);

    let approve_tool = KnowledgeApproveTool::new(manager.clone());
    let result = approve_tool
        .call(serde_json::json!({
            "tenantContext": reviewer_ctx,
            "promotionId": request_id,
            "decision": "approveAsReplacement"
        }))
        .await?;

    assert!(
        result["success"].as_bool().unwrap_or(false),
        "approve tool must succeed: {result}"
    );
    assert_eq!(result["promotionRequest"]["status"], "approved");

    Ok(())
}

#[tokio::test]
async fn test_knowledge_reject_tool_rejects_pending_request()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));

    let entry = KnowledgeEntry {
        path: "patterns/reject-me.md".to_string(),
        content: "rejectable content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Pattern,
        metadata: std::collections::HashMap::new(),
        summaries: std::collections::HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
    };
    let tenant_id = mk_core::types::TenantId::new("t1".into()).unwrap();
    let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
    let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);
    mk_core::traits::KnowledgeRepository::store(repo.as_ref(), ctx, entry, "seed reject").await?;

    let promote_tool = KnowledgePromoteTool::new(manager.clone());
    let promote_result = promote_tool
        .call(serde_json::json!({
            "tenantContext": { "tenant_id": "t1", "user_id": "u1" },
            "sourceItemId": "patterns/reject-me.md",
            "targetLayer": "team",
            "mode": "full",
            "sharedContent": "rejectable content"
        }))
        .await?;
    assert!(
        promote_result["success"].as_bool().unwrap_or(false),
        "promote must succeed: {promote_result}"
    );
    let request_id = promote_result["promotionRequest"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let reviewer_ctx = mk_core::types::TenantContext::new(
        mk_core::types::TenantId::new("t1".into()).unwrap(),
        mk_core::types::UserId::new("reviewer-1".into()).unwrap(),
    )
    .with_role(Role::TechLead);

    let reject_tool = KnowledgeRejectTool::new(manager.clone());
    let result = reject_tool
        .call(serde_json::json!({
            "tenantContext": reviewer_ctx,
            "promotionId": request_id,
            "reason": "not ready for broader team yet"
        }))
        .await?;

    assert!(
        result["success"].as_bool().unwrap_or(false),
        "reject tool must succeed: {result}"
    );
    assert_eq!(result["promotionRequest"]["status"], "rejected");

    Ok(())
}

#[tokio::test]
async fn test_knowledge_link_tool_creates_relation()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let manager = Arc::new(KnowledgeManager::new(
        repo.clone(),
        Arc::new(GovernanceEngine::new()),
    ));

    let link_tool = KnowledgeLinkTool::new(manager.clone());
    // "derivedFrom" is a valid KnowledgeRelationType camelCase variant
    let result = link_tool
        .call(serde_json::json!({
            "tenantContext": { "tenant_id": "t1", "user_id": "u1" },
            "sourceId": "patterns/link-source.md",
            "targetId": "patterns/link-target.md",
            "relationType": "derivedFrom"
        }))
        .await?;

    assert!(
        result["success"].as_bool().unwrap_or(false),
        "link tool must succeed: {result}"
    );
    assert!(
        result["relation"]["id"].as_str().is_some(),
        "link result must include a relation id"
    );

    Ok(())
}
