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
    InternalError(String)
}

impl MemoryError {
    pub fn is_retryable(&self) -> bool {
        match self {
            MemoryError::NetworkError(_)
            | MemoryError::TimeoutError(_)
            | MemoryError::RateLimited(_)
            | MemoryError::ProviderError(_) => true,
            _ => false
        }
    }

    pub fn should_backoff(&self) -> bool {
        match self {
            MemoryError::RateLimited(_) => true,
            _ => false
        }
    }

    pub fn backoff_duration(&self) -> Option<std::time::Duration> {
        match self {
            MemoryError::RateLimited(_) => Some(std::time::Duration::from_secs(5)),
            MemoryError::NetworkError(_) => Some(std::time::Duration::from_secs(1)),
            _ => None
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
    pub jitter: bool
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: std::time::Duration::from_millis(100),
            max_backoff: std::time::Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true
        }
    }
}

pub async fn with_retry<F, Fut, T>(operation: F, config: RetryConfig) -> MemoryResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = MemoryResult<T>>
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
                            (actual_backoff.as_millis() as f32 * jitter) as u64
                        );
                    }

                    tokio::time::sleep(actual_backoff).await;

                    backoff = std::time::Duration::from_millis(
                        (backoff.as_millis() as f32 * config.backoff_multiplier) as u64
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
    max_retries: usize
) -> MemoryResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = MemoryResult<T>>
{
    with_retry(
        operation,
        RetryConfig {
            max_retries,
            ..Default::default()
        }
    )
    .await
}

pub struct CircuitBreaker {
    state: std::sync::Arc<tokio::sync::RwLock<CircuitState>>,
    failure_threshold: usize,
    reset_timeout: std::time::Duration,
    _half_open_timeout: std::time::Duration
}

enum CircuitState {
    Closed { failure_count: usize },
    Open { opened_at: std::time::Instant },
    HalfOpen
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout: std::time::Duration) -> Self {
        Self {
            state: std::sync::Arc::new(tokio::sync::RwLock::new(CircuitState::Closed {
                failure_count: 0
            })),
            failure_threshold,
            reset_timeout,
            _half_open_timeout: reset_timeout / 2
        }
    }

    pub async fn execute<F, Fut, T>(&self, operation: F) -> MemoryResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = MemoryResult<T>>
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
                        "Circuit breaker is open".to_string()
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
                        opened_at: std::time::Instant::now()
                    };
                }
            }
            CircuitState::Closed {
                ref mut failure_count
            } => {
                if result.is_ok() {
                    *failure_count = 0;
                } else {
                    *failure_count += 1;
                    if *failure_count >= self.failure_threshold {
                        *state = CircuitState::Open {
                            opened_at: std::time::Instant::now()
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
            RetryConfig::default()
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
                    "Permanent failure".to_string()
                ))
            },
            RetryConfig::default()
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new(2, std::time::Duration::from_millis(100));

        let counter = AtomicUsize::new(0);

        let result1: Result<&str, _> = breaker
            .execute(|| async {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(MemoryError::NetworkError("Failure 1".to_string()))
            })
            .await;

        assert!(result1.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let result2: Result<&str, _> = breaker
            .execute(|| async {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(MemoryError::NetworkError("Failure 2".to_string()))
            })
            .await;

        assert!(result2.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        let result3: Result<&str, _> = breaker
            .execute(|| async {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok("should fail")
            })
            .await;

        assert!(result3.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let result4: Result<&str, _> = breaker
            .execute(|| async {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok("success")
            })
            .await;

        assert_eq!(result4.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
