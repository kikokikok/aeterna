use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router};
use metrics::{counter, histogram};
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use super::auth_middleware::AuthenticationLayer;
use super::{
    AppState, admin_sync, backup_api, govern_api, health, knowledge_api, lifecycle_api,
    mcp_transport, org_api, plugin_auth, project_api, role_grants, sessions, sync, team_api,
    tenant_api, user_api, webhooks,
};

pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_layer = AuthenticationLayer::new(state.plugin_auth_state.clone());

    // Routes excluded from the global auth layer:
    // - Health probes: /health, /live, /ready
    // - Auth bootstrap: /api/v1/auth/plugin/{bootstrap,refresh,logout}
    let unauthenticated = Router::new()
        .merge(health::router(state.clone()))
        .nest("/api/v1", plugin_auth::router(state.clone()));

    // All other /api/v1/* routes go through the auth layer.
    let protected_api = Router::new()
        .merge(knowledge_api::router(state.clone()))
        .merge(knowledge::api::router(state.governance_dashboard.clone()))
        .merge(sessions::router(state.clone()))
        .merge(webhooks::router(state.clone()))
        .merge(admin_sync::router(state.clone()))
        .merge(tenant_api::router(state.clone()))
        .merge(org_api::router(state.clone()))
        .merge(team_api::router(state.clone()))
        .merge(project_api::router(state.clone()))
        .merge(user_api::router(state.clone()))
        .nest("/admin", role_grants::router(state.clone()))
        .merge(govern_api::router(state.clone()))
        .merge(sync::router(state.clone()))
        .merge(backup_api::router(state.clone()))
        .merge(lifecycle_api::router(state.clone()))
        .merge(plugin_auth::admin_session_router(state.clone()))
        .layer(auth_layer.clone());

    let protected_mcp =
        mcp_transport::router(state.mcp_server.clone(), state.clone()).layer(auth_layer.clone());

    let protected_openspec = knowledge_api::router(state.clone()).layer(auth_layer);

    let mut app = unauthenticated
        .nest("/api/v1", protected_api)
        .nest("/mcp", protected_mcp)
        .nest("/openspec/v1", protected_openspec)
        .nest("/ws", state.ws_server.clone().router())
        .nest(
            "/a2a",
            agent_a2a::create_router(state.a2a_config.clone(), state.a2a_auth_state.clone()),
        );

    if let (Some(idp_config), Some(idp_sync_service), Some(idp_client)) = (
        state.idp_config.clone(),
        state.idp_sync_service.clone(),
        state.idp_client.clone(),
    ) {
        app = app.nest(
            "/webhooks",
            idp_sync::webhook_router(&idp_config, idp_sync_service, idp_client),
        );
    }

    // Admin UI static asset serving (optional — skipped if dist directory does not exist).
    let admin_ui_path = std::env::var("AETERNA_ADMIN_UI_PATH")
        .unwrap_or_else(|_| "./admin-ui/dist".to_string());
    let admin_ui_dir = PathBuf::from(&admin_ui_path);
    if admin_ui_dir.is_dir() {
        let index_html = admin_ui_dir.join("index.html");
        let serve_dir = ServeDir::new(&admin_ui_dir)
            .not_found_service(ServeFile::new(&index_html));
        app = app.nest_service("/admin", serve_dir);
        tracing::info!(path = %admin_ui_path, "Admin UI serving enabled at /admin");
    } else {
        tracing::info!(path = %admin_ui_path, "Admin UI dist directory not found — /admin route not registered");
    }

    app.layer(PropagateRequestIdLayer::x_request_id())
        .layer(axum::middleware::from_fn(record_http_metrics))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback(not_found)
}

#[tracing::instrument(skip_all, fields(method, path))]
async fn record_http_metrics(request: Request, next: axum::middleware::Next) -> impl IntoResponse {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();

    counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .increment(1);
    histogram!(
        "http_request_duration_ms",
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .record(start.elapsed().as_secs_f64() * 1000.0);

    response
}

#[tracing::instrument(skip_all)]
async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "not_found",
            "message": "Route not found"
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use tower::ServiceExt;

    #[tokio::test]
    async fn fallback_returns_json_404() {
        let response = Router::new()
            .fallback(not_found)
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn request_metrics_middleware_records_metrics() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let _ = metrics::set_global_recorder(recorder);

        let app = Router::new()
            .route("/ok", axum::routing::get(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn(record_http_metrics));

        let response = app
            .oneshot(Request::builder().uri("/ok").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let rendered = handle.render();
        assert!(rendered.contains("http_requests_total"));
        assert!(rendered.contains("http_request_duration_ms"));
    }
}
