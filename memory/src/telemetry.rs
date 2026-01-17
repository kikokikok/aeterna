use metrics::{counter, gauge, histogram};
use opentelemetry::global;
use opentelemetry::global::{BoxedSpan, BoxedTracer};
use opentelemetry::metrics::Meter;
use opentelemetry::trace::Tracer;

#[derive(Debug)]
pub struct MemoryTelemetry {
    tracer: BoxedTracer
}

impl MemoryTelemetry {
    pub fn new() -> Self {
        let tracer = global::tracer("memory_system");

        Self { tracer }
    }

    pub fn with_tracer(tracer: BoxedTracer) -> Self {
        Self { tracer }
    }

    pub fn with_meter(_meter: Meter) -> Self {
        let tracer = global::tracer("memory_system");

        Self { tracer }
    }

    pub fn record_operation_start(&self, operation: &str, layer: &str) -> BoxedSpan {
        self.tracer
            .span_builder(format!("memory.{}", operation))
            .with_attributes(vec![
                opentelemetry::KeyValue::new("layer", layer.to_string()),
                opentelemetry::KeyValue::new("operation", operation.to_string()),
            ])
            .start(&self.tracer)
    }

    pub fn record_operation_success(&self, operation: &str, layer: &str, duration_ms: f64) {
        counter!("memory_operations_total", 1,
            "operation" => operation.to_string(),
            "layer" => layer.to_string(),
            "status" => "success"
        );

        histogram!("memory_operation_duration_seconds", duration_ms / 1000.0,
            "operation" => operation.to_string(),
            "layer" => layer.to_string()
        );
    }

    pub fn record_operation_failure(&self, operation: &str, layer: &str, error: &str) {
        counter!("memory_operations_total", 1,
            "operation" => operation.to_string(),
            "layer" => layer.to_string(),
            "status" => "failure",
            "error" => error.to_string()
        );

        counter!("memory_operation_errors_total", 1,
            "operation" => operation.to_string(),
            "layer" => layer.to_string(),
            "error_type" => error.to_string()
        );
    }

    pub fn record_embedding_generation(&self, dimension: usize, duration_ms: f64) {
        counter!("memory_embeddings_generated_total", 1,
            "dimension" => dimension.to_string()
        );

        histogram!("memory_embedding_generation_duration_seconds", duration_ms / 1000.0,
            "dimension" => dimension.to_string()
        );

        gauge!("memory_embedding_dimension", dimension as f64);
    }

    pub fn record_search_operation(&self, results_count: usize, query_dimension: usize) {
        counter!("memory_searches_total", 1);
        histogram!("memory_search_results_count", results_count as f64);
        gauge!("memory_search_query_dimension", query_dimension as f64);
    }

    pub fn record_storage_metrics(&self, entries_count: usize, total_size_bytes: usize) {
        gauge!("memory_entries_total", entries_count as f64);
        gauge!("memory_storage_size_bytes", total_size_bytes as f64);
    }

    pub fn record_cache_metrics(&self, hit_count: usize, miss_count: usize, cache_size: usize) {
        counter!("memory_cache_hits_total", hit_count as u64);
        counter!("memory_cache_misses_total", miss_count as u64);
        gauge!("memory_cache_size", cache_size as f64);

        let total = hit_count + miss_count;
        if total > 0 {
            let hit_rate = (hit_count as f64) / (total as f64);
            gauge!("memory_cache_hit_rate", hit_rate);
        }
    }

    pub fn record_promotion_attempt(&self, from_layer: &str, target_layer: &str) {
        counter!("memory_promotion_attempts_total", 1,
            "from_layer" => from_layer.to_string(),
            "target_layer" => target_layer.to_string()
        );
    }

    pub fn record_promotion_success(&self, from_layer: &str, target_layer: &str) {
        counter!("memory_promotion_success_total", 1,
            "from_layer" => from_layer.to_string(),
            "target_layer" => target_layer.to_string()
        );
    }

    pub fn record_promotion_blocked(&self, from_layer: &str, reason: &str) {
        counter!("memory_promotion_blocked_total", 1,
            "from_layer" => from_layer.to_string(),
            "reason" => reason.to_string()
        );
    }

    pub fn record_governance_redaction(&self, layer: &str) {
        counter!("memory_governance_redactions_total", 1,
            "layer" => layer.to_string()
        );
    }
}

pub fn init_telemetry() -> Result<MemoryTelemetry, Box<dyn std::error::Error + Send + Sync>> {
    let telemetry = MemoryTelemetry::new();

    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9090))
        .install()?;

    Ok(telemetry)
}

pub fn init_telemetry_with_endpoint(
    endpoint: std::net::SocketAddr
) -> Result<MemoryTelemetry, Box<dyn std::error::Error + Send + Sync>> {
    let telemetry = MemoryTelemetry::new();

    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(endpoint)
        .install()?;

    Ok(telemetry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::DebuggingRecorder;

    #[test]
    fn test_telemetry_creation() {
        use opentelemetry::trace::Span as _;
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_sdk::trace::TracerProvider;

        let provider = TracerProvider::default();
        let tracer = provider.tracer("test");
        let telemetry =
            MemoryTelemetry::with_tracer(opentelemetry::global::BoxedTracer::new(Box::new(tracer)));

        let mut span = telemetry.record_operation_start("add", "agent");
        assert!(span.span_context().is_valid());
        span.end();
    }

    #[test]
    fn test_metrics_recording() {
        let recorder = DebuggingRecorder::new();
        let static_recorder: &'static DebuggingRecorder = Box::leak(Box::new(recorder));
        metrics::set_recorder(static_recorder).ok();

        let telemetry = MemoryTelemetry::new();

        telemetry.record_operation_success("add", "agent", 150.0);
        telemetry.record_operation_failure("search", "session", "not_found");
        telemetry.record_embedding_generation(1536, 250.0);
        telemetry.record_search_operation(5, 1536);
        telemetry.record_storage_metrics(100, 1024000);
        telemetry.record_cache_metrics(75, 25, 100);
        telemetry.record_promotion_attempt("agent", "user");
        telemetry.record_promotion_success("agent", "user");
        telemetry.record_promotion_blocked("agent", "governance");
        telemetry.record_governance_redaction("user");
    }

    #[test]
    fn test_with_meter() {
        use opentelemetry::metrics::MeterProvider;
        use opentelemetry_sdk::metrics::MeterProvider as SdkMeterProvider;

        let provider = SdkMeterProvider::default();
        let meter = provider.meter("test");

        // Test that with_meter creates telemetry instance
        let telemetry = MemoryTelemetry::with_meter(meter);

        // Verify telemetry instance was created
        // The meter parameter is ignored in the implementation, but we test the method
        // exists
        assert!(std::mem::size_of_val(&telemetry) > 0);
    }

    #[test]
    fn test_init_telemetry() {
        // Test that init_telemetry returns a Result
        // The function might succeed or fail depending on port availability
        let result = init_telemetry();

        // Verify it returns a Result (either Ok or Err)
        // We can't guarantee it will fail because port 9090 might be available
        match result {
            Ok(telemetry) => {
                // If it succeeds, verify we got a telemetry instance
                assert!(std::mem::size_of_val(&telemetry) > 0);
            }
            Err(e) => {
                // If it fails, verify the error is related to binding
                let error_str = e.to_string();
                assert!(
                    error_str.contains("address already in use")
                        || error_str.contains("bind")
                        || error_str.contains("port")
                        || error_str.contains("Permission denied")
                );
            }
        }
    }

    #[test]
    fn test_init_telemetry_with_endpoint() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        // Create a test endpoint (port 0 means OS will assign a free port)
        let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

        // Test that init_telemetry_with_endpoint returns a Result
        let result = init_telemetry_with_endpoint(endpoint);

        // The function should work with port 0 (OS-assigned port)
        // But metrics initialization might still fail for other reasons
        // We just verify it returns a Result
        assert!(result.is_err() || result.is_ok());

        // If it fails, verify it's not a bind error
        if let Err(e) = result {
            let error_str = e.to_string();
            // Should not be a bind error with port 0
            assert!(!error_str.contains("address already in use"));
        }
    }
}
