use thiserror::Error;

pub type IdpSyncResult<T> = Result<T, IdpSyncError>;

#[derive(Debug, Error)]
pub enum IdpSyncError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("IdP API error: {status} - {message}")]
    IdpApiError { status: u16, message: String },

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Group not found: {0}")]
    GroupNotFound(String),

    #[error("Invalid webhook payload: {0}")]
    InvalidWebhookPayload(String),

    #[error("Rate limited: retry after {retry_after_seconds}s")]
    RateLimited { retry_after_seconds: u64 },

    #[error("Sync conflict: {0}")]
    SyncConflict(String),

    #[error("OAuth error: {0}")]
    OAuthError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Scheduler error: {0}")]
    SchedulerError(String)
}

impl IdpSyncError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::HttpError(_) | Self::RateLimited { .. } | Self::DatabaseError(_)
        )
    }

    pub fn retry_after(&self) -> Option<u64> {
        if let Self::RateLimited {
            retry_after_seconds
        } = self
        {
            Some(*retry_after_seconds)
        } else {
            None
        }
    }
}
