use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloConfig {
    pub latency_p99_ms: f64,
    pub latency_p95_ms: f64,
    pub error_rate_threshold: f64,
    pub availability_target: f64,
    pub evaluation_window_secs: u64,
    pub burn_rate_short_window_secs: u64,
    pub burn_rate_long_window_secs: u64,
}

impl Default for SloConfig {
    fn default() -> Self {
        Self {
            latency_p99_ms: 500.0,
            latency_p95_ms: 200.0,
            error_rate_threshold: 0.01,
            availability_target: 0.999,
            evaluation_window_secs: 3600,
            burn_rate_short_window_secs: 300,
            burn_rate_long_window_secs: 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SloStatus {
    Healthy,
    Warning,
    Breached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloResult {
    pub name: String,
    pub status: SloStatus,
    pub current_value: f64,
    pub target_value: f64,
    pub error_budget_remaining: f64,
    pub burn_rate: f64,
    pub evaluated_at: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MetricSample {
    value: f64,
    timestamp: DateTime<Utc>,
    is_error: bool,
}

pub struct SloMonitor {
    config: SloConfig,
    samples: Arc<RwLock<HashMap<String, Vec<MetricSample>>>>,
}

impl SloMonitor {
    pub fn new(config: SloConfig) -> Self {
        Self {
            config,
            samples: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn record_latency(&self, endpoint: &str, latency_ms: f64) {
        let mut samples = self.samples.write().unwrap();
        let key = format!("latency:{}", endpoint);
        samples.entry(key).or_default().push(MetricSample {
            value: latency_ms,
            timestamp: Utc::now(),
            is_error: false,
        });
    }

    pub fn record_request(&self, endpoint: &str, is_error: bool) {
        let mut samples = self.samples.write().unwrap();
        let key = format!("requests:{}", endpoint);
        samples.entry(key).or_default().push(MetricSample {
            value: if is_error { 1.0 } else { 0.0 },
            timestamp: Utc::now(),
            is_error,
        });
    }

    pub fn record_availability(&self, service: &str, is_up: bool) {
        let mut samples = self.samples.write().unwrap();
        let key = format!("availability:{}", service);
        samples.entry(key).or_default().push(MetricSample {
            value: if is_up { 1.0 } else { 0.0 },
            timestamp: Utc::now(),
            is_error: !is_up,
        });
    }

    pub fn check_latency_slo(&self, endpoint: &str) -> SloResult {
        let samples = self.samples.read().unwrap();
        let key = format!("latency:{}", endpoint);
        let now = Utc::now();

        let window_start =
            now - chrono::Duration::seconds(self.config.evaluation_window_secs as i64);

        let window_samples: Vec<f64> = samples
            .get(&key)
            .map(|s| {
                s.iter()
                    .filter(|s| s.timestamp >= window_start)
                    .map(|s| s.value)
                    .collect()
            })
            .unwrap_or_default();

        let p99 = percentile(&window_samples, 0.99);
        let target = self.config.latency_p99_ms;

        let error_budget_remaining = if target > 0.0 {
            1.0 - (p99 / target).min(2.0)
        } else {
            1.0
        };

        let burn_rate = if target > 0.0 && !window_samples.is_empty() {
            let violations = window_samples.iter().filter(|&&v| v > target).count();
            violations as f64 / window_samples.len() as f64
        } else {
            0.0
        };

        let status = if p99 <= target * 0.8 {
            SloStatus::Healthy
        } else if p99 <= target {
            SloStatus::Warning
        } else {
            SloStatus::Breached
        };

        SloResult {
            name: format!("latency_p99:{}", endpoint),
            status,
            current_value: p99,
            target_value: target,
            error_budget_remaining,
            burn_rate,
            evaluated_at: now,
            window_start,
        }
    }

    pub fn check_error_rate_slo(&self, endpoint: &str) -> SloResult {
        let samples = self.samples.read().unwrap();
        let key = format!("requests:{}", endpoint);
        let now = Utc::now();

        let window_start =
            now - chrono::Duration::seconds(self.config.evaluation_window_secs as i64);

        let window_samples: Vec<&MetricSample> = samples
            .get(&key)
            .map(|s| s.iter().filter(|s| s.timestamp >= window_start).collect())
            .unwrap_or_default();

        let total = window_samples.len() as f64;
        let errors = window_samples.iter().filter(|s| s.is_error).count() as f64;
        let error_rate = if total > 0.0 { errors / total } else { 0.0 };

        let target = self.config.error_rate_threshold;
        let error_budget_remaining = if target > 0.0 {
            1.0 - (error_rate / target).min(2.0)
        } else {
            1.0
        };

        let burn_rate = if target > 0.0 {
            error_rate / target
        } else {
            0.0
        };

        let status = if error_rate <= target * 0.5 {
            SloStatus::Healthy
        } else if error_rate <= target {
            SloStatus::Warning
        } else {
            SloStatus::Breached
        };

        SloResult {
            name: format!("error_rate:{}", endpoint),
            status,
            current_value: error_rate,
            target_value: target,
            error_budget_remaining,
            burn_rate,
            evaluated_at: now,
            window_start,
        }
    }

    pub fn check_availability_slo(&self, service: &str) -> SloResult {
        let samples = self.samples.read().unwrap();
        let key = format!("availability:{}", service);
        let now = Utc::now();

        let window_start =
            now - chrono::Duration::seconds(self.config.evaluation_window_secs as i64);

        let window_samples: Vec<&MetricSample> = samples
            .get(&key)
            .map(|s| s.iter().filter(|s| s.timestamp >= window_start).collect())
            .unwrap_or_default();

        let total = window_samples.len() as f64;
        let up_count = window_samples.iter().filter(|s| !s.is_error).count() as f64;
        let availability = if total > 0.0 { up_count / total } else { 1.0 };

        let target = self.config.availability_target;
        let allowed_downtime = 1.0 - target;
        let actual_downtime = 1.0 - availability;
        let error_budget_remaining = if allowed_downtime > 0.0 {
            1.0 - (actual_downtime / allowed_downtime).min(2.0)
        } else if availability >= 1.0 {
            1.0
        } else {
            -1.0
        };

        let burn_rate = if allowed_downtime > 0.0 {
            actual_downtime / allowed_downtime
        } else {
            0.0
        };

        let status = if availability >= target {
            SloStatus::Healthy
        } else if availability >= target - (allowed_downtime * 0.5) {
            SloStatus::Warning
        } else {
            SloStatus::Breached
        };

        SloResult {
            name: format!("availability:{}", service),
            status,
            current_value: availability,
            target_value: target,
            error_budget_remaining,
            burn_rate,
            evaluated_at: now,
            window_start,
        }
    }

    pub fn check_all_slos(&self, endpoint: &str) -> Vec<SloResult> {
        vec![
            self.check_latency_slo(endpoint),
            self.check_error_rate_slo(endpoint),
            self.check_availability_slo(endpoint),
        ]
    }

    pub fn prune_old_samples(&self) {
        let mut samples = self.samples.write().unwrap();
        let cutoff =
            Utc::now() - chrono::Duration::seconds(self.config.evaluation_window_secs as i64 * 2);

        for samples_vec in samples.values_mut() {
            samples_vec.retain(|s| s.timestamp >= cutoff);
        }
    }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let index = (p * (sorted.len() as f64 - 1.0)).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_latency_slo() {
        let monitor = SloMonitor::new(SloConfig {
            latency_p99_ms: 500.0,
            ..Default::default()
        });

        for _ in 0..100 {
            monitor.record_latency("/api/health", 50.0);
        }

        let result = monitor.check_latency_slo("/api/health");
        assert_eq!(result.status, SloStatus::Healthy);
        assert!(result.current_value <= 500.0 * 0.8);
        assert!(result.error_budget_remaining > 0.0);
    }

    #[test]
    fn breached_latency_slo() {
        let monitor = SloMonitor::new(SloConfig {
            latency_p99_ms: 100.0,
            ..Default::default()
        });

        for _ in 0..100 {
            monitor.record_latency("/api/slow", 200.0);
        }

        let result = monitor.check_latency_slo("/api/slow");
        assert_eq!(result.status, SloStatus::Breached);
        assert!(result.current_value > 100.0);
    }

    #[test]
    fn healthy_error_rate_slo() {
        let monitor = SloMonitor::new(SloConfig {
            error_rate_threshold: 0.01,
            ..Default::default()
        });

        for _ in 0..1000 {
            monitor.record_request("/api/endpoint", false);
        }

        let result = monitor.check_error_rate_slo("/api/endpoint");
        assert_eq!(result.status, SloStatus::Healthy);
        assert_eq!(result.current_value, 0.0);
    }

    #[test]
    fn breached_error_rate_slo() {
        let monitor = SloMonitor::new(SloConfig {
            error_rate_threshold: 0.01,
            ..Default::default()
        });

        for i in 0..100 {
            monitor.record_request("/api/broken", i < 10);
        }

        let result = monitor.check_error_rate_slo("/api/broken");
        assert_eq!(result.status, SloStatus::Breached);
        assert!(result.current_value > 0.01);
    }

    #[test]
    fn healthy_availability_slo() {
        let monitor = SloMonitor::new(SloConfig {
            availability_target: 0.999,
            ..Default::default()
        });

        for _ in 0..1000 {
            monitor.record_availability("aeterna", true);
        }

        let result = monitor.check_availability_slo("aeterna");
        assert_eq!(result.status, SloStatus::Healthy);
        assert_eq!(result.current_value, 1.0);
    }

    #[test]
    fn breached_availability_slo() {
        let monitor = SloMonitor::new(SloConfig {
            availability_target: 0.999,
            ..Default::default()
        });

        for i in 0..1000 {
            monitor.record_availability("aeterna", i >= 100);
        }

        let result = monitor.check_availability_slo("aeterna");
        assert_eq!(result.status, SloStatus::Breached);
        assert!(result.current_value < 0.999);
    }

    #[test]
    fn check_all_slos_returns_three_results() {
        let monitor = SloMonitor::new(SloConfig::default());

        monitor.record_latency("/api/test", 50.0);
        monitor.record_request("/api/test", false);
        monitor.record_availability("/api/test", true);

        let results = monitor.check_all_slos("/api/test");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn empty_samples_returns_healthy() {
        let monitor = SloMonitor::new(SloConfig::default());

        let latency = monitor.check_latency_slo("/api/empty");
        assert_eq!(latency.status, SloStatus::Healthy);
        assert_eq!(latency.current_value, 0.0);

        let error_rate = monitor.check_error_rate_slo("/api/empty");
        assert_eq!(error_rate.status, SloStatus::Healthy);
        assert_eq!(error_rate.current_value, 0.0);

        let availability = monitor.check_availability_slo("/api/empty");
        assert_eq!(availability.status, SloStatus::Healthy);
        assert_eq!(availability.current_value, 1.0);
    }

    #[test]
    fn percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p99 = percentile(&values, 0.99);
        assert!(p99 >= 9.0, "p99 of 1..10 should be >= 9.0, got {}", p99);

        let p50 = percentile(&values, 0.50);
        assert!(
            (p50 - 5.0).abs() < 2.0,
            "p50 of 1..10 should be near 5.0, got {}",
            p50
        );
    }

    #[test]
    fn warning_latency_between_80_and_100_percent() {
        let monitor = SloMonitor::new(SloConfig {
            latency_p99_ms: 100.0,
            ..Default::default()
        });

        for _ in 0..100 {
            monitor.record_latency("/api/warm", 90.0);
        }

        let result = monitor.check_latency_slo("/api/warm");
        assert_eq!(result.status, SloStatus::Warning);
    }

    #[test]
    fn burn_rate_reflects_violation_ratio() {
        let monitor = SloMonitor::new(SloConfig {
            latency_p99_ms: 100.0,
            ..Default::default()
        });

        for _ in 0..50 {
            monitor.record_latency("/api/mixed", 50.0);
        }
        for _ in 0..50 {
            monitor.record_latency("/api/mixed", 200.0);
        }

        let result = monitor.check_latency_slo("/api/mixed");
        assert!(
            result.burn_rate > 0.0,
            "Burn rate should be non-zero when some requests violate SLO"
        );
        assert!(result.burn_rate <= 1.0, "Burn rate should not exceed 1.0");
    }
}
