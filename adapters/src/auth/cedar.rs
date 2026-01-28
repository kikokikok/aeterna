use async_trait::async_trait;
use cedar_policy::*;
use mk_core::traits::AuthorizationService;
use mk_core::types::{Role, TenantContext, UserId};
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CedarError {
    #[error("Cedar evaluation error: {0}")]
    Evaluation(String),
    #[error("Policy parsing error: {0}")]
    Parse(String),
    #[error("Schema error: {0}")]
    Schema(String)
}

pub struct CedarAuthorizer {
    policies: PolicySet,
    entities: Entities
}

impl CedarAuthorizer {
    pub fn new(policy_text: &str, _schema_text: &str) -> Result<Self, CedarError> {
        let policies =
            PolicySet::from_str(policy_text).map_err(|e| CedarError::Parse(e.to_string()))?;

        Ok(Self {
            policies,
            entities: Entities::empty()
        })
    }
}

#[async_trait]
impl AuthorizationService for CedarAuthorizer {
    type Error = CedarError;

    async fn check_permission(
        &self,
        ctx: &TenantContext,
        action: &str,
        resource: &str
    ) -> Result<bool, Self::Error> {
        if let Some(agent_id) = &ctx.agent_id {
            let agent_principal = EntityUid::from_str(&format!("User::\"{}\"", agent_id))
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;
            let delegate_action = EntityUid::from_str("Action::\"ActAs\"")
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;
            let user_resource = EntityUid::from_str(&format!("User::\"{}\"", ctx.user_id.as_str()))
                .map_err(|e| CedarError::Evaluation(e.to_string()))?;

            let delegation_request = Request::new(
                agent_principal,
                delegate_action,
                user_resource,
                Context::empty(),
                None
            )
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;

            let authorizer = Authorizer::new();
            let delegation_answer =
                authorizer.is_authorized(&delegation_request, &self.policies, &self.entities);

            if delegation_answer.decision() != Decision::Allow {
                return Ok(false);
            }
        }

        let principal_str = format!("User::\"{}\"", ctx.user_id.as_str());

        let principal = EntityUid::from_str(&principal_str)
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;
        let action_uid = EntityUid::from_str(&format!("Action::\"{}\"", action))
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;
        let resource_uid =
            EntityUid::from_str(resource).map_err(|e| CedarError::Evaluation(e.to_string()))?;

        let request = Request::new(principal, action_uid, resource_uid, Context::empty(), None)
            .map_err(|e| CedarError::Evaluation(e.to_string()))?;

        let authorizer = Authorizer::new();
        let answer = authorizer.is_authorized(&request, &self.policies, &self.entities);

        Ok(answer.decision() == Decision::Allow)
    }

    async fn get_user_roles(&self, _ctx: &TenantContext) -> Result<Vec<Role>, Self::Error> {
        Ok(vec![])
    }

    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: Role
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantId, UserId};

    #[tokio::test]
    async fn test_cedar_authorization() -> Result<(), Box<dyn std::error::Error>> {
        let schema = r#"{
            "": {
                "entityTypes": {
                    "User": {},
                    "Unit": {}
                },
                "actions": {
                    "View": {
                        "appliesTo": {
                            "principalTypes": ["User"],
                            "resourceTypes": ["Unit"]
                        }
                    }
                }
            }
        }"#;

        let policies = r#"
            permit(principal == User::"u1", action == Action::"View", resource == Unit::"unit1");
        "#;

        let authorizer = CedarAuthorizer::new(policies, schema)?;

        let ctx = TenantContext::new(
            TenantId::new("t1".into()).unwrap(),
            UserId::new("u1".into()).unwrap()
        );

        let allowed = authorizer
            .check_permission(&ctx, "View", "Unit::\"unit1\"")
            .await?;
        assert!(allowed);

        let denied = authorizer
            .check_permission(&ctx, "View", "Unit::\"unit2\"")
            .await?;
        assert!(!denied);

        Ok(())
    }
}
