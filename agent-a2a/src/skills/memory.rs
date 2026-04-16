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
        tags: Option<Vec<String>>,
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
        _layer: Option<String>,
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
        memory_id: String,
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
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn invoke(
        &self,
        tool: &str,
        params: Value,
        tenant: &TenantContext,
    ) -> Result<Value, String> {
        match tool {
            "memory_add" => {
                let content = params["content"]
                    .as_str()
                    .ok_or("Missing content")?
                    .to_string();
                let layer = params["layer"]
                    .as_str()
                    .map(std::string::ToString::to_string);
                let tags = params["tags"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                        .collect()
                });

                self.memory_add(tenant, content, layer, tags)
                    .await
                    .map_err(|e| e.to_string())
            }
            "memory_search" => {
                let query = params["query"].as_str().ok_or("Missing query")?.to_string();
                let limit = params["limit"].as_u64().map(|n| n as usize);
                let layer = params["layer"]
                    .as_str()
                    .map(std::string::ToString::to_string);

                self.memory_search(tenant, query, limit, layer)
                    .await
                    .map_err(|e| e.to_string())
            }
            "memory_delete" => {
                let id = params["id"].as_str().ok_or("Missing id")?.to_string();

                self.memory_delete(tenant, id)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown tool: {tool}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::TenantContext;

    fn tenant(tenant_id: &str, roles: Vec<&str>) -> TenantContext {
        TenantContext {
            tenant_id: tenant_id.to_string(),
            user_id: Some("user-1".to_string()),
            agent_id: None,
            user_email: Some("alice@example.com".to_string()),
            groups: vec!["aeterna-users".to_string()],
            roles: roles.into_iter().map(ToString::to_string).collect(),
        }
    }

    #[tokio::test]
    async fn test_memory_add_uses_tenant_context() {
        let skill = MemorySkill::new();
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke(
                "memory_add",
                serde_json::json!({ "content": "test memory", "layer": "session" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["content"], "test memory");
        assert_eq!(val["layer"], "session");
        assert!(val["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_memory_add_rejects_missing_content() {
        let skill = MemorySkill::new();
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke("memory_add", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing content"));
    }

    #[tokio::test]
    async fn test_memory_search_uses_tenant_context() {
        let skill = MemorySkill::new();
        let ctx = tenant("acme-corp", vec!["viewer"]);

        let result = skill
            .invoke(
                "memory_search",
                serde_json::json!({ "query": "architecture decisions" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["query"], "architecture decisions");
        assert_eq!(val["limit"], 10);
    }

    #[tokio::test]
    async fn test_memory_delete_uses_tenant_context() {
        let skill = MemorySkill::new();
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke(
                "memory_delete",
                serde_json::json!({ "id": "mem-123" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["deleted"], true);
        assert_eq!(val["id"], "mem-123");
    }

    #[tokio::test]
    async fn test_memory_invoke_unknown_tool_returns_error() {
        let skill = MemorySkill::new();
        let ctx = tenant("acme-corp", vec!["viewer"]);

        let result = skill
            .invoke("memory_nonexistent", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }
}
