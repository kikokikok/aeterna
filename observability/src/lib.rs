pub mod anomaly_detection;
pub mod cost_dashboard;
///! # Observability Module
///!
///! Advanced observability features including:
///! - Trace correlation across services
///! - Per-tenant cost tracking
///! - Anomaly detection
///! - SLO monitoring
pub mod cost_tracking;
pub mod trace_correlation;

pub use anomaly_detection::{
    Anomaly, AnomalyDetector, AnomalyDetectorConfig, AnomalyType, DetectionResult, MetricBaseline,
};
pub use cost_dashboard::{
    AlertHandler, AlertLevel, BudgetAlert, BudgetAlertConfig, BudgetAlertSystem, CostDashboard,
    CostDataPoint, DashboardError, DashboardSummary, NoopAlertHandler, TenantCostPanel,
};
pub use cost_tracking::{CostConfig, CostEntry, CostTracker, ResourceType, TenantCostSummary};
pub use trace_correlation::{
    FullTrace, LogLevel, MetricPoint, Span, SpanLog, SpanStatus, TraceContext, TraceCorrelator,
    extract_trace_headers, inject_trace_headers,
};
