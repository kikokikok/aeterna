pub mod auth;
pub mod handlers;
pub mod types;
pub mod workspace;

pub use auth::{ApiKeyGuard, RateLimiter};
pub use workspace::{WorkspaceInfo, WorkspaceManager, collection_prefix, workspace_name};

#[derive(thiserror::Error, Debug)]
pub enum CentralIndexError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Rate limit exceeded, retry after {retry_after_secs}s")]
    RateLimitExceeded { retry_after_secs: u64 },
    #[error("Invalid webhook signature")]
    InvalidSignature,
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
