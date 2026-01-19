use async_trait::async_trait;
use mk_core::traits::AuthorizationService;
use mk_core::types::{Role, TenantContext, UserId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PermitError {
    #[error("Permit.io API error: {0}")]
    Api(String),
    #[error("Authorization denied")]
    Denied,
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct PermitCheckRequest {
    user: String,
    action: String,
    resource: String,
    tenant: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermitCheckResponse {
    allow: bool,
}

pub struct PermitAuthorizationService {
    pdp_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl PermitAuthorizationService {
    pub fn new(api_key: &str, pdp_url: &str) -> Self {
        Self {
            pdp_url: pdp_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AuthorizationService for PermitAuthorizationService {
    type Error = PermitError;

    async fn check_permission(
        &self,
        ctx: &TenantContext,
        action: &str,
        resource: &str,
    ) -> Result<bool, Self::Error> {
        let user_id = if let Some(agent_id) = &ctx.agent_id {
            agent_id.as_str()
        } else {
            ctx.user_id.as_str()
        };
        let tenant_id = ctx.tenant_id.as_str();

        let url = format!("{}/allowed", self.pdp_url);
        let request = PermitCheckRequest {
            user: user_id.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            tenant: Some(tenant_id.to_string()),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(PermitError::Api(format!("Status: {}", response.status())));
        }

        let result: PermitCheckResponse = response.json().await?;
        Ok(result.allow)
    }

    async fn get_user_roles(&self, ctx: &TenantContext) -> Result<Vec<Role>, Self::Error> {
        let user_id = ctx.user_id.as_str();
        let tenant_id = ctx.tenant_id.as_str();

        let url = format!(
            "{}/users/{}/roles?tenant={}",
            self.pdp_url, user_id, tenant_id
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(PermitError::Api(format!(
                "Failed to fetch roles: {}",
                response.status()
            )));
        }

        let roles_str: Vec<String> = response.json().await?;
        let mut roles = Vec::new();
        for r in roles_str {
            if let Ok(role) = r.parse::<Role>() {
                roles.push(role);
            }
        }
        Ok(roles)
    }

    async fn assign_role(
        &self,
        ctx: &TenantContext,
        user_id: &UserId,
        role: Role,
    ) -> Result<(), Self::Error> {
        let tenant_id = ctx.tenant_id.as_str();

        let url = format!("{}/roles/assign", self.pdp_url);
        let request = serde_json::json!({
            "user": user_id.as_str(),
            "role": role.to_string().to_lowercase(),
            "tenant": tenant_id
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(PermitError::Api(format!(
                "Role assignment failed: {}",
                response.status()
            )));
        }

        Ok(())
    }

    async fn remove_role(
        &self,
        ctx: &TenantContext,
        user_id: &UserId,
        role: Role,
    ) -> Result<(), Self::Error> {
        let tenant_id = ctx.tenant_id.as_str();

        let url = format!("{}/roles/unassign", self.pdp_url);
        let request = serde_json::json!({
            "user": user_id.as_str(),
            "role": role.to_string().to_lowercase(),
            "tenant": tenant_id
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(PermitError::Api(format!(
                "Role removal failed: {}",
                response.status()
            )));
        }

        Ok(())
    }
}
