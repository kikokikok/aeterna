use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::MemoryProviderAdapter;
use mk_core::types::MemoryLayer;
use serde_json::json;
use std::sync::Arc;
use tools::memory::{MemoryAddTool, MemoryDeleteTool, MemorySearchTool};
use tools::tools::Tool;

#[tokio::test]
async fn test_memory_tools() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // GIVEN a MemoryManager and tools
    let memory_manager = Arc::new(
        MemoryManager::new()
            .with_embedding_service(Arc::new(memory::embedding::MockEmbeddingService::new(1536)))
    );
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync
    > = Arc::new(MockProvider::new());
    memory_manager
        .register_provider(MemoryLayer::User, provider)
        .await;

    let add_tool = MemoryAddTool::new(memory_manager.clone());
    let search_tool = MemorySearchTool::new(memory_manager.clone());
    let delete_tool = MemoryDeleteTool::new(memory_manager.clone());

    let tenant_context = json!({
        "tenant_id": "test-tenant",
        "user_id": "test-user"
    });

    // WHEN adding memory
    let add_resp = add_tool
        .call(json!({
            "content": "User prefers Rust",
            "layer": "user",
            "metadata": {
                "user_id": "test_user_123"
            },
            "tenantContext": tenant_context
        }))
        .await?;

    // THEN it should succeed
    assert!(add_resp["success"].as_bool().unwrap());
    let memory_id = add_resp["memoryId"].as_str().unwrap().to_string();

    // WHEN searching memory
    let search_resp = search_tool
        .call(json!({
            "query": "rust",
            "tenantContext": tenant_context
        }))
        .await?;

    // THEN it should find the entry
    assert!(search_resp["success"].as_bool().unwrap());
    assert_eq!(search_resp["totalCount"], 1);
    assert_eq!(search_resp["results"][0]["content"], "User prefers Rust");

    // WHEN deleting memory
    let delete_resp = delete_tool
        .call(json!({
            "memoryId": memory_id,
            "layer": "user",
            "tenantContext": tenant_context
        }))
        .await?;

    // THEN it should succeed
    assert!(delete_resp["success"].as_bool().unwrap());

    // AND search should return empty
    let search_resp = search_tool
        .call(json!({
            "query": "rust",
            "tenantContext": tenant_context
        }))
        .await?;
    assert_eq!(search_resp["totalCount"], 0);

    Ok(())
}
