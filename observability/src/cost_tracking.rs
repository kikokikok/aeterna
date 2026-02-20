///! # Cost Tracking Module
///!
///! Per-tenant cost tracking for embeddings, storage, and compute resources.
///! Provides detailed cost breakdown and budget management.
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
        let cost = (token_count as f64 / 1000.0) * self.config.embedding_cost_per_1k_tokens;
        self.record_cost(
            ctx,
            ResourceType::EmbeddingGeneration,
            "generate",
            cost,
            token_count,
            vec![("model".to_string(), model.to_string())],
        );
    }

    /// Record an LLM completion cost
    pub fn record_llm_completion(&self, ctx: &TenantContext, token_count: u64, model: &str) {
        let cost = (token_count as f64 / 1000.0) * self.config.llm_cost_per_1k_tokens;
        self.record_cost(
            ctx,
            ResourceType::LlmCompletion,
            "complete",
            cost,
            token_count,
            vec![("model".to_string(), model.to_string())],
        );
    }

    /// Record vector storage cost
    pub fn record_storage(&self, ctx: &TenantContext, bytes: u64) {
        let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let monthly_cost = gb * self.config.storage_cost_per_gb_month;
        // Prorate to daily cost
        let daily_cost = monthly_cost / 30.0;
        self.record_cost(
            ctx,
            ResourceType::VectorStorage,
            "store",
            daily_cost,
            bytes,
            vec![],
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
        let entry = CostEntry {
            tenant_id: ctx.tenant_id.as_str().to_string(),
            resource_type,
            operation: operation.to_string(),
            cost,
            currency: self.config.currency.clone(),
            units,
            timestamp: Utc::now(),
            metadata: metadata.into_iter().collect(),
        };

        if let Ok(mut entries) = self.entries.write() {
            entries.push(entry);
        }

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
}
