use metrics::{counter, gauge, histogram};
use std::time::Instant;

pub struct Telemetry;

impl Telemetry {
    pub fn record_request(skill: &str, tool: &str) {
        counter!("a2a_requests_total", "skill" => skill.to_string(), "tool" => tool.to_string())
            .increment(1);
    }

    pub fn record_error(skill: &str, error_type: &str) {
        counter!("a2a_errors_total", "skill" => skill.to_string(), "type" => error_type.to_string()).increment(1);
    }

    pub fn record_latency(skill: &str, duration_ms: f64) {
        histogram!("a2a_request_duration_ms", "skill" => skill.to_string()).record(duration_ms);
    }

    pub fn set_active_connections(count: usize) {
        gauge!("a2a_active_connections").set(count as f64);
    }
}

pub struct RequestTimer {
    start: Instant,
    skill: String
}

impl RequestTimer {
    pub fn new(skill: &str) -> Self {
        Self {
            start: Instant::now(),
            skill: skill.to_string()
        }
    }

    pub fn finish(self) {
        let duration = self.start.elapsed().as_millis() as f64;
        Telemetry::record_latency(&self.skill, duration);
    }
}
