use knowledge::federation::{
    FederationConfig, FederationProvider, KnowledgeManifest, UpstreamConfig,
};
use knowledge::governance::GovernanceEngine;
use knowledge::repository::RepositoryError;
use memory::manager::MemoryManager;
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{
    ConstraintOperator, ConstraintSeverity, ConstraintTarget, KnowledgeEntry, KnowledgeLayer,
    KnowledgeStatus, KnowledgeType, Policy, PolicyMode, PolicyRule, RuleMergeStrategy, RuleType,
    TenantContext, TenantId,
};
use std::collections::HashMap;
use std::sync::Arc;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;

struct MockFedProvider {
    config: FederationConfig,
    should_fail: bool,
}

#[async_trait::async_trait]
impl FederationProvider for MockFedProvider {
    fn config(&self) -> &FederationConfig {
        &self.config
    }

    async fn fetch_upstream_manifest(
        &self,
        _id: &str,
    ) -> Result<KnowledgeManifest, RepositoryError> {
        Ok(KnowledgeManifest {
            version: "1.0".to_string(),
            items: HashMap::new(),
        })
    }

    async fn sync_upstream(
        &self,
        _id: &str,
        _path: &std::path::Path,
    ) -> Result<(), RepositoryError> {
        if self.should_fail {
            return Err(RepositoryError::InvalidPath(
                "Local changes conflict with upstream".to_string(),
            ));
        }
        Ok(())
    }
}

struct MockRepo;

#[async_trait::async_trait]
impl KnowledgeRepository for MockRepo {
    type Error = RepositoryError;
    async fn store(
        &self,
        _ctx: TenantContext,
        _e: KnowledgeEntry,
        _m: &str,
    ) -> Result<String, Self::Error> {
        Ok("hash".into())
    }
    async fn get(
        &self,
        _ctx: TenantContext,
        _l: KnowledgeLayer,
        _p: &str,
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        Ok(None)
    }
    async fn list(
        &self,
        _ctx: TenantContext,
        _l: KnowledgeLayer,
        _p: &str,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }
    async fn delete(
        &self,
        _ctx: TenantContext,
        _l: KnowledgeLayer,
        _p: &str,
        _m: &str,
    ) -> Result<String, Self::Error> {
        Ok("hash".into())
    }
    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
        Ok(Some("head".into()))
    }
    async fn get_affected_items(
        &self,
        _ctx: TenantContext,
        _f: &str,
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        Ok(vec![])
    }
    async fn search(
        &self,
        _ctx: TenantContext,
        _q: &str,
        _l: Vec<KnowledgeLayer>,
        _li: usize,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }
    fn root_path(&self) -> Option<std::path::PathBuf> {
        Some("data/test".into())
    }
}

struct MockPersister;
#[async_trait::async_trait]
impl SyncStatePersister for MockPersister {
    async fn load(
        &self,
        _tenant_id: &TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }
    async fn save(
        &self,
        _tenant_id: &TenantId,
        _s: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[tokio::test]
async fn test_sync_federation_conflict_recording() {
    let memory = Arc::new(MemoryManager::new());
    let repo = Arc::new(MockRepo);
    let gov = Arc::new(GovernanceEngine::new());
    let fed_config = FederationConfig {
        upstreams: vec![UpstreamConfig {
            id: "hub-1".to_string(),
            url: "http://test".to_string(),
            branch: "main".to_string(),
            auth_token: None,
        }],
        sync_interval_secs: 60,
    };
    let fed = Arc::new(MockFedProvider {
        config: fed_config,
        should_fail: true,
    });
    let persister = Arc::new(MockPersister);

    let sync_manager = SyncManager::new(
        memory,
        repo,
        gov,
        config::config::DeploymentConfig::default(),
        Some(fed.clone() as Arc<dyn FederationProvider>),
        persister,
        None,
    )
    .await
    .unwrap();

    sync_manager
        .sync_federation(TenantContext::default(), fed.as_ref())
        .await
        .unwrap();

    let ctx = TenantContext::default();
    let state = sync_manager.get_state(&ctx.tenant_id).await.unwrap();
    assert_eq!(state.federation_conflicts.len(), 1);
    assert_eq!(state.federation_conflicts[0].upstream_id, "hub-1");
    assert!(state.federation_conflicts[0].reason.contains("conflict"));
}

#[tokio::test]
async fn test_sync_governance_telemetry() {
    let memory = Arc::new(MemoryManager::new());
    let repo = Arc::new(MockRepo);
    let mut gov = GovernanceEngine::new();

    gov.add_policy(Policy {
        id: "p-test".to_string(),
        name: "Test Policy".to_string(),
        description: None,
        layer: KnowledgeLayer::Project,
        mode: PolicyMode::Mandatory,
        merge_strategy: RuleMergeStrategy::Merge,
        rules: vec![PolicyRule {
            id: "r-test".to_string(),
            rule_type: RuleType::Deny,
            target: ConstraintTarget::Code,
            operator: ConstraintOperator::MustMatch,
            value: serde_json::json!("forbidden"),
            severity: ConstraintSeverity::Block,
            message: "Forbidden content detected".to_string(),
        }],
        metadata: HashMap::new(),
    });

    let gov = Arc::new(gov);
    let persister = Arc::new(MockPersister);

    let sync_manager = SyncManager::new(
        memory,
        repo,
        gov,
        config::config::DeploymentConfig::default(),
        None,
        persister,
        None,
    )
    .await
    .unwrap();

    let entry = KnowledgeEntry {
        path: "forbidden.md".to_string(),
        content: "this is forbidden".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        metadata: HashMap::new(),
        status: KnowledgeStatus::Accepted,
        commit_hash: None,
        author: None,
        updated_at: chrono::Utc::now().timestamp(),
        summaries: HashMap::new(),
    };

    let mut state = SyncState::default();
    let _ = sync_manager
        .sync_entry(TenantContext::default(), &entry, &mut state)
        .await;

    assert_eq!(state.stats.total_governance_blocks, 1);
    assert_eq!(state.failed_items.len(), 1);
    assert!(
        state.failed_items[0]
            .error
            .contains("Governance violation (BLOCK)")
    );
}
