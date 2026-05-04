use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::{AppState, authenticated_platform_context};

#[derive(Deserialize)]
struct DigestQuery {
    tenant_id: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/internal/graph/digest", get(handle_digest))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn handle_digest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<DigestQuery>,
) -> impl IntoResponse {
    let (_user_id, roles) = match authenticated_platform_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    if !roles
        .iter()
        .any(|r| *r == mk_core::types::Role::PlatformAdmin.into())
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "forbidden",
                "message": "PlatformAdmin role required"
            })),
        )
            .into_response();
    }

    let graph_store = match state.graph_store.as_ref() {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "graph_store_unavailable",
                    "message": "Graph store is not configured"
                })),
            )
                .into_response();
        }
    };

    let conn = graph_store.db_handle();
    let conn = conn.lock();
    match storage::graph_verify::compute_digest_hex(&conn, &params.tenant_id) {
        Ok(digest) => (
            StatusCode::OK,
            Json(json!({
                "tenant_id": params.tenant_id,
                "digest": digest
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "digest_failed",
                "message": e.to_string()
            })),
        )
            .into_response(),
    }
}
