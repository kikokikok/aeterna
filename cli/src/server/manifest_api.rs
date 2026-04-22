//! HTTP surface for manifest render/redact (B3 tasks 2.2 + 2.3).
//!
//! Exposes one endpoint today:
//!
//! * `GET /admin/tenants/{slug}/manifest[?redact=true]`
//!   — reverse-renders the tenant's persisted state as a `TenantManifest`
//!     document. See [`super::manifest_render`] for the module-level
//!     semantics (which sections are rendered, how redaction works, why
//!     the response carries `notRendered`).
//!
//! # Auth
//!
//! `PlatformAdmin`-gated. Even in `redact=true` mode. Redaction is for
//! operators who want to share a sanitized manifest externally — not a
//! way to demote the endpoint to a lower-trust tier.
//!
//! A follow-up (§2.4 diff + scoped tokens §10) will introduce a
//! `tenant:read` scope allowing redact-only access for delegated
//! viewers. Until that lands, PA is the single gate.
//!
//! Future additions planned in this module:
//! * `POST /admin/tenants/diff` (task 2.4)
//! * `?dryRun=true` on `provision_tenant` (task 2.1) — lives in
//!   `tenant_api.rs` because it reuses the existing provision pipeline.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use mk_core::types::{Role, RoleIdentifier};
use serde::Deserialize;
use serde_json::json;

use super::manifest_render::{RenderError, render_current_manifest};
use super::{AppState, authenticated_platform_context};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/tenants/{slug}/manifest", get(get_tenant_manifest))
        .with_state(state)
}

/// Query params for `GET /admin/tenants/{slug}/manifest`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenderQuery {
    /// When `true`, every secret reference's logical name is replaced
    /// with an opaque placeholder and the repository binding's
    /// `credentialRef` is elided. Default `false`.
    #[serde(default)]
    redact: bool,
}

/// PA gate. Mirrors the pattern from `tenant_wiring_api` — the
/// `authenticated_platform_context` helper authenticates off raw
/// headers (no tenant resolution required for platform-scoped routes).
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

/// Whether the role set grants platform-admin privileges. Unit-test
/// seam identical to the one in `tenant_wiring_api`.
fn is_platform_admin(roles: &[RoleIdentifier]) -> bool {
    let pa: RoleIdentifier = Role::PlatformAdmin.into();
    roles.contains(&pa)
}

#[tracing::instrument(skip_all, fields(slug = %slug, redact = q.redact))]
async fn get_tenant_manifest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(q): Query<RenderQuery>,
) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }
    match render_current_manifest(&state, &slug, q.redact).await {
        Ok(manifest) => Json(manifest).into_response(),
        Err(RenderError::NotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_found",
            &format!("No tenant with slug '{slug}'"),
        ),
        Err(RenderError::Storage(msg)) => {
            // Storage errors could carry query text or connection
            // detail — keep a stable error code on the wire and log
            // the full message internally.
            tracing::error!(error = %msg, "manifest render storage error");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                "Failed to read tenant state",
            )
        }
    }
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": code, "message": message}))).into_response()
}

#[cfg(test)]
mod tests {
    //! Wire-contract unit tests. End-to-end HTTP integration (real
    //! `AppState`, real Postgres) is deferred to an integration-test
    //! file under `cli/tests/` in a follow-up — same split as
    //! `tenant_wiring_api`.

    use super::*;

    #[test]
    fn render_query_defaults_redact_to_false() {
        // `?redact` omitted from the URL must mean full (un-redacted)
        // rendering. `axum::extract::Query` uses `serde` with `#[serde(default)]`,
        // so `RenderQuery::default()` reflects the "no param" path.
        let q = RenderQuery::default();
        assert!(!q.redact);
    }

    #[test]
    fn render_query_roundtrips_through_json() {
        // axum's `Query` extractor is driven by serde; locking the
        // serde shape (camelCase + bool field) is sufficient to lock
        // the wire shape. We exercise it through `serde_json` because
        // `serde_urlencoded` is not a dev-dep of this crate.
        let v = serde_json::json!({"redact": true});
        let q: RenderQuery = serde_json::from_value(v).unwrap();
        assert!(q.redact);
    }

    #[test]
    fn is_platform_admin_accepts_pa_role() {
        let pa: RoleIdentifier = Role::PlatformAdmin.into();
        assert!(is_platform_admin(&[pa]));
    }

    #[test]
    fn is_platform_admin_rejects_non_pa_roles() {
        let viewer: RoleIdentifier = Role::Viewer.into();
        assert!(!is_platform_admin(&[viewer]));
    }

    #[test]
    fn is_platform_admin_rejects_empty_roles() {
        assert!(!is_platform_admin(&[]));
    }

    #[test]
    fn error_response_preserves_status() {
        let resp = error_response(StatusCode::FORBIDDEN, "forbidden", "no");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
