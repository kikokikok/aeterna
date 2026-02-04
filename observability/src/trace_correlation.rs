///! # Trace Correlation Module
///!
///! Correlates traces, logs, and metrics across distributed services.
///! Provides unified view of system behavior for debugging and analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Unique trace ID that follows the request across all services
    pub trace_id: String,
    /// Span ID for the current operation
    pub span_id: String,
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    /// Service name
    pub service_name: String,
    /// Tenant ID for multi-tenant correlation
    pub tenant_id: Option<String>,
    /// User ID for user-specific correlation
    pub user_id: Option<String>,
}

impl TraceContext {
    pub fn new(service_name: &str) -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
            service_name: service_name.to_string(),
            tenant_id: None,
            user_id: None,
        }
    }

    pub fn with_parent(parent: &TraceContext, service_name: &str) -> Self {
        Self {
            trace_id: parent.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(parent.span_id.clone()),
            service_name: service_name.to_string(),
            tenant_id: parent.tenant_id.clone(),
            user_id: parent.user_id.clone(),
        }
    }

    pub fn with_tenant(&mut self, tenant_id: &str) -> &mut Self {
        self.tenant_id = Some(tenant_id.to_string());
        self
    }

    pub fn with_user(&mut self, user_id: &str) -> &mut Self {
        self.user_id = Some(user_id.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub service_name: String,
    pub operation_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<f64>,
    pub status: SpanStatus,
    pub tags: HashMap<String, String>,
    pub logs: Vec<SpanLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpanStatus {
    Ok,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub trace_id: String,
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

pub struct TraceCorrelator {
    spans: Arc<RwLock<HashMap<String, Vec<Span>>>>,
    metrics: Arc<RwLock<HashMap<String, Vec<MetricPoint>>>>,
}

impl TraceCorrelator {
    pub fn new() -> Self {
        Self {
            spans: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_span(
        &self,
        ctx: &TraceContext,
        operation_name: &str,
    ) -> Span {
        Span {
            trace_id: ctx.trace_id.clone(),
            span_id: ctx.span_id.clone(),
            parent_span_id: ctx.parent_span_id.clone(),
            service_name: ctx.service_name.clone(),
            operation_name: operation_name.to_string(),
            start_time: Utc::now(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::Ok,
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn end_span(&self, mut span: Span, status: SpanStatus) {
        let end_time = Utc::now();
        let duration_ms = (end_time - span.start_time).num_milliseconds() as f64;

        span.end_time = Some(end_time);
        span.duration_ms = Some(duration_ms);
        span.status = status;

        if let Ok(mut spans) = self.spans.write() {
            spans
                .entry(span.trace_id.clone())
                .or_insert_with(Vec::new)
                .push(span);
        }
    }

    pub fn record_metric(
        &self,
        trace_id: &str,
        metric_name: &str,
        value: f64,
        tags: HashMap<String, String>,
    ) {
        let point = MetricPoint {
            trace_id: trace_id.to_string(),
            timestamp: Utc::now(),
            metric_name: metric_name.to_string(),
            value,
            tags,
        };

        if let Ok(mut metrics) = self.metrics.write() {
            metrics
                .entry(trace_id.to_string())
                .or_insert_with(Vec::new)
                .push(point);
        }
    }

    pub fn get_trace(&self, trace_id: &str) -> Option<Vec<Span>> {
        self.spans.read().ok()?.get(trace_id).cloned()
    }

    pub fn get_trace_metrics(&self, trace_id: &str) -> Option<Vec<MetricPoint>> {
        self.metrics.read().ok()?.get(trace_id).cloned()
    }

    /// Get full trace with spans and metrics
    pub fn get_full_trace(&self, trace_id: &str) -> Option<FullTrace> {
        let spans = self.get_trace(trace_id)?;
        let metrics = self.get_trace_metrics(trace_id).unwrap_or_default();

        // Calculate values before moving spans
        let total_duration_ms = self.calculate_total_duration(&spans);
        let error_count = self.count_errors(&spans);

        Some(FullTrace {
            trace_id: trace_id.to_string(),
            spans,
            metrics,
            total_duration_ms,
            error_count,
        })
    }

    fn calculate_total_duration(&self, spans: &[Span]) -> f64 {
        spans
            .iter()
            .filter(|s| s.parent_span_id.is_none())
            .filter_map(|s| s.duration_ms)
            .sum()
    }

    fn count_errors(&self, spans: &[Span]) -> usize {
        spans.iter().filter(|s| s.status == SpanStatus::Error).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTrace {
    pub trace_id: String,
    pub spans: Vec<Span>,
    pub metrics: Vec<MetricPoint>,
    pub total_duration_ms: f64,
    pub error_count: usize,
}

impl Default for TraceCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to inject trace context into HTTP headers
pub fn inject_trace_headers(ctx: &TraceContext, headers: &mut HashMap<String, String>) {
    headers.insert("x-trace-id".to_string(), ctx.trace_id.clone());
    headers.insert("x-span-id".to_string(), ctx.span_id.clone());
    if let Some(parent) = &ctx.parent_span_id {
        headers.insert("x-parent-span-id".to_string(), parent.clone());
    }
    if let Some(tenant) = &ctx.tenant_id {
        headers.insert("x-tenant-id".to_string(), tenant.clone());
    }
}

/// Helper to extract trace context from HTTP headers
pub fn extract_trace_headers(
    headers: &HashMap<String, String>,
    service_name: &str,
) -> Option<TraceContext> {
    let trace_id = headers.get("x-trace-id")?.clone();
    let parent_span_id = headers.get("x-span-id").cloned();

    Some(TraceContext {
        trace_id,
        span_id: Uuid::new_v4().to_string(),
        parent_span_id,
        service_name: service_name.to_string(),
        tenant_id: headers.get("x-tenant-id").cloned(),
        user_id: headers.get("x-user-id").cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new("test-service");
        assert_eq!(ctx.service_name, "test-service");
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn test_trace_context_with_parent() {
        let parent = TraceContext::new("parent-service");
        let child = TraceContext::with_parent(&parent, "child-service");

        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
    }

    #[test]
    fn test_span_tracking() {
        let correlator = TraceCorrelator::new();
        let ctx = TraceContext::new("test-service");

        let span = correlator.start_span(&ctx, "test_operation");
        correlator.end_span(span.clone(), SpanStatus::Ok);

        let trace = correlator.get_trace(&ctx.trace_id);
        assert!(trace.is_some());
        let spans = trace.unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].operation_name, "test_operation");
    }

    #[test]
    fn test_metric_recording() {
        let correlator = TraceCorrelator::new();
        let trace_id = "test-trace-123";

        let mut tags = HashMap::new();
        tags.insert("service".to_string(), "test".to_string());

        correlator.record_metric(trace_id, "requests_total", 42.0, tags);

        let metrics = correlator.get_trace_metrics(trace_id);
        assert!(metrics.is_some());
        let points = metrics.unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 42.0);
    }
}
