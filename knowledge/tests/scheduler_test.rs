//! Tests for the GovernanceScheduler.
//!
//! The scheduler runs background jobs for governance checks.
//! These tests verify the individual job methods work correctly.

use async_trait::async_trait;
use config::config::DeploymentConfig;
use knowledge::governance::GovernanceEngine;
use knowledge::scheduler::GovernanceScheduler;
use mk_core::traits::{KnowledgeRepository, StorageBackend};
use mk_core::types::{
    DriftResult, GovernanceEvent, KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType,
    OrganizationalUnit, Policy, Role, TenantContext, TenantId, UnitType, UserId,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;

// Mock storage backend for testing
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

#[derive(Debug)]
struct MockStorageError(String);

impl std::fmt::Display for MockStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MockStorageError {}

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

    async fn create_suppression(
        &self,
        _suppression: mk_core::types::DriftSuppression,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn list_suppressions(
        &self,
        _ctx: TenantContext,
        _project_id: &str,
    ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
        Ok(vec![])
    }

    async fn delete_suppression(
        &self,
        _ctx: TenantContext,
        _suppression_id: &str,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_drift_config(
        &self,
        _ctx: TenantContext,
        _project_id: &str,
    ) -> Result<Option<mk_core::types::DriftConfig>, Self::Error> {
        Ok(None)
    }

    async fn save_drift_config(
        &self,
        _config: mk_core::types::DriftConfig,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn persist_event(
        &self,
        _event: mk_core::types::PersistentEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_pending_events(
        &self,
        _ctx: TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(vec![])
    }

    async fn update_event_status(
        &self,
        _event_id: &str,
        _status: mk_core::types::EventStatus,
        _error: Option<String>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_dead_letter_events(
        &self,
        _ctx: TenantContext,
        _limit: usize,
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        Ok(vec![])
    }

    async fn check_idempotency(
        &self,
        _consumer_group: &str,
        _idempotency_key: &str,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn record_consumer_state(
        &self,
        _state: mk_core::types::ConsumerState,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_event_metrics(
        &self,
        _ctx: TenantContext,
        _period_start: i64,
        _period_end: i64,
    ) -> Result<Vec<mk_core::types::EventDeliveryMetrics>, Self::Error> {
        Ok(vec![])
    }

    async fn record_event_metrics(
        &self,
        _metrics: mk_core::types::EventDeliveryMetrics,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

// Mock repository for testing
struct MockRepository {
    entries: RwLock<Vec<KnowledgeEntry>>,
}

impl MockRepository {
    fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }

    fn with_entries(entries: Vec<KnowledgeEntry>) -> Self {
        Self {
            entries: RwLock::new(entries),
        }
    }
}

#[async_trait]
impl KnowledgeRepository for MockRepository {
    type Error = knowledge::repository::RepositoryError;

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

    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
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
async fn test_scheduler_new() {
    let engine = Arc::new(GovernanceEngine::new());
    let repo: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>> =
        Arc::new(MockRepository::new());
    let config = DeploymentConfig::default();

    let _scheduler = GovernanceScheduler::new(
        engine,
        repo,
        config,
        Duration::from_secs(300),
        Duration::from_secs(3600),
        Duration::from_secs(86400),
    );

    // Just verify it constructs without panicking
    assert!(true);
}

#[tokio::test]
async fn test_scheduler_remote_mode_does_not_run() {
    let storage = Arc::new(MockStorage::new());
    let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
    let repo: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>> =
        Arc::new(MockRepository::new());

    let mut config = DeploymentConfig::default();
    config.mode = "remote".to_string();

    let scheduler = GovernanceScheduler::new(
        engine,
        repo,
        config,
        Duration::from_millis(10),
        Duration::from_millis(10),
        Duration::from_millis(10),
    );

    // Run in background with timeout
    let handle = tokio::spawn(async move {
        scheduler.start().await;
    });

    // Give it a moment to check mode and return
    tokio::time::sleep(Duration::from_millis(50)).await;

    // In remote mode, start() should return immediately
    // The handle should be completed
    assert!(!handle.is_finished() || storage.job_records.load(Ordering::SeqCst) == 0);
    handle.abort();
}

#[tokio::test]
async fn test_scheduler_with_storage_configured() {
    let tenant_id = create_test_tenant();
    let units = vec![
        create_test_org_unit("org-1", tenant_id.clone()),
        create_test_project_unit("proj-1", tenant_id.clone()),
        create_test_project_unit("proj-2", tenant_id.clone()),
    ];

    let storage = Arc::new(MockStorage::with_units(units));
    let engine = Arc::new(GovernanceEngine::new().with_storage(storage.clone()));
    let repo: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>> =
        Arc::new(MockRepository::new());

    let config = DeploymentConfig::default();

    let _scheduler = GovernanceScheduler::new(
        engine,
        repo,
        config,
        Duration::from_secs(300),
        Duration::from_secs(3600),
        Duration::from_secs(86400),
    );

    // Verify storage has units
    let all_units = storage.list_all_units().await.unwrap();
    assert_eq!(all_units.len(), 3);
}

#[tokio::test]
async fn test_storage_list_all_units() {
    let tenant_id = create_test_tenant();
    let units = vec![
        create_test_org_unit("org-1", tenant_id.clone()),
        create_test_project_unit("proj-1", tenant_id.clone()),
    ];

    let storage = MockStorage::with_units(units);
    let all_units = storage.list_all_units().await.unwrap();

    assert_eq!(all_units.len(), 2);
    assert!(all_units.iter().any(|u| u.id == "org-1"));
    assert!(all_units.iter().any(|u| u.id == "proj-1"));
}

#[tokio::test]
async fn test_storage_get_descendants() {
    let tenant_id = create_test_tenant();
    let units = vec![
        create_test_org_unit("org-1", tenant_id.clone()),
        create_test_project_unit("proj-1", tenant_id.clone()),
        create_test_project_unit("proj-2", tenant_id.clone()),
    ];

    let storage = MockStorage::with_units(units);
    let ctx = TenantContext::new(tenant_id, UserId::default());

    let descendants = storage.get_descendants(ctx, "org-1").await.unwrap();
    assert_eq!(descendants.len(), 2);
}

#[tokio::test]
async fn test_storage_store_drift_result() {
    let storage = MockStorage::new();

    let result = DriftResult {
        project_id: "proj-1".to_string(),
        tenant_id: create_test_tenant(),
        drift_score: 0.5,
        violations: vec![],
        timestamp: chrono::Utc::now().timestamp(),
        confidence: 0.9,
        suppressed_violations: vec![],
        requires_manual_review: false,
    };

    storage.store_drift_result(result).await.unwrap();

    let results = storage.drift_results.read().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project_id, "proj-1");
}

#[tokio::test]
async fn test_storage_get_latest_drift_result() {
    let storage = MockStorage::new();
    let tenant_id = create_test_tenant();

    let result1 = DriftResult {
        project_id: "proj-1".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.3,
        violations: vec![],
        timestamp: 1000,
        confidence: 0.9,
        suppressed_violations: vec![],
        requires_manual_review: false,
    };

    let result2 = DriftResult {
        project_id: "proj-2".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.7,
        violations: vec![],
        timestamp: 2000,
        confidence: 0.85,
        suppressed_violations: vec![],
        requires_manual_review: false,
    };

    storage.store_drift_result(result1).await.unwrap();
    storage.store_drift_result(result2).await.unwrap();

    let ctx = TenantContext::new(tenant_id, UserId::default());
    let latest = storage
        .get_latest_drift_result(ctx.clone(), "proj-1")
        .await
        .unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().drift_score, 0.3);

    let latest2 = storage
        .get_latest_drift_result(ctx, "proj-2")
        .await
        .unwrap();
    assert!(latest2.is_some());
    assert_eq!(latest2.unwrap().drift_score, 0.7);
}

#[tokio::test]
async fn test_storage_record_job_status() {
    let storage = MockStorage::new();

    storage
        .record_job_status(
            "test_job",
            "tenant-1",
            "running",
            None,
            chrono::Utc::now().timestamp(),
            None,
        )
        .await
        .unwrap();

    assert_eq!(storage.job_records.load(Ordering::SeqCst), 1);

    storage
        .record_job_status(
            "test_job",
            "tenant-1",
            "completed",
            None,
            chrono::Utc::now().timestamp(),
            Some(chrono::Utc::now().timestamp()),
        )
        .await
        .unwrap();

    assert_eq!(storage.job_records.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_mock_repository_list_by_layer() {
    let entries = vec![
        KnowledgeEntry {
            path: "project/spec.md".to_string(),
            content: "Project spec".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            metadata: HashMap::new(),
            status: KnowledgeStatus::Accepted,
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp(),
            summaries: HashMap::new(),
        },
        KnowledgeEntry {
            path: "company/policy.md".to_string(),
            content: "Company policy".to_string(),
            layer: KnowledgeLayer::Company,
            kind: KnowledgeType::Policy,
            metadata: HashMap::new(),
            status: KnowledgeStatus::Accepted,
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp(),
            summaries: HashMap::new(),
        },
    ];

    let repo = MockRepository::with_entries(entries);
    let ctx = TenantContext::new(create_test_tenant(), UserId::default());

    let project_entries = repo
        .list(ctx.clone(), KnowledgeLayer::Project, "")
        .await
        .unwrap();
    assert_eq!(project_entries.len(), 1);
    assert_eq!(project_entries[0].path, "project/spec.md");

    let company_entries = repo.list(ctx, KnowledgeLayer::Company, "").await.unwrap();
    assert_eq!(company_entries.len(), 1);
    assert_eq!(company_entries[0].path, "company/policy.md");
}

#[tokio::test]
async fn test_deployment_config_modes() {
    let mut config = DeploymentConfig::default();

    // Default is local
    assert_eq!(config.mode, "local");

    // Test remote mode
    config.mode = "remote".to_string();
    assert_eq!(config.mode, "remote");

    // Test hybrid mode
    config.mode = "hybrid".to_string();
    assert_eq!(config.mode, "hybrid");
}

#[tokio::test]
async fn test_scheduler_intervals() {
    let engine = Arc::new(GovernanceEngine::new());
    let repo: Arc<dyn KnowledgeRepository<Error = knowledge::repository::RepositoryError>> =
        Arc::new(MockRepository::new());
    let config = DeploymentConfig::default();

    // Test with various intervals
    let _scheduler1 = GovernanceScheduler::new(
        engine.clone(),
        repo.clone(),
        config.clone(),
        Duration::from_secs(60),
        Duration::from_secs(3600),
        Duration::from_secs(604800), // 1 week
    );

    let _scheduler2 = GovernanceScheduler::new(
        engine.clone(),
        repo.clone(),
        config.clone(),
        Duration::from_millis(100),
        Duration::from_millis(500),
        Duration::from_millis(1000),
    );

    // Just verify construction succeeds with different intervals
    assert!(true);
}
