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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mk_core::traits::StorageBackend;
    use mk_core::types::{
        GovernanceEvent, KnowledgeEntry, KnowledgeLayer, OrganizationalUnit, Policy, Role, TenantId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::RwLock;

    struct MockStorage {
        units: RwLock<Vec<OrganizationalUnit>>,
        drift_results: RwLock<Vec<DriftResult>>,
        job_records: Arc<AtomicUsize>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                units: RwLock::new(Vec::new()),
                drift_results: RwLock::new(Vec::new()),
                job_records: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn with_units(units: Vec<OrganizationalUnit>) -> Self {
            Self {
                units: RwLock::new(units),
                drift_results: RwLock::new(Vec::new()),
                job_records: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl StorageBackend for MockStorage {
        type Error = storage::postgres::PostgresError;

        async fn store(
            &self,
            _ctx: TenantContext,
            _key: &str,
            _value: &[u8],
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn retrieve(
            &self,
            _ctx: TenantContext,
            _key: &str,
        ) -> Result<Option<Vec<u8>>, Self::Error> {
            Ok(None)
        }

        async fn delete(&self, _ctx: TenantContext, _key: &str) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn exists(&self, _ctx: TenantContext, _key: &str) -> Result<bool, Self::Error> {
            Ok(false)
        }

        async fn get_ancestors(
            &self,
            _ctx: TenantContext,
            _unit_id: &str,
        ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
            Ok(Vec::new())
        }

        async fn get_descendants(
            &self,
            _ctx: TenantContext,
            unit_id: &str,
        ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
            let units = self.units.read().await;
            let children: Vec<_> = units
                .iter()
                .filter(|u| u.parent_id.as_deref() == Some(unit_id))
                .cloned()
                .collect();
            Ok(children)
        }

        async fn get_unit_policies(
            &self,
            _ctx: TenantContext,
            _unit_id: &str,
        ) -> Result<Vec<Policy>, Self::Error> {
            Ok(Vec::new())
        }

        async fn create_unit(&self, _unit: &OrganizationalUnit) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn add_unit_policy(
            &self,
            _ctx: &TenantContext,
            _unit_id: &str,
            _policy: &Policy,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn assign_role(
            &self,
            _user_id: &UserId,
            _tenant_id: &TenantId,
            _unit_id: &str,
            _role: Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_role(
            &self,
            _user_id: &UserId,
            _tenant_id: &TenantId,
            _unit_id: &str,
            _role: Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn store_drift_result(&self, result: DriftResult) -> Result<(), Self::Error> {
            self.drift_results.write().await.push(result);
            Ok(())
        }

        async fn get_latest_drift_result(
            &self,
            _ctx: TenantContext,
            project_id: &str,
        ) -> Result<Option<DriftResult>, Self::Error> {
            let results = self.drift_results.read().await;
            let result = results.iter().find(|r| r.project_id == project_id).cloned();
            Ok(result)
        }

        async fn list_all_units(&self) -> Result<Vec<OrganizationalUnit>, Self::Error> {
            Ok(self.units.read().await.clone())
        }

        async fn record_job_status(
            &self,
            _job_name: &str,
            _tenant_id: &str,
            _status: &str,
            _message: Option<&str>,
            _started_at: i64,
            _finished_at: Option<i64>,
        ) -> Result<(), Self::Error> {
            self.job_records.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn get_governance_events(
            &self,
            _ctx: TenantContext,
            _since_timestamp: i64,
            _limit: usize,
        ) -> Result<Vec<GovernanceEvent>, Self::Error> {
            Ok(Vec::new())
        }
    }

    struct MockRepository {
        entries: RwLock<Vec<KnowledgeEntry>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                entries: RwLock::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl mk_core::traits::KnowledgeRepository for MockRepository {
        type Error = crate::repository::RepositoryError;

        async fn get(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            path: &str,
        ) -> Result<Option<KnowledgeEntry>, Self::Error> {
            let entries = self.entries.read().await;
            Ok(entries.iter().find(|e| e.path == path).cloned())
        }

        async fn store(
            &self,
            _ctx: TenantContext,
            entry: KnowledgeEntry,
            _message: &str,
        ) -> Result<String, Self::Error> {
            self.entries.write().await.push(entry);
            Ok("hash123".to_string())
        }

        async fn list(
            &self,
            _ctx: TenantContext,
            layer: KnowledgeLayer,
            _prefix: &str,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            let entries = self.entries.read().await;
            Ok(entries
                .iter()
                .filter(|e| e.layer == layer)
                .cloned()
                .collect())
        }

        async fn delete(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
            _message: &str,
        ) -> Result<String, Self::Error> {
            Ok("hash123".to_string())
        }

        async fn get_head_commit(
            &self,
            _ctx: TenantContext,
        ) -> Result<Option<String>, Self::Error> {
            Ok(Some("head123".to_string()))
        }

        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _since_commit: &str,
        ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
            Ok(Vec::new())
        }

        async fn search(
            &self,
            _ctx: TenantContext,
            _query: &str,
            _layers: Vec<KnowledgeLayer>,
            _limit: usize,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }

        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
        }
    }

    fn create_test_tenant() -> TenantId {
        TenantId::new("test-tenant".to_string()).unwrap()
    }

    fn create_test_project_unit(id: &str, tenant_id: TenantId) -> OrganizationalUnit {
        OrganizationalUnit {
            id: id.to_string(),
            name: format!("Project {}", id),
            unit_type: UnitType::Project,
            tenant_id,
            parent_id: Some("org-1".to_string()),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    fn create_test_org_unit(id: &str, tenant_id: TenantId) -> OrganizationalUnit {
        OrganizationalUnit {
            id: id.to_string(),
            name: format!("Org {}", id),
            unit_type: UnitType::Organization,
            tenant_id,
            parent_id: None,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_run_batch_drift_scan_with_projects() {
        let tenant_id = create_test_tenant();
        let units = vec![
            create_test_org_unit("org-1", tenant_id.clone()),
            create_test_project_unit("proj-1", tenant_id.clone()),
            create_test_project_unit("proj-2", tenant_id.clone()),
        ];

        let storage = Arc::new(MockStorage::with_units(units));
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_batch_drift_scan().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_batch_drift_scan_without_storage() {
        let engine = Arc::new(GovernanceEngine::new());
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_batch_drift_scan().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Storage not configured")
        );
    }

    #[tokio::test]
    async fn test_run_semantic_analysis_job_without_llm() {
        let tenant_id = create_test_tenant();
        let units = vec![
            create_test_org_unit("org-1", tenant_id.clone()),
            create_test_project_unit("proj-1", tenant_id.clone()),
        ];

        let storage = Arc::new(MockStorage::with_units(units));
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_semantic_analysis_job().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("LLM service not configured")
        );
    }

    #[tokio::test]
    async fn test_run_weekly_report_job_with_org_and_projects() {
        let tenant_id = create_test_tenant();
        let units = vec![
            create_test_org_unit("org-1", tenant_id.clone()),
            create_test_project_unit("proj-1", tenant_id.clone()),
            create_test_project_unit("proj-2", tenant_id.clone()),
        ];

        let storage = Arc::new(MockStorage::with_units(units));

        let result1 = DriftResult {
            project_id: "proj-1".to_string(),
            tenant_id: tenant_id.clone(),
            drift_score: 0.3,
            violations: vec![],
            timestamp: chrono::Utc::now().timestamp(),
        };
        let result2 = DriftResult {
            project_id: "proj-2".to_string(),
            tenant_id: tenant_id.clone(),
            drift_score: 0.7,
            violations: vec![],
            timestamp: chrono::Utc::now().timestamp(),
        };
        storage.store_drift_result(result1).await.unwrap();
        storage.store_drift_result(result2).await.unwrap();

        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_weekly_report_job().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_weekly_report_job_without_storage() {
        let engine = Arc::new(GovernanceEngine::new());
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_weekly_report_job().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Storage not configured")
        );
    }

    #[tokio::test]
    async fn test_run_job_success() {
        let storage = Arc::new(MockStorage::new());
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler
            .run_job("test_job", "test-tenant", async { Ok(()) })
            .await;
        assert!(result.is_ok());
        assert_eq!(storage.job_records.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_run_job_failure() {
        let storage = Arc::new(MockStorage::new());
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler
            .run_job("test_job", "test-tenant", async {
                Err(anyhow::anyhow!("Job failed"))
            })
            .await;
        assert!(result.is_err());
        assert_eq!(storage.job_records.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_run_job_without_storage() {
        let engine = Arc::new(GovernanceEngine::new());
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler
            .run_job("test_job", "test-tenant", async { Ok(()) })
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Storage not configured")
        );
    }

    #[tokio::test]
    async fn test_weekly_report_with_no_projects() {
        let tenant_id = create_test_tenant();
        let units = vec![create_test_org_unit("org-1", tenant_id.clone())];

        let storage = Arc::new(MockStorage::with_units(units));
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_weekly_report_job().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_weekly_report_with_old_drift_results() {
        let tenant_id = create_test_tenant();
        let units = vec![
            create_test_org_unit("org-1", tenant_id.clone()),
            create_test_project_unit("proj-1", tenant_id.clone()),
        ];

        let storage = Arc::new(MockStorage::with_units(units));

        let old_result = DriftResult {
            project_id: "proj-1".to_string(),
            tenant_id: tenant_id.clone(),
            drift_score: 0.5,
            violations: vec![],
            timestamp: chrono::Utc::now().timestamp() - (14 * 24 * 60 * 60),
        };
        storage.store_drift_result(old_result).await.unwrap();

        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_weekly_report_job().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_drift_scan_with_empty_units() {
        let storage = Arc::new(MockStorage::new());
        let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
        let repo: Arc<
            dyn mk_core::traits::KnowledgeRepository<Error = crate::repository::RepositoryError>,
        > = Arc::new(MockRepository::new());

        let config = DeploymentConfig::default();

        let scheduler = GovernanceScheduler::new(
            engine,
            repo,
            config,
            Duration::from_secs(300),
            Duration::from_secs(3600),
            Duration::from_secs(86400),
        );

        let result = scheduler.run_batch_drift_scan().await;
        assert!(result.is_ok());
    }
}
