use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use metrics::{counter, histogram};
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use super::auth_middleware::AuthenticationLayer;
use super::{
    AppState, admin_sync, backup_api, bootstrap_api, govern_api, health, knowledge_api,
    lifecycle_api, mcp_transport, memory_api, org_api, plugin_auth, project_api, role_grants,
    sessions, sync, team_api, tenant_api, tenant_wiring_api, user_api, webhooks,
};

pub fn build_router(state: Arc<AppState>) -> Router {
    let auth_layer = AuthenticationLayer::new(state.plugin_auth_state.clone());

    // Routes excluded from the global auth layer:
    // - Health probes: /health, /live, /ready
    // - Auth bootstrap: /api/v1/auth/plugin/{bootstrap,refresh,logout}
    let unauthenticated = Router::new()
        .merge(health::router(state.clone()))
        .nest("/api/v1", plugin_auth::router(state.clone()))
        .nest("/api/v1", plugin_auth::web_oauth_router(state.clone()));

    // All other /api/v1/* routes go through the auth layer.
    let protected_api = Router::new()
        .merge(knowledge_api::router(state.clone()))
        .merge(knowledge::api::router(state.governance_dashboard.clone()))
        .merge(sessions::router(state.clone()))
        .merge(webhooks::router(state.clone()))
        .merge(admin_sync::router(state.clone()))
        // B2 task 6.1: PA-only bootstrap introspection. Returns the
        // per-pod phase log captured during startup.
        .merge(bootstrap_api::router(state.clone()))
        // B2 task 5.5: PA-only wiring status endpoints. Mounted
        // alongside admin_sync so the /admin/tenants/... namespace
        // stays contiguous in the router.
        .merge(tenant_wiring_api::router(state.clone()))
        .merge(tenant_api::router(state.clone()))
        .merge(org_api::router(state.clone()))
        .merge(team_api::router(state.clone()))
        .merge(project_api::router(state.clone()))
        .merge(user_api::router(state.clone()))
        .nest("/admin", role_grants::router(state.clone()))
        .merge(govern_api::router(state.clone()))
        .merge(sync::router(state.clone()))
        .merge(memory_api::router(state.clone()))
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
    //
    // SPA fallback policy (see OpenSpec change `fix-spa-fallback-status-code`, #47):
    //   - Real static assets (main.js, styles.css, favicon, ...) are served
    //     by ServeDir with their correct status codes.
    //   - Unknown /admin/* paths fall through to an index.html payload served
    //     with HTTP 200, not the historic 404. The React Router inside the SPA
    //     is authoritative for "page not found" rendering — the server has no
    //     way to know whether /admin/things/42 corresponds to a real client-
    //     side route without executing the bundle.
    //   - Serving index.html with 200 matches the behavior of every mainstream
    //     static host (Netlify, Cloudflare Pages, S3 + CloudFront "SPA mode")
    //     and fixes false positives on monitoring dashboards.
    //
    // The index.html body is read exactly once at startup and cached in an
    // Arc<Bytes> to avoid per-request disk IO.
    let admin_ui_path =
        std::env::var("AETERNA_ADMIN_UI_PATH").unwrap_or_else(|_| "./admin-ui/dist".to_string());
    let admin_ui_dir = PathBuf::from(&admin_ui_path);
    if admin_ui_dir.is_dir() {
        let index_path = admin_ui_dir.join("index.html");
        match std::fs::read(&index_path) {
            Ok(bytes) => {
                let cached: Arc<Vec<u8>> = Arc::new(bytes);
                let spa_fallback = {
                    let cached = cached.clone();
                    axum::routing::any(move || {
                        let body = cached.clone();
                        async move { spa_index_response(body) }
                    })
                };
                let serve_dir = ServeDir::new(&admin_ui_dir).fallback(spa_fallback);
                app = app.nest_service("/admin", serve_dir);
                tracing::info!(
                    path = %admin_ui_path,
                    index_bytes = cached.len(),
                    "Admin UI serving enabled at /admin (SPA fallback -> 200)"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %admin_ui_path,
                    error = %e,
                    "Admin UI dist directory present but index.html unreadable — /admin route not registered"
                );
            }
        }
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

/// Build the SPA fallback response: 200 OK, `text/html; charset=utf-8`, and
/// the cached `index.html` payload. See OpenSpec `fix-spa-fallback-status-code`.
fn spa_index_response(body: Arc<Vec<u8>>) -> Response {
    // Body::from(Vec<u8>) clones into the body; since the cache is an Arc
    // we deref-clone the inner Vec here. For a ~5-20 KiB index.html shell
    // this is negligible; we can swap to Bytes later if index.html ever
    // grows to hundreds of KiB.
    let mut resp = Response::new(Body::from((*body).clone()));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    // Do NOT cache the shell — ETag-free so clients always re-fetch to pick
    // up new hashed asset URLs referenced from within.
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// Build a minimal router that mirrors the `/admin` nest in `build_router`.
    /// Keeps the integration tests independent of full `AppState` plumbing.
    fn admin_router(admin_ui_dir: &std::path::Path) -> Router {
        let index_path = admin_ui_dir.join("index.html");
        let cached: Arc<Vec<u8>> =
            Arc::new(std::fs::read(&index_path).expect("index.html readable"));
        let spa_fallback = {
            let cached = cached.clone();
            axum::routing::any(move || {
                let body = cached.clone();
                async move { spa_index_response(body) }
            })
        };
        let serve_dir = ServeDir::new(admin_ui_dir).fallback(spa_fallback);
        Router::new()
            .nest_service("/admin", serve_dir)
            .fallback(not_found)
    }

    fn seed_admin_dist() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("index.html"),
            b"<!DOCTYPE html>\n<html><body>aeterna admin</body></html>\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.js"), b"console.log('aeterna');").unwrap();
        dir
    }

    #[tokio::test]
    async fn admin_spa_fallback_returns_200_html_for_deep_path() {
        let dir = seed_admin_dist();
        let app = admin_router(dir.path());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/nonexistent/deep/path")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(
            body.starts_with(b"<!DOCTYPE html>"),
            "SPA fallback body must start with <!DOCTYPE html>; got {:?}",
            std::str::from_utf8(&body).unwrap_or("<binary>")
        );
    }

    #[tokio::test]
    async fn admin_real_static_file_served_with_correct_content_type() {
        let dir = seed_admin_dist();
        let app = admin_router(dir.path());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/main.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let ct = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.starts_with("application/javascript") || ct.starts_with("text/javascript"),
            "main.js should be served as JavaScript, got {ct:?}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(
            body.starts_with(b"console.log"),
            "real static file must be returned, not the SPA shell"
        );
    }

    #[tokio::test]
    async fn non_admin_paths_get_default_404() {
        let dir = seed_admin_dist();
        let app = admin_router(dir.path());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent-top-level-path")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

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
