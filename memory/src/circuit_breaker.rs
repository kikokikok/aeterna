use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen
}

pub struct CircuitBreakerConfig {
    pub failure_threshold_percent: f64,
    pub window_duration_secs: u64,
    pub min_requests_in_window: u64,
    pub recovery_timeout_secs: u64,
    pub half_open_max_requests: u64
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold_percent: 5.0,
            window_duration_secs: 300,
            min_requests_in_window: 10,
            recovery_timeout_secs: 60,
            half_open_max_requests: 3
        }
    }
}

struct WindowMetrics {
    successes: u64,
    failures: u64,
    window_start: i64
}

impl WindowMetrics {
    fn new() -> Self {
        Self {
            successes: 0,
            failures: 0,
            window_start: chrono::Utc::now().timestamp()
        }
    }

    fn total(&self) -> u64 {
        self.successes + self.failures
    }

    fn failure_rate(&self) -> f64 {
        if self.total() == 0 {
            return 0.0;
        }
        (self.failures as f64 / self.total() as f64) * 100.0
    }

    fn reset(&mut self) {
        self.successes = 0;
        self.failures = 0;
        self.window_start = chrono::Utc::now().timestamp();
    }
}

pub struct ReasoningCircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    metrics: RwLock<WindowMetrics>,
    opened_at: AtomicU64,
    half_open_requests: AtomicU64,
    is_open: AtomicBool,
    telemetry: Arc<crate::telemetry::MemoryTelemetry>
}

impl ReasoningCircuitBreaker {
    pub fn new(
        config: CircuitBreakerConfig,
        telemetry: Arc<crate::telemetry::MemoryTelemetry>
    ) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            metrics: RwLock::new(WindowMetrics::new()),
            opened_at: AtomicU64::new(0),
            half_open_requests: AtomicU64::new(0),
            is_open: AtomicBool::new(false),
            telemetry
        }
    }

    pub async fn is_allowed(&self) -> bool {
        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let opened_at = self.opened_at.load(Ordering::SeqCst);
                let now = chrono::Utc::now().timestamp() as u64;
                if now >= opened_at + self.config.recovery_timeout_secs {
                    self.transition_to_half_open().await;
                    let current = self.half_open_requests.fetch_add(1, Ordering::SeqCst);
                    current < self.config.half_open_max_requests
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                let current = self.half_open_requests.fetch_add(1, Ordering::SeqCst);
                current < self.config.half_open_max_requests
            }
        }
    }

    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                self.maybe_reset_window().await;
                let mut metrics = self.metrics.write().await;
                metrics.successes += 1;
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Closed;
                self.is_open.store(false, Ordering::SeqCst);
                self.half_open_requests.store(0, Ordering::SeqCst);
                let mut metrics = self.metrics.write().await;
                metrics.reset();
                self.telemetry.record_reasoning_circuit_closed();
                tracing::info!(
                    "Reasoning circuit breaker closed after successful half-open request"
                );
            }
            CircuitState::Open => {}
        }
    }

    pub async fn record_failure(&self, error: &str) {
        let mut state = self.state.write().await;

        self.telemetry.record_reasoning_failure(error);
        tracing::warn!(error = error, "Reasoning failure recorded");

        match *state {
            CircuitState::Closed => {
                self.maybe_reset_window().await;
                let mut metrics = self.metrics.write().await;
                metrics.failures += 1;

                if metrics.total() >= self.config.min_requests_in_window
                    && metrics.failure_rate() >= self.config.failure_threshold_percent
                {
                    drop(metrics);
                    *state = CircuitState::Open;
                    self.opened_at
                        .store(chrono::Utc::now().timestamp() as u64, Ordering::SeqCst);
                    self.is_open.store(true, Ordering::SeqCst);
                    self.telemetry
                        .record_reasoning_circuit_opened(self.metrics.read().await.failure_rate());
                    tracing::error!(
                        "Reasoning circuit breaker OPENED - failure rate exceeded threshold"
                    );
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                self.opened_at
                    .store(chrono::Utc::now().timestamp() as u64, Ordering::SeqCst);
                self.half_open_requests.store(0, Ordering::SeqCst);
                self.telemetry.record_reasoning_circuit_opened(100.0);
                tracing::error!("Reasoning circuit breaker re-OPENED after half-open failure");
            }
            CircuitState::Open => {}
        }
    }

    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        if *state == CircuitState::Open {
            *state = CircuitState::HalfOpen;
            self.half_open_requests.store(0, Ordering::SeqCst);
            self.telemetry.record_reasoning_circuit_half_open();
            tracing::info!("Reasoning circuit breaker transitioned to HALF-OPEN");
        }
    }

    async fn maybe_reset_window(&self) {
        let now = chrono::Utc::now().timestamp();
        let metrics = self.metrics.read().await;
        if now - metrics.window_start >= self.config.window_duration_secs as i64 {
            drop(metrics);
            let mut metrics = self.metrics.write().await;
            if now - metrics.window_start >= self.config.window_duration_secs as i64 {
                metrics.reset();
            }
        }
    }

    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    pub fn is_open_fast(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    pub async fn get_metrics(&self) -> (u64, u64, f64) {
        let metrics = self.metrics.read().await;
        (metrics.successes, metrics.failures, metrics.failure_rate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_telemetry() -> Arc<crate::telemetry::MemoryTelemetry> {
        Arc::new(crate::telemetry::MemoryTelemetry::new())
    }

    #[tokio::test]
    async fn test_circuit_breaker_closed_allows_requests() {
        let cb = ReasoningCircuitBreaker::new(CircuitBreakerConfig::default(), test_telemetry());
        assert!(cb.is_allowed().await);
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold_percent: 50.0,
            min_requests_in_window: 4,
            window_duration_secs: 300,
            recovery_timeout_secs: 60,
            half_open_max_requests: 3
        };
        let cb = ReasoningCircuitBreaker::new(config, test_telemetry());

        cb.record_success().await;
        cb.record_success().await;
        cb.record_failure("error1").await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        cb.record_failure("error2").await;
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold_percent: 50.0,
            min_requests_in_window: 2,
            window_duration_secs: 300,
            recovery_timeout_secs: 0,
            half_open_max_requests: 3
        };
        let cb = ReasoningCircuitBreaker::new(config, test_telemetry());

        cb.record_failure("error1").await;
        cb.record_failure("error2").await;
        assert_eq!(cb.state().await, CircuitState::Open);

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert!(cb.is_allowed().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_on_half_open_success() {
        let config = CircuitBreakerConfig {
            failure_threshold_percent: 50.0,
            min_requests_in_window: 2,
            window_duration_secs: 300,
            recovery_timeout_secs: 0,
            half_open_max_requests: 3
        };
        let cb = ReasoningCircuitBreaker::new(config, test_telemetry());

        cb.record_failure("error1").await;
        cb.record_failure("error2").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        cb.is_allowed().await;

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reopens_on_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold_percent: 50.0,
            min_requests_in_window: 2,
            window_duration_secs: 300,
            recovery_timeout_secs: 0,
            half_open_max_requests: 3
        };
        let cb = ReasoningCircuitBreaker::new(config, test_telemetry());

        cb.record_failure("error1").await;
        cb.record_failure("error2").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        cb.is_allowed().await;

        cb.record_failure("error3").await;
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_limits_half_open_requests() {
        let config = CircuitBreakerConfig {
            failure_threshold_percent: 50.0,
            min_requests_in_window: 2,
            window_duration_secs: 300,
            recovery_timeout_secs: 0,
            half_open_max_requests: 2
        };
        let cb = ReasoningCircuitBreaker::new(config, test_telemetry());

        cb.record_failure("error1").await;
        cb.record_failure("error2").await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert!(cb.is_allowed().await);
        assert!(cb.is_allowed().await);
        assert!(!cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_metrics() {
        let cb = ReasoningCircuitBreaker::new(CircuitBreakerConfig::default(), test_telemetry());

        cb.record_success().await;
        cb.record_success().await;
        cb.record_failure("error").await;

        let (successes, failures, rate) = cb.get_metrics().await;
        assert_eq!(successes, 2);
        assert_eq!(failures, 1);
        assert!((rate - 33.33).abs() < 1.0);
    }

    #[test]
    fn test_is_open_fast() {
        let cb = ReasoningCircuitBreaker::new(CircuitBreakerConfig::default(), test_telemetry());
        assert!(!cb.is_open_fast());
    }
}
