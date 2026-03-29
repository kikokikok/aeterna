use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use idp_sync::config::GitHubConfig;
use serde_json::json;
use uuid::Uuid;

use super::AppState;

static SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/sync/github", post(handle_github_sync))
        .with_state(state)
}

async fn handle_github_sync(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if SYNC_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "sync_in_progress",
                "message": "A GitHub organization sync is already running"
            })),
        )
            .into_response();
    }

    let result = run_sync(&state).await;

    SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);

    match result {
        Ok(report) => (StatusCode::OK, Json(json!(report))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "sync_failed",
                "message": format!("{err:?}")
            })),
        )
            .into_response(),
    }
}

async fn run_sync(state: &Arc<AppState>) -> anyhow::Result<idp_sync::sync::SyncReport> {
    let github_config = build_github_config()?;
    let tenant_id = resolve_tenant_id(state).await?;

    tracing::info!(
        org = %github_config.org_name,
        tenant_id = %tenant_id,
        "Starting GitHub organization sync"
    );

    let report =
        idp_sync::github::run_github_sync(&github_config, state.postgres.pool(), tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("GitHub sync failed: {e:?}"))?;

    tracing::info!(
        users_created = report.users_created,
        users_updated = report.users_updated,
        groups_synced = report.groups_synced,
        memberships_added = report.memberships_added,
        "GitHub organization sync completed"
    );

    Ok(report)
}

fn build_github_config() -> anyhow::Result<GitHubConfig> {
    build_github_config_from_env()
}

pub(crate) fn build_github_config_from_env() -> anyhow::Result<GitHubConfig> {
    let org_name = std::env::var("AETERNA_GITHUB_ORG_NAME")
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_ORG_NAME is required for GitHub org sync"))?;

    let app_id: u64 = std::env::var("AETERNA_GITHUB_APP_ID")
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_APP_ID is required"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_APP_ID must be a number"))?;

    let installation_id: u64 = std::env::var("AETERNA_GITHUB_INSTALLATION_ID")
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_INSTALLATION_ID is required"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_INSTALLATION_ID must be a number"))?;

    let private_key_pem = std::env::var("AETERNA_GITHUB_APP_PEM")
        .map_err(|_| anyhow::anyhow!("AETERNA_GITHUB_APP_PEM is required"))?;

    let team_filter = std::env::var("AETERNA_GITHUB_TEAM_FILTER").ok();
    let sync_repos_as_projects = std::env::var("AETERNA_GITHUB_SYNC_REPOS_AS_PROJECTS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    Ok(GitHubConfig {
        org_name,
        app_id,
        installation_id,
        private_key_pem,
        team_filter,
        sync_repos_as_projects,
        api_base_url: None,
    })
}

async fn resolve_tenant_id(state: &Arc<AppState>) -> anyhow::Result<Uuid> {
    resolve_tenant_id_from_pool(state.postgres.pool()).await
}

pub(crate) async fn resolve_tenant_id_from_pool(pool: &sqlx::PgPool) -> anyhow::Result<Uuid> {
    let tenant_str = std::env::var("AETERNA_TENANT_ID").unwrap_or_else(|_| "default".to_string());
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM tenants WHERE name = $1 OR id::text = $1 LIMIT 1")
            .bind(&tenant_str)
            .fetch_optional(pool)
            .await?;

    match row {
        Some((id,)) => Ok(id),
        None => {
            tracing::info!(tenant = %tenant_str, "Tenant not found, creating default");
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO tenants (id, name, created_at) VALUES ($1, $2, NOW()) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id")
                .bind(id)
                .bind(&tenant_str)
                .execute(pool)
                .await?;
            Ok(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_guard_prevents_concurrent_execution() {
        SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
        assert!(
            SYNC_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        );
        assert!(
            SYNC_IN_PROGRESS
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        );
        SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}
