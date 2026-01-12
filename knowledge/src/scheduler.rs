use crate::governance::GovernanceEngine;
use config::config::DeploymentConfig;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{DriftResult, KnowledgeLayer, TenantContext, UnitType, UserId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

pub struct GovernanceScheduler {
    engine: Arc<GovernanceEngine>,
    repository: Arc<dyn KnowledgeRepository<Error = crate::repository::RepositoryError>>,
    deployment_config: DeploymentConfig,
    quick_scan_interval: Duration,
    semantic_scan_interval: Duration,
    report_interval: Duration,
}

impl GovernanceScheduler {
    pub fn new(
        engine: Arc<GovernanceEngine>,
        repository: Arc<dyn KnowledgeRepository<Error = crate::repository::RepositoryError>>,
        deployment_config: DeploymentConfig,
        quick_scan_interval: Duration,
        semantic_scan_interval: Duration,
        report_interval: Duration,
    ) -> Self {
        Self {
            engine,
            repository,
            deployment_config,
            quick_scan_interval,
            semantic_scan_interval,
            report_interval,
        }
    }

    pub async fn start(&self) {
        if self.deployment_config.mode == "remote" {
            tracing::info!("Governance scheduler disabled in Remote mode");
            return;
        }

        let mut quick_interval = time::interval(self.quick_scan_interval);
        let mut semantic_interval = time::interval(self.semantic_scan_interval);
        let mut report_interval = time::interval(self.report_interval);

        loop {
            tokio::select! {
                _ = quick_interval.tick() => {
                    let _ = self.run_job("quick_drift_scan", "all", self.run_batch_drift_scan()).await;
                }
                _ = semantic_interval.tick() => {
                    if self.deployment_config.mode != "hybrid" {
                        let _ = self.run_job("semantic_analysis", "all", self.run_semantic_analysis_job()).await;
                    } else {
                        tracing::debug!("Skipping local semantic analysis in Hybrid mode (relying on remote)");
                    }
                }
                _ = report_interval.tick() => {
                    if self.deployment_config.mode == "local" {
                        let _ = self.run_job("weekly_report", "all", self.run_weekly_report_job()).await;
                    }
                }
            }
        }
    }

    async fn run_job<F>(&self, name: &str, tenant_id: &str, job_future: F) -> anyhow::Result<()>
    where
        F: std::future::Future<Output = anyhow::Result<()>>,
    {
        let started_at = chrono::Utc::now().timestamp();
        tracing::info!("Starting job: {}", name);

        let storage = self
            .engine
            .storage()
            .ok_or_else(|| anyhow::anyhow!("Storage not configured"))?;

        let _ = storage
            .record_job_status(name, tenant_id, "running", None, started_at, None)
            .await;

        match job_future.await {
            Ok(_) => {
                let finished_at = chrono::Utc::now().timestamp();
                let _ = storage
                    .record_job_status(
                        name,
                        tenant_id,
                        "completed",
                        None,
                        started_at,
                        Some(finished_at),
                    )
                    .await;
                Ok(())
            }
            Err(e) => {
                let finished_at = chrono::Utc::now().timestamp();
                let message = format!("{:?}", e);
                let _ = storage
                    .record_job_status(
                        name,
                        tenant_id,
                        "failed",
                        Some(&message),
                        started_at,
                        Some(finished_at),
                    )
                    .await;
                Err(e)
            }
        }
    }

    async fn run_batch_drift_scan(&self) -> anyhow::Result<()> {
        tracing::info!("Starting batch drift scan");

        let storage = self
            .engine
            .storage()
            .ok_or_else(|| anyhow::anyhow!("Storage not configured"))?;
        let units = storage
            .list_all_units()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list units: {:?}", e))?;

        for unit in units {
            if unit.unit_type == UnitType::Project {
                let tenant_ctx =
                    TenantContext::new(unit.tenant_id.clone(), mk_core::types::UserId::default());
                let mut context = HashMap::new();
                context.insert("projectId".to_string(), serde_json::json!(unit.id));
                context.insert("content".to_string(), serde_json::json!(""));

                let _ = self
                    .engine
                    .check_drift(&tenant_ctx, &unit.id, &context)
                    .await;
            }
        }

        Ok(())
    }

    async fn run_semantic_analysis_job(&self) -> anyhow::Result<()> {
        tracing::info!("Starting daily semantic analysis job");

        let llm = self
            .engine
            .llm_service()
            .ok_or_else(|| anyhow::anyhow!("LLM service not configured"))?;

        let storage = self
            .engine
            .storage()
            .ok_or_else(|| anyhow::anyhow!("Storage not configured"))?;

        let units = storage
            .list_all_units()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list units: {:?}", e))?;

        for unit in units {
            if unit.unit_type == UnitType::Project {
                let ctx = TenantContext::new(unit.tenant_id.clone(), UserId::default());

                let policies = storage
                    .get_unit_policies(ctx.clone(), &unit.id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to fetch policies: {:?}", e))?;

                if policies.is_empty() {
                    continue;
                }

                let entries = self
                    .repository
                    .list(ctx.clone(), KnowledgeLayer::Project, "")
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to list project files: {:?}", e))?;

                let content = entries
                    .into_iter()
                    .map(|e| format!("File: {}\n---\n{}\n---", e.path, e.content))
                    .collect::<Vec<_>>()
                    .join("\n\n");

                if content.is_empty() {
                    continue;
                }

                match llm.analyze_drift(&content, &policies).await {
                    Ok(result) => {
                        let drift_score = if result.is_valid { 0.0 } else { 1.0 };
                        let _ = storage
                            .store_drift_result(DriftResult {
                                project_id: unit.id.clone(),
                                tenant_id: unit.tenant_id.clone(),
                                drift_score,
                                violations: result.violations,
                                timestamp: chrono::Utc::now().timestamp(),
                            })
                            .await;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Semantic analysis failed for project {}: {:?}",
                            unit.id,
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_weekly_report_job(&self) -> anyhow::Result<()> {
        tracing::info!("Starting weekly governance report job");

        let storage = self
            .engine
            .storage()
            .ok_or_else(|| anyhow::anyhow!("Storage not configured"))?;

        let units = storage
            .list_all_units()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list units: {:?}", e))?;

        let now = chrono::Utc::now().timestamp();
        let one_week_ago = now - (7 * 24 * 60 * 60);

        for unit in units {
            if unit.unit_type == UnitType::Organization {
                let mut report_data = HashMap::new();
                let children = storage
                    .get_descendants(
                        TenantContext::new(unit.tenant_id.clone(), UserId::default()),
                        &unit.id,
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to list projects: {:?}", e))?;

                let mut total_drift = 0.0;
                let mut project_count = 0;
                let mut all_violations = Vec::new();

                for child in children {
                    if child.unit_type == UnitType::Project {
                        if let Some(result) = storage
                            .get_latest_drift_result(
                                TenantContext::new(child.tenant_id.clone(), UserId::default()),
                                &child.id,
                            )
                            .await
                            .map_err(|e| anyhow::anyhow!("Failed to fetch drift: {:?}", e))?
                        {
                            if result.timestamp >= one_week_ago {
                                total_drift += result.drift_score;
                                project_count += 1;
                                all_violations.extend(result.violations);
                            }
                        }
                    }
                }

                let avg_drift = if project_count > 0 {
                    total_drift / project_count as f32
                } else {
                    0.0
                };

                report_data.insert("average_drift".to_string(), serde_json::json!(avg_drift));
                report_data.insert(
                    "project_count".to_string(),
                    serde_json::json!(project_count),
                );
                report_data.insert(
                    "violation_count".to_string(),
                    serde_json::json!(all_violations.len()),
                );

                tracing::info!(
                    "Weekly report for Org {}: Avg Drift: {}, Projects: {}, Violations: {}",
                    unit.id,
                    avg_drift,
                    project_count,
                    all_violations.len()
                );
            }
        }

        Ok(())
    }
}
