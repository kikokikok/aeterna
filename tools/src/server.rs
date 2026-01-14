use crate::bridge::{ResolveFederationConflictTool, SyncNowTool, SyncStatusTool};
use crate::governance::{
    HierarchyNavigateTool, UnitCreateTool, UnitPolicyAddTool, UserRoleAssignTool,
    UserRoleRemoveTool,
};
use crate::knowledge::{KnowledgeGetTool, KnowledgeListTool, KnowledgeQueryTool};
use crate::memory::{
    GraphNeighborsTool, GraphPathTool, GraphQueryTool, MemoryAddTool, MemoryCloseTool,
    MemoryDeleteTool, MemoryFeedbackTool, MemoryOptimizeTool, MemoryReasonTool, MemorySearchTool,
};
use crate::tools::{ToolDefinition, ToolRegistry};
use knowledge::governance::GovernanceEngine;
use memory::manager::MemoryManager;
use mk_core::traits::{AuthorizationService, EventPublisher, KnowledgeRepository};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use storage::events::EventError;
use storage::graph_duckdb::DuckDbGraphStore;
use sync::bridge::SyncManager;
use tokio::time::timeout;
use tracing::{Span, debug, error, info, instrument};

/// MCP JSON-RPC server for tool orchestration.
///
/// Handles tool discovery and execution with integrated timeouts and tracing.
pub struct McpServer {
    registry: ToolRegistry,
    auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error>>,
    event_publisher: Option<Arc<dyn EventPublisher<Error = EventError>>>,
    timeout_duration: Duration,
}

impl McpServer {
    /// Creates a new McpServer with initialized core tools.
    pub fn new(
        memory_manager: Arc<MemoryManager>,
        sync_manager: Arc<SyncManager>,
        knowledge_repository: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>,
        >,
        storage_backend: Arc<
            dyn mk_core::traits::StorageBackend<Error = storage::postgres::PostgresError>,
        >,
        governance_engine: Arc<GovernanceEngine>,
        reflective_reasoner: Arc<dyn memory::reasoning::ReflectiveReasoner>,
        auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error>>,
        event_publisher: Option<Arc<dyn EventPublisher<Error = EventError>>>,
        graph_store: Option<Arc<DuckDbGraphStore>>,
    ) -> Self {
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(MemoryAddTool::new(memory_manager.clone())));
        registry.register(Box::new(MemorySearchTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryDeleteTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryCloseTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryFeedbackTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryOptimizeTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryReasonTool::new(reflective_reasoner)));

        if let Some(graph) = graph_store {
            registry.register(Box::new(GraphQueryTool::new(graph.clone())));
            registry.register(Box::new(GraphNeighborsTool::new(graph.clone())));
            registry.register(Box::new(GraphPathTool::new(graph)));
        }

        registry.register(Box::new(KnowledgeGetTool::new(
            knowledge_repository.clone(),
        )));
        registry.register(Box::new(KnowledgeListTool::new(
            knowledge_repository.clone(),
        )));
        registry.register(Box::new(KnowledgeQueryTool::new(
            memory_manager.clone(),
            knowledge_repository.clone(),
        )));

        registry.register(Box::new(SyncNowTool::new(sync_manager.clone())));
        registry.register(Box::new(SyncStatusTool::new(sync_manager.clone())));
        registry.register(Box::new(ResolveFederationConflictTool::new(sync_manager)));

        registry.register(Box::new(UnitCreateTool::new(
            storage_backend.clone(),
            governance_engine.clone(),
        )));
        registry.register(Box::new(UnitPolicyAddTool::new(
            storage_backend.clone(),
            governance_engine.clone(),
        )));
        registry.register(Box::new(UserRoleAssignTool::new(
            storage_backend.clone(),
            governance_engine.clone(),
        )));
        registry.register(Box::new(UserRoleRemoveTool::new(
            storage_backend.clone(),
            governance_engine,
        )));
        registry.register(Box::new(HierarchyNavigateTool::new(storage_backend)));

        Self {
            registry,
            auth_service,
            event_publisher,
            timeout_duration: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout_duration = duration;
        self
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.registry.list_tools()
    }

    #[instrument(skip(self, request), fields(method = %request.method, request_id = ?request.id))]
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(method = %request.method, "Handling JSON-RPC request");

        let timeout_duration = self.timeout_duration;

        let result = timeout(timeout_duration, self.dispatch(request)).await;

        match result {
            Ok(response) => response,
            Err(_) => {
                error!("Request timed out");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError::request_timeout("Request timed out")),
                }
            }
        }
    }

    async fn dispatch(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "aeterna-tools",
                        "version": "0.1.0"
                    }
                })),
                error: None,
            },
            "tools/list" => {
                let tools = self.registry.list_tools();
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serde_json::to_value(tools).unwrap()),
                    error: None,
                }
            }
            "tools/call" => {
                let params = match request.params {
                    Some(p) => p,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError::invalid_params("Invalid params")),
                        };
                    }
                };

                let tenant_context: mk_core::types::TenantContext =
                    match serde_json::from_value(params["tenantContext"].clone()) {
                        Ok(ctx) => ctx,
                        Err(_) => {
                            return JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: None,
                                error: Some(JsonRpcError::invalid_params(
                                    "Missing or invalid tenant context",
                                )),
                            };
                        }
                    };

                let (name, tool_params) = match self.extract_call_params(&params, &tenant_context) {
                    Ok(res) => res,
                    Err(e) => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError::invalid_params(e)),
                        };
                    }
                };

                Span::current().record("tool_name", &name);
                info!(tool = %name, "Calling tool");

                let auth_result = self
                    .auth_service
                    .check_permission(&tenant_context, "call_tool", &name)
                    .await;

                match auth_result {
                    Ok(allowed) => {
                        if !allowed {
                            error!(tool = %name, "Authorization denied");
                            return JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: None,
                                error: Some(JsonRpcError {
                                    code: -32002,
                                    message: format!(
                                        "Authorization error: access denied for tool {}",
                                        name
                                    ),
                                    data: None,
                                }),
                            };
                        }
                    }
                    Err(e) => {
                        error!(tool = %name, error = %e, "Authorization check failed");
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32002,
                                message: format!("Authorization error: {}", e),
                                data: None,
                            }),
                        };
                    }
                }

                let call_result = self.registry.call(&name, tool_params).await;

                match call_result {
                    Ok(result) => {
                        info!(tool = %name, "Tool call successful");

                        if let Some(ref publisher) = self.event_publisher {
                            let timestamp = chrono::Utc::now().timestamp();
                            let event = match name.as_str() {
                                "unit_create" => {
                                    Some(mk_core::types::GovernanceEvent::UnitCreated {
                                        unit_id: result["unit_id"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        unit_type: serde_json::from_value(
                                            result["unit_type"].clone(),
                                        )
                                        .unwrap_or(mk_core::types::UnitType::Project),
                                        tenant_id: tenant_context.tenant_id.clone(),
                                        parent_id: result["parent_id"]
                                            .as_str()
                                            .map(|s| s.to_string()),
                                        timestamp,
                                    })
                                }
                                "role_assign" => {
                                    Some(mk_core::types::GovernanceEvent::RoleAssigned {
                                        user_id: serde_json::from_value(result["user_id"].clone())
                                            .unwrap_or_default(),
                                        unit_id: result["unit_id"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        role: serde_json::from_value(result["role"].clone())
                                            .unwrap_or(mk_core::types::Role::Developer),
                                        tenant_id: tenant_context.tenant_id.clone(),
                                        timestamp,
                                    })
                                }
                                "role_remove" => {
                                    Some(mk_core::types::GovernanceEvent::RoleRemoved {
                                        user_id: serde_json::from_value(result["user_id"].clone())
                                            .unwrap_or_default(),
                                        unit_id: result["unit_id"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        role: serde_json::from_value(result["role"].clone())
                                            .unwrap_or(mk_core::types::Role::Developer),
                                        tenant_id: tenant_context.tenant_id.clone(),
                                        timestamp,
                                    })
                                }
                                "unit_policy_add" => {
                                    Some(mk_core::types::GovernanceEvent::PolicyUpdated {
                                        policy_id: result["policy_id"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        layer: serde_json::from_value(result["layer"].clone())
                                            .unwrap_or(mk_core::types::KnowledgeLayer::Project),
                                        tenant_id: tenant_context.tenant_id.clone(),
                                        timestamp,
                                    })
                                }
                                _ => None,
                            };

                            if let Some(event) = event {
                                if let Err(e) = publisher.publish(event).await {
                                    error!(error = %e, "Failed to publish governance event");
                                }
                            }
                        }

                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        error!(tool = %name, error = %error_str, "Tool call failed");
                        let rpc_error = if error_str.contains("not found") {
                            JsonRpcError::method_not_found(error_str)
                        } else if e.is::<serde_json::Error>()
                            || error_str.contains("Validation error")
                        {
                            JsonRpcError::invalid_params(error_str)
                        } else {
                            JsonRpcError::internal_error(error_str)
                        };

                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(rpc_error),
                        }
                    }
                }
            }
            _ => {
                debug!(method = %request.method, "Method not found");
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError::method_not_found("Method not found")),
                }
            }
        }
    }

    fn extract_call_params(
        &self,
        params: &Value,
        tenant_context: &mk_core::types::TenantContext,
    ) -> Result<(String, Value), String> {
        let name = match params["name"].as_str() {
            Some(n) => n.to_string(),
            None => return Err("Missing tool name".to_string()),
        };

        let mut tool_params = params["arguments"].clone();
        if tool_params.is_null() {
            tool_params = serde_json::json!({});
        }

        if let Some(obj) = tool_params.as_object_mut() {
            obj.insert(
                "tenant_context".to_string(),
                serde_json::to_value(tenant_context).unwrap(),
            );
            obj.insert(
                "tenantContext".to_string(),
                serde_json::to_value(tenant_context).unwrap(),
            );
        } else {
            tool_params = serde_json::json!({
                "tenant_context": tenant_context,
                "tenantContext": tenant_context
            });
        }

        Ok((name, tool_params))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
            data: None,
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
            data: None,
        }
    }

    pub fn request_timeout(message: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory::manager::MemoryManager;
    use mk_core::traits::KnowledgeRepository;
    use mk_core::types::{KnowledgeEntry, KnowledgeLayer};
    use serde_json::json;
    use sync::bridge::SyncManager;
    use sync::state_persister::SyncStatePersister;
    use testcontainers::ContainerAsync;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    struct MockRepo;
    #[async_trait::async_trait]
    impl KnowledgeRepository for MockRepo {
        type Error = knowledge::repository::RepositoryError;
        async fn store(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: KnowledgeEntry,
            _: &str,
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".into())
        }
        async fn get(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: KnowledgeLayer,
            _: &str,
        ) -> std::result::Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }
        async fn list(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: KnowledgeLayer,
            _: &str,
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(vec![])
        }
        async fn delete(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: KnowledgeLayer,
            _: &str,
            _: &str,
        ) -> std::result::Result<String, Self::Error> {
            Ok("hash".into())
        }
        async fn get_head_commit(
            &self,
            _ctx: mk_core::types::TenantContext,
        ) -> std::result::Result<Option<String>, Self::Error> {
            Ok(None)
        }
        async fn get_affected_items(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: &str,
        ) -> std::result::Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(vec![])
        }
        async fn search(
            &self,
            _ctx: mk_core::types::TenantContext,
            _: &str,
            _: Vec<KnowledgeLayer>,
            _: usize,
        ) -> std::result::Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(vec![])
        }
        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    struct MockPersister;
    #[async_trait::async_trait]
    impl SyncStatePersister for MockPersister {
        async fn load(
            &self,
            _tenant_id: &mk_core::types::TenantId,
        ) -> std::result::Result<sync::state::SyncState, Box<dyn std::error::Error + Send + Sync>>
        {
            Ok(sync::state::SyncState::default())
        }
        async fn save(
            &self,
            _tenant_id: &mk_core::types::TenantId,
            _: &sync::state::SyncState,
        ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    struct MockAuthService;
    #[async_trait::async_trait]
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

    async fn setup_postgres_container()
    -> Result<(ContainerAsync<Postgres>, String), Box<dyn std::error::Error + Send + Sync>> {
        let container = Postgres::default()
            .with_db_name("testdb")
            .with_user("testuser")
            .with_password("testpass")
            .start()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        let connection_url = format!(
            "postgres://testuser:testpass@localhost:{}/testdb",
            container
                .get_host_port_ipv4(5432)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        );

        Ok((container, connection_url))
    }

    async fn setup_server() -> McpServer {
        let memory_manager = Arc::new(MemoryManager::new());
        let repo = Arc::new(MockRepo);
        let governance = Arc::new(knowledge::governance::GovernanceEngine::new());
        let sync_manager = Arc::new(
            SyncManager::new(
                memory_manager.clone(),
                repo.clone(),
                governance.clone(),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(MockPersister),
            )
            .await
            .unwrap(),
        );

        let (container, connection_url) = setup_postgres_container()
            .await
            .expect("Failed to setup PostgreSQL test container. Make sure Docker is running.");

        let backend = storage::postgres::PostgresBackend::new(&connection_url)
            .await
            .expect("Failed to connect to PostgreSQL test container");

        let _container = container;

        let mock_reasoner = Arc::new(memory::reasoning::DefaultReflectiveReasoner::new(Arc::new(
            memory::llm::mock::MockLlmService::new(),
        )));

        McpServer::new(
            memory_manager,
            sync_manager,
            repo,
            Arc::new(backend),
            governance,
            mock_reasoner,
            Arc::new(MockAuthService),
            None,
            None,
        )
    }

    #[tokio::test]
    async fn test_server_initialize() {
        let server = setup_server().await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "initialize".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn test_server_list_tools() {
        let server = setup_server().await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        let tools = response.result.unwrap();
        assert!(tools.as_array().unwrap().len() >= 8);
    }

    #[tokio::test]
    async fn test_server_method_not_found() {
        let server = setup_server().await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "unknown_method".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_server_invalid_params() {
        let server = setup_server().await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "tools/call".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32602);
    }

    #[tokio::test]
    async fn test_server_tool_not_found() {
        let server = setup_server().await;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "tools/call".to_string(),
            params: Some(json!({
                "tenantContext": {
                    "tenant_id": "c1",
                    "user_id": "u1"
                },
                "name": "non_existent_tool",
                "arguments": {}
            })),
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_extract_tenant_context() {
        let server = setup_server().await;

        let params = json!({
            "tenantContext": {
                "tenantId": "company_1",
                "userId": "user_1"
            },
            "name": "memory_add",
            "arguments": {
                "content": "test"
            }
        });

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let _response = server.handle_request(request).await;
    }

    #[tokio::test]
    async fn test_extract_tenant_context_missing() {
        let server = setup_server().await;

        let params = json!({
            "name": "memory_add",
            "arguments": {
                "content": "test"
            }
        });

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let response = server.handle_request(request).await;
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing or invalid tenant context"));
    }

    #[tokio::test]
    async fn test_server_timeout() {
        let server = setup_server().await.with_timeout(Duration::from_millis(1));

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: "initialize".to_string(),
            params: None,
        };

        let _response = server.handle_request(request).await;
    }
}
