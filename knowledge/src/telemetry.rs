use metrics::{gauge, histogram, increment_counter};

pub struct KnowledgeTelemetry;

impl KnowledgeTelemetry {
    pub fn record_operation(&self, operation: &str, status: &str) {
        // GIVEN an operation and status
        // WHEN recording metrics
        // THEN increment the operation counter with status label

        increment_counter!("knowledge_operations_total", "operation" => operation.to_string(), "status" => status.to_string());
    }

    pub fn record_violation(&self, layer: &str, severity: &str) {
        // GIVEN a layer and severity
        // WHEN recording metrics
        // THEN increment the violation counter with layer and severity labels

        increment_counter!("knowledge_violations_total", "layer" => layer.to_string(), "severity" => severity.to_string());
    }

    pub fn record_summary_generation(
        &self,
        depth: &str,
        status: &str,
        tokens_used: u32,
        latency_ms: f64,
    ) {
        increment_counter!("cca_summary_generation_total", "depth" => depth.to_string(), "status" => status.to_string());
        histogram!("cca_summary_generation_tokens", tokens_used as f64, "depth" => depth.to_string());
        histogram!("cca_summary_generation_latency_ms", latency_ms, "depth" => depth.to_string());
    }

    pub fn record_note_distillation(&self, status: &str, events_count: usize, latency_ms: f64) {
        increment_counter!("cca_note_distillation_total", "status" => status.to_string());
        histogram!("cca_note_distillation_events", events_count as f64);
        histogram!("cca_note_distillation_latency_ms", latency_ms);
    }

    pub fn record_hindsight_query(&self, status: &str, patterns_found: usize, latency_ms: f64) {
        increment_counter!("cca_hindsight_query_total", "status" => status.to_string());
        histogram!("cca_hindsight_query_patterns_found", patterns_found as f64);
        histogram!("cca_hindsight_query_latency_ms", latency_ms);
    }

    pub fn record_meta_agent_loop(
        &self,
        phase: &str,
        status: &str,
        iteration: u32,
        latency_ms: f64,
    ) {
        increment_counter!("cca_meta_agent_loop_total", "phase" => phase.to_string(), "status" => status.to_string());
        gauge!("cca_meta_agent_loop_iteration", iteration as f64, "phase" => phase.to_string());
        histogram!("cca_meta_agent_loop_latency_ms", latency_ms, "phase" => phase.to_string());
    }

    pub fn record_context_assembly(
        &self,
        status: &str,
        layers_included: usize,
        tokens_used: u32,
        latency_ms: f64,
    ) {
        increment_counter!("cca_context_assembly_total", "status" => status.to_string());
        histogram!("cca_context_assembly_layers", layers_included as f64);
        histogram!("cca_context_assembly_tokens", tokens_used as f64);
        histogram!("cca_context_assembly_latency_ms", latency_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_operation() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_operation("read", "success");
        telemetry.record_operation("write", "failure");
        telemetry.record_operation("delete", "success");
    }

    #[test]
    fn test_record_violation() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_violation("team", "warn");
        telemetry.record_violation("project", "block");
        telemetry.record_violation("org", "info");
    }

    #[test]
    fn test_record_summary_generation() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_summary_generation("sentence", "success", 45, 125.5);
        telemetry.record_summary_generation("paragraph", "failure", 0, 250.0);
    }

    #[test]
    fn test_record_note_distillation() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_note_distillation("success", 5, 320.0);
        telemetry.record_note_distillation("failure", 0, 100.0);
    }

    #[test]
    fn test_record_hindsight_query() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_hindsight_query("success", 3, 45.5);
        telemetry.record_hindsight_query("failure", 0, 20.0);
    }

    #[test]
    fn test_record_meta_agent_loop() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_meta_agent_loop("build", "success", 1, 1500.0);
        telemetry.record_meta_agent_loop("test", "failure", 2, 800.0);
    }

    #[test]
    fn test_record_context_assembly() {
        let telemetry = KnowledgeTelemetry;
        telemetry.record_context_assembly("success", 3, 1200, 85.5);
        telemetry.record_context_assembly("failure", 0, 0, 10.0);
    }
}
