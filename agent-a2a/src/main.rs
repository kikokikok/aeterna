use axum::{
    Router,
    routing::{get, post}
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{Level, info};

use agent_a2a::{
    Config,
    auth::{AuthState, auth_middleware, tenant_context_middleware}
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting A2A Agent Server");

    let config = Config::from_env().unwrap_or_default();
    info!("Configuration loaded");

    let auth_state = Arc::new(AuthState {
        enabled: config.auth.enabled,
        api_key: config.auth.api_key.clone()
    });

    let app = create_router(auth_state);

    let addr = config.socket_addr()?;
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn create_router(auth_state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/.well-known/agent.json", get(agent_card_handler))
        .route("/tasks/send", post(tasks_send_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware
        ))
        .layer(axum::middleware::from_fn(tenant_context_middleware))
}

async fn metrics_handler() -> axum::response::Response<axum::body::Body> {
    let metrics_text = format!(
        "# HELP a2a_requests_total Total A2A requests\n# TYPE a2a_requests_total \
         counter\na2a_requests_total{{skill=\"memory\"}} 0\n\n# HELP a2a_active_connections \
         Active connections\n# TYPE a2a_active_connections gauge\na2a_active_connections 0\n"
    );

    axum::response::Response::builder()
        .header("Content-Type", "text/plain")
        .body(axum::body::Body::from(metrics_text))
        .unwrap()
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn agent_card_handler() -> String {
    serde_json::json!({
        "name": "Aeterna A2A Agent",
        "version": env!("CARGO_PKG_VERSION"),
        "skills": [
            {
                "name": "memory",
                "description": "Manage ephemeral memories",
                "tools": ["memory_add", "memory_search", "memory_delete"]
            },
            {
                "name": "knowledge",
                "description": "Query knowledge base",
                "tools": ["knowledge_query", "knowledge_show", "knowledge_check"]
            },
            {
                "name": "governance",
                "description": "Validate policies and check drift",
                "tools": ["governance_validate", "governance_drift_check"]
            }
        ]
    })
    .to_string()
}

async fn tasks_send_handler() -> String {
    serde_json::json!({
        "status": "completed",
        "result": {}
    })
    .to_string()
}
