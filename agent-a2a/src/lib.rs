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

pub use auth::{AuthState, TenantContext};
pub use config::Config;
pub use errors::{A2AError, A2AResult};

#[cfg(test)]
pub mod test_helpers {
    use crate::Config;
    use axum::{
        Router,
        routing::{get, post}
    };
    use std::sync::Arc;
    use tower_http::cors::CorsLayer;
    use tower_http::trace::TraceLayer;

    pub fn create_test_router() -> Router<Arc<Config>> {
        Router::new()
            .route("/health", get(crate::routes::health_handler))
            .route("/metrics", get(crate::routes::metrics_handler))
            .route("/.well-known/agent.json", get(agent_card_handler))
            .route("/tasks/send", post(tasks_send_handler))
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
    }

    async fn agent_card_handler() -> String {
        serde_json::json!({
            "name": "Aeterna A2A Agent",
            "version": "0.1.0",
            "skills": [
                {"name": "memory", "description": "Manage ephemeral memories"},
                {"name": "knowledge", "description": "Query knowledge base"},
                {"name": "governance", "description": "Validate policies"}
            ]
        })
        .to_string()
    }

    async fn tasks_send_handler() -> String {
        serde_json::json!({"status": "completed"}).to_string()
    }
}
