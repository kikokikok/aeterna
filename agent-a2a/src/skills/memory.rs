use super::Skill;
use crate::auth::TenantContext;
use crate::errors::A2AResult;
use async_trait::async_trait;
use serde_json::Value;

pub struct MemorySkill;

impl MemorySkill {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub async fn memory_add(
        &self,
        _tenant: &TenantContext,
        content: String,
        layer: Option<String>,
        tags: Option<Vec<String>>
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "content": content,
            "layer": layer.unwrap_or_else(|| "session".to_string()),
            "tags": tags.unwrap_or_default(),
            "created_at": chrono::Utc::now().to_rfc3339(),
        }))
    }

    pub async fn memory_search(
        &self,
        _tenant: &TenantContext,
        query: String,
        limit: Option<usize>,
        _layer: Option<String>
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({
            "results": [],
            "query": query,
            "limit": limit.unwrap_or(10)
        }))
    }

    pub async fn memory_delete(
        &self,
        _tenant: &TenantContext,
        memory_id: String
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({ "deleted": true, "id": memory_id }))
    }
}

impl Default for MemorySkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for MemorySkill {
    fn name(&self) -> &str {
        "memory"
    }

    async fn invoke(&self, tool: &str, params: Value) -> Result<Value, String> {
        let tenant = TenantContext {
            tenant_id: "default".to_string(),
            user_id: None,
            agent_id: None
        };

        match tool {
            "memory_add" => {
                let content = params["content"]
                    .as_str()
                    .ok_or("Missing content")?
                    .to_string();
                let layer = params["layer"].as_str().map(|s| s.to_string());
                let tags = params["tags"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });

                self.memory_add(&tenant, content, layer, tags)
                    .await
                    .map_err(|e| e.to_string())
            }
            "memory_search" => {
                let query = params["query"].as_str().ok_or("Missing query")?.to_string();
                let limit = params["limit"].as_u64().map(|n| n as usize);
                let layer = params["layer"].as_str().map(|s| s.to_string());

                self.memory_search(&tenant, query, limit, layer)
                    .await
                    .map_err(|e| e.to_string())
            }
            "memory_delete" => {
                let id = params["id"].as_str().ok_or("Missing id")?.to_string();

                self.memory_delete(&tenant, id)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown tool: {}", tool))
        }
    }
}
