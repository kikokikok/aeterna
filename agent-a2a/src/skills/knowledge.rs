use super::Skill;
use crate::auth::TenantContext;
use crate::errors::A2AResult;
use async_trait::async_trait;
use serde_json::Value;

pub struct KnowledgeSkill;

impl KnowledgeSkill {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub async fn knowledge_query(
        &self,
        _tenant: &TenantContext,
        query: String,
        doc_type: Option<String>,
        limit: Option<usize>
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({
            "results": [],
            "query": query,
            "doc_type": doc_type,
            "limit": limit.unwrap_or(10)
        }))
    }

    pub async fn knowledge_show(
        &self,
        _tenant: &TenantContext,
        doc_id: String
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({
            "id": doc_id,
            "title": "Placeholder",
            "content": "",
            "doc_type": "unknown",
            "path": "",
            "metadata": {},
        }))
    }

    pub async fn knowledge_check(
        &self,
        _tenant: &TenantContext,
        _content: String,
        doc_type: String
    ) -> A2AResult<Value> {
        Ok(serde_json::json!({
            "valid": true,
            "doc_type": doc_type,
            "errors": [],
            "warnings": [],
        }))
    }
}

impl Default for KnowledgeSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for KnowledgeSkill {
    fn name(&self) -> &str {
        "knowledge"
    }

    async fn invoke(&self, tool: &str, params: Value) -> Result<Value, String> {
        let tenant = TenantContext {
            tenant_id: "default".to_string(),
            user_id: None,
            agent_id: None
        };

        match tool {
            "knowledge_query" => {
                let query = params["query"].as_str().ok_or("Missing query")?.to_string();
                let doc_type = params["doc_type"].as_str().map(|s| s.to_string());
                let limit = params["limit"].as_u64().map(|n| n as usize);

                self.knowledge_query(&tenant, query, doc_type, limit)
                    .await
                    .map_err(|e| e.to_string())
            }
            "knowledge_show" => {
                let id = params["id"].as_str().ok_or("Missing id")?.to_string();

                self.knowledge_show(&tenant, id)
                    .await
                    .map_err(|e| e.to_string())
            }
            "knowledge_check" => {
                let content = params["content"]
                    .as_str()
                    .ok_or("Missing content")?
                    .to_string();
                let doc_type = params["doc_type"]
                    .as_str()
                    .ok_or("Missing doc_type")?
                    .to_string();

                self.knowledge_check(&tenant, content, doc_type)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown tool: {}", tool))
        }
    }
}
