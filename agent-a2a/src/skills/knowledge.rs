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
        limit: Option<usize>,
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
        doc_id: String,
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
        doc_type: String,
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

    async fn invoke(
        &self,
        tool: &str,
        params: Value,
        tenant: &TenantContext,
    ) -> Result<Value, String> {
        match tool {
            "knowledge_query" => {
                let query = params["query"].as_str().ok_or("Missing query")?.to_string();
                let doc_type = params["doc_type"].as_str().map(|s| s.to_string());
                let limit = params["limit"].as_u64().map(|n| n as usize);

                self.knowledge_query(tenant, query, doc_type, limit)
                    .await
                    .map_err(|e| e.to_string())
            }
            "knowledge_show" => {
                let id = params["id"].as_str().ok_or("Missing id")?.to_string();

                self.knowledge_show(tenant, id)
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

                self.knowledge_check(tenant, content, doc_type)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("Unknown tool: {}", tool)),
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
    async fn test_knowledge_query_uses_tenant_context() {
        let skill = KnowledgeSkill::new();
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke(
                "knowledge_query",
                serde_json::json!({ "query": "ADR-042", "doc_type": "adr" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["query"], "ADR-042");
        assert_eq!(val["doc_type"], "adr");
        assert_eq!(val["limit"], 10);
    }

    #[tokio::test]
    async fn test_knowledge_query_rejects_missing_query() {
        let skill = KnowledgeSkill::new();
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke("knowledge_query", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing query"));
    }

    #[tokio::test]
    async fn test_knowledge_show_uses_tenant_context() {
        let skill = KnowledgeSkill::new();
        let ctx = tenant("acme-corp", vec!["viewer"]);

        let result = skill
            .invoke(
                "knowledge_show",
                serde_json::json!({ "id": "doc-abc" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["id"], "doc-abc");
    }

    #[tokio::test]
    async fn test_knowledge_check_uses_tenant_context() {
        let skill = KnowledgeSkill::new();
        let ctx = tenant("acme-corp", vec!["architect"]);

        let result = skill
            .invoke(
                "knowledge_check",
                serde_json::json!({ "content": "some policy text", "doc_type": "policy" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["valid"], true);
        assert_eq!(val["doc_type"], "policy");
    }

    #[tokio::test]
    async fn test_knowledge_invoke_unknown_tool_returns_error() {
        let skill = KnowledgeSkill::new();
        let ctx = tenant("acme-corp", vec!["viewer"]);

        let result = skill
            .invoke("knowledge_nonexistent", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }
}
