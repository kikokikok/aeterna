use axum::{Json, extract::State, http::StatusCode, response::Response};
use std::sync::Arc;

use crate::{Config, auth::AuthState};

fn auth_ready(config: &Config) -> bool {
    AuthState {
        api_key: config.auth.api_key.clone(),
        jwt_secret: config.auth.jwt_secret.clone(),
        enabled: config.auth.enabled,
        trusted_identity: config.auth.trusted_identity.clone(),
    }
    .is_ready()
}

fn backend_ready() -> bool {
    false
}

fn tasks_ready() -> bool {
    false
}

fn readiness(config: &Config) -> bool {
    auth_ready(config) && backend_ready() && tasks_ready()
}

pub async fn health_handler(
    State(config): State<Arc<Config>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let auth_ready = auth_ready(&config);
    let backend_ready = backend_ready();
    let tasks_ready = tasks_ready();
    let ready = readiness(&config);
    let auth_status = if auth_ready {
        "ready"
    } else if config.auth.jwt_secret.is_some() {
        "unsupported"
    } else if config.auth.enabled {
        "unconfigured"
    } else {
        "disabled"
    };

    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(serde_json::json!({
            "status": if ready { "ready" } else { "degraded" },
            "ready": ready,
            "checks": {
                "auth": auth_status,
                "backend": if backend_ready { "ready" } else { "unavailable" },
                "task_execution": if tasks_ready { "ready" } else { "not_implemented" }
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    )
}

pub async fn metrics_handler(State(config): State<Arc<Config>>) -> Response<String> {
    let auth_ready = auth_ready(&config);
    let backend_ready = backend_ready();
    let tasks_ready = tasks_ready();
    let ready = readiness(&config);

    let metrics_text = format!(
        "# HELP a2a_runtime_ready Whether the A2A runtime is ready to serve requests\n\
# TYPE a2a_runtime_ready gauge\n\
a2a_runtime_ready {}\n\n\
# HELP a2a_auth_ready Whether auth configuration is ready\n\
# TYPE a2a_auth_ready gauge\n\
a2a_auth_ready {}\n\n\
# HELP a2a_backend_ready Whether backend dependencies are available\n\
# TYPE a2a_backend_ready gauge\n\
a2a_backend_ready {}\n\n\
# HELP a2a_task_execution_ready Whether task execution is implemented and ready\n\
# TYPE a2a_task_execution_ready gauge\n\
a2a_task_execution_ready {}\n\n\
# HELP a2a_requests_total Total A2A requests\n\
# TYPE a2a_requests_total counter\n\
a2a_requests_total{{skill=\"memory\"}} 0\n\n\
# HELP a2a_active_connections Active connections\n\
# TYPE a2a_active_connections gauge\n\
a2a_active_connections 0\n",
        if ready { 1 } else { 0 },
        if auth_ready { 1 } else { 0 },
        if backend_ready { 1 } else { 0 },
        if tasks_ready { 1 } else { 0 },
    );

    Response::builder()
        .header("Content-Type", "text/plain")
        .body(metrics_text.to_string())
        .unwrap()
}
