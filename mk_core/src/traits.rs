//! Core traits for memory-knowledge system

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Storage backend trait for extensible storage implementations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    type Error;

    async fn store(
        &self,
        ctx: crate::types::TenantContext,
        key: &str,
        value: &[u8],
    ) -> Result<(), Self::Error>;

    async fn retrieve(
        &self,
        ctx: crate::types::TenantContext,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Self::Error>;

    async fn delete(&self, ctx: crate::types::TenantContext, key: &str) -> Result<(), Self::Error>;

    async fn exists(
        &self,
        ctx: crate::types::TenantContext,
        key: &str,
    ) -> Result<bool, Self::Error>;

    async fn get_ancestors(
        &self,
        ctx: crate::types::TenantContext,
        unit_id: &str,
    ) -> Result<Vec<crate::types::OrganizationalUnit>, Self::Error>;

    async fn get_descendants(
        &self,
        ctx: crate::types::TenantContext,
        unit_id: &str,
    ) -> Result<Vec<crate::types::OrganizationalUnit>, Self::Error>;

    async fn get_unit_policies(
        &self,
        ctx: crate::types::TenantContext,
        unit_id: &str,
    ) -> Result<Vec<crate::types::Policy>, Self::Error>;

    async fn create_unit(&self, unit: &crate::types::OrganizationalUnit)
    -> Result<(), Self::Error>;

    async fn add_unit_policy(
        &self,
        ctx: &crate::types::TenantContext,
        unit_id: &str,
        policy: &crate::types::Policy,
    ) -> Result<(), Self::Error>;

    async fn assign_role(
        &self,
        user_id: &crate::types::UserId,
        tenant_id: &crate::types::TenantId,
        unit_id: &str,
        role: crate::types::Role,
    ) -> Result<(), Self::Error>;

    async fn remove_role(
        &self,
        user_id: &crate::types::UserId,
        tenant_id: &crate::types::TenantId,
        unit_id: &str,
        role: crate::types::Role,
    ) -> Result<(), Self::Error>;

    async fn store_drift_result(
        &self,
        result: crate::types::DriftResult,
    ) -> Result<(), Self::Error>;

    async fn get_latest_drift_result(
        &self,
        ctx: crate::types::TenantContext,
        project_id: &str,
    ) -> Result<Option<crate::types::DriftResult>, Self::Error>;

    async fn list_all_units(&self) -> Result<Vec<crate::types::OrganizationalUnit>, Self::Error>;
    async fn record_job_status(
        &self,
        job_name: &str,
        tenant_id: &str,
        status: &str,
        message: Option<&str>,
        started_at: i64,
        finished_at: Option<i64>,
    ) -> Result<(), Self::Error>;

    async fn get_governance_events(
        &self,
        ctx: crate::types::TenantContext,
        since_timestamp: i64,
        limit: usize,
    ) -> Result<Vec<crate::types::GovernanceEvent>, Self::Error>;

    async fn create_suppression(
        &self,
        suppression: crate::types::DriftSuppression,
    ) -> Result<(), Self::Error>;

    async fn list_suppressions(
        &self,
        ctx: crate::types::TenantContext,
        project_id: &str,
    ) -> Result<Vec<crate::types::DriftSuppression>, Self::Error>;

    async fn delete_suppression(
        &self,
        ctx: crate::types::TenantContext,
        suppression_id: &str,
    ) -> Result<(), Self::Error>;

    async fn get_drift_config(
        &self,
        ctx: crate::types::TenantContext,
        project_id: &str,
    ) -> Result<Option<crate::types::DriftConfig>, Self::Error>;

    async fn save_drift_config(&self, config: crate::types::DriftConfig)
    -> Result<(), Self::Error>;

    async fn persist_event(&self, event: crate::types::PersistentEvent) -> Result<(), Self::Error>;

    async fn get_pending_events(
        &self,
        ctx: crate::types::TenantContext,
        limit: usize,
    ) -> Result<Vec<crate::types::PersistentEvent>, Self::Error>;

    async fn update_event_status(
        &self,
        event_id: &str,
        status: crate::types::EventStatus,
        error: Option<String>,
    ) -> Result<(), Self::Error>;

    async fn get_dead_letter_events(
        &self,
        ctx: crate::types::TenantContext,
        limit: usize,
    ) -> Result<Vec<crate::types::PersistentEvent>, Self::Error>;

    async fn check_idempotency(
        &self,
        consumer_group: &str,
        idempotency_key: &str,
    ) -> Result<bool, Self::Error>;

    async fn record_consumer_state(
        &self,
        state: crate::types::ConsumerState,
    ) -> Result<(), Self::Error>;

    async fn get_event_metrics(
        &self,
        ctx: crate::types::TenantContext,
        period_start: i64,
        period_end: i64,
    ) -> Result<Vec<crate::types::EventDeliveryMetrics>, Self::Error>;

    async fn record_event_metrics(
        &self,
        metrics: crate::types::EventDeliveryMetrics,
    ) -> Result<(), Self::Error>;
}

/// Health check capability for service monitoring
pub trait HealthCheck: Send + Sync {
    fn health_check(&self) -> Result<HealthStatus, Box<dyn std::error::Error + Send + Sync>>;
}

/// Health status for service monitoring
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[async_trait]
pub trait MemoryProviderAdapter: Send + Sync {
    type Error;

    async fn add(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::MemoryEntry,
    ) -> Result<String, Self::Error>;

    async fn search(
        &self,
        ctx: crate::types::TenantContext,
        query_vector: Vec<f32>,
        limit: usize,
        filters: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Vec<crate::types::MemoryEntry>, Self::Error>;

    async fn get(
        &self,
        ctx: crate::types::TenantContext,
        id: &str,
    ) -> Result<Option<crate::types::MemoryEntry>, Self::Error>;

    async fn update(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::MemoryEntry,
    ) -> Result<(), Self::Error>;

    async fn delete(&self, ctx: crate::types::TenantContext, id: &str) -> Result<(), Self::Error>;

    async fn list(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::MemoryLayer,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<(Vec<crate::types::MemoryEntry>, Option<String>), Self::Error>;
}

#[async_trait]
pub trait KnowledgeRepository: Send + Sync {
    type Error;

    async fn get(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        path: &str,
    ) -> Result<Option<crate::types::KnowledgeEntry>, Self::Error>;

    async fn store(
        &self,
        ctx: crate::types::TenantContext,
        entry: crate::types::KnowledgeEntry,
        message: &str,
    ) -> Result<String, Self::Error>;

    async fn list(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        prefix: &str,
    ) -> Result<Vec<crate::types::KnowledgeEntry>, Self::Error>;

    async fn delete(
        &self,
        ctx: crate::types::TenantContext,
        layer: crate::types::KnowledgeLayer,
        path: &str,
        message: &str,
    ) -> Result<String, Self::Error>;

    async fn get_head_commit(
        &self,
        ctx: crate::types::TenantContext,
    ) -> Result<Option<String>, Self::Error>;

    async fn get_affected_items(
        &self,
        ctx: crate::types::TenantContext,
        since_commit: &str,
    ) -> Result<Vec<(crate::types::KnowledgeLayer, String)>, Self::Error>;

    async fn search(
        &self,
        ctx: crate::types::TenantContext,
        query: &str,
        layers: Vec<crate::types::KnowledgeLayer>,
        limit: usize,
    ) -> Result<Vec<crate::types::KnowledgeEntry>, Self::Error>;

    fn root_path(&self) -> Option<std::path::PathBuf>;
}

#[async_trait]
pub trait AuthorizationService: Send + Sync {
    type Error;

    async fn check_permission(
        &self,
        ctx: &crate::types::TenantContext,
        action: &str,
        resource: &str,
    ) -> Result<bool, Self::Error>;

    async fn get_user_roles(
        &self,
        ctx: &crate::types::TenantContext,
    ) -> Result<Vec<crate::types::Role>, Self::Error>;

    async fn assign_role(
        &self,
        ctx: &crate::types::TenantContext,
        user_id: &crate::types::UserId,
        role: crate::types::Role,
    ) -> Result<(), Self::Error>;

    async fn remove_role(
        &self,
        ctx: &crate::types::TenantContext,
        user_id: &crate::types::UserId,
        role: crate::types::Role,
    ) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait ContextHooks: Send + Sync {
    async fn on_session_start(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
    ) -> anyhow::Result<()>;
    async fn on_session_end(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
    ) -> anyhow::Result<()>;
    async fn on_message(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
        message: &str,
    ) -> anyhow::Result<()>;
    async fn on_tool_use(
        &self,
        ctx: crate::types::TenantContext,
        session_id: &str,
        tool_name: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait EmbeddingService: Send + Sync {
    type Error;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error>;

    fn dimension(&self) -> usize;

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Self::Error> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

#[async_trait]
pub trait EventPublisher: Send + Sync {
    type Error;

    async fn publish(&self, event: crate::types::GovernanceEvent) -> Result<(), Self::Error>;

    async fn subscribe(
        &self,
        channels: &[&str],
    ) -> Result<tokio::sync::mpsc::Receiver<crate::types::GovernanceEvent>, Self::Error>;
}

/// LLM service trait for text generation and reasoning
#[async_trait]
pub trait LlmService: Send + Sync {
    type Error;

    /// Generates text based on a prompt
    async fn generate(&self, prompt: &str) -> Result<String, Self::Error>;

    /// Analyzes content against a set of policies
    async fn analyze_drift(
        &self,
        content: &str,
        policies: &[crate::types::Policy],
    ) -> Result<crate::types::ValidationResult, Self::Error>;
}
