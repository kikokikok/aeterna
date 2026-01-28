use metrics::counter;

use super::MetaAgentTelemetry;

pub struct MetaAgentTelemetrySink;

impl MetaAgentTelemetrySink {
    pub fn record(&self, telemetry: &MetaAgentTelemetry) {
        let status = if telemetry.success {
            "success"
        } else {
            "failure"
        };
        counter!("meta_agent_loops_total", "status" => status.to_string()).increment(1);
        counter!("meta_agent_iterations_total", "count" => telemetry.iterations.to_string())
            .increment(1);
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
