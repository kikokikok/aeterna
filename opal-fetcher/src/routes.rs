//! Route definitions for the OPAL Data Fetcher.

use axum::{Router, routing::get};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer
};

use crate::handlers;
use crate::state::AppState;

/// Creates the Axum router with all routes configured.
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS configuration - allow any origin for OPAL clients
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API v1 routes
    let api_v1 = Router::new()
        .route("/hierarchy", get(handlers::get_hierarchy))
        .route("/users", get(handlers::get_users))
        .route("/agents", get(handlers::get_agents))
        .route("/all", get(handlers::get_all_entities));

    // Root router
    Router::new()
        .route("/health", get(handlers::health))
        .route("/metrics", get(handlers::metrics))
        .nest("/v1", api_v1)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use crate::state::FetcherConfig;

    #[test]
    fn test_router_construction() {
        let config = FetcherConfig {
            database_url: "postgres://localhost/test".to_string(),
            ..Default::default()
        };
        assert!(!config.database_url.is_empty());
    }
}
