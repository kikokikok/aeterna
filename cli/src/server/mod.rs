pub mod bootstrap;
pub mod health;
pub mod mcp_transport;
pub mod metrics;
pub mod openspec;
pub mod router;
pub mod webhooks;

use std::sync::Arc;

use agent_a2a::{AuthState as A2aAuthState, Config as A2aConfig};
use idp_sync::config::IdpSyncConfig;
use idp_sync::{IdpClient, IdpSyncService};
use knowledge::api::GovernanceDashboardApi;
use knowledge::git_provider::GitProvider;
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::RepositoryError;
use memory::manager::MemoryManager;
use memory::reasoning::ReflectiveReasoner;
use mk_core::traits::{AuthorizationService, EventPublisher, KnowledgeRepository};
use storage::events::EventError;
use storage::governance::GovernanceStorage;
use storage::graph_duckdb::DuckDbGraphStore;
use storage::postgres::PostgresBackend;
use sync::bridge::SyncManager;
use sync::websocket::WsServer;
use tools::server::McpServer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub postgres: Arc<PostgresBackend>,
    pub memory_manager: Arc<MemoryManager>,
    pub knowledge_manager: Arc<KnowledgeManager>,
    pub knowledge_repository: Arc<dyn KnowledgeRepository<Error = RepositoryError> + Send + Sync>,
    pub governance_engine: Arc<GovernanceEngine>,
    pub governance_dashboard: Arc<GovernanceDashboardApi>,
    pub auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync>,
    pub mcp_server: Arc<McpServer>,
    pub sync_manager: Arc<SyncManager>,
    pub git_provider: Option<Arc<dyn GitProvider>>,
    pub webhook_secret: Option<String>,
    pub event_publisher: Option<Arc<dyn EventPublisher<Error = EventError> + Send + Sync>>,
    pub graph_store: Option<Arc<DuckDbGraphStore>>,
    pub governance_storage: Option<Arc<GovernanceStorage>>,
    pub reasoner: Option<Arc<dyn ReflectiveReasoner>>,
    pub ws_server: Arc<WsServer>,
    pub a2a_config: Arc<A2aConfig>,
    pub a2a_auth_state: Arc<A2aAuthState>,
    pub idp_config: Option<Arc<IdpSyncConfig>>,
    pub idp_sync_service: Option<Arc<IdpSyncService>>,
    pub idp_client: Option<Arc<dyn IdpClient>>,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
}
