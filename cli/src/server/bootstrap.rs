use std::path::PathBuf;
use std::sync::Arc;

use adapters::auth::cedar::CedarAuthorizer;
use adapters::auth::permit::PermitAuthorizationService;
use agent_a2a::{AuthState as A2aAuthState, Config as A2aConfig};
use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use idp_sync::azure::AzureAdClient;
use idp_sync::config::{AzureAdConfig, IdpProvider, IdpSyncConfig, OktaConfig};
use idp_sync::okta::OktaClient;
use idp_sync::{IdpClient, IdpSyncService};
use knowledge::api::GovernanceDashboardApi;
use knowledge::git_provider::{GitHubProvider, GitProvider};
use knowledge::governance::GovernanceEngine;
use knowledge::manager::KnowledgeManager;
use knowledge::repository::{GitRepository, RemoteConfig};
use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
use memory::embedding::create_embedding_service_from_env;
use memory::llm::create_llm_service_from_env;
use memory::manager::MemoryManager;
use memory::reasoning::{DefaultReflectiveReasoner, ReflectiveReasoner};
use mk_core::traits::AuthorizationService;
use mk_core::types::{
    INSTANCE_SCOPE_TENANT_ID, ReasoningStrategy, ReasoningTrace, Role, RoleIdentifier,
    TenantContext, UserId,
};
use storage::git_provider_connection_store::InMemoryGitProviderConnectionStore;
use storage::governance::GovernanceStorage;
use storage::graph_duckdb::{DuckDbGraphConfig, DuckDbGraphStore};
use storage::postgres::PostgresBackend;
use storage::secret_provider::LocalSecretProvider;
use storage::tenant_config_provider::KubernetesTenantConfigProvider;
use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
use sync::bridge::SyncManager;
use sync::state_persister::DatabasePersister;
use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
use tools::server::McpServer;

use super::plugin_auth::RefreshTokenStore;
use super::{AppState, PluginAuthState};

pub async fn bootstrap() -> anyhow::Result<Arc<AppState>> {
    validate_required_env()?;

    let config = Arc::new(config::load_from_env()?);
    let postgres = Arc::new(PostgresBackend::new(&postgres_connection_url(&config)).await?);
    postgres.initialize_schema().await?;

    if config.admin_bootstrap.email.is_some() {
        seed_platform_admin(postgres.pool(), &config.admin_bootstrap).await?;
    }

    let governance_storage = Some(Arc::new(GovernanceStorage::new(postgres.pool().clone())));

    let git_provider: Option<Arc<dyn GitProvider>> = if let (Some(owner), Some(repo)) = (
        &config.knowledge_repo.github_owner,
        &config.knowledge_repo.github_repo,
    ) {
        if let (Some(app_id), Some(installation_id), Some(pem)) = (
            config.knowledge_repo.github_app_id,
            config.knowledge_repo.github_installation_id,
            &config.knowledge_repo.github_app_pem,
        ) {
            tracing::info!(
                "Initializing GitHub App auth (app_id={app_id}, installation_id={installation_id})"
            );
            Some(Arc::new(
                GitHubProvider::new_with_app(
                    app_id,
                    installation_id,
                    pem,
                    owner.clone(),
                    repo.clone(),
                    config.knowledge_repo.webhook_secret.clone(),
                )
                .await
                .context("Failed to build GitHub App provider")?,
            ))
        } else if let Some(token) = &config.knowledge_repo.github_token {
            tracing::info!("Initializing GitHub PAT auth");
            Some(Arc::new(
                GitHubProvider::new(
                    token,
                    owner.clone(),
                    repo.clone(),
                    config.knowledge_repo.webhook_secret.clone(),
                )
                .context("Failed to build GitHub PAT provider")?,
            ))
        } else {
            None
        }
    } else {
        None
    };

    let remote_config = config
        .knowledge_repo
        .remote_url
        .as_ref()
        .map(|url| RemoteConfig {
            url: url.clone(),
            branch: config.knowledge_repo.branch.clone(),
            git_provider: git_provider.clone(),
        });

    let knowledge_repository = Arc::new(
        GitRepository::new_with_remote(knowledge_repo_path(), remote_config)
            .context("Failed to initialize knowledge repository")?,
    );

    let auth_for_memory = build_boxed_auth_service()?;
    let auth_service = build_anyhow_auth_service()?;

    let features = RuntimeFeatures::from_env();

    let llm_service = if features.reflective {
        create_llm_service_from_env()?
    } else {
        None
    };
    let reasoner = llm_service
        .clone()
        .map(|llm| Arc::new(DefaultReflectiveReasoner::new(llm)) as Arc<dyn ReflectiveReasoner>);
    let mcp_reasoner = reasoner
        .clone()
        .unwrap_or_else(|| Arc::new(NoopReflectiveReasoner) as Arc<dyn ReflectiveReasoner>);

    let graph_store = create_graph_store(&config)?;

    let mut memory_config = config.memory.clone();
    if !features.rlm {
        memory_config.rlm.enabled = false;
    }
    if !features.reflective {
        memory_config.reasoning.enabled = false;
    }
    if !features.cca {
        tracing::info!("CCA feature disabled via AETERNA_FEATURE_CCA");
    }
    if !features.radkit {
        tracing::info!("Radkit/A2A feature disabled via AETERNA_FEATURE_RADKIT");
    }

    let mut memory_manager = MemoryManager::new()
        .with_config(memory_config)
        .with_auth_service(auth_for_memory);

    if let Some(embedding_service) = create_embedding_service_from_env()? {
        memory_manager = memory_manager.with_embedding_service(embedding_service.clone());
    }

    if let Some(llm_service) = llm_service.clone() {
        memory_manager = memory_manager.with_llm_service(llm_service);
    }

    if let Some(reasoner) = reasoner.clone() {
        memory_manager = memory_manager.with_reasoner(reasoner);
    }

    if let Some(graph_store) = graph_store.clone() {
        memory_manager = memory_manager.with_graph_store(graph_store.clone());
    }

    let governance_engine = Arc::new(build_governance_engine(
        postgres.clone(),
        knowledge_repository.clone(),
        llm_service,
    ));

    let knowledge_manager = Arc::new(KnowledgeManager::new(
        knowledge_repository.clone(),
        governance_engine.clone(),
    ));
    memory_manager = memory_manager.with_knowledge_manager(knowledge_manager.clone());

    let memory_manager = Arc::new(memory_manager);

    let persister = Arc::new(DatabasePersister::new(postgres.clone(), "sync".to_string()));
    let sync_manager = Arc::new(
        SyncManager::new(
            memory_manager.clone(),
            knowledge_manager.clone(),
            config.deployment.clone(),
            None,
            persister,
            None,
        )
        .await?,
    );

    let governance_dashboard = Arc::new(GovernanceDashboardApi::new(
        governance_engine.clone(),
        postgres.clone(),
        config.deployment.clone(),
    ));

    let mcp_server = Arc::new(McpServer::new(
        memory_manager.clone(),
        sync_manager.clone(),
        knowledge_manager.clone(),
        knowledge_repository.clone(),
        postgres.clone(),
        governance_engine.clone(),
        mcp_reasoner,
        auth_service.clone(),
        None,
        graph_store.clone().map(|g| g as Arc<DuckDbGraphStore>),
        governance_storage.clone(),
    ));

    let a2a_config = Arc::new(if features.radkit {
        A2aConfig::from_env().unwrap_or_default()
    } else {
        A2aConfig::default()
    });
    let a2a_auth_state = Arc::new(A2aAuthState {
        api_key: a2a_config.auth.api_key.clone(),
        jwt_secret: a2a_config.auth.jwt_secret.clone(),
        enabled: a2a_config.auth.enabled,
        trusted_identity: a2a_config.auth.trusted_identity.clone(),
    });
    a2a_auth_state.validate()?;
    let plugin_auth_state = Arc::new(PluginAuthState {
        config: config.plugin_auth.clone(),
        postgres: Some(postgres.clone()),
        refresh_store: RefreshTokenStore::new(),
    });

    let (idp_config, idp_client, idp_sync_service) = build_optional_idp_services(postgres.clone())?;
    let ws_server = Arc::new(WsServer::new(Arc::new(AllowAllTokenValidator)));
    let webhook_secret = config.knowledge_repo.webhook_secret.clone();

    let (shutdown_tx, _) = tokio::sync::watch::channel(false);

    // Tenant stores and repository resolver
    let tenant_store = Arc::new(TenantStore::new(postgres.pool().clone()));
    let tenant_repository_binding_store =
        Arc::new(TenantRepositoryBindingStore::new(postgres.pool().clone()));
    let secret_provider = Arc::new(LocalSecretProvider::new(std::collections::HashMap::new()));
    let git_provider_connection_registry = Arc::new(InMemoryGitProviderConnectionStore::new());
    let tenant_repo_resolver = Arc::new(
        TenantRepositoryResolver::new(
            tenant_repository_binding_store.clone(),
            knowledge_repo_path(),
            secret_provider,
        )
        .with_connection_registry(git_provider_connection_registry.clone()),
    );
    let tenant_config_provider = Arc::new(KubernetesTenantConfigProvider::new(
        tenant_config_provider_namespace(),
    ));

    Ok(Arc::new(AppState {
        config,
        postgres,
        memory_manager,
        knowledge_manager,
        knowledge_repository,
        governance_engine,
        governance_dashboard,
        auth_service,
        mcp_server,
        sync_manager,
        git_provider,
        webhook_secret,
        event_publisher: None,
        graph_store,
        governance_storage,
        reasoner,
        ws_server,
        a2a_config,
        a2a_auth_state,
        plugin_auth_state,
        idp_config,
        idp_sync_service,
        idp_client,
        shutdown_tx: Arc::new(shutdown_tx),
        tenant_store,
        tenant_repository_binding_store,
        tenant_repo_resolver,
        tenant_config_provider,
        git_provider_connection_registry,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeFeatures {
    cca: bool,
    radkit: bool,
    rlm: bool,
    reflective: bool,
}

impl RuntimeFeatures {
    fn from_env() -> Self {
        Self {
            cca: feature_enabled("AETERNA_FEATURE_CCA", true),
            radkit: feature_enabled("AETERNA_FEATURE_RADKIT", true),
            rlm: feature_enabled("AETERNA_FEATURE_RLM", true),
            reflective: feature_enabled("AETERNA_FEATURE_REFLECTIVE", true),
        }
    }
}

fn feature_enabled(key: &str, default: bool) -> bool {
    std::env::var(key)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(default)
}

fn validate_required_env() -> anyhow::Result<()> {
    let mut missing = Vec::new();

    if std::env::var("AETERNA_POSTGRESQL_HOST").is_err() && std::env::var("PG_HOST").is_err() {
        missing.push("AETERNA_POSTGRESQL_HOST|PG_HOST");
    }
    if std::env::var("AETERNA_POSTGRESQL_DATABASE").is_err()
        && std::env::var("PG_DATABASE").is_err()
    {
        missing.push("AETERNA_POSTGRESQL_DATABASE|PG_DATABASE");
    }
    if std::env::var("AETERNA_REDIS_HOST").is_err() && std::env::var("RD_HOST").is_err() {
        missing.push("AETERNA_REDIS_HOST|RD_HOST");
    }

    if missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "Required environment variables are not set: {}",
            missing.join(", ")
        )
    }
}

fn postgres_connection_url(config: &config::Config) -> String {
    let pg = &config.providers.postgres;
    format!(
        "postgres://{}:{}@{}:{}/{}",
        pg.username, pg.password, pg.host, pg.port, pg.database
    )
}

fn knowledge_repo_path() -> PathBuf {
    std::env::var("AETERNA_KNOWLEDGE_REPO_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./knowledge-repo"))
}

fn tenant_config_provider_namespace() -> String {
    std::env::var("AETERNA_K8S_NAMESPACE").unwrap_or_else(|_| "default".to_string())
}

fn create_graph_store(config: &config::Config) -> anyhow::Result<Option<Arc<DuckDbGraphStore>>> {
    if !config.providers.graph.enabled {
        return Ok(None);
    }

    let graph = DuckDbGraphStore::new(DuckDbGraphConfig {
        path: config.providers.graph.database_path.clone(),
        s3_bucket: config.providers.graph.s3_bucket.clone(),
        s3_prefix: config.providers.graph.s3_prefix.clone(),
        s3_endpoint: config.providers.graph.s3_endpoint.clone(),
        s3_region: Some(config.providers.graph.s3_region.clone()),
        ..Default::default()
    })?;

    Ok(Some(Arc::new(graph)))
}

fn build_governance_engine(
    postgres: Arc<PostgresBackend>,
    repository: Arc<GitRepository>,
    llm_service: Option<
        Arc<
            dyn mk_core::traits::LlmService<Error = Box<dyn std::error::Error + Send + Sync>>
                + Send
                + Sync,
        >,
    >,
) -> GovernanceEngine {
    let mut engine = GovernanceEngine::new()
        .with_storage(postgres)
        .with_repository(repository);

    if let Some(llm_service) = llm_service {
        engine = engine.with_llm_service(llm_service);
    }

    engine
}

fn build_anyhow_auth_service()
-> anyhow::Result<Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync>> {
    let backend = std::env::var("AETERNA_AUTH_BACKEND").unwrap_or_else(|_| "allow-all".to_string());

    match backend.as_str() {
        "cedar" => {
            let policy = std::env::var("AETERNA_CEDAR_POLICY_TEXT")
                .context("AETERNA_CEDAR_POLICY_TEXT is required for cedar auth")?;
            let schema = std::env::var("AETERNA_CEDAR_SCHEMA_TEXT")
                .context("AETERNA_CEDAR_SCHEMA_TEXT is required for cedar auth")?;
            let mut service = CedarAuthorizer::new(&policy, &schema)?;
            if let Ok(url) = std::env::var("AETERNA_OPAL_FETCHER_URL") {
                tracing::info!(url = %url, "Cedar auth: OPAL fetcher configured");
                service = service.with_opal_fetcher(url);
            }
            Ok(Arc::new(AnyhowAuthWrapper { inner: service }))
        }
        "permit" => {
            let api_key = std::env::var("AETERNA_PERMIT_API_KEY")
                .context("AETERNA_PERMIT_API_KEY is required for permit auth")?;
            let pdp_url = std::env::var("AETERNA_PERMIT_PDP_URL")
                .context("AETERNA_PERMIT_PDP_URL is required for permit auth")?;
            Ok(Arc::new(AnyhowAuthWrapper {
                inner: PermitAuthorizationService::new(&api_key, &pdp_url),
            }))
        }
        _ => {
            // Allow-all is only safe in local development or test environments.
            // Emit a warning so operators know this is active.  To suppress this
            // warning in a legitimate dev environment, set
            // AETERNA_ALLOW_PERMISSIVE_AUTH=dev.
            let permissive_mode =
                std::env::var("AETERNA_ALLOW_PERMISSIVE_AUTH").unwrap_or_default();
            if permissive_mode != "dev" {
                tracing::warn!(
                    backend = %backend,
                    "Using allow-all authorization service.                      This grants every caller full access and MUST NOT be used in production.                      Set AETERNA_AUTH_BACKEND=cedar or AETERNA_AUTH_BACKEND=permit,                      or set AETERNA_ALLOW_PERMISSIVE_AUTH=dev to silence this warning."
                );
            } else {
                tracing::debug!("Allow-all auth active (AETERNA_ALLOW_PERMISSIVE_AUTH=dev)");
            }
            Ok(Arc::new(AllowAllAuthService))
        }
    }
}

fn build_boxed_auth_service() -> anyhow::Result<
    Arc<dyn AuthorizationService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
> {
    Ok(Arc::new(AllowAllBoxedAuthService))
}

fn build_optional_idp_services(
    postgres: Arc<PostgresBackend>,
) -> anyhow::Result<(
    Option<Arc<IdpSyncConfig>>,
    Option<Arc<dyn IdpClient>>,
    Option<Arc<IdpSyncService>>,
)> {
    let provider = match std::env::var("AETERNA_IDP_PROVIDER") {
        Ok(provider) => provider,
        Err(_) => return Ok((None, None, None)),
    };

    let database_url = std::env::var("AETERNA_IDP_DATABASE_URL").unwrap_or_default();
    let webhook_port = std::env::var("AETERNA_IDP_WEBHOOK_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8090);
    let webhook_secret = std::env::var("AETERNA_IDP_WEBHOOK_SECRET").ok();

    let provider = match provider.as_str() {
        "okta" => IdpProvider::Okta(OktaConfig {
            domain: std::env::var("AETERNA_OKTA_DOMAIN")?,
            api_token: std::env::var("AETERNA_OKTA_API_TOKEN")?,
            scim_enabled: std::env::var("AETERNA_OKTA_SCIM_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            group_filter: std::env::var("AETERNA_OKTA_GROUP_FILTER").ok(),
            user_filter: std::env::var("AETERNA_OKTA_USER_FILTER").ok(),
        }),
        "azure" => IdpProvider::AzureAd(AzureAdConfig {
            tenant_id: std::env::var("AETERNA_AZURE_TENANT_ID")?,
            client_id: std::env::var("AETERNA_AZURE_CLIENT_ID")?,
            client_secret: std::env::var("AETERNA_AZURE_CLIENT_SECRET")?,
            group_filter: std::env::var("AETERNA_AZURE_GROUP_FILTER").ok(),
            include_nested_groups: std::env::var("AETERNA_AZURE_INCLUDE_NESTED_GROUPS")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }),
        "github" => {
            tracing::info!("GitHub org sync uses the dedicated /api/v1/admin/sync/github endpoint");
            return Ok((None, None, None));
        }
        _ => return Ok((None, None, None)),
    };

    let config = Arc::new(IdpSyncConfig {
        provider: provider.clone(),
        sync_interval_seconds: std::env::var("AETERNA_IDP_SYNC_INTERVAL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300),
        batch_size: std::env::var("AETERNA_IDP_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100),
        database_url,
        webhook_port,
        webhook_secret,
        dry_run: std::env::var("AETERNA_IDP_DRY_RUN")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false),
        retry: idp_sync::config::RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 1000,
            max_backoff_ms: 30000,
        },
    });

    let client: Arc<dyn IdpClient> = match &config.provider {
        IdpProvider::Okta(okta) => Arc::new(OktaClient::new(okta.clone())?),
        IdpProvider::AzureAd(azure) => Arc::new(AzureAdClient::new(azure.clone())?),
        IdpProvider::GitHub(_) => {
            unreachable!("GitHub provider is handled by the dedicated admin sync endpoint")
        }
    };

    let sync_service = Arc::new(IdpSyncService::new(
        (*config).clone(),
        client.clone(),
        postgres.pool().clone(),
    ));

    Ok((Some(config), Some(client), Some(sync_service)))
}

struct AnyhowAuthWrapper<S> {
    inner: S,
}

#[async_trait]
impl<S> AuthorizationService for AnyhowAuthWrapper<S>
where
    S: AuthorizationService + Send + Sync,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = anyhow::Error;

    async fn check_permission(
        &self,
        ctx: &TenantContext,
        action: &str,
        resource: &str,
    ) -> Result<bool, Self::Error> {
        self.inner
            .check_permission(ctx, action, resource)
            .await
            .map_err(anyhow::Error::from)
    }

    async fn get_user_roles(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<RoleIdentifier>, Self::Error> {
        self.inner
            .get_user_roles(ctx)
            .await
            .map_err(anyhow::Error::from)
    }

    async fn assign_role(
        &self,
        ctx: &TenantContext,
        user_id: &UserId,
        role: RoleIdentifier,
    ) -> Result<(), Self::Error> {
        self.inner
            .assign_role(ctx, user_id, role)
            .await
            .map_err(anyhow::Error::from)
    }

    async fn remove_role(
        &self,
        ctx: &TenantContext,
        user_id: &UserId,
        role: RoleIdentifier,
    ) -> Result<(), Self::Error> {
        self.inner
            .remove_role(ctx, user_id, role)
            .await
            .map_err(anyhow::Error::from)
    }
}

struct AllowAllAuthService;

#[async_trait]
impl AuthorizationService for AllowAllAuthService {
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

struct AllowAllBoxedAuthService;

#[async_trait]
impl AuthorizationService for AllowAllBoxedAuthService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

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

struct NoopReflectiveReasoner;

#[async_trait]
impl ReflectiveReasoner for NoopReflectiveReasoner {
    async fn reason(
        &self,
        query: &str,
        _context_summary: Option<&str>,
    ) -> anyhow::Result<ReasoningTrace> {
        let now = Utc::now();
        Ok(ReasoningTrace {
            strategy: ReasoningStrategy::SemanticOnly,
            thought_process: "Reflective reasoning disabled".to_string(),
            refined_query: Some(query.to_string()),
            start_time: now,
            end_time: now,
            timed_out: false,
            duration_ms: 0,
            metadata: Default::default(),
        })
    }
}

struct AllowAllTokenValidator;

#[async_trait]
impl TokenValidator for AllowAllTokenValidator {
    async fn validate(&self, token: &str) -> WsResult<AuthToken> {
        Ok(AuthToken {
            user_id: token.to_string(),
            tenant_id: "default".to_string(),
            permissions: vec!["read".to_string(), "write".to_string()],
            expires_at: Utc::now().timestamp() + 3600,
        })
    }
}

async fn seed_platform_admin(
    pool: &sqlx::Pool<sqlx::Postgres>,
    cfg: &config::AdminBootstrapConfig,
) -> anyhow::Result<()> {
    let email = match &cfg.email {
        Some(e) => e,
        None => {
            tracing::warn!("admin bootstrap skipped: AETERNA_ADMIN_BOOTSTRAP_EMAIL not set");
            return Ok(());
        }
    };

    let provider = &cfg.provider;
    let subject = cfg.provider_subject.as_deref().unwrap_or(email.as_str());
    let now_epoch = chrono::Utc::now().timestamp();

    let mut tx = pool.begin().await.context("begin seed transaction")?;

    sqlx::query(
        "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, created_at, updated_at)
         VALUES ($1, $2, 'company', NULL, $1, '{}', $3, $3)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(INSTANCE_SCOPE_TENANT_ID)
    .bind("Instance")
    .bind(now_epoch)
    .execute(&mut *tx)
    .await
    .context("upsert instance-scope organizational unit")?;

    sqlx::query(
        "UPDATE user_roles
         SET tenant_id = $1, unit_id = $1
         WHERE role = 'PlatformAdmin' AND tenant_id = 'default'",
    )
    .bind(INSTANCE_SCOPE_TENANT_ID)
    .execute(&mut *tx)
    .await
    .context("migrate legacy PlatformAdmin rows to instance scope")?;

    let company_id = "default";
    sqlx::query(
        "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, created_at, updated_at)
         VALUES ($1, $2, 'company', NULL, $3, '{}', $4, $4)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(company_id)
    .bind("Default")
    .bind(company_id)
    .bind(now_epoch)
    .execute(&mut *tx)
    .await
    .context("upsert organizational_units company")?;

    let company_slug = "default";
    sqlx::query(
        "INSERT INTO companies (slug, name, settings, created_at, updated_at)
         VALUES ($1, $2, '{}', NOW(), NOW())
         ON CONFLICT (slug) DO NOTHING",
    )
    .bind(company_slug)
    .bind("Default")
    .execute(&mut *tx)
    .await
    .context("upsert companies row")?;

    let company_uuid: uuid::Uuid = sqlx::query_scalar("SELECT id FROM companies WHERE slug = $1")
        .bind(company_slug)
        .fetch_one(&mut *tx)
        .await
        .context("fetch company uuid")?;

    sqlx::query(
        "INSERT INTO organizations (company_id, slug, name, created_at, updated_at)
         VALUES ($1, 'platform', 'Platform', NOW(), NOW())
         ON CONFLICT (company_id, slug) DO NOTHING",
    )
    .bind(company_uuid)
    .execute(&mut *tx)
    .await
    .context("upsert bootstrap organization")?;

    let org_uuid: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM organizations WHERE company_id = $1 AND slug = 'platform'",
    )
    .bind(company_uuid)
    .fetch_one(&mut *tx)
    .await
    .context("fetch org uuid")?;

    sqlx::query(
        "INSERT INTO teams (org_id, slug, name, created_at, updated_at)
         VALUES ($1, 'admins', 'Admins', NOW(), NOW())
         ON CONFLICT (org_id, slug) DO NOTHING",
    )
    .bind(org_uuid)
    .execute(&mut *tx)
    .await
    .context("upsert bootstrap team")?;

    let team_uuid: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM teams WHERE org_id = $1 AND slug = 'admins'")
            .bind(org_uuid)
            .fetch_one(&mut *tx)
            .await
            .context("fetch team uuid")?;

    let user_uuid: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, name, idp_provider, idp_subject, status, created_at, updated_at)
         VALUES ($1, $1, $2, $3, 'active', NOW(), NOW())
         ON CONFLICT (email) DO UPDATE SET idp_provider = EXCLUDED.idp_provider, idp_subject = EXCLUDED.idp_subject, updated_at = NOW()
         RETURNING id",
    )
    .bind(email)
    .bind(provider)
    .bind(subject)
    .fetch_one(&mut *tx)
    .await
    .context("upsert admin user")?;

    sqlx::query(
        "INSERT INTO memberships (user_id, team_id, role, status, created_at, updated_at)
         VALUES ($1, $2, 'admin', 'active', NOW(), NOW())
         ON CONFLICT (user_id, team_id) DO NOTHING",
    )
    .bind(user_uuid)
    .bind(team_uuid)
    .execute(&mut *tx)
    .await
    .context("upsert membership")?;

    let user_id_str = user_uuid.to_string();
    sqlx::query(
        "INSERT INTO user_roles (user_id, tenant_id, unit_id, role, created_at)
         VALUES ($1, $2, $3, 'PlatformAdmin', $4)
         ON CONFLICT (user_id, tenant_id, unit_id, role) DO NOTHING",
    )
    .bind(&user_id_str)
    .bind(INSTANCE_SCOPE_TENANT_ID)
    .bind(INSTANCE_SCOPE_TENANT_ID)
    .bind(now_epoch)
    .execute(&mut *tx)
    .await
    .context("upsert PlatformAdmin role")?;

    tx.commit().await.context("commit seed transaction")?;

    tracing::info!(
        email = %email,
        provider = %provider,
        subject = %subject,
        "platform admin seeded successfully"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn set_env<K: AsRef<str>, V: AsRef<str>>(key: K, value: V) {
        unsafe { std::env::set_var(key.as_ref(), value.as_ref()) }
    }

    fn remove_env<K: AsRef<str>>(key: K) {
        unsafe { std::env::remove_var(key.as_ref()) }
    }

    #[test]
    #[serial]
    fn validate_required_env_accepts_pg_prefixes() {
        unsafe {
            std::env::set_var("PG_HOST", "localhost");
            std::env::set_var("PG_DATABASE", "aeterna");
            std::env::set_var("RD_HOST", "localhost");
        }

        assert!(validate_required_env().is_ok());

        unsafe {
            std::env::remove_var("PG_HOST");
            std::env::remove_var("PG_DATABASE");
            std::env::remove_var("RD_HOST");
        }
    }

    #[tokio::test]
    async fn noop_reasoner_returns_semantic_only_trace() {
        let trace = NoopReflectiveReasoner.reason("hello", None).await.unwrap();
        assert_eq!(trace.strategy, ReasoningStrategy::SemanticOnly);
        assert_eq!(trace.refined_query.as_deref(), Some("hello"));
    }

    #[test]
    fn knowledge_repo_path_uses_default_when_env_missing() {
        unsafe {
            std::env::remove_var("AETERNA_KNOWLEDGE_REPO_PATH");
        }
        assert_eq!(knowledge_repo_path(), PathBuf::from("./knowledge-repo"));
    }

    #[test]
    #[serial]
    fn tenant_config_provider_namespace_uses_env_override() {
        set_env("AETERNA_K8S_NAMESPACE", "aeterna");
        assert_eq!(tenant_config_provider_namespace(), "aeterna");
        remove_env("AETERNA_K8S_NAMESPACE");
    }

    #[test]
    #[serial]
    fn tenant_config_provider_namespace_defaults_to_default() {
        remove_env("AETERNA_K8S_NAMESPACE");
        assert_eq!(tenant_config_provider_namespace(), "default");
    }

    #[test]
    fn feature_enabled_respects_default_and_env() {
        remove_env("AETERNA_FEATURE_REFLECTIVE");
        assert!(feature_enabled("AETERNA_FEATURE_REFLECTIVE", true));
        assert!(!feature_enabled("AETERNA_FEATURE_REFLECTIVE", false));

        set_env("AETERNA_FEATURE_REFLECTIVE", "false");
        assert!(!feature_enabled("AETERNA_FEATURE_REFLECTIVE", true));

        set_env("AETERNA_FEATURE_REFLECTIVE", "true");
        assert!(feature_enabled("AETERNA_FEATURE_REFLECTIVE", false));

        remove_env("AETERNA_FEATURE_REFLECTIVE");
    }

    #[test]
    #[serial]
    fn feature_enabled_accepts_additional_truthy_variants() {
        for value in ["1", "TRUE", "yes", "on"] {
            set_env("AETERNA_FEATURE_REFLECTIVE", value);
            assert!(feature_enabled("AETERNA_FEATURE_REFLECTIVE", false));
        }
        remove_env("AETERNA_FEATURE_REFLECTIVE");
    }

    #[test]
    #[serial]
    fn runtime_features_reads_all_feature_flags() {
        set_env("AETERNA_FEATURE_CCA", "false");
        set_env("AETERNA_FEATURE_RADKIT", "true");
        set_env("AETERNA_FEATURE_RLM", "0");
        set_env("AETERNA_FEATURE_REFLECTIVE", "1");

        let features = RuntimeFeatures::from_env();
        assert_eq!(
            features,
            RuntimeFeatures {
                cca: false,
                radkit: true,
                rlm: false,
                reflective: true,
            }
        );

        remove_env("AETERNA_FEATURE_CCA");
        remove_env("AETERNA_FEATURE_RADKIT");
        remove_env("AETERNA_FEATURE_RLM");
        remove_env("AETERNA_FEATURE_REFLECTIVE");
    }

    #[test]
    fn postgres_connection_url_uses_provider_config_values() {
        let mut config = config::Config::default();
        config.providers.postgres.host = "db.internal".to_string();
        config.providers.postgres.port = 5433;
        config.providers.postgres.database = "aeterna".to_string();
        config.providers.postgres.username = "svc".to_string();
        config.providers.postgres.password = "secret".to_string();

        assert_eq!(
            postgres_connection_url(&config),
            "postgres://svc:secret@db.internal:5433/aeterna"
        );
    }

    #[test]
    fn create_graph_store_returns_none_when_graph_disabled() {
        let mut config = config::Config::default();
        config.providers.graph.enabled = false;

        let graph_store = create_graph_store(&config).unwrap();
        assert!(graph_store.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn build_anyhow_auth_service_defaults_to_allow_all() {
        remove_env("AETERNA_AUTH_BACKEND");

        let auth = build_anyhow_auth_service().unwrap();
        let allowed = auth
            .check_permission(&TenantContext::default(), "read", "resource")
            .await
            .unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    #[serial]
    async fn build_anyhow_auth_service_ignores_plugin_auth_env() {
        remove_env("AETERNA_AUTH_BACKEND");
        set_env("AETERNA_PLUGIN_AUTH_ENABLED", "true");
        set_env("AETERNA_PLUGIN_AUTH_JWT_SECRET", "plugin-secret");

        let auth = build_anyhow_auth_service().unwrap();
        let allowed = auth
            .check_permission(&TenantContext::default(), "read", "resource")
            .await
            .unwrap();
        assert!(allowed);

        remove_env("AETERNA_PLUGIN_AUTH_ENABLED");
        remove_env("AETERNA_PLUGIN_AUTH_JWT_SECRET");
    }

    #[test]
    #[serial]
    fn build_anyhow_auth_service_errors_for_missing_cedar_inputs() {
        set_env("AETERNA_AUTH_BACKEND", "cedar");
        remove_env("AETERNA_CEDAR_POLICY_TEXT");
        remove_env("AETERNA_CEDAR_SCHEMA_TEXT");

        let error = build_anyhow_auth_service().err().unwrap();
        assert!(
            error
                .to_string()
                .contains("AETERNA_CEDAR_POLICY_TEXT is required for cedar auth")
        );

        remove_env("AETERNA_AUTH_BACKEND");
    }

    #[tokio::test]
    #[serial]
    async fn build_optional_idp_services_returns_none_when_provider_missing() {
        remove_env("AETERNA_IDP_PROVIDER");

        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .unwrap();
        let postgres = Arc::new(PostgresBackend::from_pool(lazy_pool));

        let (config, client, service) = build_optional_idp_services(postgres).unwrap();
        assert!(config.is_none());
        assert!(client.is_none());
        assert!(service.is_none());
    }

    #[test]
    fn github_app_bootstrap_uses_knowledge_repo_fields_not_plugin_auth_fields() {
        let mut config = config::Config::default();
        config.knowledge_repo.github_owner = Some("acme".to_string());
        config.knowledge_repo.github_repo = Some("knowledge".to_string());
        config.knowledge_repo.github_app_id = Some(101);
        config.knowledge_repo.github_installation_id = Some(202);
        config.knowledge_repo.github_app_pem = Some("knowledge-pem".to_string());

        config.plugin_auth.enabled = true;
        config.plugin_auth.github_app_id = Some(999);
        config.plugin_auth.github_app_pem = Some("plugin-pem".to_string());

        assert_eq!(config.knowledge_repo.github_app_id, Some(101));
        assert_eq!(config.knowledge_repo.github_installation_id, Some(202));
        assert_eq!(
            config.knowledge_repo.github_app_pem.as_deref(),
            Some("knowledge-pem")
        );
        assert_eq!(config.plugin_auth.github_app_id, Some(999));
        assert_eq!(
            config.plugin_auth.github_app_pem.as_deref(),
            Some("plugin-pem")
        );
        assert_ne!(
            config.knowledge_repo.github_app_pem,
            config.plugin_auth.github_app_pem
        );
    }
}
