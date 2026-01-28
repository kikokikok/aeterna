//! # Memory-Knowledge Errors
//!
//! Comprehensive error handling for Memory-Knowledge system.
//!
//! Follows Microsoft Pragmatic Rust Guidelines:
//! - Uses `thiserror` for structured error definitions
//! - Provides `Display` and `Error` trait implementations
//! - Includes error context for debugging

use thiserror::Error;

/// Memory-specific errors
#[derive(Debug, Error)]
pub enum MemoryError {
    // FIX: Use named field {layer} instead of positional {0}
    #[error("Invalid memory layer: {layer}")]
    InvalidLayer { layer: String },

    // FIX: Use named field {identifier} instead of positional {0}
    #[error("Missing required identifier: {identifier}")]
    MissingIdentifier { identifier: String },

    // FIX: Use named field {id} instead of positional {0}
    #[error("Memory not found: {id}")]
    MemoryNotFound { id: String },

    // FIX: Use named fields {length} and {max} instead of positional {0} and {1}
    #[error("Content too long: {length} characters max {max}")]
    ContentTooLong { length: usize, max: usize },

    // FIX: Use named fields {length} and {max} instead of positional {0} and {1}
    #[error("Query too long: {length} characters max {max}")]
    QueryTooLong { length: usize, max: usize },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("Embedding generation failed: {reason}")]
    EmbeddingFailed { reason: String },

    // FIX: Use named fields {source_name} and {reason} to match the struct
    #[error("Provider error: {source_name} - {reason}")]
    ProviderError { source_name: String, reason: String },

    // FIX: Use named field {retry_after} instead of positional {0}
    #[error("Rate limited: retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("Unauthorized access: {reason}")]
    Unauthorized { reason: String },

    // FIX: Use named field {message} instead of positional {0}
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String }
}

/// Knowledge repository errors
#[derive(Debug, Error)]
pub enum KnowledgeError {
    // FIX: Use named field {id} instead of positional {0}
    #[error("Knowledge item not found: {id}")]
    ItemNotFound { id: String },

    // FIX: Use named field {type_} instead of positional {0}
    #[error("Invalid knowledge type: {type_}")]
    InvalidType { type_: String },

    // FIX: Use named field {layer} instead of positional {0}
    #[error("Invalid knowledge layer: {layer}")]
    InvalidLayer { layer: String },

    // FIX: Use named fields {from} and {to} instead of positional {0} and {1}
    #[error("Invalid status transition: {from} to {to}")]
    InvalidStatusTransition { from: String, to: String },

    // FIX: Use named fields {operation} and {reason} instead of positional {0} and {1}
    #[error("Git operation: {operation} failed: {reason}")]
    GitError { operation: String, reason: String },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("Manifest corrupted: {reason}")]
    ManifestCorrupted { reason: String },

    // FIX: Use named field {constraint_id} instead of positional {0}
    #[error("Constraint violation: {constraint_id}")]
    ConstraintViolation { constraint_id: String }
}

/// Sync bridge errors
#[derive(Debug, Error)]
pub enum SyncError {
    // FIX: Use named field {reason} instead of positional {0}
    #[error("Knowledge unavailable: {reason}")]
    KnowledgeUnavailable { reason: String },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("Memory unavailable: {reason}")]
    MemoryUnavailable { reason: String },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("State corrupted: {reason}")]
    StateCorrupted { reason: String },

    // FIX: Use named field {checkpoint_id} instead of positional {0}
    #[error("Checkpoint failed: {checkpoint_id}")]
    CheckpointFailed { checkpoint_id: String },

    // FIX: Use named fields {checkpoint_id} and {reason} instead of positional {0} and {1}
    #[error("Rollback of {checkpoint_id} failed: {reason}")]
    RollbackFailed {
        checkpoint_id: String,
        reason: String
    },

    // FIX: Use named field {conflict_id} instead of positional {0}
    #[error("Conflict unresolvable: {conflict_id}")]
    ConflictUnresolvable { conflict_id: String },

    // FIX: Use named field {failed_items} instead of positional {0}
    #[error("Partial failure: {failed_items:?} items failed")]
    PartialFailure { failed_items: Vec<String> }
}

/// Tool interface errors
#[derive(Debug, Error)]
pub enum ToolError {
    // FIX: Use named fields {field} and {reason} instead of positional {0}
    #[error("Invalid input: {field} reason: {reason}")]
    InvalidInput { field: String, reason: String },

    // FIX: Use named fields {resource} and {id} instead of positional {0} and {1}
    #[error("Resource not found: {resource}:{id}")]
    NotFound { resource: String, id: String },

    // FIX: Use named fields {source_name} and {reason} to match the struct
    #[error("Provider error: {source_name} - {reason}")]
    ProviderError { source_name: String, reason: String },

    // FIX: Use named field {retry_after} instead of positional {0}
    #[error("Rate limited: retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    // FIX: Use named field {reason} instead of positional {0}
    #[error("Unauthorized access: {reason}")]
    Unauthorized { reason: String },

    // FIX: Use named field {timeout_ms} instead of positional {0}
    #[error("Timeout: operation took longer than {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    // FIX: Use named fields {conflict_id} and {details} instead of positional {0} and {1}
    #[error("Conflict: {conflict_id}: {details}")]
    Conflict {
        conflict_id: String,
        details: String
    }
}

/// Storage layer errors
#[derive(Debug, Error)]
pub enum StorageError {
    // FIX: Use named fields {backend} and {reason} instead of positional {0} and {1}
    #[error("Connection to {backend} failed: {reason}")]
    ConnectionError { backend: String, reason: String },

    // FIX: Use named fields {backend} and {reason} instead of positional {0} and {1}
    #[error("Query on {backend} failed: {reason}")]
    QueryError { backend: String, reason: String },

    // FIX: Use named fields {error_type} and {reason} instead of positional {0} and {1}
    #[error("Serialization error: {error_type} - {reason}")]
    SerializationError { error_type: String, reason: String },

    // FIX: Use named fields {backend} and {id} instead of positional {0} and {1}
    #[error("Not found on {backend}:{id}")]
    NotFound { backend: String, id: String },

    // FIX: Use named fields {backend} and {reason} instead of positional {0} and {1}
    #[error("Transaction on {backend} failed: {reason}")]
    TransactionError { backend: String, reason: String }
}
