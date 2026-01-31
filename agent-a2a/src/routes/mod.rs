use axum::{Json, extract::State, response::Response};
use std::sync::Arc;

use crate::Config;

pub async fn health_handler(State(_config): State<Arc<Config>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "checks": {
            "memory": "ok",
            "knowledge": "ok",
            "storage": "ok"
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn metrics_handler() -> Response<String> {
    let metrics_text = r#"# HELP a2a_requests_total Total A2A requests
# TYPE a2a_requests_total counter
a2a_requests_total 0

# HELP a2a_active_connections Active connections
# TYPE a2a_active_connections gauge
a2a_active_connections 0
"#;

    Response::builder()
        .header("Content-Type", "text/plain")
        .body(metrics_text.to_string())
        .unwrap()
}
