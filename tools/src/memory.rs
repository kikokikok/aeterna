use crate::tools::Tool;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use validator::Validate;

pub struct MemoryAddTool {
    memory_manager: Arc<MemoryManager>
}

impl MemoryAddTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryAddParams {
    pub content: String,
    pub layer: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, Value>
}

#[async_trait]
impl Tool for MemoryAddTool {
    fn name(&self) -> &str {
        "memory_add"
    }

    fn description(&self) -> &str {
        "Add a new memory to a specific layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "layer": { "type": "string" },
                "metadata": { "type": "object" }
            },
            "required": ["content", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryAddParams = serde_json::from_value(params)?;
        p.validate()?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into())
        };
        let entry = mk_core::types::MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            content: p.content,
            embedding: None,
            layer,
            metadata: p.metadata.into_iter().collect(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp()
        };

        let id = self.memory_manager.add_to_layer(layer, entry).await?;
        Ok(json!({ "success": true, "memoryId": id }))
    }
}

pub struct MemorySearchTool {
    memory_manager: Arc<MemoryManager>
}

impl MemorySearchTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemorySearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    #[serde(default)]
    pub filters: serde_json::Map<String, Value>
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "Search for memories across layers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer" },
                "threshold": { "type": "number" },
                "filters": { "type": "object" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemorySearchParams = serde_json::from_value(params)?;
        p.validate()?;

        let limit = p.limit.unwrap_or(10);
        let threshold = p.threshold.unwrap_or(0.0);
        let filters = p.filters.into_iter().collect();

        let results = self
            .memory_manager
            .search_text_with_threshold(&p.query, limit, threshold, filters)
            .await?;

        Ok(json!({
            "success": true,
            "results": results,
            "totalCount": results.len()
        }))
    }
}

pub struct MemoryDeleteTool {
    memory_manager: Arc<MemoryManager>
}

impl MemoryDeleteTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryDeleteParams {
    pub memory_id: String,
    pub layer: String
}

#[async_trait]
impl Tool for MemoryDeleteTool {
    fn name(&self) -> &str {
        "memory_delete"
    }

    fn description(&self) -> &str {
        "Delete a memory from a specific layer."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "layer": { "type": "string" }
            },
            "required": ["id", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryDeleteParams = serde_json::from_value(params)?;
        p.validate()?;

        let layer = match p.layer.to_lowercase().as_str() {
            "agent" => mk_core::types::MemoryLayer::Agent,
            "user" => mk_core::types::MemoryLayer::User,
            "session" => mk_core::types::MemoryLayer::Session,
            "project" => mk_core::types::MemoryLayer::Project,
            "team" => mk_core::types::MemoryLayer::Team,
            "org" => mk_core::types::MemoryLayer::Org,
            "company" => mk_core::types::MemoryLayer::Company,
            _ => return Err(format!("Unknown layer: {}", p.layer).into())
        };
        self.memory_manager
            .delete_from_layer(layer, &p.memory_id)
            .await?;
        Ok(json!({ "success": true }))
    }
}

pub struct MemoryCloseTool {
    memory_manager: Arc<MemoryManager>
}

impl MemoryCloseTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryCloseParams {
    pub id: String,
    pub target: CloseTarget
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub enum CloseTarget {
    Session,
    Agent
}

#[async_trait]
impl Tool for MemoryCloseTool {
    fn name(&self) -> &str {
        "memory_close"
    }

    fn description(&self) -> &str {
        "Close a session or agent, triggering memory promotion and cleanup."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Session or Agent ID" },
                "target": {
                    "type": "string",
                    "enum": ["session", "agent"],
                    "description": "What to close"
                }
            },
            "required": ["id", "target"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryCloseParams = serde_json::from_value(params)?;
        p.validate()?;

        match p.target {
            CloseTarget::Session => self.memory_manager.close_session(&p.id).await?,
            CloseTarget::Agent => self.memory_manager.close_agent(&p.id).await?
        }

        Ok(json!({
            "success": true,
            "message": format!("{:?} closed successfully", p.target)
        }))
    }
}
