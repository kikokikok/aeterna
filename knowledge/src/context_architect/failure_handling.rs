use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

// Removed unused 'backoff' crate import to avoid conflicts with tokio_retry
use mk_core::types::LayerSummary;
use tracing::warn;

// Assuming these exist in your project structure

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    pub success_threshold: usize,
    pub timeout_secs: u64,
    pub half_open_max_calls: usize
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout_secs: 60,
            half_open_max_calls: 3
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
    pub jitter_factor: f64
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            multiplier: 2.0,
            jitter_factor: 0.1
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailureMetrics {
    failures_total: Arc<AtomicU64>,
    retries_total: Arc<AtomicU64>,
    circuit_trips: Arc<AtomicU64>,
    consecutive_failures: Arc<AtomicUsize>,
    cached_fallbacks: Arc<AtomicU64>,
    raw_content_fallbacks: Arc<AtomicU64>,
    fallback_model_uses: Arc<AtomicU64>
}

impl FailureMetrics {
    pub fn new() -> Self {
        Self {
            failures_total: Arc::new(AtomicU64::new(0)),
            retries_total: Arc::new(AtomicU64::new(0)),
            circuit_trips: Arc::new(AtomicU64::new(0)),
            consecutive_failures: Arc::new(AtomicUsize::new(0)),
            cached_fallbacks: Arc::new(AtomicU64::new(0)),
            raw_content_fallbacks: Arc::new(AtomicU64::new(0)),
            fallback_model_uses: Arc::new(AtomicU64::new(0))
        }
    }

    pub fn record_failure(&self) {
        self.failures_total.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_retry(&self) {
        self.retries_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_circuit_trip(&self) {
        self.circuit_trips.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    pub fn record_cached_fallback(&self) {
        self.cached_fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_raw_content_fallback(&self) {
        self.raw_content_fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_fallback_model_use(&self) {
        self.fallback_model_uses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn failures_total(&self) -> u64 {
        self.failures_total.load(Ordering::Relaxed)
    }

    pub fn retries_total(&self) -> u64 {
        self.retries_total.load(Ordering::Relaxed)
    }

    pub fn circuit_trips(&self) -> u64 {
        self.circuit_trips.load(Ordering::Relaxed)
    }

    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    pub fn cached_fallbacks(&self) -> u64 {
        self.cached_fallbacks.load(Ordering::Relaxed)
    }

    pub fn raw_content_fallbacks(&self) -> u64 {
        self.raw_content_fallbacks.load(Ordering::Relaxed)
    }

    pub fn fallback_model_uses(&self) -> u64 {
        self.fallback_model_uses.load(Ordering::Relaxed)
    }
}

impl Default for FailureMetrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CircuitBreaker {
    state: Arc<std::sync::Mutex<CircuitState>>,
    config: CircuitBreakerConfig,
    failure_count: Arc<AtomicUsize>,
    success_count: Arc<AtomicUsize>,
    opened_at: Arc<std::sync::Mutex<Option<Instant>>>,
    metrics: Arc<FailureMetrics>,
    half_open_calls: Arc<AtomicUsize>
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig, metrics: Arc<FailureMetrics>) -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(CircuitState::Closed)),
            config,
            failure_count: Arc::new(AtomicUsize::new(0)),
            success_count: Arc::new(AtomicUsize::new(0)),
            opened_at: Arc::new(std::sync::Mutex::new(None)),
            metrics,
            half_open_calls: Arc::new(AtomicUsize::new(0))
        }
    }

    pub fn should_allow_request(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Open => {
                if let Some(opened_at) = *self.opened_at.lock().unwrap() {
                    if opened_at.elapsed() >= Duration::from_secs(self.config.timeout_secs) {
                        *state = CircuitState::HalfOpen;
                        self.half_open_calls.store(0, Ordering::Relaxed);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                let calls = self.half_open_calls.fetch_add(1, Ordering::Relaxed);
                calls < self.config.half_open_max_calls
            }
            CircuitState::Closed => true
        }
    }

    pub fn record_success(&self) {
        self.metrics.record_success();
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                }
            }
            CircuitState::Closed => {
                self.success_count.fetch_add(1, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        self.metrics.record_failure();
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed | CircuitState::HalfOpen => {
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= self.config.failure_threshold {
                    *state = CircuitState::Open;
                    *self.opened_at.lock().unwrap() = Some(Instant::now());
                    self.metrics.record_circuit_trip();
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    warn!("Circuit breaker opened after {} failures", failures);
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }
}

pub struct CachedSummaryStore {
    cache: Arc<dashmap::DashMap<String, CachedEntry>>
}

#[derive(Debug, Clone)]
struct CachedEntry {
    summary: LayerSummary,
    timestamp: Instant
}

impl CachedSummaryStore {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(dashmap::DashMap::new())
        }
    }

    pub fn get(&self, key: &str, max_age_secs: u64) -> Option<LayerSummary> {
        self.cache.get(key).and_then(|entry| {
            if entry.timestamp.elapsed() < Duration::from_secs(max_age_secs) {
                Some(entry.summary.clone())
            } else {
                None
            }
        })
    }

    pub fn put(&self, key: String, summary: LayerSummary) {
        self.cache.insert(
            key,
            CachedEntry {
                summary,
                timestamp: Instant::now()
            }
        );
    }

    pub fn remove(&self, key: &str) {
        self.cache.remove(key);
    }

    pub fn clear(&self) {
        self.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

impl Default for CachedSummaryStore {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn retry_with_backoff<F, Fut, T, E>(
    retry_config: &RetryConfig,
    metrics: &Arc<FailureMetrics>,
    operation: F
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>
{
    use tokio_retry::strategy::{ExponentialBackoff, jitter};

    // Initialize the backoff strategy iterator
    let mut backoff_strategy =
        ExponentialBackoff::from_millis(retry_config.initial_delay_ms).map(|duration| {
            let capped = duration.min(Duration::from_millis(retry_config.max_delay_ms));
            // Apply jitter to the capped duration
            jitter(capped)
        });

    let mut attempt = 0;

    loop {
        attempt += 1;
        let result = operation().await;

        match result {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt > retry_config.max_retries {
                    metrics.record_failure();
                    return Err(e);
                }

                metrics.record_retry();

                // Retrieve the next delay from the strategy
                if let Some(delay) = backoff_strategy.next() {
                    tokio::time::sleep(delay).await;
                } else {
                    // Fallback if strategy runs out (unlikely with infinite ExponentialBackoff)
                    metrics.record_failure();
                    return Err(e);
                }
            }
        }
    }
}

pub fn alert_on_consecutive_failures(metrics: &FailureMetrics, threshold: usize) -> bool {
    metrics.consecutive_failures() >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_closed() {
        let config = CircuitBreakerConfig::default();
        let metrics = Arc::new(FailureMetrics::new());
        let breaker = CircuitBreaker::new(config, metrics);

        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_trips_on_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let metrics = Arc::new(FailureMetrics::new());
        let breaker = CircuitBreaker::new(config, metrics);

        assert!(breaker.should_allow_request());
        breaker.record_failure();
        assert!(breaker.should_allow_request());
        breaker.record_failure();
        assert!(breaker.should_allow_request());
        breaker.record_failure();

        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.should_allow_request());
    }

    #[test]
    fn test_circuit_breaker_recovers() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_secs: 1,
            ..Default::default()
        };
        let metrics = Arc::new(FailureMetrics::new());
        let breaker = CircuitBreaker::new(config, metrics);

        breaker.record_failure();
        breaker.record_failure();

        assert_eq!(breaker.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(1100));

        let _ = breaker.should_allow_request(); // Triggers state transition check
        let recovered_state = breaker.state();
        assert_eq!(recovered_state, CircuitState::HalfOpen);

        let can_request = breaker.should_allow_request();
        assert!(
            can_request,
            "Should allow request after timeout recovery to HalfOpen state"
        );
    }

    #[test]
    fn test_cached_summary_store() {
        use mk_core::types::SummaryDepth;
        let store = CachedSummaryStore::new();

        let summary = LayerSummary {
            depth: SummaryDepth::Sentence,
            content: "Test summary".to_string(),
            token_count: 10,
            generated_at: 1000,
            source_hash: "hash".to_string(),
            content_hash: None,
            personalized: false,
            personalization_context: None
        };

        store.put("key1".to_string(), summary.clone());

        let retrieved = store.get("key1", 3600);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "Test summary");

        let expired = store.get("key1", 0);
        assert!(expired.is_none());
    }

    #[test]
    fn test_failure_metrics() {
        let metrics = FailureMetrics::new();

        assert_eq!(metrics.failures_total(), 0);
        assert_eq!(metrics.consecutive_failures(), 0);

        metrics.record_failure();
        metrics.record_failure();
        metrics.record_retry();

        assert_eq!(metrics.failures_total(), 2);
        assert_eq!(metrics.consecutive_failures(), 2);
        assert_eq!(metrics.retries_total(), 1);

        metrics.record_success();
        assert_eq!(metrics.consecutive_failures(), 0);
    }

    #[test]
    fn test_alert_on_consecutive_failures() {
        let metrics = FailureMetrics::new();

        assert!(!alert_on_consecutive_failures(&metrics, 3));

        metrics.record_failure();
        metrics.record_failure();

        assert!(!alert_on_consecutive_failures(&metrics, 3));

        metrics.record_failure();

        assert!(alert_on_consecutive_failures(&metrics, 3));
    }
}
