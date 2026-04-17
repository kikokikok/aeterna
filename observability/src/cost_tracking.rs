//! # Cost Tracking Module
//!
//! Per-tenant cost tracking for embeddings, storage, and compute resources.
//! Provides detailed cost breakdown and budget management.
use chrono::{DateTime, Utc};
use mk_core::types::TenantContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    pub tenant_id: String,
    pub resource_type: ResourceType,
    pub operation: String,
    pub cost: f64,
    pub currency: String,
    pub units: u64,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResourceType {
    EmbeddingGeneration,
    VectorStorage,
    LlmCompletion,
    DatabaseQuery,
    CacheStorage,
    DataTransfer,
}

impl ResourceType {
    pub fn as_str(&self) -> &str {
        match self {
            ResourceType::EmbeddingGeneration => "embedding_generation",
            ResourceType::VectorStorage => "vector_storage",
            ResourceType::LlmCompletion => "llm_completion",
            ResourceType::DatabaseQuery => "database_query",
            ResourceType::CacheStorage => "cache_storage",
            ResourceType::DataTransfer => "data_transfer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCostSummary {
    pub tenant_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_cost: f64,
    pub currency: String,
    pub by_resource_type: HashMap<ResourceType, f64>,
    pub by_operation: HashMap<String, f64>,
    pub budget_limit: Option<f64>,
    pub budget_used_percent: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CostConfig {
    /// Cost per 1K tokens for embedding generation
    pub embedding_cost_per_1k_tokens: f64,
    /// Cost per 1K tokens for LLM completion
    pub llm_cost_per_1k_tokens: f64,
    /// Cost per GB per month for vector storage
    pub storage_cost_per_gb_month: f64,
    /// Cost per 1M database queries
    pub query_cost_per_1m: f64,
    /// Currency for all costs
    pub currency: String,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            // OpenAI text-embedding-ada-002 pricing
            embedding_cost_per_1k_tokens: 0.0001,
            // OpenAI GPT-4 pricing (average)
            llm_cost_per_1k_tokens: 0.03,
            // Estimated vector storage cost
            storage_cost_per_gb_month: 0.25,
            // Estimated database query cost
            query_cost_per_1m: 0.10,
            currency: "USD".to_string(),
        }
    }
}

pub struct CostTracker {
    config: CostConfig,
    entries: Arc<RwLock<Vec<CostEntry>>>,
    budgets: Arc<RwLock<HashMap<String, f64>>>,
}

impl CostTracker {
    pub fn new(config: CostConfig) -> Self {
        Self {
            config,
            entries: Arc::new(RwLock::new(Vec::new())),
            budgets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record an embedding generation cost
    pub fn record_embedding_generation(&self, ctx: &TenantContext, token_count: u64, model: &str) {
        self.record_embedding_generation_scoped(ctx, token_count, model, None, None);
    }

    pub fn record_embedding_generation_scoped(
        &self,
        ctx: &TenantContext,
        token_count: u64,
        model: &str,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) {
        let cost = (token_count as f64 / 1000.0) * self.config.embedding_cost_per_1k_tokens;
        self.record_cost_scoped(
            ctx,
            ResourceType::EmbeddingGeneration,
            "generate",
            cost,
            token_count,
            vec![("model".to_string(), model.to_string())],
            team_id,
            project_id,
        );
    }

    /// Record an LLM completion cost
    pub fn record_llm_completion(&self, ctx: &TenantContext, token_count: u64, model: &str) {
        self.record_llm_completion_scoped(ctx, token_count, model, None, None);
    }

    pub fn record_llm_completion_scoped(
        &self,
        ctx: &TenantContext,
        token_count: u64,
        model: &str,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) {
        let cost = (token_count as f64 / 1000.0) * self.config.llm_cost_per_1k_tokens;
        self.record_cost_scoped(
            ctx,
            ResourceType::LlmCompletion,
            "complete",
            cost,
            token_count,
            vec![("model".to_string(), model.to_string())],
            team_id,
            project_id,
        );
    }

    /// Record vector storage cost
    pub fn record_storage(&self, ctx: &TenantContext, bytes: u64) {
        self.record_storage_scoped(ctx, bytes, None, None);
    }

    pub fn record_storage_scoped(
        &self,
        ctx: &TenantContext,
        bytes: u64,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let monthly_cost = gb * self.config.storage_cost_per_gb_month;
        let daily_cost = monthly_cost / 30.0;
        self.record_cost_scoped(
            ctx,
            ResourceType::VectorStorage,
            "store",
            daily_cost,
            bytes,
            vec![],
            team_id,
            project_id,
        );
    }

    /// Record a generic cost
    pub fn record_cost(
        &self,
        ctx: &TenantContext,
        resource_type: ResourceType,
        operation: &str,
        cost: f64,
        units: u64,
        metadata: Vec<(String, String)>,
    ) {
        self.record_cost_scoped(
            ctx,
            resource_type,
            operation,
            cost,
            units,
            metadata,
            None,
            None,
        );
    }

    pub fn record_cost_scoped(
        &self,
        ctx: &TenantContext,
        resource_type: ResourceType,
        operation: &str,
        cost: f64,
        units: u64,
        metadata: Vec<(String, String)>,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) {
        let tenant_id_str = ctx.tenant_id.as_str().to_string();
        let resource_str = resource_type.as_str().to_string();
        let mut metadata_map: HashMap<String, String> = metadata.into_iter().collect();
        if let Some(team_id) = team_id {
            metadata_map.insert("team_id".to_string(), team_id.to_string());
        }
        if let Some(project_id) = project_id {
            metadata_map.insert("project_id".to_string(), project_id.to_string());
        }

        let team_label = metadata_map
            .get("team_id")
            .or_else(|| metadata_map.get("team"))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let project_label = metadata_map
            .get("project_id")
            .or_else(|| metadata_map.get("project"))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let entry = CostEntry {
            tenant_id: tenant_id_str.clone(),
            resource_type,
            operation: operation.to_string(),
            cost,
            currency: self.config.currency.clone(),
            units,
            timestamp: Utc::now(),
            metadata: metadata_map,
        };

        if let Ok(mut entries) = self.entries.write() {
            entries.push(entry);
        }

        metrics::counter!(
            "aeterna_cost_operations_total",
            "tenant_id" => tenant_id_str.clone(),
            "team_id" => team_label.clone(),
            "project_id" => project_label.clone(),
            "resource_type" => resource_str.clone(),
            "operation" => operation.to_string()
        )
        .increment(1);
        metrics::counter!(
            "aeterna_cost_units_total",
            "tenant_id" => tenant_id_str.clone(),
            "team_id" => team_label.clone(),
            "project_id" => project_label.clone(),
            "resource_type" => resource_str.clone(),
            "operation" => operation.to_string()
        )
        .increment(units);
        metrics::histogram!(
            "aeterna_cost_amount_dollars",
            "tenant_id" => tenant_id_str.clone(),
            "team_id" => team_label.clone(),
            "project_id" => project_label.clone(),
            "resource_type" => resource_str,
            "operation" => operation.to_string()
        )
        .record(cost);

        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        let now = Utc::now();
        let entries = self.entries.read().unwrap();
        let scoped_total: f64 = entries
            .iter()
            .filter(|entry| entry.timestamp >= thirty_days_ago && entry.timestamp <= now)
            .filter(|entry| entry.tenant_id == tenant_id_str)
            .filter(|entry| {
                entry
                    .metadata
                    .get("team_id")
                    .or_else(|| entry.metadata.get("team"))
                    .map_or(team_label == "unknown", |value| value == &team_label)
            })
            .filter(|entry| {
                entry
                    .metadata
                    .get("project_id")
                    .or_else(|| entry.metadata.get("project"))
                    .map_or(project_label == "unknown", |value| value == &project_label)
            })
            .map(|entry| entry.cost)
            .sum();

        metrics::gauge!(
            "aeterna_cost_total_30d_dollars",
            "tenant_id" => tenant_id_str,
            "team_id" => team_label,
            "project_id" => project_label
        )
        .set(scoped_total);

        tracing::debug!(
            "Recorded cost for tenant {}: ${:.4}",
            ctx.tenant_id.as_str(),
            cost
        );
    }

    /// Get cost summary for a tenant
    pub fn get_tenant_summary(
        &self,
        tenant_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> TenantCostSummary {
        let entries = self.entries.read().unwrap();
        let filtered: Vec<_> = entries
            .iter()
            .filter(|e| e.tenant_id == tenant_id && e.timestamp >= start && e.timestamp <= end)
            .collect();

        let total_cost: f64 = filtered.iter().map(|e| e.cost).sum();

        let mut by_resource_type = HashMap::new();
        let mut by_operation = HashMap::new();

        for entry in &filtered {
            *by_resource_type
                .entry(entry.resource_type.clone())
                .or_insert(0.0) += entry.cost;
            *by_operation.entry(entry.operation.clone()).or_insert(0.0) += entry.cost;
        }

        let budget_limit = self.budgets.read().unwrap().get(tenant_id).copied();
        let budget_used_percent = budget_limit.map(|limit| {
            if limit > 0.0 {
                (total_cost / limit) * 100.0
            } else {
                0.0
            }
        });

        TenantCostSummary {
            tenant_id: tenant_id.to_string(),
            period_start: start,
            period_end: end,
            total_cost,
            currency: self.config.currency.clone(),
            by_resource_type,
            by_operation,
            budget_limit,
            budget_used_percent,
        }
    }

    /// Set budget limit for a tenant
    pub fn set_budget(&self, tenant_id: &str, limit: f64) {
        if let Ok(mut budgets) = self.budgets.write() {
            budgets.insert(tenant_id.to_string(), limit);
        }
    }

    /// Check if tenant is over budget
    pub fn is_over_budget(&self, tenant_id: &str) -> bool {
        let summary = self.get_tenant_summary(
            tenant_id,
            Utc::now() - chrono::Duration::days(30),
            Utc::now(),
        );

        match summary.budget_limit {
            Some(limit) => summary.total_cost >= limit,
            None => false,
        }
    }

    /// Get budget warning level (0.0 to 1.0)
    pub fn get_budget_warning_level(&self, tenant_id: &str) -> f64 {
        let summary = self.get_tenant_summary(
            tenant_id,
            Utc::now() - chrono::Duration::days(30),
            Utc::now(),
        );

        match (summary.budget_limit, summary.total_cost) {
            (Some(limit), cost) if limit > 0.0 => (cost / limit).min(1.0),
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantId, UserId};

    fn test_tenant_context() -> TenantContext {
        TenantContext::new(
            TenantId::new("test-tenant".to_string()).unwrap(),
            UserId::new("test-user".to_string()).unwrap(),
        )
    }

    #[test]
    fn test_cost_tracking() {
        let tracker = CostTracker::new(CostConfig::default());
        let ctx = test_tenant_context();

        tracker.record_embedding_generation(&ctx, 1000, "text-embedding-ada-002");
        tracker.record_llm_completion(&ctx, 500, "gpt-4");

        let summary = tracker.get_tenant_summary(
            "test-tenant",
            Utc::now() - chrono::Duration::hours(1),
            Utc::now(),
        );

        assert!(summary.total_cost > 0.0);
        assert_eq!(summary.tenant_id, "test-tenant");
        assert!(
            summary
                .by_resource_type
                .contains_key(&ResourceType::EmbeddingGeneration)
        );
    }

    #[test]
    fn test_budget_management() {
        let tracker = CostTracker::new(CostConfig::default());
        let ctx = test_tenant_context();

        tracker.set_budget("test-tenant", 1.0);
        assert!(!tracker.is_over_budget("test-tenant"));

        // Add costs
        tracker.record_llm_completion(&ctx, 50000, "gpt-4"); // ~$1.50

        assert!(tracker.is_over_budget("test-tenant"));
    }

    #[test]
    fn test_scoped_cost_tracking_persists_team_and_project_metadata() {
        let tracker = CostTracker::new(CostConfig::default());
        let ctx = test_tenant_context();

        tracker.record_llm_completion_scoped(
            &ctx,
            1_000,
            "gpt-4",
            Some("team-platform"),
            Some("project-aeterna"),
        );

        let entries = tracker.entries.read().unwrap();
        let entry = entries.last().unwrap();
        assert_eq!(
            entry.metadata.get("team_id").map(String::as_str),
            Some("team-platform")
        );
        assert_eq!(
            entry.metadata.get("project_id").map(String::as_str),
            Some("project-aeterna")
        );
    }
}
