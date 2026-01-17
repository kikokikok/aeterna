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
}
