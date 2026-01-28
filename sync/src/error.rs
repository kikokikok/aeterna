use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Governance violation: {0}")]
    GovernanceBlock(String),
    #[error("Knowledge repository error: {0}")]
    Repository(#[from] knowledge::repository::RepositoryError),
    #[error("Knowledge manager error: {0}")]
    KnowledgeManager(#[from] knowledge::manager::KnowledgeManagerError),
    #[error("Governance internal error: {0}")]
    Governance(#[from] knowledge::governance::GovernanceError),
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
    #[error("Storage error: {0}")]
    Storage(#[from] errors::StorageError),
    #[error("Distributed lock error: {0}")]
    DistributedLock(#[from] distributed_lock::LockError),
    #[error("Other error: {0}")]
    Other(String)
}

impl From<Box<dyn std::error::Error + Send + Sync>> for SyncError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        SyncError::Other(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SyncError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_error_from_boxed_error() {
        let boxed_err: Box<dyn std::error::Error + Send + Sync> =
            Box::new(std::io::Error::other("test error"));
        let sync_err: SyncError = boxed_err.into();

        match sync_err {
            SyncError::Other(msg) => assert!(msg.contains("test error")),
            _ => panic!("Expected SyncError::Other")
        }
    }

    #[test]
    fn test_sync_error_display() {
        let errors = vec![
            (
                SyncError::GovernanceBlock("policy violated".to_string()),
                "Governance violation: policy violated"
            ),
            (
                SyncError::ConflictDetection("hash mismatch".to_string()),
                "Conflict detection failed: hash mismatch"
            ),
            (
                SyncError::Persistence("disk full".to_string()),
                "State persistence failed: disk full"
            ),
            (
                SyncError::Internal("unexpected".to_string()),
                "Internal error: unexpected"
            ),
            (
                SyncError::Other("unknown".to_string()),
                "Other error: unknown"
            ),
        ];

        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected);
        }
    }
}
