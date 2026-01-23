//! Error types for the OPAL Data Fetcher.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// Result type alias for the fetcher.
pub type Result<T> = std::result::Result<T, FetcherError>;

/// Errors that can occur in the OPAL Data Fetcher.
#[derive(Error, Debug)]
pub enum FetcherError {
    /// Database connection or query error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Entity transformation error.
    #[error("Entity transformation error: {0}")]
    EntityTransform(String),

    /// Cedar policy error.
    #[error("Cedar policy error: {0}")]
    Cedar(String),

    /// Server startup error.
    #[error("Server error: {0}")]
    Server(String),

    /// Listener (PostgreSQL NOTIFY) error.
    #[error("Listener error: {0}")]
    Listener(String),

    /// JSON serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal error for unexpected conditions.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Error response body for HTTP endpoints.
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl IntoResponse for FetcherError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match &self {
            Self::Database(e) => {
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    "A database error occurred",
                    None,
                )
            }
            Self::Configuration(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIGURATION_ERROR",
                msg.as_str(),
                None,
            ),
            Self::EntityTransform(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ENTITY_TRANSFORM_ERROR",
                msg.as_str(),
                None,
            ),
            Self::Cedar(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CEDAR_ERROR",
                msg.as_str(),
                None,
            ),
            Self::Server(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "SERVER_ERROR",
                msg.as_str(),
                None,
            ),
            Self::Listener(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "LISTENER_ERROR",
                msg.as_str(),
                None,
            ),
            Self::Serialization(e) => {
                tracing::error!(error = %e, "Serialization error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "SERIALIZATION_ERROR",
                    "Failed to serialize response",
                    Some(e.to_string()),
                )
            }
            Self::Internal(msg) => {
                tracing::error!(message = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An internal error occurred",
                    Some(msg.clone()),
                )
            }
        };

        let body = ErrorResponse {
            error: message.to_string(),
            code: code.to_string(),
            details,
        };

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_display() {
        // We can't easily create a sqlx::Error, so test other variants
        let err = FetcherError::Configuration("test config error".to_string());
        assert_eq!(err.to_string(), "Configuration error: test config error");
    }

    #[test]
    fn test_entity_transform_error_display() {
        let err = FetcherError::EntityTransform("invalid entity".to_string());
        assert_eq!(
            err.to_string(),
            "Entity transformation error: invalid entity"
        );
    }

    #[test]
    fn test_cedar_error_display() {
        let err = FetcherError::Cedar("policy parse failed".to_string());
        assert_eq!(err.to_string(), "Cedar policy error: policy parse failed");
    }

    #[test]
    fn test_internal_error_display() {
        let err = FetcherError::Internal("unexpected state".to_string());
        assert_eq!(err.to_string(), "Internal error: unexpected state");
    }

    #[test]
    fn test_error_response_serialization() {
        let resp = ErrorResponse {
            error: "test error".to_string(),
            code: "TEST_ERROR".to_string(),
            details: Some("additional info".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("test error"));
        assert!(json.contains("TEST_ERROR"));
        assert!(json.contains("additional info"));
    }

    #[test]
    fn test_error_response_without_details() {
        let resp = ErrorResponse {
            error: "test error".to_string(),
            code: "TEST_ERROR".to_string(),
            details: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("details"));
    }
}
