//! Admin endpoint for bootstrap introspection (B2 task 6.1).
//!
//! `GET /api/v1/admin/bootstrap/status` — returns the in-memory
//! [`BootstrapStatus`] snapshot produced by
//! [`crate::server::bootstrap_tracker::BootstrapTracker`].
//!
//! # Auth
//!
//! PlatformAdmin-only. The snapshot can contain upstream error strings
//! (DB connect failures, K8s-API detail, etc.); exposing those behind
//! PA matches the precedent set by [`admin_sync`] and the tenant wiring
//! admin endpoints.
//!
//! # Scope
//!
//! Per-pod. In HA deployments each replica holds its own tracker; ops
//! investigating a rolling-deploy regression must query each pod
//! directly (via `kubectl port-forward` or a Service that targets a
//! specific pod). A cluster-wide aggregator is explicitly out of scope
//! for this task — Prometheus scrapes of bootstrap durations would be
//! the right tool for fleet-level views.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use super::{AppState, authenticated_platform_context};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/bootstrap/status", get(handle_status))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn handle_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
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

    let snapshot = state.bootstrap_tracker.snapshot();
    (StatusCode::OK, Json(snapshot)).into_response()
}
