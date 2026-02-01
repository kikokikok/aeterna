///! # Observability Module
///!
///! Advanced observability features including:
///! - Trace correlation across services
///! - Per-tenant cost tracking
///! - Anomaly detection
///! - SLO monitoring

pub mod cost_tracking;
pub mod trace_correlation;
pub mod anomaly_detection;

pub use cost_tracking::{
    CostConfig, CostEntry, CostTracker, ResourceType, TenantCostSummary,
};
pub use trace_correlation::{
    TraceContext, TraceCorrelator, Span, SpanStatus, SpanLog, LogLevel,
    MetricPoint, FullTrace, inject_trace_headers, extract_trace_headers,
};
pub use anomaly_detection::{
    AnomalyDetector, AnomalyDetectorConfig, Anomaly, AnomalyType,
    MetricBaseline, DetectionResult,
};
