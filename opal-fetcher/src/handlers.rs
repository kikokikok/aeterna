//! HTTP request handlers for the OPAL Data Fetcher.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use std::sync::Arc;

use crate::entities::{
    AgentPermissionRow, CedarEntitiesResponse, HierarchyRow, UserPermissionRow, transform_agents,
    transform_hierarchy, transform_users
};
use crate::error::Result;
use crate::state::AppState;

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: String
}

/// Health check endpoint.
///
/// Returns 200 if the server is healthy and can connect to the database.
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Check database connectivity
    let db_status = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => "connected",
        Err(e) => {
            tracing::warn!(error = %e, "Database health check failed");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unhealthy".to_string(),
                    database: "disconnected".to_string()
                })
            );
        }
    };

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "healthy".to_string(),
            database: db_status.to_string()
        })
    )
}

/// Metrics endpoint placeholder.
///
/// Returns Prometheus-format metrics.
pub async fn metrics() -> impl IntoResponse {
    // TODO: Integrate with metrics-exporter-prometheus
    // For now, return basic metrics
    let metrics = r#"# HELP opal_fetcher_requests_total Total number of requests
# TYPE opal_fetcher_requests_total counter
opal_fetcher_requests_total{endpoint="hierarchy"} 0
opal_fetcher_requests_total{endpoint="users"} 0
opal_fetcher_requests_total{endpoint="agents"} 0
# HELP opal_fetcher_up Whether the fetcher is up
# TYPE opal_fetcher_up gauge
opal_fetcher_up 1
"#;

    (StatusCode::OK, metrics)
}

/// GET /v1/hierarchy
///
/// Returns the organizational hierarchy (Company → Organization → Team →
/// Project) as Cedar entities for OPAL consumption.
pub async fn get_hierarchy(
    State(state): State<Arc<AppState>>
) -> Result<Json<CedarEntitiesResponse>> {
    tracing::debug!("Fetching organizational hierarchy");

    let rows: Vec<HierarchyRow> = sqlx::query_as(
        r"
        SELECT
            company_id,
            company_slug,
            company_name,
            org_id,
            org_slug,
            org_name,
            team_id,
            team_slug,
            team_name,
            project_id,
            project_slug,
            project_name,
            git_remote
        FROM v_hierarchy
        "
    )
    .fetch_all(&state.pool)
    .await?;

    let count = rows.len();
    tracing::debug!(row_count = count, "Fetched hierarchy rows");

    let entities = transform_hierarchy(rows)?;
    let response = CedarEntitiesResponse::new(entities);

    tracing::info!(
        entity_count = response.count,
        "Returning hierarchy entities"
    );

    Ok(Json(response))
}

/// GET /v1/users
///
/// Returns users with their team memberships and roles as Cedar entities.
pub async fn get_users(State(state): State<Arc<AppState>>) -> Result<Json<CedarEntitiesResponse>> {
    tracing::debug!("Fetching user permissions");

    let rows: Vec<UserPermissionRow> = sqlx::query_as(
        r"
        SELECT
            user_id,
            email,
            user_name,
            user_status,
            team_id,
            role,
            permissions,
            org_id,
            company_id,
            company_slug,
            org_slug,
            team_slug
        FROM v_user_permissions
        "
    )
    .fetch_all(&state.pool)
    .await?;

    let count = rows.len();
    tracing::debug!(row_count = count, "Fetched user permission rows");

    let entities = transform_users(rows)?;
    let response = CedarEntitiesResponse::new(entities);

    tracing::info!(entity_count = response.count, "Returning user entities");

    Ok(Json(response))
}

/// GET /v1/agents
///
/// Returns agents with their delegation chains and capabilities as Cedar
/// entities.
pub async fn get_agents(State(state): State<Arc<AppState>>) -> Result<Json<CedarEntitiesResponse>> {
    tracing::debug!("Fetching agent permissions");

    let rows: Vec<AgentPermissionRow> = sqlx::query_as(
        r"
        SELECT
            agent_id,
            agent_name,
            agent_type,
            delegated_by_user_id,
            delegated_by_agent_id,
            delegation_depth,
            capabilities,
            allowed_company_ids,
            allowed_org_ids,
            allowed_team_ids,
            allowed_project_ids,
            agent_status,
            delegating_user_email,
            delegating_user_name
        FROM v_agent_permissions
        "
    )
    .fetch_all(&state.pool)
    .await?;

    let count = rows.len();
    tracing::debug!(row_count = count, "Fetched agent permission rows");

    let entities = transform_agents(rows)?;
    let response = CedarEntitiesResponse::new(entities);

    tracing::info!(entity_count = response.count, "Returning agent entities");

    Ok(Json(response))
}

/// GET /v1/all
///
/// Returns all entities (hierarchy, users, agents, and Code Search) in a single response.
/// Useful for initial full sync.
pub async fn get_all_entities(State(state): State<Arc<AppState>>) -> Result<Json<CedarEntitiesResponse>> {
    tracing::debug!("Fetching all entities");

    // Fetch all entity types in parallel
    let (hierarchy_rows, user_rows, agent_rows, codesearch_repo_rows, codesearch_req_rows, codesearch_id_rows) = tokio::try_join!(
        sqlx::query_as::<_, HierarchyRow>(
            r"SELECT company_id, company_slug, company_name, org_id, org_slug, org_name, team_id, team_slug, team_name, project_id, project_slug, project_name, git_remote FROM v_hierarchy"
        ).fetch_all(&state.pool),
        sqlx::query_as::<_, UserPermissionRow>(
            r"SELECT user_id, email, user_name, user_status, team_id, role, permissions, org_id, company_id, company_slug, org_slug, team_slug FROM v_user_permissions"
        ).fetch_all(&state.pool),
        sqlx::query_as::<_, AgentPermissionRow>(
            r"SELECT agent_id, agent_name, agent_type, delegated_by_user_id, delegated_by_agent_id, delegation_depth, capabilities, allowed_company_ids, allowed_org_ids, allowed_team_ids, allowed_project_ids, agent_status, delegating_user_email, delegating_user_name FROM v_agent_permissions"
        ).fetch_all(&state.pool),
        sqlx::query_as::<_, crate::entities::CodeSearchRepositoryRow>(
            r"SELECT id, tenant_id, name, status, sync_strategy, current_branch FROM v_code_search_repositories"
        ).fetch_all(&state.pool),
        sqlx::query_as::<_, crate::entities::CodeSearchRequestRow>(
            r"SELECT id, repository_id, requester_id, status, tenant_id FROM v_code_search_requests"
        ).fetch_all(&state.pool),
        sqlx::query_as::<_, crate::entities::CodeSearchIdentityRow>(
            r"SELECT id, tenant_id, name, provider FROM v_code_search_identities"
        ).fetch_all(&state.pool),
    )?;

    // Transform all entities
    let mut all_entities = transform_hierarchy(hierarchy_rows)?;
    all_entities.extend(transform_users(user_rows)?);
    all_entities.extend(transform_agents(agent_rows)?);
    
    // Add Code Search entities
    all_entities.extend(crate::entities::transform_code_search_repositories(codesearch_repo_rows));
    all_entities.extend(crate::entities::transform_code_search_requests(codesearch_req_rows));
    all_entities.extend(crate::entities::transform_code_search_identities(codesearch_id_rows));

    let response = CedarEntitiesResponse::new(all_entities);

    tracing::info!(entity_count = response.count, "Returning all entities (including Code Search)");

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            database: "connected".to_string()
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("connected"));
    }
}
