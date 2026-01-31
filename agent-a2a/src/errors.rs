use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response}
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum A2AError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String)
}

impl A2AError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            A2AError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            A2AError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            A2AError::NotFound(_) => StatusCode::NOT_FOUND,
            A2AError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            A2AError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            A2AError::SkillNotFound(_) => StatusCode::NOT_FOUND,
            A2AError::ToolExecutionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR
        }
    }

    pub fn error_code(&self) -> String {
        match self {
            A2AError::InvalidRequest(_) => "INVALID_REQUEST",
            A2AError::Unauthorized(_) => "UNAUTHORIZED",
            A2AError::NotFound(_) => "NOT_FOUND",
            A2AError::InternalError(_) => "INTERNAL_ERROR",
            A2AError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            A2AError::SkillNotFound(_) => "SKILL_NOT_FOUND",
            A2AError::ToolExecutionFailed(_) => "TOOL_EXECUTION_FAILED"
        }
        .to_string()
    }
}

impl IntoResponse for A2AError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code();
        let message = self.to_string();

        let body = Json(json!({
            "error": {
                "code": error_code,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

impl From<memory::error::MemoryError> for A2AError {
    fn from(err: memory::error::MemoryError) -> Self {
        A2AError::ToolExecutionFailed(err.to_string())
    }
}

impl From<anyhow::Error> for A2AError {
    fn from(err: anyhow::Error) -> Self {
        A2AError::InternalError(err.to_string())
    }
}

pub type A2AResult<T> = Result<T, A2AError>;
