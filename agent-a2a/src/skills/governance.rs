use super::Skill;
use crate::auth::TenantContext;
use crate::errors::{A2AError, A2AResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct GovernanceSkill {
    engine: Arc<dyn GovernanceEngine>,
}

#[async_trait::async_trait]
pub trait GovernanceEngine: Send + Sync {
    async fn validate(&self, tenant_id: &str, policy: &str) -> Result<ValidationResult, String>;
    async fn check_drift(&self, tenant_id: &str) -> Result<DriftResult, String>;
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct DriftResult {
    pub has_drift: bool,
    pub violations: Vec<String>,
}

impl GovernanceSkill {
    pub fn new(engine: Arc<dyn GovernanceEngine>) -> Self {
        Self { engine }
    }

    pub async fn governance_validate(
        &self,
        tenant: &TenantContext,
        policy: String,
    ) -> A2AResult<Value> {
        let result = self
            .engine
            .validate(&tenant.tenant_id, &policy)
            .await
            .map_err(A2AError::ToolExecutionFailed)?;

        Ok(serde_json::json!({
            "valid": result.valid,
            "errors": result.errors,
            "warnings": result.warnings,
        }))
    }

    pub async fn governance_drift_check(&self, tenant: &TenantContext) -> A2AResult<Value> {
        let result = self
            .engine
            .check_drift(&tenant.tenant_id)
            .await
            .map_err(A2AError::ToolExecutionFailed)?;

        Ok(serde_json::json!({
            "has_drift": result.has_drift,
            "violations": result.violations,
        }))
    }
}

#[async_trait]
impl Skill for GovernanceSkill {
    fn name(&self) -> &'static str {
        "governance"
    }

    async fn invoke(
        &self,
        tool: &str,
        params: Value,
        tenant: &TenantContext,
    ) -> Result<Value, String> {
        match tool {
            "governance_validate" => {
                let policy = params["policy"]
                    .as_str()
                    .ok_or("Missing policy")?
                    .to_string();

                self.governance_validate(tenant, policy)
                    .await
                    .map_err(|e| e.to_string())
            }
            "governance_drift_check" => self
                .governance_drift_check(tenant)
                .await
                .map_err(|e| e.to_string()),
            _ => Err(format!("Unknown tool: {tool}")),
        }
    }
}

pub struct MockGovernanceEngine;

#[async_trait::async_trait]
impl GovernanceEngine for MockGovernanceEngine {
    async fn validate(&self, _tenant_id: &str, _policy: &str) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    async fn check_drift(&self, _tenant_id: &str) -> Result<DriftResult, String> {
        Ok(DriftResult {
            has_drift: false,
            violations: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::TenantContext;
    use std::sync::Arc;

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
    async fn test_governance_validate_uses_tenant_context() {
        let skill = GovernanceSkill::new(Arc::new(MockGovernanceEngine));
        let ctx = tenant("acme-corp", vec!["developer"]);

        let result = skill
            .invoke(
                "governance_validate",
                serde_json::json!({ "policy": "permit everything;" }),
                &ctx,
            )
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["valid"], true);
    }

    #[tokio::test]
    async fn test_governance_drift_check_uses_tenant_context() {
        let skill = GovernanceSkill::new(Arc::new(MockGovernanceEngine));
        let ctx = tenant("acme-corp", vec!["architect"]);

        let result = skill
            .invoke("governance_drift_check", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["has_drift"], false);
    }

    #[tokio::test]
    async fn test_governance_validate_rejects_missing_policy_param() {
        let skill = GovernanceSkill::new(Arc::new(MockGovernanceEngine));
        let ctx = tenant("acme-corp", vec!["admin"]);

        let result = skill
            .invoke("governance_validate", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing policy"));
    }

    #[tokio::test]
    async fn test_governance_invoke_unknown_tool_returns_error() {
        let skill = GovernanceSkill::new(Arc::new(MockGovernanceEngine));
        let ctx = tenant("acme-corp", vec!["viewer"]);

        let result = skill
            .invoke("governance_nonexistent", serde_json::json!({}), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }
}
