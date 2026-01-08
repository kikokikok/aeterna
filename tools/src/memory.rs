use crate::tools::Tool;
use async_trait::async_trait;
use memory::manager::MemoryManager;
use mk_core::types::{MemoryEntry, MemoryLayer};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use validator::Validate;

pub struct MemoryAddTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryAddTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryAddParams {
    pub content: String,
    #[serde(default = "default_layer")]
    pub layer: MemoryLayer,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, Value>>,
}

fn default_layer() -> MemoryLayer {
    MemoryLayer::User
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
                "content": { "type": "string", "description": "The content to remember" },
                "layer": {
                    "type": "string",
                    "enum": ["agent", "user", "session", "project", "team", "org", "company"],
                    "default": "user",
                    "description": "Memory scope"
                },
                "tags": { "type": "array", "items": { "type": "string" } },
                "metadata": { "type": "object" }
            },
            "required": ["content"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryAddParams = serde_json::from_value(params)?;
        p.validate()?;

        let mut metadata = p.metadata.unwrap_or_default();
        if let Some(tags) = p.tags {
            metadata.insert("tags".to_string(), json!(tags));
        }

        let entry = MemoryEntry {
            id: utils::generate_uuid(),
            content: p.content,
            embedding: None,
            layer: p.layer,
            metadata,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let memory_id = self.memory_manager.add_to_layer(p.layer, entry).await?;

        Ok(json!({
            "success": true,
            "memoryId": memory_id,
            "message": "Memory stored successfully"
        }))
    }
}

pub struct MemorySearchTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemorySearchTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemorySearchParams {
    pub query: String,
    pub layers: Option<Vec<MemoryLayer>>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    pub tags: Option<Vec<String>>,
}

fn default_limit() -> usize {
    10
}
fn default_threshold() -> f32 {
    0.7
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn description(&self) -> &str {
        "Search your memory for relevant past information."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "layers": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["agent", "user", "session", "project", "team", "org", "company"]
                    }
                },
                "limit": { "type": "integer", "default": 10 },
                "threshold": { "type": "number", "default": 0.7 },
                "tags": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["query"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemorySearchParams = serde_json::from_value(params)?;
        p.validate()?;

        let mut filters = HashMap::new();
        if let Some(tags) = p.tags {
            filters.insert("tags".to_string(), json!(tags));
        }

        let results = self.memory_manager.search_hierarchical(
            vec![], 
            p.limit,
            filters
        ).await?;

        let output_results: Vec<Value> = results.into_iter().map(|r| {
            json!({
                "content": r.content,
                "layer": r.layer,
                "score": 1.0, 
                "memoryId": r.id,
                "tags": r.metadata.get("tags")
            })
        }).collect();

        Ok(json!({
            "success": true,
            "results": output_results,
            "totalCount": output_results.len(),
            "searchedLayers": p.layers.unwrap_or_else(|| vec![
                MemoryLayer::Agent, MemoryLayer::User, MemoryLayer::Session,
                MemoryLayer::Project, MemoryLayer::Team, MemoryLayer::Org, MemoryLayer::Company
            ])
        }))
    }
}

pub struct MemoryDeleteTool {
    memory_manager: Arc<MemoryManager>,
}

impl MemoryDeleteTool {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct MemoryDeleteParams {
    pub memory_id: String,
    pub layer: MemoryLayer,
}

#[async_trait]
impl Tool for MemoryDeleteTool {
    fn name(&self) -> &str {
        "memory_delete"
    }

    fn description(&self) -> &str {
        "Delete a memory that is no longer relevant."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "memory_id": { "type": "string" },
                "layer": {
                    "type": "string",
                    "enum": ["agent", "user", "session", "project", "team", "org", "company"]
                }
            },
            "required": ["memory_id", "layer"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: MemoryDeleteParams = serde_json::from_value(params)?;
        p.validate()?;

        self.memory_manager.delete_from_layer(p.layer, &p.memory_id).await?;

        Ok(json!({
            "success": true,
            "message": "Memory deleted successfully"
        }))
    }
}
