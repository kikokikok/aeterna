use crate::bridge::{SyncNowTool, SyncStatusTool};
use crate::knowledge::{KnowledgeCheckTool, KnowledgeQueryTool, KnowledgeShowTool};
use crate::memory::{MemoryAddTool, MemoryDeleteTool, MemorySearchTool};
use crate::tools::{ToolDefinition, ToolRegistry};
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use sync::bridge::SyncManager;
use tokio::time::timeout;
use tracing::{Span, debug, error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>
}

impl JsonRpcError {
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None
        }
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
            data: None
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
            data: None
        }
    }

    pub fn request_timeout(message: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: message.into(),
            data: None
        }
    }
}

/// MCP JSON-RPC server for tool orchestration.
///
/// Handles tool discovery and execution with integrated timeouts and tracing.
pub struct McpServer {
    registry: ToolRegistry,
    timeout_duration: Duration
}

impl McpServer {
    /// Creates a new McpServer with initialized core tools.
    pub fn new(
        memory_manager: Arc<MemoryManager>,
        sync_manager: Arc<SyncManager>,
        knowledge_repository: Arc<
            dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>
        >
    ) -> Self {
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(MemoryAddTool::new(memory_manager.clone())));
        registry.register(Box::new(MemorySearchTool::new(memory_manager.clone())));
        registry.register(Box::new(MemoryDeleteTool::new(memory_manager)));

        registry.register(Box::new(KnowledgeQueryTool::new(
            knowledge_repository.clone()
        )));
        registry.register(Box::new(KnowledgeShowTool::new(
            knowledge_repository.clone()
        )));
        registry.register(Box::new(KnowledgeCheckTool::new()));

        registry.register(Box::new(SyncNowTool::new(sync_manager.clone())));
        registry.register(Box::new(SyncStatusTool::new(sync_manager)));

        Self {
            registry,
            timeout_duration: Duration::from_secs(30)
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
                    error: Some(JsonRpcError::request_timeout("Request timed out"))
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
                error: None
            },
            "tools/list" => {
                let tools = self.registry.list_tools();
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: Some(serde_json::to_value(tools).unwrap()),
                    error: None
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
                            error: Some(JsonRpcError::invalid_params("Invalid params"))
                        };
                    }
                };

                let name = match params["name"].as_str() {
                    Some(n) => n,
                    None => {
                        return JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError::invalid_params("Missing tool name"))
                        };
                    }
                };

                Span::current().record("tool_name", name);
                info!(tool = %name, "Calling tool");

                let tool_params = params["arguments"].clone();

                match self.registry.call(name, tool_params).await {
                    Ok(result) => {
                        info!(tool = %name, "Tool call successful");
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: Some(result),
                            error: None
                        }
                    }
                    Err(e) => {
                        error!(tool = %name, error = %e, "Tool call failed");
                        let rpc_error = if e.is::<serde_json::Error>() {
                            JsonRpcError::invalid_params(e.to_string())
                        } else if e.to_string().contains("Validation error") {
                            JsonRpcError::invalid_params(e.to_string())
                        } else {
                            JsonRpcError::internal_error(e.to_string())
                        };

                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: request.id,
                            result: None,
                            error: Some(rpc_error)
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
                    error: Some(JsonRpcError::method_not_found("Method not found"))
                }
            }
        }
    }
}
