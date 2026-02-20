use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Connection failed to {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Collection/index not found: {0}")]
    CollectionNotFound(String),

    #[error("Vector dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Rate limit exceeded: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Vector not found: {0}")]
    NotFound(String),

    #[error("Tenant isolation violation: {0}")]
    TenantViolation(String),

    #[error("Backend unavailable: {0}")]
    Unavailable(String),

    #[error("Operation timeout after {0}ms")]
    Timeout(u64),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal backend error: {0}")]
    Internal(String),

    #[error("Circuit breaker open for backend: {0}")]
    CircuitOpen(String),
}

impl BackendError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BackendError::ConnectionFailed(_)
                | BackendError::RateLimited { .. }
                | BackendError::Unavailable(_)
                | BackendError::Timeout(_)
                | BackendError::CircuitOpen(_)
        )
    }

    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            BackendError::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            BackendError::Timeout(_) => Some(1000),
            BackendError::ConnectionFailed(_) | BackendError::Unavailable(_) => Some(5000),
            BackendError::CircuitOpen(_) => Some(10000),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for BackendError {
    fn from(e: serde_json::Error) -> Self {
        BackendError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        assert!(BackendError::ConnectionFailed("host".into()).is_retryable());
        assert!(
            BackendError::RateLimited {
                retry_after_ms: 1000
            }
            .is_retryable()
        );
        assert!(BackendError::Unavailable("down".into()).is_retryable());
        assert!(BackendError::Timeout(5000).is_retryable());

        assert!(!BackendError::AuthenticationFailed("bad creds".into()).is_retryable());
        assert!(!BackendError::NotFound("id".into()).is_retryable());
        assert!(!BackendError::Configuration("bad config".into()).is_retryable());
    }

    #[test]
    fn test_retry_after() {
        assert_eq!(
            BackendError::RateLimited {
                retry_after_ms: 5000
            }
            .retry_after_ms(),
            Some(5000)
        );
        assert_eq!(BackendError::Timeout(3000).retry_after_ms(), Some(1000));
        assert_eq!(
            BackendError::ConnectionFailed("host".into()).retry_after_ms(),
            Some(5000)
        );
        assert_eq!(BackendError::NotFound("id".into()).retry_after_ms(), None);
    }
}
