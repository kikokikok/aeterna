use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub agent_id: Option<String>
}

impl TenantContext {
    pub fn from_headers(request: &Request) -> Option<Self> {
        let headers = request.headers();

        let tenant_id = headers
            .get("x-tenant-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())?;

        let user_id = headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let agent_id = headers
            .get("x-agent-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        Some(Self {
            tenant_id,
            user_id,
            agent_id
        })
    }

    pub fn from_auth_header(auth_header: &str) -> Option<Self> {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];
            Self::from_jwt(token)
        } else {
            None
        }
    }

    fn from_jwt(_token: &str) -> Option<Self> {
        None
    }
}

#[derive(Clone)]
pub struct AuthState {
    pub api_key: Option<String>,
    pub enabled: bool
}

pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    request: Request,
    next: Next
) -> Result<Response, StatusCode> {
    if !state.enabled {
        return Ok(next.run(request).await);
    }

    if let Some(api_key) = &state.api_key {
        if let Some(auth_header) = request.headers().get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str == format!("Bearer {}", api_key) {
                    return Ok(next.run(request).await);
                }
            }
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

pub async fn tenant_context_middleware(mut request: Request, next: Next) -> Response {
    if let Some(tenant_context) = TenantContext::from_headers(&request) {
        request.extensions_mut().insert(tenant_context);
    }

    next.run(request).await
}
