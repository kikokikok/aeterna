pub mod auth;
pub mod config;
pub mod errors;
pub mod jobs;
pub mod middleware;
pub mod persistence;
pub mod routes;
pub mod sdk_abstraction;
pub mod skills;
pub mod telemetry;

use axum::{
    Json, Router,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub use auth::{AuthState, TenantContext};
pub use config::Config;
pub use errors::{A2AError, A2AResult};

pub fn create_router(config: Arc<Config>, auth_state: Arc<AuthState>) -> Router {
    Router::new()
        .route("/health", get(crate::routes::health_handler))
        .route("/metrics", get(crate::routes::metrics_handler))
        .route("/.well-known/agent.json", get(agent_card_handler))
        .route("/tasks/send", post(tasks_send_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(axum::Extension(auth_state.clone()))
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            crate::auth::auth_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::auth::tenant_context_middleware,
        ))
        .with_state(config)
}

async fn agent_card_handler() -> String {
    serde_json::json!({
        "name": "Aeterna A2A Agent",
        "version": env!("CARGO_PKG_VERSION"),
        "skills": [
            {"name": "memory", "description": "Manage ephemeral memories"},
            {"name": "knowledge", "description": "Query knowledge base"},
            {"name": "governance", "description": "Validate policies"}
        ]
    })
    .to_string()
}

async fn tasks_send_handler() -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "not_implemented",
            "message": "Task execution is not yet implemented in this version of the A2A agent"
        })),
    )
}

pub mod test_helpers {
    use crate::{AuthState, Config, create_router};
    use axum::Router;
    use std::sync::Arc;

    pub fn create_test_router() -> Router {
        create_test_router_with(
            Config::default(),
            AuthState {
                api_key: None,
                jwt_secret: None,
                enabled: false,
                trusted_identity: crate::config::TrustedIdentityConfig::default(),
            },
        )
    }

    pub fn create_test_router_with(config: Config, auth_state: AuthState) -> Router {
        create_router(Arc::new(config), Arc::new(auth_state))
    }
}
