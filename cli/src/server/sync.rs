use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::AppState;

#[derive(Debug, Deserialize)]
pub struct SyncPushRequest {
    pub entries: Vec<SyncPushEntry>,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncPushEntry {
    pub id: String,
    pub content: String,
    pub layer: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub importance: f64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncPushResponse {
    pub cursor: String,
    pub conflicts: Vec<SyncConflictEntry>,
    pub embeddings: HashMap<String, Vec<f32>>,
}

#[derive(Debug, Serialize)]
pub struct SyncConflictEntry {
    pub id: String,
    pub remote_content: String,
    pub remote_updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncPullQuery {
    pub since_cursor: Option<String>,
    pub layers: Option<String>,
    #[serde(default = "default_pull_limit")]
    pub limit: i64,
}

fn default_pull_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
pub struct SyncPullResponse {
    pub entries: Vec<SyncPullEntry>,
    pub cursor: String,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncPullEntry {
    pub id: String,
    pub content: String,
    pub layer: String,
    pub embedding: Option<Vec<f32>>,
    pub tags: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub importance: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sync/push", post(push_handler))
        .route("/sync/pull", get(pull_handler))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn push_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SyncPushRequest>,
) -> axum::response::Response {
    if extract_auth_token(&headers).is_none() {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "auth_required",
            "Bearer token required",
        );
    }

    let ctx = default_tenant_context();
    let pool = state.postgres.pool();

    let mut conflicts = Vec::new();
    let embeddings = HashMap::new();

    for entry in &req.entries {
        let created_at = parse_timestamp(&entry.created_at).unwrap_or(0);
        let updated_at = parse_timestamp(&entry.updated_at).unwrap_or(0);
        let deleted_at = entry.deleted_at.as_ref().and_then(|s| parse_timestamp(s));

        let existing: Option<(String, i64)> = sqlx::query_as(
            "SELECT content, updated_at FROM memory_entries WHERE id = $1 AND tenant_id = $2",
        )
        .bind(&entry.id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((remote_content, remote_updated)) = existing {
            if remote_updated > updated_at {
                conflicts.push(SyncConflictEntry {
                    id: entry.id.clone(),
                    remote_content,
                    remote_updated_at: format_timestamp(remote_updated),
                });
                continue;
            }
        }

        let properties = entry.metadata.clone().unwrap_or(serde_json::json!({}));
        let tags_value = serde_json::to_value(&entry.tags).unwrap_or(serde_json::json!([]));
        let merged_properties = merge_tags_into_properties(properties, tags_value);

        if let Some(deleted_at_value) = deleted_at {
            sqlx::query(
                "UPDATE memory_entries SET deleted_at = $1, updated_at = $2, device_id = $3 WHERE id = $4 AND tenant_id = $5",
            )
            .bind(deleted_at_value)
            .bind(updated_at)
            .bind(&req.device_id)
            .bind(&entry.id)
            .bind(ctx.tenant_id.as_str())
            .execute(pool)
            .await
            .ok();
        } else {
            sqlx::query(
                "INSERT INTO memory_entries (id, tenant_id, content, memory_layer, properties, importance_score, device_id, created_at, updated_at, deleted_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL) \
                 ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, memory_layer = EXCLUDED.memory_layer, \
                 properties = EXCLUDED.properties, importance_score = EXCLUDED.importance_score, \
                 device_id = EXCLUDED.device_id, updated_at = EXCLUDED.updated_at, deleted_at = NULL",
            )
            .bind(&entry.id)
            .bind(ctx.tenant_id.as_str())
            .bind(&entry.content)
            .bind(&entry.layer)
            .bind(&merged_properties)
            .bind(entry.importance as f32)
            .bind(&req.device_id)
            .bind(created_at)
            .bind(updated_at)
            .execute(pool)
            .await
            .ok();
        }
    }

    let cursor = chrono::Utc::now().timestamp_millis().to_string();
    (
        StatusCode::OK,
        Json(SyncPushResponse {
            cursor,
            conflicts,
            embeddings,
        }),
    )
        .into_response()
}

#[tracing::instrument(skip_all)]
async fn pull_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SyncPullQuery>,
) -> axum::response::Response {
    if extract_auth_token(&headers).is_none() {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "auth_required",
            "Bearer token required",
        );
    }

    let ctx = default_tenant_context();
    let pool = state.postgres.pool();

    let since_cursor = query
        .since_cursor
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let layers: Vec<String> = query
        .layers
        .as_deref()
        .map(|s| s.split(',').map(|l| l.trim().to_string()).collect())
        .unwrap_or_else(|| {
            vec![
                "project".into(),
                "team".into(),
                "org".into(),
                "company".into(),
            ]
        });

    let limit = query.limit.max(1);
    let fetch_limit = limit + 1;

    let rows: Vec<(String, String, String, Option<serde_json::Value>, f32, i64, i64)> =
        sqlx::query_as(
            "SELECT id, content, memory_layer, properties, COALESCE(importance_score, 0.0) as importance_score, created_at, updated_at \
             FROM memory_entries \
             WHERE tenant_id = $1 AND updated_at > $2 AND deleted_at IS NULL \
             ORDER BY updated_at ASC, id ASC \
             LIMIT $3",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(since_cursor)
        .bind(fetch_limit * 2)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let layer_set: HashSet<&str> = layers.iter().map(String::as_str).collect();
    let all_matching: Vec<_> = rows
        .iter()
        .filter(|row| layer_set.contains(row.2.as_str()))
        .collect();

    let entries: Vec<SyncPullEntry> = all_matching
        .iter()
        .take(limit as usize)
        .map(|row| {
            let tags = row
                .3
                .as_ref()
                .and_then(|p| p.get("tags"))
                .and_then(|t| serde_json::from_value::<Vec<String>>(t.clone()).ok())
                .unwrap_or_default();

            SyncPullEntry {
                id: row.0.clone(),
                content: row.1.clone(),
                layer: row.2.clone(),
                embedding: None,
                tags,
                metadata: row.3.clone(),
                importance: row.4 as f64,
                created_at: format_timestamp(row.5),
                updated_at: format_timestamp(row.6),
            }
        })
        .collect();

    let has_more = all_matching.len() > limit as usize;
    let cursor = entries
        .last()
        .map(|entry| parse_timestamp(&entry.updated_at).unwrap_or(0).to_string())
        .unwrap_or_else(|| since_cursor.to_string());

    (
        StatusCode::OK,
        Json(SyncPullResponse {
            entries,
            cursor,
            has_more,
        }),
    )
        .into_response()
}

fn extract_auth_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(ToString::to_string)
}

fn default_tenant_context() -> mk_core::types::TenantContext {
    mk_core::types::TenantContext::new(
        mk_core::types::TenantId::new("default".to_string()).expect("default tenant id"),
        mk_core::types::UserId::new("system".to_string()).expect("default user id"),
    )
}

fn parse_timestamp(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis())
        .ok()
        .or_else(|| s.parse::<i64>().ok())
}

fn format_timestamp(millis: i64) -> String {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| millis.to_string())
}

fn merge_tags_into_properties(
    mut properties: serde_json::Value,
    tags: serde_json::Value,
) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut map) = properties {
        map.insert("tags".to_string(), tags);
    }
    properties
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
            message: message.to_string(),
        }),
    )
        .into_response()
}
