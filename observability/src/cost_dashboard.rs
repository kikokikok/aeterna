use crate::cost_tracking::{CostTracker, ResourceType, TenantCostSummary};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DashboardError {
    #[error("No data available for tenant '{tenant}'")]
    NoData { tenant: String },
    #[error("Invalid time range: start must be before end")]
    InvalidTimeRange,
    #[error("Budget alert dispatch failed: {reason}")]
    AlertDispatchFailed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDataPoint {
    pub timestamp: DateTime<Utc>,
    pub cost: f64,
    pub currency: String,
    pub tenant_id: String,
    pub resource_type: Option<ResourceType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCostPanel {
    pub tenant_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_cost: f64,
    pub currency: String,
    pub by_resource: HashMap<String, f64>,
    pub by_operation: HashMap<String, f64>,
    pub budget_used_percent: Option<f64>,
    pub budget_limit: Option<f64>,
    pub alert_level: AlertLevel,
}

impl From<TenantCostSummary> for TenantCostPanel {
    fn from(s: TenantCostSummary) -> Self {
        let alert_level = AlertLevel::from_budget_percent(s.budget_used_percent);
        let by_resource = s
            .by_resource_type
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect();
        TenantCostPanel {
            tenant_id: s.tenant_id,
            period_start: s.period_start,
            period_end: s.period_end,
            total_cost: s.total_cost,
            currency: s.currency,
            by_resource,
            by_operation: s.by_operation,
            budget_used_percent: s.budget_used_percent,
            budget_limit: s.budget_limit,
            alert_level,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    Ok,
    Warning,
    Critical,
    OverBudget,
    NoBudget,
}

impl AlertLevel {
    fn from_budget_percent(pct: Option<f64>) -> Self {
        match pct {
            None => AlertLevel::NoBudget,
            Some(p) if p >= 100.0 => AlertLevel::OverBudget,
            Some(p) if p >= 90.0 => AlertLevel::Critical,
            Some(p) if p >= 70.0 => AlertLevel::Warning,
            _ => AlertLevel::Ok,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub generated_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_cost_all_tenants: f64,
    pub currency: String,
    pub tenant_panels: Vec<TenantCostPanel>,
    pub alert_counts: HashMap<String, usize>,
}

pub struct CostDashboard {
    tracker: Arc<CostTracker>,
}

impl CostDashboard {
    pub fn new(tracker: Arc<CostTracker>) -> Self {
        Self { tracker }
    }

    pub fn summary(
        &self,
        tenant_ids: &[&str],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<DashboardSummary, DashboardError> {
        if start >= end {
            return Err(DashboardError::InvalidTimeRange);
        }

        let mut tenant_panels: Vec<TenantCostPanel> = Vec::new();
        let mut total = 0.0f64;
        let mut currency = "USD".to_string();

        for &tid in tenant_ids {
            let summary = self.tracker.get_tenant_summary(tid, start, end);
            total += summary.total_cost;
            currency = summary.currency.clone();
            tenant_panels.push(TenantCostPanel::from(summary));
        }

        tenant_panels.sort_by(|a, b| b.total_cost.partial_cmp(&a.total_cost).unwrap());

        let mut alert_counts: HashMap<String, usize> = HashMap::new();
        for panel in &tenant_panels {
            let key = format!("{:?}", panel.alert_level).to_lowercase();
            *alert_counts.entry(key).or_insert(0) += 1;
        }

        Ok(DashboardSummary {
            generated_at: Utc::now(),
            period_start: start,
            period_end: end,
            total_cost_all_tenants: total,
            currency,
            tenant_panels,
            alert_counts,
        })
    }

    pub fn last_30_days(&self, tenant_ids: &[&str]) -> Result<DashboardSummary, DashboardError> {
        let end = Utc::now();
        let start = end - Duration::days(30);
        self.summary(tenant_ids, start, end)
    }

    pub fn tenant_panel(
        &self,
        tenant_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TenantCostPanel, DashboardError> {
        if start >= end {
            return Err(DashboardError::InvalidTimeRange);
        }
        let summary = self.tracker.get_tenant_summary(tenant_id, start, end);
        Ok(TenantCostPanel::from(summary))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    pub tenant_id: String,
    pub alert_level: AlertLevel,
    pub budget_limit: f64,
    pub current_spend: f64,
    pub budget_used_percent: f64,
    pub fired_at: DateTime<Utc>,
    pub message: String,
}

impl BudgetAlert {
    fn new(tenant_id: &str, panel: &TenantCostPanel) -> Option<Self> {
        let limit = panel.budget_limit?;
        let pct = panel.budget_used_percent?;
        if panel.alert_level == AlertLevel::Ok || panel.alert_level == AlertLevel::NoBudget {
            return None;
        }
        let message = match panel.alert_level {
            AlertLevel::OverBudget => format!(
                "OVER BUDGET: tenant '{}' has spent ${:.2} ({:.1}%) of ${:.2} budget.",
                tenant_id, panel.total_cost, pct, limit
            ),
            AlertLevel::Critical => format!(
                "CRITICAL: tenant '{}' at {:.1}% of ${:.2} budget (${:.2} spent).",
                tenant_id, pct, limit, panel.total_cost
            ),
            AlertLevel::Warning => format!(
                "WARNING: tenant '{}' at {:.1}% of ${:.2} budget (${:.2} spent).",
                tenant_id, pct, limit, panel.total_cost
            ),
            _ => return None,
        };
        Some(BudgetAlert {
            tenant_id: tenant_id.to_string(),
            alert_level: panel.alert_level.clone(),
            budget_limit: limit,
            current_spend: panel.total_cost,
            budget_used_percent: pct,
            fired_at: Utc::now(),
            message,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BudgetAlertConfig {
    pub warning_threshold_pct: f64,
    pub critical_threshold_pct: f64,
    pub cooldown_seconds: u64,
}

impl Default for BudgetAlertConfig {
    fn default() -> Self {
        Self {
            warning_threshold_pct: 70.0,
            critical_threshold_pct: 90.0,
            cooldown_seconds: 3600,
        }
    }
}

pub trait AlertHandler: Send + Sync {
    fn on_alert(&self, alert: &BudgetAlert);
}

pub struct NoopAlertHandler;
impl AlertHandler for NoopAlertHandler {
    fn on_alert(&self, _alert: &BudgetAlert) {}
}

pub struct BudgetAlertSystem {
    dashboard: Arc<CostDashboard>,
    config: BudgetAlertConfig,
    handler: Arc<dyn AlertHandler>,
    last_fired: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl BudgetAlertSystem {
    pub fn new(
        dashboard: Arc<CostDashboard>,
        config: BudgetAlertConfig,
        handler: Arc<dyn AlertHandler>,
    ) -> Self {
        Self {
            dashboard,
            config,
            handler,
            last_fired: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn check(
        &self,
        tenant_ids: &[&str],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<BudgetAlert>, DashboardError> {
        let summary = self.dashboard.summary(tenant_ids, start, end)?;
        let mut fired: Vec<BudgetAlert> = Vec::new();
        let cooldown = Duration::seconds(self.config.cooldown_seconds as i64);
        let now = Utc::now();

        for panel in &summary.tenant_panels {
            if panel.alert_level == AlertLevel::Ok || panel.alert_level == AlertLevel::NoBudget {
                continue;
            }

            if let Ok(last) = self.last_fired.read() {
                if let Some(&fired_at) = last.get(&panel.tenant_id) {
                    if now - fired_at < cooldown {
                        continue;
                    }
                }
            }

            if let Some(alert) = BudgetAlert::new(&panel.tenant_id, panel) {
                let pct = alert.budget_used_percent;
                if pct < self.config.warning_threshold_pct {
                    continue;
                }

                tracing::warn!(
                    tenant = %alert.tenant_id,
                    pct = %pct,
                    level = ?alert.alert_level,
                    "{}",
                    alert.message
                );

                self.handler.on_alert(&alert);

                if let Ok(mut last) = self.last_fired.write() {
                    last.insert(panel.tenant_id.clone(), now);
                }

                fired.push(alert);
            }
        }

        Ok(fired)
    }

    pub fn check_last_30_days(
        &self,
        tenant_ids: &[&str],
    ) -> Result<Vec<BudgetAlert>, DashboardError> {
        let end = Utc::now();
        let start = end - Duration::days(30);
        self.check(tenant_ids, start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost_tracking::{CostConfig, CostTracker};
    use mk_core::types::{TenantContext, TenantId, UserId};
    use std::sync::Mutex;

    fn make_ctx(tenant: &str) -> TenantContext {
        TenantContext::new(
            TenantId::new(tenant.to_string()).unwrap(),
            UserId::new("user-1".to_string()).unwrap(),
        )
    }

    fn make_tracker() -> Arc<CostTracker> {
        Arc::new(CostTracker::new(CostConfig::default()))
    }

    #[test]
    fn test_dashboard_summary_empty_tenants() {
        let tracker = make_tracker();
        let dashboard = CostDashboard::new(tracker);
        let now = Utc::now();
        let result = dashboard.summary(&[], now - Duration::hours(1), now);
        assert!(result.is_ok());
        let summary = result.unwrap();
        assert_eq!(summary.total_cost_all_tenants, 0.0);
        assert!(summary.tenant_panels.is_empty());
    }

    #[test]
    fn test_dashboard_summary_invalid_range() {
        let tracker = make_tracker();
        let dashboard = CostDashboard::new(tracker);
        let now = Utc::now();
        let result = dashboard.summary(&["t1"], now, now - Duration::hours(1));
        assert!(matches!(result, Err(DashboardError::InvalidTimeRange)));
    }

    #[test]
    fn test_dashboard_summary_aggregates_tenants() {
        let tracker = make_tracker();
        let ctx_a = make_ctx("tenant-a");
        let ctx_b = make_ctx("tenant-b");

        tracker.record_embedding_generation(&ctx_a, 5000, "ada-002");
        tracker.record_llm_completion(&ctx_b, 2000, "gpt-4");

        let dashboard = CostDashboard::new(Arc::clone(&tracker));
        let now = Utc::now();
        let summary = dashboard
            .summary(&["tenant-a", "tenant-b"], now - Duration::hours(1), now)
            .unwrap();

        assert_eq!(summary.tenant_panels.len(), 2);
        assert!(summary.total_cost_all_tenants > 0.0);
        assert!(summary.tenant_panels[0].total_cost >= summary.tenant_panels[1].total_cost);
    }

    #[test]
    fn test_dashboard_tenant_panel_no_budget() {
        let tracker = make_tracker();
        let ctx = make_ctx("tenant-c");
        tracker.record_storage(&ctx, 1024 * 1024 * 100);

        let dashboard = CostDashboard::new(Arc::clone(&tracker));
        let now = Utc::now();
        let panel = dashboard
            .tenant_panel("tenant-c", now - Duration::hours(1), now)
            .unwrap();

        assert_eq!(panel.alert_level, AlertLevel::NoBudget);
        assert!(panel.budget_limit.is_none());
        assert!(panel.by_resource.contains_key("vector_storage"));
    }

    #[test]
    fn test_alert_level_derivation() {
        assert_eq!(AlertLevel::from_budget_percent(None), AlertLevel::NoBudget);
        assert_eq!(AlertLevel::from_budget_percent(Some(50.0)), AlertLevel::Ok);
        assert_eq!(
            AlertLevel::from_budget_percent(Some(70.0)),
            AlertLevel::Warning
        );
        assert_eq!(
            AlertLevel::from_budget_percent(Some(90.0)),
            AlertLevel::Critical
        );
        assert_eq!(
            AlertLevel::from_budget_percent(Some(100.0)),
            AlertLevel::OverBudget
        );
        assert_eq!(
            AlertLevel::from_budget_percent(Some(120.0)),
            AlertLevel::OverBudget
        );
    }

    #[derive(Default)]
    struct CapturingHandler {
        alerts: Mutex<Vec<BudgetAlert>>,
    }
    impl AlertHandler for CapturingHandler {
        fn on_alert(&self, alert: &BudgetAlert) {
            self.alerts.lock().unwrap().push(alert.clone());
        }
    }

    #[test]
    fn test_budget_alert_fires_on_over_budget() {
        let tracker = make_tracker();
        let ctx = make_ctx("heavy-tenant");

        tracker.set_budget("heavy-tenant", 0.10);
        tracker.record_llm_completion(&ctx, 10_000, "gpt-4");

        let dashboard = Arc::new(CostDashboard::new(Arc::clone(&tracker)));
        let handler = Arc::new(CapturingHandler::default());
        let system = BudgetAlertSystem::new(
            Arc::clone(&dashboard),
            BudgetAlertConfig::default(),
            Arc::clone(&handler) as Arc<dyn AlertHandler>,
        );

        let now = Utc::now();
        let alerts = system
            .check(&["heavy-tenant"], now - Duration::hours(1), now)
            .unwrap();

        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].alert_level, AlertLevel::OverBudget);
        assert_eq!(handler.alerts.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_budget_alert_not_fired_under_threshold() {
        let tracker = make_tracker();
        let ctx = make_ctx("light-tenant");

        tracker.set_budget("light-tenant", 10.0);
        tracker.record_embedding_generation(&ctx, 100, "ada-002");

        let dashboard = Arc::new(CostDashboard::new(Arc::clone(&tracker)));
        let handler = Arc::new(CapturingHandler::default());
        let system = BudgetAlertSystem::new(
            Arc::clone(&dashboard),
            BudgetAlertConfig::default(),
            Arc::clone(&handler) as Arc<dyn AlertHandler>,
        );

        let now = Utc::now();
        let alerts = system
            .check(&["light-tenant"], now - Duration::hours(1), now)
            .unwrap();

        assert!(alerts.is_empty());
        assert_eq!(handler.alerts.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_budget_alert_cooldown_prevents_duplicate() {
        let tracker = make_tracker();
        let ctx = make_ctx("dup-tenant");

        tracker.set_budget("dup-tenant", 0.01);
        tracker.record_llm_completion(&ctx, 5_000, "gpt-4");

        let dashboard = Arc::new(CostDashboard::new(Arc::clone(&tracker)));
        let handler = Arc::new(CapturingHandler::default());
        let system = BudgetAlertSystem::new(
            Arc::clone(&dashboard),
            BudgetAlertConfig::default(),
            Arc::clone(&handler) as Arc<dyn AlertHandler>,
        );

        let now = Utc::now();
        let range = (now - Duration::hours(1), now);

        let first = system.check(&["dup-tenant"], range.0, range.1).unwrap();
        assert_eq!(first.len(), 1);

        let second = system.check(&["dup-tenant"], range.0, range.1).unwrap();
        assert_eq!(second.len(), 0);

        assert_eq!(handler.alerts.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_noop_alert_handler() {
        let handler = NoopAlertHandler;
        let panel = TenantCostPanel {
            tenant_id: "t".to_string(),
            period_start: Utc::now(),
            period_end: Utc::now(),
            total_cost: 999.0,
            currency: "USD".to_string(),
            by_resource: HashMap::new(),
            by_operation: HashMap::new(),
            budget_used_percent: Some(150.0),
            budget_limit: Some(100.0),
            alert_level: AlertLevel::OverBudget,
        };
        let alert = BudgetAlert {
            tenant_id: "t".to_string(),
            alert_level: AlertLevel::OverBudget,
            budget_limit: 100.0,
            current_spend: 999.0,
            budget_used_percent: 150.0,
            fired_at: Utc::now(),
            message: "over budget".to_string(),
        };
        handler.on_alert(&alert);
        drop(panel);
    }
}
