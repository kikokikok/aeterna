//! Shared test harness for end-to-end server-runtime tests.
//!
//! Originally inlined inside `server_runtime_test.rs`; extracted here so
//! the §13 tenant-provisioning consistency suite can reuse the same
//! `AppState` constructor without duplicating ~150 lines of mock wiring.
//!
//! The legacy `server_runtime_test.rs` still carries its own private
//! copy of these helpers — we keep that duplication on purpose to keep
//! the diff for §13.2 zero-impact on the existing 50+ tests in that
//! file. A follow-up cleanup pass can de-dupe once the consistency
//! suite has stabilised in CI.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use aeterna::server::plugin_auth::{
    PluginTokenClaims, RefreshTokenStore, RefreshTokenStoreBackend,
};
use aeterna::server::{AppState, PluginAuthState};
use agent_a2a::config::TrustedIdentityConfig;
use async_trait::async_trait;
use knowledge::api::GovernanceDashboardApi;
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::{GitRepository, RepositoryError};
use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
use memory::manager::MemoryManager;
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::{AuthorizationService, KnowledgeRepository};
use mk_core::types::{
    KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, ReasoningStrategy,
    ReasoningTrace, Role, RoleIdentifier, TenantContext, TenantId, UserId,
};
use storage::governance::GovernanceStorage;
use storage::postgres::PostgresBackend;
use storage::secret_provider::LocalSecretProvider;
use storage::tenant_config_provider::KubernetesTenantConfigProvider;
use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
use sync::bridge::SyncManager;
use sync::state_persister::FilePersister;
use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
use tempfile::TempDir;
use testing::postgres;
use tools::server::McpServer;

pub struct MockAuth;

#[async_trait]
impl AuthorizationService for MockAuth {
    type Error = anyhow::Error;

    async fn check_permission(
        &self,
        _ctx: &TenantContext,
        _action: &str,
        _resource: &str,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn get_user_roles(
        &self,
        _ctx: &TenantContext,
    ) -> Result<Vec<RoleIdentifier>, Self::Error> {
        Ok(vec![Role::Developer.into()])
    }

    async fn assign_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: RoleIdentifier,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn remove_role(
        &self,
        _ctx: &TenantContext,
        _user_id: &UserId,
        _role: RoleIdentifier,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct MockRepo {
    items: tokio::sync::RwLock<HashMap<(KnowledgeLayer, String), KnowledgeEntry>>,
}

impl MockRepo {
    pub fn new() -> Self {
        Self {
            items: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl KnowledgeRepository for MockRepo {
    type Error = RepositoryError;

    async fn get(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        path: &str,
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        Ok(self
            .items
            .read()
            .await
            .get(&(layer, path.to_string()))
            .cloned())
    }

    async fn store(
        &self,
        _ctx: TenantContext,
        entry: KnowledgeEntry,
        _message: &str,
    ) -> Result<String, Self::Error> {
        self.items
            .write()
            .await
            .insert((entry.layer, entry.path.clone()), entry);
        Ok("mock-commit".to_string())
    }

    async fn list(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        prefix: &str,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(self
            .items
            .read()
            .await
            .iter()
            .filter(|((l, p), _)| *l == layer && p.starts_with(prefix))
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn delete(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        path: &str,
        _message: &str,
    ) -> Result<String, Self::Error> {
        self.items.write().await.remove(&(layer, path.to_string()));
        Ok("mock-commit".to_string())
    }

    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
        Ok(Some("mock-commit".to_string()))
    }

    async fn get_affected_items(
        &self,
        _ctx: TenantContext,
        _since: &str,
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        Ok(vec![])
    }

    async fn search(
        &self,
        _ctx: TenantContext,
        _query: &str,
        _layers: Vec<KnowledgeLayer>,
        _limit: usize,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(vec![])
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

pub struct TestNoopReasoner;

#[async_trait]
impl ReflectiveReasoner for TestNoopReasoner {
    async fn reason(&self, query: &str, _ctx: Option<&str>) -> anyhow::Result<ReasoningTrace> {
        let now = chrono::Utc::now();
        Ok(ReasoningTrace {
            strategy: ReasoningStrategy::SemanticOnly,
            thought_process: "test noop".to_string(),
            refined_query: Some(query.to_string()),
            start_time: now,
            end_time: now,
            timed_out: false,
            duration_ms: 0,
            metadata: HashMap::new(),
        })
    }
}

pub struct MockTokenValidator;

#[async_trait]
impl TokenValidator for MockTokenValidator {
    async fn validate(&self, token: &str) -> WsResult<AuthToken> {
        Ok(AuthToken {
            user_id: token.to_string(),
            tenant_id: "default".to_string(),
            permissions: vec![],
            expires_at: 0,
        })
    }
}

fn sample_entry(path: &str) -> KnowledgeEntry {
    KnowledgeEntry {
        path: path.to_string(),
        content: "sample content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Spec,
        status: KnowledgeStatus::Draft,
        summaries: HashMap::new(),
        metadata: HashMap::new(),
        commit_hash: None,
        author: None,
        updated_at: 0,
    }
}

/// Build a fully-wired `AppState` against a fresh testcontainer Postgres.
///
/// Returns `None` when Docker is unavailable so the caller can skip the
/// test gracefully (matches the existing `server_runtime_test` pattern).
pub async fn build_test_state() -> Option<(Arc<AppState>, TempDir)> {
    build_test_state_with_plugin_auth(config::PluginAuthConfig::default()).await
}

pub async fn build_test_state_with_plugin_auth(
    plugin_auth_config: config::PluginAuthConfig,
) -> Option<(Arc<AppState>, TempDir)> {
    let tempdir = tempfile::tempdir().unwrap();
    let repo = Arc::new(MockRepo::new());
    repo.store(
        TenantContext::new(
            TenantId::new("default".to_string()).unwrap(),
            UserId::new("system".to_string()).unwrap(),
        ),
        sample_entry("specs/example.md"),
        "seed",
    )
    .await
    .unwrap();

    let fixture = postgres().await?;
    let postgres = Arc::new(PostgresBackend::new(fixture.url()).await.ok()?);
    postgres.initialize_schema().await.ok()?;
    let governance_engine = Arc::new(GovernanceEngine::new());
    let git_repo = Arc::new(GitRepository::new(tempdir.path()).unwrap());
    let knowledge_manager = Arc::new(KnowledgeManager::new(
        git_repo.clone(),
        governance_engine.clone(),
    ));
    let memory_manager = Arc::new(MemoryManager::new());
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_manager.clone(),
            config::config::DeploymentConfig::default(),
            None,
            Arc::new(FilePersister::new(std::env::temp_dir())),
            None,
        )
        .await
        .unwrap(),
    );
    let auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync> =
        Arc::new(MockAuth);
    let dashboard = Arc::new(GovernanceDashboardApi::new(
        governance_engine.clone(),
        postgres.clone(),
        config::config::DeploymentConfig::default(),
    ));
    let mcp_server = Arc::new(McpServer::new(
        memory_manager.clone(),
        sync_manager.clone(),
        knowledge_manager.clone(),
        git_repo.clone(),
        postgres.clone(),
        governance_engine.clone(),
        Arc::new(TestNoopReasoner),
        auth_service.clone(),
        None,
        None,
        None,
    ));
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);
    let tenant_store = Arc::new(TenantStore::new(postgres.pool().clone()));
    let tenant_repository_binding_store =
        Arc::new(TenantRepositoryBindingStore::new(postgres.pool().clone()));
    let git_provider_connection_registry =
        Arc::new(storage::git_provider_connection_store::InMemoryGitProviderConnectionStore::new());
    let tenant_repo_resolver = Arc::new(
        TenantRepositoryResolver::new(
            tenant_repository_binding_store.clone(),
            std::env::temp_dir(),
            Arc::new(LocalSecretProvider::new(HashMap::new())),
        )
        .with_connection_registry(git_provider_connection_registry.clone()),
    );

    Some((
        Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            revocation_cache: Default::default(),
            postgres: postgres.clone(),
            memory_manager,
            knowledge_manager,
            knowledge_repository: repo,
            governance_engine,
            governance_dashboard: dashboard,
            auth_service,
            mcp_server,
            sync_manager,
            git_provider: None,
            webhook_secret: None,
            event_publisher: None,
            graph_store: None,
            governance_storage: Some(Arc::new(GovernanceStorage::new(postgres.pool().clone()))),
            reasoner: None,
            ws_server: Arc::new(WsServer::new(Arc::new(MockTokenValidator))),
            a2a_config: Arc::new(agent_a2a::Config::default()),
            a2a_auth_state: Arc::new(agent_a2a::AuthState {
                api_key: None,
                jwt_secret: None,
                enabled: false,
                trusted_identity: TrustedIdentityConfig::default(),
            }),
            plugin_auth_state: Arc::new(PluginAuthState {
                config: plugin_auth_config,
                postgres: Some(postgres.clone()),
                refresh_store: RefreshTokenStoreBackend::InMemory(RefreshTokenStore::new()),
            }),
            k8s_auth_config: config::KubernetesAuthConfig::default(),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider: Arc::new(
                KubernetesTenantConfigProvider::new_in_memory_for_tests("default".to_string()),
            ),
            provider_registry: Arc::new(memory::provider_registry::TenantProviderRegistry::new(
                None, None,
            )),
            git_provider_connection_registry,
            redis_conn: None,
            redis_url: None,
            tenant_runtime_state: Arc::new(
                aeterna::server::tenant_runtime_state::TenantRuntimeRegistry::new(),
            ),
            bootstrap_tracker: Arc::new(aeterna::server::bootstrap_tracker::BootstrapTracker::new()),
        }),
        tempdir,
    ))
}

/// Marker so unused `PluginTokenClaims` import doesn't trigger warnings
/// in callers that don't exercise plugin auth paths.
#[allow(dead_code)]
fn _claims_marker(_: PluginTokenClaims) {}
