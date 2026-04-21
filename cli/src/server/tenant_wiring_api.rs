//! Admin-only endpoints for inspecting pod-local tenant wiring state.
//!
//! B2 task 5.5. Completes the observability triangle:
//!
//! | Endpoint                                | Auth        | Reveals reasons? |
//! |-----------------------------------------|-------------|------------------|
//! | `/ready`                                | none        | no (counts only) |
//! | handler 503 via `require_available_tenant` | user auth | no               |
//! | `/api/v1/admin/tenants/.../wiring`      | PA          | **yes**          |
//!
//! Reasons can carry upstream error text, redacted-secret hints, or
//! tenant-identifying detail that must not escape to unauthenticated
//! surfaces. PlatformAdmin auth is the contract for seeing them.
//!
//! # Per-pod semantics
//!
//! The response reflects the state of the pod serving the request,
//! *not* a cluster-wide view. Operators investigating a divergence
//! (“pod A reports Available, pod B reports LoadingFailed”) should
//! hit each pod directly — usually via `kubectl port-forward` or a
//! debug service that bypasses the LB. Exposing a cluster-wide
//! aggregator would require cross-pod RPC and is out of scope for B2.
//!
//! # Endpoints
//!
//! * `GET /admin/tenants/wiring`
//!     — every tenant known to this pod, in registry-order
//!
//! * `GET /admin/tenants/{slug}/wiring`
//!     — one tenant; 404 if this pod has no entry for it (the pod
//!       hasn't wired it and no `/ready` or shield call has triggered
//!       lazy wiring)
//!
//! Both endpoints are `GET` with no side effects — they do NOT call
//! `ensure_wired`, intentionally. A status endpoint that silently
//! primes state would make it impossible to distinguish “this pod
//! has never seen this tenant” from “this pod failed to wire it”.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use mk_core::types::{Role, RoleIdentifier};
use serde::Serialize;
use serde_json::json;

use super::tenant_runtime_state::TenantRuntimeState;
use super::{AppState, authenticated_platform_context};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/tenants/wiring", get(list_wiring))
        .route("/admin/tenants/{slug}/wiring", get(get_wiring))
        .with_state(state)
}

/// Per-tenant wire envelope. `TenantRuntimeState` already uses
/// `#[serde(tag = "state", rename_all_fields = "camelCase")]`, so
/// flattening produces a flat object with `state` as the discriminator
/// and the payload fields (`reason`, `retryCount`, `lastAttemptAt`,
/// `rev`, `wiredAt`, `since`) at the top level.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WiringEnvelope {
    slug: String,
    #[serde(flatten)]
    runtime: TenantRuntimeState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WiringListResponse {
    /// In-registry iteration order. Not sorted on purpose: sort order
    /// is chosen by the client (an admin UI typically sorts by status
    /// descending; Prometheus ingestion cares about labels, not order).
    tenants: Vec<WiringEnvelope>,
    total: usize,
}

/// PA gate. Kept local rather than reusing `context::require_platform_admin`
/// because this endpoint authenticates off raw headers (no full
/// `RequestContext` — there’s no tenant to resolve), which matches the
/// admin_sync.rs pattern for platform-scoped admin routes.
async fn require_platform_admin(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let (_uid, roles) = authenticated_platform_context(state, headers).await?;
    if !is_platform_admin(&roles) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        ));
    }
    Ok(())
}

/// Whether the role set grants platform-admin privileges.
/// Split out so unit tests can exercise the predicate directly without
/// spinning an `AppState`.
fn is_platform_admin(roles: &[RoleIdentifier]) -> bool {
    let pa: RoleIdentifier = Role::PlatformAdmin.into();
    roles.contains(&pa)
}

#[tracing::instrument(skip_all, fields(slug = %slug))]
async fn get_wiring(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }
    match state.tenant_runtime_state.get(&slug).await {
        Some(runtime) => Json(WiringEnvelope { slug, runtime }).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_wired",
            &format!(
                "No wiring state on this pod for tenant '{slug}'. The pod may \
                 not have touched this tenant yet; a handler request to the \
                 tenant (or a /ready scrape after lazy wire) will populate it."
            ),
        ),
    }
}

#[tracing::instrument(skip_all)]
async fn list_wiring(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }
    // `snapshot()` is a `RwLock::read` clone of the inner map. O(N) in
    // the number of tenants on this pod; single-digit thousands is well
    // inside the admin endpoint budget, and we accept tearing-free point-
    // in-time semantics. If the map ever grows beyond that, we paginate.
    let snapshot = state.tenant_runtime_state.snapshot().await;
    let total = snapshot.len();
    let tenants = snapshot
        .into_iter()
        .map(|(slug, runtime)| WiringEnvelope { slug, runtime })
        .collect();
    Json(WiringListResponse { tenants, total }).into_response()
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": code, "message": message}))).into_response()
}

#[cfg(test)]
mod tests {
    //! End-to-end HTTP tests for this router live in
    //! `cli/tests/tenant_wiring_api_integration_test.rs` (follow-up) where
    //! a real `AppState` is available. The unit tests here lock the wire
    //! contract that the integration layer would otherwise duplicate.

    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn envelope_flattens_available_state() {
        let env = WiringEnvelope {
            slug: "acme".to_string(),
            runtime: TenantRuntimeState::Available {
                rev: 7,
                wired_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            },
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["slug"], "acme");
        assert_eq!(v["state"], "available");
        assert_eq!(v["rev"], 7);
        // `wired_at` field is camelCased by `rename_all_fields` on the
        // enum and serialized as epoch seconds by `serde_epoch`.
        assert_eq!(v["wiredAt"], 1_700_000_000);
    }

    #[test]
    fn envelope_flattens_failed_state_including_reason() {
        // Reason IS exposed here by design — the PA gate is the
        // authorisation boundary. If PA auth is ever bypassed (bug),
        // this test is the canary: it locks the wire to carry the
        // reason so a broken gate shows up as a visible regression
        // elsewhere, not a silent information leak.
        let env = WiringEnvelope {
            slug: "acme".to_string(),
            runtime: TenantRuntimeState::LoadingFailed {
                reason: "secret openai.api_key missing".to_string(),
                last_attempt_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                retry_count: 3,
            },
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["state"], "loadingFailed");
        assert_eq!(v["reason"], "secret openai.api_key missing");
        assert_eq!(v["retryCount"], 3);
        assert_eq!(v["lastAttemptAt"], 1_700_000_000);
    }

    #[test]
    fn list_response_includes_total() {
        let resp = WiringListResponse {
            tenants: vec![
                WiringEnvelope {
                    slug: "a".into(),
                    runtime: TenantRuntimeState::available_now(1),
                },
                WiringEnvelope {
                    slug: "b".into(),
                    runtime: TenantRuntimeState::loading_now(),
                },
            ],
            total: 2,
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["total"], 2);
        assert_eq!(v["tenants"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn is_platform_admin_accepts_pa_role() {
        let pa: RoleIdentifier = Role::PlatformAdmin.into();
        assert!(is_platform_admin(&[pa]));
    }

    #[test]
    fn is_platform_admin_rejects_other_roles() {
        // Viewer is the lowest-privilege non-PA role; if this test
        // passes for Viewer it passes a fortiori for every other
        // non-PA role (Developer, TechLead, Architect, Admin,
        // TenantAdmin, Agent).
        let viewer: RoleIdentifier = Role::Viewer.into();
        assert!(!is_platform_admin(&[viewer]));
    }

    #[test]
    fn is_platform_admin_rejects_empty_roles() {
        assert!(!is_platform_admin(&[]));
    }

    #[test]
    fn error_response_shape_is_stable() {
        // Admin UIs key on the `error` field; don’t let a drive-by
        // refactor change it to `code` or `type`.
        let resp = error_response(StatusCode::FORBIDDEN, "forbidden", "no");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
