use crate::ecosystem::EcosystemAdapter;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use tools::server::McpServer;

pub struct LangChainAdapter {
    server: Arc<McpServer>,
}

impl LangChainAdapter {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub fn to_langchain_tools(&self) -> Vec<Value> {
        self.server
            .list_tools()
            .into_iter()
            .map(|tool| {
                let mut schema = tool.input_schema.clone();
                if let Some(obj) = schema.as_object_mut() {
                    obj.insert(
                        "$schema".to_string(),
                        json!("http://json-schema.org/draft-07/schema#"),
                    );
                    obj.insert("additionalProperties".to_string(), json!(false));
                }

                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": schema,
                })
            })
            .collect()
    }
}

#[async_trait]
impl EcosystemAdapter for LangChainAdapter {
    fn name(&self) -> &str {
        "langchain"
    }

    async fn handle_mcp_request(&self, request: Value) -> anyhow::Result<Value> {
        let name = request["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
        let arguments = request["arguments"].clone();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let response = self
            .server
            .handle_request(serde_json::from_value(mcp_request)?)
            .await;
        Ok(serde_json::to_value(response)?)
    }
}
