use metrics::increment_counter;

use super::MetaAgentTelemetry;

pub struct MetaAgentTelemetrySink;

impl MetaAgentTelemetrySink {
    pub fn record(&self, telemetry: &MetaAgentTelemetry) {
        let status = if telemetry.success {
            "success"
        } else {
            "failure"
        };
        increment_counter!("meta_agent_loops_total", "status" => status.to_string());
        increment_counter!("meta_agent_iterations_total", "count" => telemetry.iterations.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_meta_agent_telemetry() {
        let sink = MetaAgentTelemetrySink;
        sink.record(&MetaAgentTelemetry {
            iterations: 2,
            success: true
        });
    }
}
