use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tools::server::JsonRpcRequest;
use tools::server::McpServer;

#[async_trait]
pub trait EcosystemAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value>;
}

pub struct OpenCodeAdapter {
    server: Arc<McpServer>
}

impl OpenCodeAdapter {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub fn get_memory_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("memory_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }

    pub fn get_knowledge_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .filter(|t| t.name.starts_with("knowledge_"))
            .map(|t| serde_json::to_value(t).unwrap())
            .collect()
    }
}

#[async_trait]
impl EcosystemAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value> {
        let rpc_request: JsonRpcRequest = serde_json::from_value(request)?;
        let response = self.server.handle_request(rpc_request).await;
        Ok(serde_json::to_value(response)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_adapter_name() {
        fn assert_ecosystem_adapter<T: EcosystemAdapter>() {}

        assert_ecosystem_adapter::<OpenCodeAdapter>();
    }

    #[test]
    fn test_ecosystem_adapter_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<OpenCodeAdapter>();
    }

    #[test]
    fn test_opencode_adapter_method_signatures() {
        let _: fn(Arc<McpServer>) -> OpenCodeAdapter = OpenCodeAdapter::new;
    }
}
