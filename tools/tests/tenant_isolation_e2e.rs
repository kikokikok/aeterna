use async_trait::async_trait;
use knowledge::repository::GitRepository;
use memory::providers::MockProvider;
use mk_core::traits::{AuthorizationService, KnowledgeRepository, StorageBackend};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeType, OrganizationalUnit, Policy, Role, TenantContext,
    TenantId, UserId,
};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;
use tools::server::{JsonRpcRequest, McpServer};

struct MockAuthService;

#[async_trait]
impl AuthorizationService for MockAuthService {
    type Error = anyhow::Error;

    async fn check_permission(
        &self,
        ctx: &TenantContext,
        _action: &str,
        _resource: &str,
    ) -> anyhow::Result<bool> {
        let tenant_id = ctx.tenant_id.as_str();
        let user_id = ctx.user_id.as_str();

        // Strict isolation for test
        if (tenant_id == "t1" && user_id == "u1") || (tenant_id == "t2" && user_id == "u2") {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_user_roles(&self, _ctx: &TenantContext) -> anyhow::Result<Vec<Role>> {
        Ok(vec![])
    }
    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

struct MockStorage;

#[async_trait]
impl StorageBackend for MockStorage {
    type Error = storage::postgres::PostgresError;

    async fn store(
        &self,
        _ctx: TenantContext,
        _key: &str,
        _value: &[u8],
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn retrieve(
        &self,
        _ctx: TenantContext,
        _key: &str,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }
    async fn delete(&self, _ctx: TenantContext, _key: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn exists(&self, _ctx: TenantContext, _key: &str) -> Result<bool, Self::Error> {
        Ok(false)
    }
    async fn get_ancestors(
        &self,
        _ctx: TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        Ok(vec![])
    }
    async fn get_descendants(
        &self,
        _ctx: TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        Ok(vec![])
    }
    async fn get_unit_policies(
        &self,
        _ctx: TenantContext,
        _unit_id: &str,
    ) -> Result<Vec<Policy>, Self::Error> {
        Ok(vec![])
    }
    async fn create_unit(&self, _unit: &OrganizationalUnit) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn add_unit_policy(
        &self,
        _ctx: &TenantContext,
        _unit_id: &str,
        _policy: &Policy,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn assign_role(
        &self,
        _user_id: &UserId,
        _tenant_id: &TenantId,
        _unit_id: &str,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn remove_role(
        &self,
        _user_id: &UserId,
        _tenant_id: &TenantId,
        _unit_id: &str,
        _role: Role,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn store_drift_result(
        &self,
        _result: mk_core::types::DriftResult,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_latest_drift_result(
        &self,
        _ctx: TenantContext,
        _project_id: &str,
    ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
        Ok(None)
    }
    async fn list_all_units(&self) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        Ok(vec![])
    }
    async fn record_job_status(
        &self,
        _job_name: &str,
        _tenant_id: &str,
        _status: &str,
        _message: Option<&str>,
        _started_at: i64,
        _finished_at: Option<i64>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn get_governance_events(
        &self,
        _ctx: TenantContext,
        _since_timestamp: i64,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
        Ok(vec![])
    }
}

struct MockPersister;
#[async_trait]
impl sync::state_persister::SyncStatePersister for MockPersister {
    async fn load(
        &self,
        _tenant_id: &TenantId,
    ) -> Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(sync::state::SyncState::default())
    }
    async fn save(
        &self,
        _tenant_id: &TenantId,
        _state: &sync::state::SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[tokio::test]
async fn test_tenant_isolation_e2e() -> anyhow::Result<()> {
    // 1. Setup Environment
    let dir = tempdir()?;
    let repo = Arc::new(GitRepository::new(dir.path())?);
    let memory_manager = Arc::new(memory::manager::MemoryManager::new());
    memory_manager
        .register_provider(
            mk_core::types::MemoryLayer::User,
            Box::new(MockProvider::new()),
        )
        .await;

    let governance_engine = Arc::new(knowledge::governance::GovernanceEngine::new());

    let auth_service = Arc::new(MockAuthService);
    let storage_backend = Arc::new(MockStorage);

    let sync_manager = Arc::new(
        sync::bridge::SyncManager::new(
            memory_manager.clone(),
            repo.clone(),
            governance_engine.clone(),
            config::config::DeploymentConfig::default(),
            None,
            Arc::new(MockPersister),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    );

    let server = McpServer::new(
        memory_manager,
        sync_manager,
        repo,
        storage_backend,
        governance_engine,
        auth_service,
        None,
    );

    // 2. Test Success: User u1 calling tool with Tenant t1
    let success_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "t1",
                "user_id": "u1"
            },
            "name": "memory_add",
            "arguments": {
                "content": "Secret for t1",
                "layer": "user"
            }
        })),
    };

    let response = server.handle_request(success_request).await;
    assert!(
        response.error.is_none(),
        "Should succeed for authorized user: {:?}",
        response.error
    );

    // 3. Test Failure: User u1 calling tool with Tenant t2 (Cross-tenant attempt)
    let failure_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!(2),
        method: "tools/call".to_string(),
        params: Some(json!({
            "tenantContext": {
                "tenant_id": "t2",
                "user_id": "u1"
            },
            "name": "memory_add",
            "arguments": {
                "content": "Attempted breach",
                "layer": "user"
            }
        })),
    };

    let response = server.handle_request(failure_request).await;
    assert!(
        response.error.is_some(),
        "Should fail for cross-tenant access"
    );
    let error = response.error.unwrap();
    assert_eq!(error.code, -32002); // Authorization error code
    assert!(error.message.contains("Authorization error"));

    Ok(())
}
