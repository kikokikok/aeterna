use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Governance violation: {0}")]
    GovernanceBlock(String),
    #[error("Knowledge repository error: {0}")]
    Repository(#[from] knowledge::repository::RepositoryError),
    #[error("Memory manager error: {0}")]
    Memory(#[from] memory::error::MemoryError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Conflict detection failed: {0}")]
    ConflictDetection(String),
    #[error("State persistence failed: {0}")]
    Persistence(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Other error: {0}")]
    Other(String)
}

impl From<Box<dyn std::error::Error + Send + Sync>> for SyncError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        SyncError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SyncError>;
