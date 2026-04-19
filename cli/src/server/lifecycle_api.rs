//! REST API for remediation requests and lifecycle operations.
//!
//! All endpoints require `PlatformAdmin` role. Provides listing, approval,
//! rejection, and status inspection for the human-in-the-loop remediation
//! workflow.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use storage::dead_letter::DeadLetterQueue;
use storage::remediation_store::{RemediationStore, RemediationStoreError};

use super::{AppState, authenticated_platform_context};

const RETENTION_PURGE_INTERVAL_SECS: u64 = 86400;
const JOB_CLEANUP_INTERVAL_SECS: u64 = 3600;
const REMEDIATION_EXPIRY_INTERVAL_SECS: u64 = 86400;
const DLQ_CLEANUP_INTERVAL_SECS: u64 = 86400;
const IMPORTANCE_DECAY_INTERVAL_SECS: u64 = 3600;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/lifecycle/remediations", get(list_remediations))
        .route("/admin/lifecycle/remediations/{id}", get(get_remediation))
        .route(
            "/admin/lifecycle/remediations/{id}/approve",
            post(approve_remediation),
        )
        .route(
            "/admin/lifecycle/remediations/{id}/reject",
            post(reject_remediation),
        )
        .route("/admin/lifecycle/status", get(lifecycle_status))
        .route("/admin/lifecycle/dead-letter", get(list_dead_letter))
        .route(
            "/admin/lifecycle/dead-letter/{id}/retry",
            post(retry_dead_letter),
        )
        .route(
            "/admin/lifecycle/dead-letter/{id}/discard",
            post(discard_dead_letter),
        )
        .with_state(state)
}

async fn require_platform_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), axum::response::Response> {
    let (_user_id, roles) = authenticated_platform_context(state, headers).await?;
    if !roles
        .iter()
        .any(|r| *r == mk_core::types::Role::PlatformAdmin.into())
    {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/admin/lifecycle/remediations`
///
/// Returns all pending remediation requests. Pass `?all=true` to include
/// every status.
#[tracing::instrument(skip_all)]
async fn list_remediations(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<ListParams>,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let store = RemediationStore::global();
    let items = if params.all.unwrap_or(false) {
        store.list_all().await
    } else {
        store.list_pending().await
    };

    (
        StatusCode::OK,
        Json(json!({ "items": items, "count": items.len() })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct ListParams {
    all: Option<bool>,
}

/// `GET /api/v1/admin/lifecycle/remediations/{id}`
#[tracing::instrument(skip_all, fields(id = %id))]
async fn get_remediation(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let store = RemediationStore::global();
    match store.get(&id).await {
        Some(req) => (StatusCode::OK, Json(json!(req))).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Remediation request not found",
        ),
    }
}

/// `POST /api/v1/admin/lifecycle/remediations/{id}/approve`
#[tracing::instrument(skip_all, fields(id = %id))]
async fn approve_remediation(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<ApproveBody>>,
) -> impl IntoResponse {
    let (user_id, roles) = match authenticated_platform_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    if !roles
        .iter()
        .any(|r| *r == mk_core::types::Role::PlatformAdmin.into())
    {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        );
    }

    let reviewer = user_id.as_str().to_string();
    let notes = body.and_then(|b| b.notes.clone());

    let store = RemediationStore::global();
    match store.approve(&id, &reviewer, notes).await {
        Ok(req) => (StatusCode::OK, Json(json!(req))).into_response(),
        Err(RemediationStoreError::NotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Remediation request not found",
        ),
        Err(e) => error_response(StatusCode::CONFLICT, "approve_failed", &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct ApproveBody {
    notes: Option<String>,
}

/// `POST /api/v1/admin/lifecycle/remediations/{id}/reject`
#[tracing::instrument(skip_all, fields(id = %id))]
async fn reject_remediation(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RejectBody>,
) -> impl IntoResponse {
    let (user_id, roles) = match authenticated_platform_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    if !roles
        .iter()
        .any(|r| *r == mk_core::types::Role::PlatformAdmin.into())
    {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        );
    }

    let reviewer = user_id.as_str().to_string();

    let store = RemediationStore::global();
    match store.reject(&id, &reviewer, &body.reason).await {
        Ok(req) => (StatusCode::OK, Json(json!(req))).into_response(),
        Err(RemediationStoreError::NotFound(_)) => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Remediation request not found",
        ),
        Err(e) => error_response(StatusCode::CONFLICT, "reject_failed", &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct RejectBody {
    reason: String,
}

/// `GET /api/v1/admin/lifecycle/status`
///
/// Returns the current lifecycle manager status including task schedules
/// and last-run information.
#[tracing::instrument(skip_all)]
async fn lifecycle_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let store = RemediationStore::global();
    let pending = store.list_pending().await.len();
    let total = store.list_all().await.len();

    let dlq = DeadLetterQueue::global();
    let dead_letter_active = dlq.active_count().await;
    let dead_letter_total = dlq.list_all().await.len();

    (
        StatusCode::OK,
        Json(json!({
            "lifecycle_manager": "running",
            "tasks": [
                {
                    "name": "retention_purge",
                    "interval_secs": RETENTION_PURGE_INTERVAL_SECS,
                    "description": "Purge expired soft-deleted records and old remediation requests"
                },
                {
                    "name": "job_cleanup",
                    "interval_secs": JOB_CLEANUP_INTERVAL_SECS,
                    "description": "Clean up expired export/import jobs and temp files"
                },
                {
                    "name": "remediation_expiry",
                    "interval_secs": REMEDIATION_EXPIRY_INTERVAL_SECS,
                    "description": "Expire stale pending remediation requests and remove old records"
                },
                {
                    "name": "dead_letter_cleanup",
                    "interval_secs": DLQ_CLEANUP_INTERVAL_SECS,
                    "description": "Clean up discarded dead-letter items older than 30 days"
                },
                {
                    "name": "importance_decay",
                    "interval_secs": IMPORTANCE_DECAY_INTERVAL_SECS,
                    "description": "Apply exponential importance decay to memory entries"
                }
            ],
            "remediation_summary": {
                "pending": pending,
                "total": total
            },
            "dead_letter_summary": {
                "active": dead_letter_active,
                "total": dead_letter_total
            }
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Dead-letter handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/admin/lifecycle/dead-letter`
///
/// Returns active dead-letter items. Pass `?all=true` to include every status,
/// or `?tenant_id=<id>` to filter by tenant.
#[tracing::instrument(skip_all)]
async fn list_dead_letter(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<DeadLetterListParams>,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let dlq = DeadLetterQueue::global();
    let items = if params.all.unwrap_or(false) {
        dlq.list_all().await
    } else {
        dlq.list_active(params.tenant_id.as_deref()).await
    };

    (
        StatusCode::OK,
        Json(json!({ "items": items, "count": items.len() })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct DeadLetterListParams {
    all: Option<bool>,
    tenant_id: Option<String>,
}

/// `POST /api/v1/admin/lifecycle/dead-letter/{id}/retry`
///
/// Marks the dead-letter item for retry.
#[tracing::instrument(skip_all, fields(id = %id))]
async fn retry_dead_letter(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let dlq = DeadLetterQueue::global();
    match dlq.mark_retrying(&id).await {
        Ok(()) => {
            let item = dlq.get(&id).await;
            (
                StatusCode::OK,
                Json(json!({ "status": "retrying", "item": item })),
            )
                .into_response()
        }
        Err(_) => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Dead-letter item not found",
        ),
    }
}

/// `POST /api/v1/admin/lifecycle/dead-letter/{id}/discard`
///
/// Permanently discards a dead-letter item.
#[tracing::instrument(skip_all, fields(id = %id))]
async fn discard_dead_letter(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let dlq = DeadLetterQueue::global();
    match dlq.discard(&id).await {
        Ok(()) => {
            let item = dlq.get(&id).await;
            (
                StatusCode::OK,
                Json(json!({ "status": "discarded", "item": item })),
            )
                .into_response()
        }
        Err(_) => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Dead-letter item not found",
        ),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{RemediationRiskTier, RemediationStatus};

    #[test]
    fn approve_body_deserializes() {
        let json_str = r#"{"notes": "looks good"}"#;
        let body: ApproveBody = serde_json::from_str(json_str).unwrap();
        assert_eq!(body.notes.as_deref(), Some("looks good"));
    }

    #[test]
    fn approve_body_without_notes() {
        let json_str = r"{}";
        let body: ApproveBody = serde_json::from_str(json_str).unwrap();
        assert!(body.notes.is_none());
    }

    #[test]
    fn reject_body_deserializes() {
        let json_str = r#"{"reason": "too risky"}"#;
        let body: RejectBody = serde_json::from_str(json_str).unwrap();
        assert_eq!(body.reason, "too risky");
    }

    #[test]
    fn list_params_defaults_to_pending() {
        let json_str = r"{}";
        let params: ListParams = serde_json::from_str(json_str).unwrap();
        assert!(params.all.is_none());
    }

    #[test]
    fn remediation_request_serializes_to_camel_case() {
        let req = mk_core::types::RemediationRequest {
            id: "r1".to_string(),
            request_type: "retention_purge".to_string(),
            risk_tier: RemediationRiskTier::RequireApproval,
            entity_type: "memory".to_string(),
            entity_ids: vec!["m1".to_string()],
            tenant_id: Some("t1".to_string()),
            description: "Delete stale memories".to_string(),
            proposed_action: "hard_delete".to_string(),
            detected_by: "lifecycle_manager".to_string(),
            status: RemediationStatus::Pending,
            created_at: 1_700_000_000,
            reviewed_by: None,
            reviewed_at: None,
            resolution_notes: None,
            executed_at: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        // Verify camelCase field naming
        assert!(json.get("requestType").is_some());
        assert!(json.get("riskTier").is_some());
        assert!(json.get("entityType").is_some());
        assert!(json.get("entityIds").is_some());
        assert!(json.get("tenantId").is_some());
        assert!(json.get("proposedAction").is_some());
        assert!(json.get("detectedBy").is_some());
        assert!(json.get("createdAt").is_some());
    }

    #[test]
    fn error_response_produces_json() {
        let resp = error_response(StatusCode::NOT_FOUND, "not_found", "not here");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn dead_letter_list_params_defaults() {
        let json_str = r"{}";
        let params: DeadLetterListParams = serde_json::from_str(json_str).unwrap();
        assert!(params.all.is_none());
        assert!(params.tenant_id.is_none());
    }

    #[test]
    fn dead_letter_list_params_with_tenant() {
        let json_str = r#"{"tenant_id": "t1", "all": true}"#;
        let params: DeadLetterListParams = serde_json::from_str(json_str).unwrap();
        assert_eq!(params.all, Some(true));
        assert_eq!(params.tenant_id.as_deref(), Some("t1"));
    }
}
