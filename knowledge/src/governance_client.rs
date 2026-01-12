use async_trait::async_trait;
use mk_core::types::{
    DriftResult, GovernanceEvent, KnowledgeEntry, KnowledgeLayer, TenantContext, ValidationResult
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum GovernanceClientError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    Internal(String)
}

pub type Result<T> = std::result::Result<T, GovernanceClientError>;

#[async_trait]
pub trait GovernanceClient: Send + Sync {
    async fn validate(
        &self,
        ctx: &TenantContext,
        layer: KnowledgeLayer,
        context: &std::collections::HashMap<String, serde_json::Value>
    ) -> Result<ValidationResult>;

    async fn get_drift_status(
        &self,
        ctx: &TenantContext,
        project_id: &str
    ) -> Result<Option<DriftResult>>;

    async fn list_proposals(
        &self,
        ctx: &TenantContext,
        layer: Option<KnowledgeLayer>
    ) -> Result<Vec<KnowledgeEntry>>;

    async fn replay_events(
        &self,
        ctx: &TenantContext,
        since_timestamp: i64,
        limit: usize
    ) -> Result<Vec<GovernanceEvent>>;
}

pub struct RemoteGovernanceClient {
    client: reqwest::Client,
    base_url: String
}

impl RemoteGovernanceClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url
        }
    }
}

#[async_trait]
impl GovernanceClient for RemoteGovernanceClient {
    async fn validate(
        &self,
        ctx: &TenantContext,
        layer: KnowledgeLayer,
        context: &std::collections::HashMap<String, serde_json::Value>
    ) -> Result<ValidationResult> {
        let url = format!("{}/api/v1/governance/validate", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-Tenant-Id", ctx.tenant_id.as_str())
            .header("X-User-Id", ctx.user_id.as_str())
            .json(&serde_json::json!({
                "layer": layer,
                "context": context
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(GovernanceClientError::Api(response.text().await?))
        }
    }

    async fn get_drift_status(
        &self,
        ctx: &TenantContext,
        project_id: &str
    ) -> Result<Option<DriftResult>> {
        let url = format!("{}/api/v1/governance/drift/{}", self.base_url, project_id);
        let response = self
            .client
            .get(&url)
            .header("X-Tenant-Id", ctx.tenant_id.as_str())
            .header("X-User-Id", ctx.user_id.as_str())
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(GovernanceClientError::Api(response.text().await?))
        }
    }

    async fn list_proposals(
        &self,
        ctx: &TenantContext,
        layer: Option<KnowledgeLayer>
    ) -> Result<Vec<KnowledgeEntry>> {
        let mut url = format!("{}/api/v1/governance/proposals", self.base_url);
        if let Some(l) = layer {
            url.push_str(&format!("?layer={:?}", l));
        }

        let response = self
            .client
            .get(&url)
            .header("X-Tenant-Id", ctx.tenant_id.as_str())
            .header("X-User-Id", ctx.user_id.as_str())
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(GovernanceClientError::Api(response.text().await?))
        }
    }

    async fn replay_events(
        &self,
        ctx: &TenantContext,
        since_timestamp: i64,
        limit: usize
    ) -> Result<Vec<GovernanceEvent>> {
        let url = format!(
            "{}/api/v1/governance/events/replay?since_timestamp={}&limit={}",
            self.base_url, since_timestamp, limit
        );

        let response = self
            .client
            .get(&url)
            .header("X-Tenant-Id", ctx.tenant_id.as_str())
            .header("X-User-Id", ctx.user_id.as_str())
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(GovernanceClientError::Api(response.text().await?))
        }
    }
}
