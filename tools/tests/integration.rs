use adapters::ecosystem::{EcosystemAdapter, OpenCodeAdapter};
use adapters::langchain::LangChainAdapter;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use memory::providers::MockProvider;
use mk_core::traits::{KnowledgeRepository, MemoryProviderAdapter};
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, MemoryLayer, TenantId};
use serde_json::json;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use testing::postgres;
use tools::server::{JsonRpcRequest, McpServer};

struct MockKnowledgeRepo;

#[async_trait]
impl KnowledgeRepository for MockKnowledgeRepo {
    type Error = knowledge::repository::RepositoryError;

    async fn get(
        &self,
        _ctx: mk_core::types::TenantContext,
        _layer: KnowledgeLayer,
        _path: &str,
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        Ok(None)
    }

    async fn store(
        &self,
        _ctx: mk_core::types::TenantContext,
        _entry: KnowledgeEntry,
        _message: &str,
    ) -> Result<String, Self::Error> {
        Ok("hash".to_string())
    }

    async fn list(
        &self,
        _ctx: mk_core::types::TenantContext,
        _layer: KnowledgeLayer,
        _prefix: &str,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }

    async fn delete(
        &self,
        _ctx: mk_core::types::TenantContext,
        _layer: KnowledgeLayer,
        _path: &str,
        _message: &str,
    ) -> Result<String, Self::Error> {
        Ok("hash".to_string())
    }

    async fn get_head_commit(
        &self,
        _ctx: mk_core::types::TenantContext,
    ) -> Result<Option<String>, Self::Error> {
        Ok(Some("head".to_string()))
    }

    async fn get_affected_items(
        &self,
        _ctx: mk_core::types::TenantContext,
        _since_commit: &str,
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        Ok(vec![])
    }

    async fn search(
        &self,
        _ctx: mk_core::types::TenantContext,
        _query: &str,
        _layers: Vec<KnowledgeLayer>,
        _limit: usize,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

struct MockPersister;

#[async_trait]
impl SyncStatePersister for MockPersister {
    async fn load(
        &self,
        _tenant_id: &TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }
    async fn save(
        &self,
        _tenant_id: &TenantId,
        _state: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

struct MockAuthService;
#[async_trait]
impl mk_core::traits::AuthorizationService for MockAuthService {
    type Error = anyhow::Error;
    async fn check_permission(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _action: &str,
        _resource: &str,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
    async fn get_user_roles(
        &self,
        _ctx: &mk_core::types::TenantContext,
    ) -> anyhow::Result<Vec<mk_core::types::Role>> {
        Ok(vec![])
    }
    async fn assign_role(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _user_id: &mk_core::types::UserId,
        _role: mk_core::types::Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn remove_role(
        &self,
        _ctx: &mk_core::types::TenantContext,
        _user_id: &mk_core::types::UserId,
        _role: mk_core::types::Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_full_integration_mcp_to_adapters() -> anyhow::Result<()> {
    let Some(pg_fixture) = postgres().await else {
        eprintln!("Skipping test: Docker not available");
        return Ok(());
    };

    let memory_manager = Arc::new(MemoryManager::new());
    let provider: Arc<
        dyn MemoryProviderAdapter<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
    > = Arc::new(MockProvider::new());
    memory_manager
        .register_provider(MemoryLayer::User, provider)
        .await;

    let knowledge_repo = Arc::new(knowledge::repository::GitRepository::new_mock());
    let governance_engine = Arc::new(knowledge::governance::GovernanceEngine::new());
    let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
        knowledge_repo.clone(),
        governance_engine.clone(),
    ));
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_manager,
            config::config::DeploymentConfig::default(),
            None,
            Arc::new(MockPersister),
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    );

    let server = Arc::new(McpServer::new(
        memory_manager,
        sync_manager,
        knowledge_repo.clone(),
        Arc::new(
            storage::postgres::PostgresBackend::new(pg_fixture.url())
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ),
        governance_engine,
        Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        ))),
        Arc::new(MockAuthService),
        None,
        None,
    ));

    let opencode = OpenCodeAdapter::new(server.clone());
    let memory_tools = opencode.get_memory_tools();
    assert!(!memory_tools.is_empty());

    let langchain = LangChainAdapter::new(server.clone());
    let lc_tools = langchain.to_langchain_tools();
    assert!(
        lc_tools.len() >= 18,
        "Expected at least 18 LangChain tools, got {}",
        lc_tools.len()
    );

    let response = langchain
        .handle_mcp_request(json!({
            "tenantContext": {
                "tenant_id": "c1",
                "user_id": "u1"
            },
            "name": "memory_add",
            "arguments": {
                "content": "Integrated test",
                "layer": "user"
            }
        }))
        .await?;

    if let Some(error) = response["error"].as_object() {
        panic!("Tool call failed: {:?}", error);
    }
    assert!(!response["result"].is_null());

    Ok(())
}

#[tokio::test]
async fn test_server_timeout() -> anyhow::Result<()> {
    let Some(pg_fixture) = postgres().await else {
        eprintln!("Skipping test: Docker not available");
        return Ok(());
    };

    let memory_manager = Arc::new(MemoryManager::new());
    let knowledge_repo = Arc::new(knowledge::repository::GitRepository::new_mock());
    let governance_engine = Arc::new(knowledge::governance::GovernanceEngine::new());
    let knowledge_manager = Arc::new(knowledge::manager::KnowledgeManager::new(
        knowledge_repo.clone(),
        governance_engine.clone(),
    ));
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_manager,
            config::config::DeploymentConfig::default(),
            None,
            Arc::new(MockPersister),
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    );

    let _server = McpServer::new(
        memory_manager.clone(),
        sync_manager.clone(),
        knowledge_repo.clone(),
        Arc::new(
            storage::postgres::PostgresBackend::new(pg_fixture.url())
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ),
        governance_engine.clone(),
        Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        ))),
        Arc::new(MockAuthService),
        None,
        None,
    );

    struct DenyAuthService;
    #[async_trait]
    impl mk_core::traits::AuthorizationService for DenyAuthService {
        type Error = anyhow::Error;
        async fn check_permission(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _action: &str,
            _resource: &str,
        ) -> anyhow::Result<bool> {
            Ok(false)
        }
        async fn get_user_roles(
            &self,
            _ctx: &mk_core::types::TenantContext,
        ) -> anyhow::Result<Vec<mk_core::types::Role>> {
            Ok(vec![])
        }
        async fn assign_role(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _user_id: &mk_core::types::UserId,
            _role: mk_core::types::Role,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn remove_role(
            &self,
            _ctx: &mk_core::types::TenantContext,
            _user_id: &mk_core::types::UserId,
            _role: mk_core::types::Role,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    let server = McpServer::new(
        memory_manager.clone(),
        sync_manager.clone(),
        knowledge_repo.clone(),
        Arc::new(
            storage::postgres::PostgresBackend::new(pg_fixture.url())
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ),
        Arc::new(knowledge::governance::GovernanceEngine::new()),
        Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        ))),
        Arc::new(DenyAuthService),
        None,
        None,
    );

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "c1",
                "user_id": "u1"
            },
            "name": "memory_search",
            "arguments": {
                "query": "test"
            }
        })),
    };

    let response = server.handle_request(request).await;

    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert_eq!(error.code, -32002);
    assert!(error.message.contains("Authorization error"));

    Ok(())
}
