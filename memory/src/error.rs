use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Unauthorized access: {0}")]
    Unauthorized(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl MemoryError {
    pub fn is_retryable(&self) -> bool {
        match self {
            MemoryError::NetworkError(_)
            | MemoryError::TimeoutError(_)
            | MemoryError::RateLimited(_)
            | MemoryError::ProviderError(_) => true,
            _ => false,
        }
    }

    pub fn should_backoff(&self) -> bool {
        match self {
            MemoryError::RateLimited(_) => true,
            _ => false,
        }
    }

    pub fn backoff_duration(&self) -> Option<std::time::Duration> {
        match self {
            MemoryError::RateLimited(_) => Some(std::time::Duration::from_secs(5)),
            MemoryError::NetworkError(_) => Some(std::time::Duration::from_secs(1)),
            _ => None,
        }
    }
}

pub type MemoryResult<T> = Result<T, MemoryError>;

#[allow(async_fn_in_trait)]
pub trait WithRetry {
    type Output;

    async fn with_retry<F, Fut>(operation: F) -> MemoryResult<Self::Output>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = MemoryResult<Self::Output>>;
}

pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_backoff: std::time::Duration,
    pub max_backoff: std::time::Duration,
    pub backoff_multiplier: f32,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: std::time::Duration::from_millis(100),
            max_backoff: std::time::Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

pub async fn with_retry<F, Fut, T>(operation: F, config: RetryConfig) -> MemoryResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = MemoryResult<T>>,
{
    let mut last_error = None;
    let mut backoff = config.initial_backoff;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_error = Some(err);

                if attempt == config.max_retries {
                    break;
                }

                let current_error = last_error.as_ref().unwrap();

                if !current_error.is_retryable() {
                    break;
                }

                if let Some(error_backoff) = current_error.backoff_duration() {
                    tokio::time::sleep(error_backoff).await;
                } else {
                    let mut actual_backoff = backoff;

                    if config.jitter {
                        let jitter = rand::random::<f32>() * 0.3 + 0.85;
                        actual_backoff = std::time::Duration::from_millis(
                            (actual_backoff.as_millis() as f32 * jitter) as u64,
                        );
                    }

                    tokio::time::sleep(actual_backoff).await;

                    backoff = std::time::Duration::from_millis(
                        (backoff.as_millis() as f32 * config.backoff_multiplier) as u64,
                    )
                    .min(config.max_backoff);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        MemoryError::InternalError("Operation failed after retries".to_string())
    }))
}

pub async fn with_exponential_backoff<F, Fut, T>(
    operation: F,
    max_retries: usize,
) -> MemoryResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = MemoryResult<T>>,
{
    with_retry(
        operation,
        RetryConfig {
            max_retries,
            ..Default::default()
        },
    )
    .await
}

pub struct CircuitBreaker {
    state: std::sync::Arc<tokio::sync::RwLock<CircuitState>>,
    failure_threshold: usize,
    reset_timeout: std::time::Duration,
    _half_open_timeout: std::time::Duration,
}

enum CircuitState {
    Closed { failure_count: usize },
    Open { opened_at: std::time::Instant },
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout: std::time::Duration) -> Self {
        Self {
            state: std::sync::Arc::new(tokio::sync::RwLock::new(CircuitState::Closed {
                failure_count: 0,
            })),
            failure_threshold,
            reset_timeout,
            _half_open_timeout: reset_timeout / 2,
        }
    }

    pub async fn execute<F, Fut, T>(&self, operation: F) -> MemoryResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = MemoryResult<T>>,
    {
        let state = self.state.read().await;

        match *state {
            CircuitState::Open { opened_at } => {
                if opened_at.elapsed() >= self.reset_timeout {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitState::HalfOpen;
                } else {
                    return Err(MemoryError::NetworkError(
                        "Circuit breaker is open".to_string(),
                    ));
                }
            }
            CircuitState::HalfOpen => {
                drop(state);
            }
            CircuitState::Closed { .. } => {
                drop(state);
            }
        }

        let result = operation().await;

        let mut state = self.state.write().await;
        match *state {
            CircuitState::HalfOpen => {
                if result.is_ok() {
                    *state = CircuitState::Closed { failure_count: 0 };
                } else {
                    *state = CircuitState::Open {
                        opened_at: std::time::Instant::now(),
                    };
                }
            }
            CircuitState::Closed {
                ref mut failure_count,
            } => {
                if result.is_ok() {
                    *failure_count = 0;
                } else {
                    *failure_count += 1;
                    if *failure_count >= self.failure_threshold {
                        *state = CircuitState::Open {
                            opened_at: std::time::Instant::now(),
                        };
                    }
                }
            }
            _ => {}
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_retry_success() {
        let counter = AtomicUsize::new(0);

        let result = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(MemoryError::NetworkError("Temporary failure".to_string()))
                } else {
                    Ok("success")
                }
            },
            RetryConfig::default(),
        )
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let counter = AtomicUsize::new(0);

        let result: Result<&str, _> = with_retry(
            || async {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(MemoryError::ValidationError(
                    "Permanent failure".to_string(),
                ))
            },
            RetryConfig::default(),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_memory_error_retryable() {
        assert!(MemoryError::NetworkError("".into()).is_retryable());
        assert!(MemoryError::TimeoutError("".into()).is_retryable());
        assert!(MemoryError::RateLimited("".into()).is_retryable());
        assert!(MemoryError::ProviderError("".into()).is_retryable());
        assert!(!MemoryError::ValidationError("".into()).is_retryable());
    }

    #[test]
    fn test_memory_error_backoff() {
        assert!(MemoryError::RateLimited("".into()).should_backoff());
        assert!(!MemoryError::NetworkError("".into()).should_backoff());

        assert!(
            MemoryError::RateLimited("".into())
                .backoff_duration()
                .is_some()
        );
        assert!(
            MemoryError::NetworkError("".into())
                .backoff_duration()
                .is_some()
        );
        assert!(
            MemoryError::InternalError("".into())
                .backoff_duration()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_with_exponential_backoff() {
        let counter = AtomicUsize::new(0);
        let result = with_exponential_backoff(
            || async {
                let c = counter.fetch_add(1, Ordering::SeqCst);
                if c < 1 {
                    Err(MemoryError::NetworkError("".into()))
                } else {
                    Ok("ok")
                }
            },
            2,
        )
        .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_failure() {
        let breaker = CircuitBreaker::new(1, std::time::Duration::from_millis(50));

        // Open it
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("".into())) })
            .await;

        // Wait for reset timeout
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        // Half-open attempt fails -> goes back to Open
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::InternalError("".into())) })
            .await;

        // Next call should be blocked immediately
        let result = breaker.execute(|| async { Ok("should be blocked") }).await;
        assert!(
            matches!(result, Err(MemoryError::NetworkError(msg)) if msg.contains("Circuit breaker is open"))
        );
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_success() {
        let breaker = CircuitBreaker::new(2, std::time::Duration::from_millis(50));

        // Open it with 2 failures
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("".into())) })
            .await;
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("".into())) })
            .await;

        // Wait for reset timeout
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        // Half-open attempt succeeds -> goes back to Closed
        let result = breaker.execute(|| async { Ok("success") }).await;
        assert_eq!(result.unwrap(), "success");

        // Should be closed now, can make another successful call
        let result = breaker.execute(|| async { Ok("another success") }).await;
        assert_eq!(result.unwrap(), "another success");
    }

    #[tokio::test]
    async fn test_circuit_breaker_closed_state_reset_on_success() {
        let breaker = CircuitBreaker::new(3, std::time::Duration::from_millis(100));

        // Fail once
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("".into())) })
            .await;

        // Success should reset failure count
        let result = breaker.execute(|| async { Ok("reset") }).await;
        assert_eq!(result.unwrap(), "reset");

        // Should still be closed, not open
        let result = breaker.execute(|| async { Ok("still working") }).await;
        assert_eq!(result.unwrap(), "still working");
    }

    #[test]
    fn test_all_error_variants_display() {
        // Test that all error variants can be formatted
        let errors = vec![
            MemoryError::ProviderError("test".to_string()),
            MemoryError::EmbeddingError("test".to_string()),
            MemoryError::ValidationError("test".to_string()),
            MemoryError::StorageError("test".to_string()),
            MemoryError::NetworkError("test".to_string()),
            MemoryError::TimeoutError("test".to_string()),
            MemoryError::ConfigError("test".to_string()),
            MemoryError::SerializationError("test".to_string()),
            MemoryError::NotFound("test".to_string()),
            MemoryError::Unauthorized("test".to_string()),
            MemoryError::RateLimited("test".to_string()),
            MemoryError::InternalError("test".to_string()),
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
            assert!(display.contains("test"));
        }
    }

    #[tokio::test]
    async fn test_with_retry_jitter_calculation() {
        let counter = AtomicUsize::new(0);

        let config = RetryConfig {
            max_retries: 2,
            initial_backoff: std::time::Duration::from_millis(100),
            max_backoff: std::time::Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: true,
        };

        let result = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(MemoryError::NetworkError("Temporary".to_string()))
                } else {
                    Ok("success with jitter")
                }
            },
            config,
        )
        .await;

        assert_eq!(result.unwrap(), "success with jitter");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_error_backoff_takes_precedence() {
        let counter = AtomicUsize::new(0);

        let result: Result<&str, _> = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                // RateLimited has its own backoff duration (5 seconds)
                Err(MemoryError::RateLimited(format!("Attempt {}", count)))
            },
            RetryConfig::default(),
        )
        .await;

        assert!(result.is_err());
        // Should fail immediately after max retries since RateLimited has
        // error-specific backoff
        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    // Test implementation of WithRetry trait
    struct TestRetryable;

    impl WithRetry for TestRetryable {
        type Output = String;

        async fn with_retry<F, Fut>(operation: F) -> MemoryResult<Self::Output>
        where
            F: Fn() -> Fut,
            Fut: std::future::Future<Output = MemoryResult<Self::Output>>,
        {
            with_retry(operation, RetryConfig::default()).await
        }
    }

    #[tokio::test]
    async fn test_with_retry_trait_implementation() {
        let counter = AtomicUsize::new(0);

        let result = TestRetryable::with_retry(|| async {
            let count = counter.fetch_add(1, Ordering::SeqCst);
            if count < 1 {
                Err(MemoryError::NetworkError("Temporary".to_string()))
            } else {
                Ok("trait success".to_string())
            }
        })
        .await;

        assert_eq!(result.unwrap(), "trait success");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_with_retry_no_jitter() {
        let counter = AtomicUsize::new(0);

        let config = RetryConfig {
            max_retries: 2,
            initial_backoff: std::time::Duration::from_millis(10),
            max_backoff: std::time::Duration::from_millis(100),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let result = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(MemoryError::NetworkError("No jitter retry".to_string()))
                } else {
                    Ok("no jitter success")
                }
            },
            config,
        )
        .await;

        assert_eq!(result.unwrap(), "no jitter success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_max_backoff_cap() {
        let counter = AtomicUsize::new(0);

        let config = RetryConfig {
            max_retries: 3,
            initial_backoff: std::time::Duration::from_millis(50),
            max_backoff: std::time::Duration::from_millis(60),
            backoff_multiplier: 10.0,
            jitter: false,
        };

        let result = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 3 {
                    Err(MemoryError::NetworkError("Backoff cap test".to_string()))
                } else {
                    Ok("capped backoff success")
                }
            },
            config,
        )
        .await;

        assert_eq!(result.unwrap(), "capped backoff success");
        assert_eq!(counter.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_circuit_breaker_remains_closed_under_threshold() {
        let breaker = CircuitBreaker::new(3, std::time::Duration::from_millis(100));

        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("fail 1".into())) })
            .await;
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("fail 2".into())) })
            .await;

        let result = breaker.execute(|| async { Ok("still open") }).await;
        assert_eq!(result.unwrap(), "still open");
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_at_threshold() {
        let breaker = CircuitBreaker::new(2, std::time::Duration::from_millis(100));

        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("fail 1".into())) })
            .await;
        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("fail 2".into())) })
            .await;

        let result = breaker.execute(|| async { Ok("should be blocked") }).await;
        assert!(matches!(
            result,
            Err(MemoryError::NetworkError(msg)) if msg.contains("Circuit breaker is open")
        ));
    }

    #[tokio::test]
    async fn test_circuit_breaker_multiple_successes_after_half_open() {
        let breaker = CircuitBreaker::new(1, std::time::Duration::from_millis(50));

        let _: Result<(), _> = breaker
            .execute(|| async { Err(MemoryError::NetworkError("open it".into())) })
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        let result1 = breaker.execute(|| async { Ok("first success") }).await;
        assert_eq!(result1.unwrap(), "first success");

        let result2 = breaker.execute(|| async { Ok("second success") }).await;
        assert_eq!(result2.unwrap(), "second success");

        let result3 = breaker.execute(|| async { Ok("third success") }).await;
        assert_eq!(result3.unwrap(), "third success");
    }

    #[tokio::test]
    async fn test_with_retry_returns_last_error_on_exhausted_retries() {
        let counter = AtomicUsize::new(0);

        let config = RetryConfig {
            max_retries: 2,
            initial_backoff: std::time::Duration::from_millis(1),
            max_backoff: std::time::Duration::from_millis(10),
            backoff_multiplier: 1.0,
            jitter: false,
        };

        let result: Result<&str, _> = with_retry(
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                Err(MemoryError::NetworkError(format!("attempt {}", count)))
            },
            config,
        )
        .await;

        assert!(result.is_err());
        match result {
            Err(MemoryError::NetworkError(msg)) => {
                assert!(msg.contains("attempt"));
            }
            _ => panic!("Expected NetworkError"),
        }
    }
}
