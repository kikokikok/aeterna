use metrics::increment_counter;

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
}
