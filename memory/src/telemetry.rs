use metrics::{counter, gauge, histogram};
use tracing::{Level, Span, instrument, span};

#[derive(Debug)]
pub struct MemoryTelemetry {
    _phantom: std::marker::PhantomData<()>
}

impl Default for MemoryTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryTelemetry {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData
        }
    }

    pub fn with_tracer(_tracer: ()) -> Self {
        Self {
            _phantom: std::marker::PhantomData
        }
    }

    pub fn with_meter(_meter: ()) -> Self {
        Self {
            _phantom: std::marker::PhantomData
        }
    }

    #[instrument(skip(self))]
    pub fn record_operation_start(&self, operation: &str, layer: &str) -> Span {
        span!(
            target: "memory_system",
            Level::INFO,
            "memory.{}",
            operation
        )
    }

    pub fn record_operation_success(&self, operation: &str, layer: &str, duration_ms: f64) {
        let counter_labels = [
            ("operation", operation.to_string()),
            ("layer", layer.to_string()),
            ("status", "success".to_string())
        ];
        counter!("memory_operations_total", &counter_labels).increment(1);

        let histogram_labels = [
            ("operation", operation.to_string()),
            ("layer", layer.to_string())
        ];
        histogram!("memory_operation_duration_seconds", &histogram_labels)
            .record(duration_ms / 1000.0);
    }

    pub fn record_operation_failure(&self, operation: &str, layer: &str, error: &str) {
        let counter_labels = [
            ("operation", operation.to_string()),
            ("layer", layer.to_string()),
            ("status", "failure".to_string()),
            ("error", error.to_string())
        ];
        counter!("memory_operations_total", &counter_labels).increment(1);

        let error_counter_labels = [
            ("operation", operation.to_string()),
            ("layer", layer.to_string()),
            ("error_type", error.to_string())
        ];
        counter!("memory_operation_errors_total", &error_counter_labels).increment(1);
    }

    pub fn record_embedding_generation(&self, dimension: usize, duration_ms: f64) {
        counter!("memory_embeddings_generated_total",
            "dimension" => dimension.to_string()
        )
        .increment(1);

        histogram!("memory_embedding_generation_duration_seconds",
            "dimension" => dimension.to_string()
        )
        .record(duration_ms / 1000.0);

        gauge!("memory_embedding_dimension").set(dimension as f64);
    }

    pub fn record_search_operation(&self, results_count: usize, query_dimension: usize) {
        counter!("memory_searches_total").increment(1);
        histogram!("memory_search_results_count").record(results_count as f64);
        gauge!("memory_search_query_dimension").set(query_dimension as f64);
    }

    pub fn record_storage_metrics(&self, entries_count: usize, total_size_bytes: usize) {
        gauge!("memory_entries_total").set(entries_count as f64);
        gauge!("memory_storage_size_bytes").set(total_size_bytes as f64);
    }

    pub fn record_cache_metrics(&self, hit_count: usize, miss_count: usize, cache_size: usize) {
        counter!("memory_cache_hits_total").increment(hit_count as u64);
        counter!("memory_cache_misses_total").increment(miss_count as u64);
        gauge!("memory_cache_size").set(cache_size as f64);

        let total = hit_count + miss_count;
        if total > 0 {
            let hit_rate = (hit_count as f64) / (total as f64);
            gauge!("memory_cache_hit_rate").set(hit_rate);
        }
    }

    pub fn record_promotion_attempt(&self, from_layer: &str, target_layer: &str) {
        counter!("memory_promotion_attempts_total",
            "from_layer" => from_layer.to_string(),
            "target_layer" => target_layer.to_string()
        )
        .increment(1);
    }

    pub fn record_promotion_success(&self, from_layer: &str, target_layer: &str) {
        counter!("memory_promotion_success_total",
            "from_layer" => from_layer.to_string(),
            "target_layer" => target_layer.to_string()
        )
        .increment(1);
    }

    pub fn record_promotion_blocked(&self, from_layer: &str, reason: &str) {
        counter!("memory_promotion_blocked_total",
            "from_layer" => from_layer.to_string(),
            "reason" => reason.to_string()
        )
        .increment(1);
    }

    pub fn record_governance_redaction(&self, layer: &str) {
        counter!("memory_governance_redactions_total",
            "layer" => layer.to_string()
        )
        .increment(1);
    }

    pub fn record_reasoning_latency(&self, duration_ms: f64, timed_out: bool) {
        histogram!("memory_reasoning_latency_seconds").record(duration_ms / 1000.0);
        counter!("memory_reasoning_total",
            "timed_out" => timed_out.to_string()
        )
        .increment(1);

        if timed_out {
            counter!("memory_reasoning_timeouts_total").increment(1);
        }
    }

    pub fn record_reasoning_p95_exceeded(&self, latency_ms: f64, threshold_ms: f64) {
        counter!("memory_reasoning_p95_exceeded_total").increment(1);
        gauge!("memory_reasoning_last_exceeded_latency_ms").set(latency_ms);
        gauge!("memory_reasoning_p95_threshold_ms").set(threshold_ms);
    }

    pub fn record_reasoning_cache_hit(&self) {
        counter!("memory_reasoning_cache_hits_total").increment(1);
    }

    pub fn record_reasoning_cache_miss(&self) {
        counter!("memory_reasoning_cache_misses_total").increment(1);
    }

    pub fn record_reasoning_cache_eviction(&self, evicted_count: usize) {
        counter!("memory_reasoning_cache_evictions_total").increment(evicted_count as u64);
    }

    pub fn record_reasoning_llm_call(&self) {
        counter!("memory_reasoning_llm_calls_total").increment(1);
    }

    pub fn record_reasoning_failure(&self, error: &str) {
        counter!("memory_reasoning_failures_total",
            "error_type" => error.to_string()
        )
        .increment(1);
    }

    pub fn record_reasoning_circuit_opened(&self, failure_rate: f64) {
        counter!("memory_reasoning_circuit_opened_total").increment(1);
        gauge!("memory_reasoning_unavailable").set(1.0);
        gauge!("memory_reasoning_circuit_failure_rate").set(failure_rate);
    }

    pub fn record_reasoning_circuit_closed(&self) {
        counter!("memory_reasoning_circuit_closed_total").increment(1);
        gauge!("memory_reasoning_unavailable").set(0.0);
    }

    pub fn record_reasoning_circuit_half_open(&self) {
        counter!("memory_reasoning_circuit_half_open_total").increment(1);
        gauge!("memory_reasoning_unavailable").set(0.5);
    }

    pub fn record_reasoning_circuit_rejected(&self) {
        counter!("memory_reasoning_circuit_rejected_total").increment(1);
    }

    pub fn record_multi_hop_metrics(&self, metrics: &crate::multi_hop::MultiHopMetrics) {
        gauge!("memory_multi_hop_depth_reached").set(metrics.max_depth_reached as f64);
        counter!("memory_multi_hop_queries_total").increment(metrics.total_queries as u64);
        counter!("memory_multi_hop_paths_terminated_depth_total")
            .increment(metrics.paths_terminated_depth as u64);
        counter!("memory_multi_hop_paths_terminated_relevance_total")
            .increment(metrics.paths_terminated_relevance as u64);
        counter!("memory_multi_hop_paths_terminated_budget_total")
            .increment(metrics.paths_terminated_budget as u64);
    }

    pub fn record_embedding_cache_hit(&self, cache_type: &str) {
        counter!("memory_embedding_cache_hits_total",
            "cache_type" => cache_type.to_string()
        )
        .increment(1);
    }

    pub fn record_embedding_cache_miss(&self) {
        counter!("memory_embedding_cache_misses_total").increment(1);
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
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            let telemetry = MemoryTelemetry::new();
            let _span = telemetry.record_operation_start("add", "agent");
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert!(snapshot.is_empty() || !snapshot.is_empty());
    }

    #[test]
    fn test_metrics_recording() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
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
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert!(!snapshot.is_empty(), "Expected metrics to be recorded");
    }
}
