use crate::governance::GovernanceEngine;
use crate::governance_client::{GovernanceClient, RemoteGovernanceClient};
use config::config::DeploymentConfig;
use mk_core::traits::StorageBackend;
use mk_core::types::{
    DriftConfig, DriftResult, DriftSuppression, GovernanceEvent, KnowledgeLayer, TenantContext
};
use std::sync::Arc;
use storage::postgres::PostgresBackend;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        get_drift_status,
        get_org_report,
        approve_proposal,
        reject_proposal,
        get_job_status,
        replay_events,
        create_suppression,
        list_suppressions,
        delete_suppression,
        get_drift_config,
        save_drift_config
    ),
    components(
        schemas(
            mk_core::types::DriftResult,
            mk_core::types::PolicyViolation,
            mk_core::types::GovernanceEvent,
            mk_core::types::DriftSuppression,
            mk_core::types::DriftConfig
        )
    ),
    tags(
        (name = "governance", description = "Governance Dashboard API")
    )
)]
pub struct GovernanceApiDoc;

pub struct GovernanceDashboardApi {
    engine: Arc<GovernanceEngine>,
    storage: Arc<PostgresBackend>,
    governance_client: Option<Arc<dyn GovernanceClient>>,
    deployment_config: DeploymentConfig
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/drift/{project_id}",
    responses(
        (status = 200, description = "Drift status fetched successfully", body = Option<DriftResult>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn get_drift_status(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    project_id: &str
) -> anyhow::Result<Option<DriftResult>> {
    if api.deployment_config.mode == "remote"
        && let Some(client) = &api.governance_client
    {
        return client
            .get_drift_status(ctx, project_id)
            .await
            .map_err(|e| anyhow::anyhow!("Remote drift status failed: {}", e));
    }

    let result =
        StorageBackend::get_latest_drift_result(api.storage.as_ref(), ctx.clone(), project_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch drift result: {:?}", e))?;

    Ok(result)
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/reports/{org_id}",
    responses(
        (status = 200, description = "Organization report fetched successfully", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("org_id" = String, Path, description = "Organization ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn get_org_report(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    org_id: &str
) -> anyhow::Result<serde_json::Value> {
    let descendants = StorageBackend::get_descendants(api.storage.as_ref(), ctx.clone(), org_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch descendants: {:?}", e))?;

    let mut project_drifts = Vec::new();
    for unit in descendants {
        if unit.unit_type == mk_core::types::UnitType::Project
            && let Some(drift) = get_drift_status(api.clone(), ctx, &unit.id).await?
        {
            project_drifts.push(drift);
        }
    }

    let avg_drift = if project_drifts.is_empty() {
        0.0
    } else {
        project_drifts.iter().map(|d| d.drift_score).sum::<f32>() / project_drifts.len() as f32
    };

    Ok(serde_json::json!({
        "orgId": org_id,
        "averageDrift": avg_drift,
        "projectCount": project_drifts.len(),
        "projects": project_drifts,
        "timestamp": chrono::Utc::now().timestamp()
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/proposals/{proposal_id}/approve",
    responses(
        (status = 200, description = "Proposal approved successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Proposal not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("proposal_id" = String, Path, description = "Proposal ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn approve_proposal(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    proposal_id: &str
) -> anyhow::Result<()> {
    let repo = api
        .engine
        .repository()
        .ok_or_else(|| anyhow::anyhow!("Repository not configured"))?;

    let entry = repo
        .get(
            ctx.clone(),
            mk_core::types::KnowledgeLayer::Project,
            proposal_id
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch proposal: {:?}", e))?
        .ok_or_else(|| anyhow::anyhow!("Proposal not found"))?;

    let mut accepted_entry = entry.clone();
    accepted_entry.status = mk_core::types::KnowledgeStatus::Accepted;

    repo.store(ctx.clone(), accepted_entry, "Proposal approved")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to approve proposal: {:?}", e))?;

    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/proposals/{proposal_id}/reject",
    responses(
        (status = 200, description = "Proposal rejected successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Proposal not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("proposal_id" = String, Path, description = "Proposal ID"),
        ("reason" = String, Query, description = "Rejection reason")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn reject_proposal(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    proposal_id: &str,
    reason: &str
) -> anyhow::Result<()> {
    let repo = api
        .engine
        .repository()
        .ok_or_else(|| anyhow::anyhow!("Repository not configured"))?;

    let entry = repo
        .get(
            ctx.clone(),
            mk_core::types::KnowledgeLayer::Project,
            proposal_id
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch proposal: {:?}", e))?
        .ok_or_else(|| anyhow::anyhow!("Proposal not found"))?;

    let mut rejected_entry = entry.clone();
    rejected_entry.status = mk_core::types::KnowledgeStatus::Draft;
    rejected_entry
        .metadata
        .insert("rejection_reason".to_string(), serde_json::json!(reason));

    repo.store(
        ctx.clone(),
        rejected_entry,
        &format!("Proposal rejected: {}", reason)
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to reject proposal: {:?}", e))?;

    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/jobs",
    responses(
        (status = 200, description = "Job status fetched successfully", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("job_name" = Option<String>, Query, description = "Filter by job name")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn get_job_status(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    job_name: Option<&str>
) -> anyhow::Result<serde_json::Value> {
    let rows = sqlx::query(
        "SELECT id, job_name, status, message, started_at, finished_at, duration_ms 
         FROM job_status 
         WHERE tenant_id = $1 OR tenant_id = 'all' 
         ORDER BY started_at DESC LIMIT 50"
    )
    .bind(ctx.tenant_id.as_str())
    .fetch_all(api.storage.pool())
    .await
    .map_err(|e| anyhow::anyhow!("Failed to fetch job status: {:?}", e))?;

    let mut jobs = Vec::new();
    for row in rows {
        use sqlx::Row;
        let name: String = row.get("job_name");
        if let Some(filter) = job_name
            && name != filter
        {
            continue;
        }

        jobs.push(serde_json::json!({
            "id": row.get::<uuid::Uuid, _>("id"),
            "jobName": name,
            "status": row.get::<String, _>("status"),
            "message": row.get::<Option<String>, _>("message"),
            "startedAt": row.get::<i64, _>("started_at"),
            "finishedAt": row.get::<Option<i64>, _>("finished_at"),
            "durationMs": row.get::<Option<i64>, _>("duration_ms"),
        }));
    }

    Ok(serde_json::json!(jobs))
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/events/replay",
    responses(
        (status = 200, description = "Events replayed successfully", body = Vec<GovernanceEvent>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("since_timestamp" = i64, Query, description = "Replay events after this timestamp"),
        ("limit" = usize, Query, description = "Maximum number of events to return")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn replay_events(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    since_timestamp: i64,
    limit: usize
) -> anyhow::Result<Vec<mk_core::types::GovernanceEvent>> {
    if api.deployment_config.mode == "remote"
        && let Some(client) = &api.governance_client
    {
        return client
            .replay_events(ctx, since_timestamp, limit)
            .await
            .map_err(|e| anyhow::anyhow!("Remote replay events failed: {}", e));
    }

    let events = StorageBackend::get_governance_events(
        api.storage.as_ref(),
        ctx.clone(),
        since_timestamp,
        limit
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to replay governance events: {:?}", e))?;

    Ok(events)
}

#[utoipa::path(
    post,
    path = "/api/v1/governance/suppressions",
    responses(
        (status = 201, description = "Suppression created successfully", body = DriftSuppression),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("project_id" = String, Query, description = "Project ID"),
        ("policy_id" = String, Query, description = "Policy ID to suppress"),
        ("reason" = String, Query, description = "Reason for suppression"),
        ("rule_pattern" = Option<String>, Query, description = "Optional regex pattern to match violations"),
        ("expires_at" = Option<i64>, Query, description = "Optional expiration timestamp")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn create_suppression(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    project_id: &str,
    policy_id: &str,
    reason: &str,
    rule_pattern: Option<&str>,
    expires_at: Option<i64>
) -> anyhow::Result<DriftSuppression> {
    let mut suppression = DriftSuppression::new(
        project_id.to_string(),
        ctx.tenant_id.clone(),
        policy_id.to_string(),
        reason.to_string(),
        ctx.user_id.clone()
    );

    if let Some(pattern) = rule_pattern {
        suppression = suppression.with_pattern(pattern.to_string());
    }

    if let Some(expires) = expires_at {
        suppression = suppression.with_expiry(expires);
    }

    StorageBackend::create_suppression(api.storage.as_ref(), suppression.clone())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create suppression: {:?}", e))?;

    Ok(suppression)
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/suppressions/{project_id}",
    responses(
        (status = 200, description = "Suppressions fetched successfully", body = Vec<DriftSuppression>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn list_suppressions(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    project_id: &str
) -> anyhow::Result<Vec<DriftSuppression>> {
    let suppressions =
        StorageBackend::list_suppressions(api.storage.as_ref(), ctx.clone(), project_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list suppressions: {:?}", e))?;

    let active_suppressions: Vec<DriftSuppression> = suppressions
        .into_iter()
        .filter(|s| !s.is_expired())
        .collect();

    Ok(active_suppressions)
}

#[utoipa::path(
    delete,
    path = "/api/v1/governance/suppressions/{suppression_id}",
    responses(
        (status = 200, description = "Suppression deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Suppression not found"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("suppression_id" = String, Path, description = "Suppression ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn delete_suppression(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    suppression_id: &str
) -> anyhow::Result<()> {
    StorageBackend::delete_suppression(api.storage.as_ref(), ctx.clone(), suppression_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete suppression: {:?}", e))?;

    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/governance/drift-config/{project_id}",
    responses(
        (status = 200, description = "Drift config fetched successfully", body = Option<DriftConfig>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn get_drift_config(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    project_id: &str
) -> anyhow::Result<DriftConfig> {
    let config = StorageBackend::get_drift_config(api.storage.as_ref(), ctx.clone(), project_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get drift config: {:?}", e))?;

    Ok(config
        .unwrap_or_else(|| DriftConfig::for_project(project_id.to_string(), ctx.tenant_id.clone())))
}

#[utoipa::path(
    put,
    path = "/api/v1/governance/drift-config/{project_id}",
    responses(
        (status = 200, description = "Drift config saved successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    params(
        ("project_id" = String, Path, description = "Project ID")
    ),
    request_body = DriftConfig,
    security(
        ("tenant_auth" = [])
    )
)]
pub async fn save_drift_config(
    api: Arc<GovernanceDashboardApi>,
    ctx: &TenantContext,
    project_id: &str,
    threshold: f32,
    low_confidence_threshold: Option<f32>,
    auto_suppress_info: Option<bool>
) -> anyhow::Result<()> {
    let mut config = DriftConfig::for_project(project_id.to_string(), ctx.tenant_id.clone());
    config.threshold = threshold;
    if let Some(lct) = low_confidence_threshold {
        config.low_confidence_threshold = lct;
    }
    if let Some(asi) = auto_suppress_info {
        config.auto_suppress_info = asi;
    }

    StorageBackend::save_drift_config(api.storage.as_ref(), config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save drift config: {:?}", e))?;

    Ok(())
}

impl GovernanceDashboardApi {
    pub fn new(
        engine: Arc<GovernanceEngine>,
        storage: Arc<PostgresBackend>,
        deployment_config: DeploymentConfig
    ) -> Self {
        let governance_client = if deployment_config.mode == "remote" {
            deployment_config.remote_url.as_ref().map(|url: &String| {
                Arc::new(RemoteGovernanceClient::new(url.clone())) as Arc<dyn GovernanceClient>
            })
        } else {
            None
        };

        Self {
            engine,
            storage,
            governance_client,
            deployment_config
        }
    }

    pub async fn list_proposals(
        &self,
        ctx: &TenantContext,
        layer: Option<KnowledgeLayer>
    ) -> anyhow::Result<Vec<mk_core::types::KnowledgeEntry>> {
        if self.deployment_config.mode == "remote"
            && let Some(client) = &self.governance_client
        {
            return client
                .list_proposals(ctx, layer)
                .await
                .map_err(|e| anyhow::anyhow!("Remote list proposals failed: {}", e));
        }

        let repo = self
            .engine
            .repository()
            .ok_or_else(|| anyhow::anyhow!("Repository not configured"))?;

        let layers = if let Some(l) = layer {
            vec![l]
        } else {
            vec![
                KnowledgeLayer::Project,
                KnowledgeLayer::Team,
                KnowledgeLayer::Org,
                KnowledgeLayer::Company,
            ]
        };

        let mut proposals = Vec::new();
        for l in layers {
            let entries: Vec<mk_core::types::KnowledgeEntry> = repo
                .list(ctx.clone(), l, "")
                .await
                .map_err(|e| anyhow::anyhow!("Failed to list entries in layer {:?}: {:?}", l, e))?;

            for entry in entries {
                if entry.status == mk_core::types::KnowledgeStatus::Proposed {
                    proposals.push(entry);
                }
            }
        }

        Ok(proposals)
    }
}
