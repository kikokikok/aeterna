use crate::tools::Tool;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use mk_core::types::TenantContext;
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
    pub metadata: serde_json::Map<String, Value>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemorySearchParams {
    pub query: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    #[serde(default)]
    pub filters: serde_json::Map<String, Value>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryDeleteParams {
    pub memory_id: String,
    pub layer: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub enum CloseTarget {
    Session,
    Agent
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryCloseParams {
    pub id: String,
    pub target: CloseTarget,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[async_trait]
impl Tool for MemoryAddTool {
    fn name(&self) -> &str {
        "memory_add"
    }

    fn description(&self) -> &str {
        "Store a piece of information in memory for future reference."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "layer": { "type": "string" },
                "metadata": { "type": "object" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["content", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryAddParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

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

        let id = self.memory_manager.add_to_layer(ctx, layer, entry).await?;
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
                "filters": { "type": "object" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemorySearchParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let limit = p.limit.unwrap_or(10);
        let threshold = p.threshold.unwrap_or(0.0);
        let filters: std::collections::HashMap<String, Value> = p.filters.into_iter().collect();

        let results = self
            .memory_manager
            .search_text_with_threshold(ctx, &p.query, limit, threshold, filters)
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
                "layer": { "type": "string" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["id", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryDeleteParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

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
            .delete_from_layer(ctx, layer, &p.memory_id)
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
                },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["id", "target"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryCloseParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        match p.target {
            CloseTarget::Session => self.memory_manager.close_session(ctx, &p.id).await?,
            CloseTarget::Agent => self.memory_manager.close_agent(ctx, &p.id).await?
        }

        Ok(json!({
            "success": true,
            "message": format!("{:?} closed successfully", p.target)
        }))
    }
}
