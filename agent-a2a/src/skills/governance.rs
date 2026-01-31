use super::Skill;
use crate::auth::TenantContext;
use crate::errors::{A2AError, A2AResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct GovernanceSkill {
    engine: Arc<dyn GovernanceEngine>
}

#[async_trait::async_trait]
pub trait GovernanceEngine: Send + Sync {
    async fn validate(&self, tenant_id: &str, policy: &str) -> Result<ValidationResult, String>;
    async fn check_drift(&self, tenant_id: &str) -> Result<DriftResult, String>;
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>
}

pub struct DriftResult {
    pub has_drift: bool,
    pub violations: Vec<String>
}

impl GovernanceSkill {
    pub fn new(engine: Arc<dyn GovernanceEngine>) -> Self {
        Self { engine }
    }

    pub async fn governance_validate(
        &self,
        tenant: &TenantContext,
        policy: String
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
    fn name(&self) -> &str {
        "governance"
    }

    async fn invoke(&self, tool: &str, params: Value) -> Result<Value, String> {
        let tenant = TenantContext {
            tenant_id: "default".to_string(),
            user_id: None,
            agent_id: None
        };

        match tool {
            "governance_validate" => {
                let policy = params["policy"]
                    .as_str()
                    .ok_or("Missing policy")?
                    .to_string();

                self.governance_validate(&tenant, policy)
                    .await
                    .map_err(|e| e.to_string())
            }
            "governance_drift_check" => self
                .governance_drift_check(&tenant)
                .await
                .map_err(|e| e.to_string()),
            _ => Err(format!("Unknown tool: {}", tool))
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
            warnings: vec![]
        })
    }

    async fn check_drift(&self, _tenant_id: &str) -> Result<DriftResult, String> {
        Ok(DriftResult {
            has_drift: false,
            violations: vec![]
        })
    }
}
