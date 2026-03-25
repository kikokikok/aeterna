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
use tower_http::trace::TraceLayer;

use super::{AppState, health, mcp_transport, openspec};

pub fn build_router(state: Arc<AppState>) -> Router {
    let mut app = Router::new()
        .merge(health::router(state.clone()))
        .nest(
            "/api/v1",
            knowledge::api::router(state.governance_dashboard.clone()),
        )
        .nest("/openspec/v1", openspec::router(state.clone()))
        .nest("/mcp", mcp_transport::router(state.mcp_server.clone()))
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

    app.layer(PropagateRequestIdLayer::x_request_id())
        .layer(axum::middleware::from_fn(record_http_metrics))
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback(not_found)
}

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
