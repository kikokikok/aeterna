use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use knowledge::git_provider::WebhookEvent;
use mk_core::types::{GovernanceEvent, TenantContext};
use serde::Deserialize;
use serde_json::json;

use super::AppState;
use super::plugin_auth::resolve_tenant_for_github_user;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/webhooks/github", post(handle_github_webhook))
        .with_state(state)
}

#[tracing::instrument(skip_all, fields(event_type))]
async fn handle_github_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if state.webhook_secret.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Webhooks not configured"})),
        )
            .into_response();
    }

    let event_type = match headers
        .get("X-GitHub-Event")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) => value,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing X-GitHub-Event header"})),
            )
                .into_response();
        }
    };

    if matches!(
        event_type,
        "organization" | "team" | "membership" | "member"
    ) {
        return handle_org_sync_webhook(&state, event_type, &body).await;
    }

    let signature = headers
        .get("X-Hub-Signature-256")
        .and_then(|value| value.to_str().ok());

    let Some(git_provider) = &state.git_provider else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Git provider not configured"})),
        )
            .into_response();
    };

    let event = match git_provider
        .parse_webhook(event_type, signature, &body)
        .await
    {
        Ok(event) => event,
        Err(err) => {
            tracing::warn!("Webhook parse error: {:?}", err);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid webhook signature"})),
            )
                .into_response();
        }
    };

    handle_event(&state, event).await;

    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

#[derive(Debug, Deserialize)]
struct OrgSyncWebhookPayload {
    action: String,
    #[serde(default)]
    member: Option<GitHubMember>,
    #[serde(default)]
    team: Option<GitHubTeamPayload>,
    #[serde(default)]
    membership: Option<GitHubMembershipPayload>,
    #[serde(default)]
    organization: Option<GitHubOrgPayload>,
}

#[derive(Debug, Deserialize)]
struct GitHubMember {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitHubTeamPayload {
    slug: String,
    name: String,
    #[serde(default)]
    parent: Option<GitHubTeamParent>,
}

#[derive(Debug, Deserialize)]
struct GitHubTeamParent {
    slug: String,
}

#[derive(Debug, Deserialize)]
struct GitHubMembershipPayload {
    user: GitHubMember,
    team: GitHubTeamPayload,
    role: String,
}

#[derive(Debug, Deserialize)]
struct GitHubOrgPayload {
    login: String,
}

#[tracing::instrument(skip_all, fields(event_type, org_name))]
async fn handle_org_sync_webhook(
    state: &Arc<AppState>,
    event_type: &str,
    body: &[u8],
) -> axum::response::Response {
    let payload: OrgSyncWebhookPayload = match serde_json::from_slice(body) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(
                event_type,
                "Failed to parse org sync webhook payload: {err:?}"
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid webhook payload"})),
            )
                .into_response();
        }
    };

    let org_name = payload
        .organization
        .as_ref()
        .map(|o| o.login.as_str())
        .unwrap_or("unknown");

    match event_type {
        "organization" => match payload.action.as_str() {
            "member_added" => {
                let login = payload
                    .member
                    .as_ref()
                    .map(|m| m.login.as_str())
                    .unwrap_or("?");
                tracing::info!(org = org_name, member = login, "GitHub org member added");
            }
            "member_removed" => {
                let login = payload
                    .member
                    .as_ref()
                    .map(|m| m.login.as_str())
                    .unwrap_or("?");
                tracing::info!(org = org_name, member = login, "GitHub org member removed");
            }
            action => {
                tracing::debug!(org = org_name, action, "Ignored organization event action");
                return (StatusCode::OK, Json(json!({"status": "ignored"}))).into_response();
            }
        },
        "team" => {
            let slug = payload
                .team
                .as_ref()
                .map(|t| t.slug.as_str())
                .unwrap_or("?");
            let name = payload
                .team
                .as_ref()
                .map(|t| t.name.as_str())
                .unwrap_or("?");
            match payload.action.as_str() {
                "created" => {
                    let parent = payload
                        .team
                        .as_ref()
                        .and_then(|t| t.parent.as_ref())
                        .map(|p| p.slug.as_str())
                        .unwrap_or("(top-level)");
                    tracing::info!(org = org_name, slug, name, parent, "GitHub team created");
                }
                "deleted" => {
                    tracing::info!(org = org_name, slug, name, "GitHub team deleted");
                }
                "edited" => {
                    let parent = payload
                        .team
                        .as_ref()
                        .and_then(|t| t.parent.as_ref())
                        .map(|p| p.slug.as_str())
                        .unwrap_or("(top-level)");
                    tracing::info!(
                        org = org_name,
                        slug,
                        name,
                        parent,
                        "GitHub team edited (possible reparent)"
                    );
                }
                action => {
                    tracing::debug!(org = org_name, slug, action, "Ignored team event action");
                    return (StatusCode::OK, Json(json!({"status": "ignored"}))).into_response();
                }
            }
        }
        "membership" => {
            if let Some(ref ms) = payload.membership {
                match payload.action.as_str() {
                    "added" => {
                        tracing::info!(
                            org = org_name,
                            user = %ms.user.login,
                            team = %ms.team.slug,
                            role = %ms.role,
                            "GitHub team membership added"
                        );
                    }
                    "removed" => {
                        tracing::info!(
                            org = org_name,
                            user = %ms.user.login,
                            team = %ms.team.slug,
                            "GitHub team membership removed"
                        );
                    }
                    action => {
                        tracing::debug!(org = org_name, action, "Ignored membership event action");
                        return (StatusCode::OK, Json(json!({"status": "ignored"})))
                            .into_response();
                    }
                }
            }
        }
        _ => {
            tracing::debug!(event_type, "Unhandled org sync event type");
            return (StatusCode::OK, Json(json!({"status": "ignored"}))).into_response();
        }
    }

    trigger_incremental_sync(state, org_name).await;

    (
        StatusCode::OK,
        Json(json!({"status": "ok", "sync": "triggered"})),
    )
        .into_response()
}

#[tracing::instrument(skip_all, fields(org_name))]
async fn trigger_incremental_sync(state: &Arc<AppState>, org_name: &str) {
    tracing::info!(
        org = org_name,
        "Triggering incremental GitHub org sync from webhook"
    );

    let github_config = match super::admin_sync::build_github_config_from_env() {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!("Cannot trigger org sync — GitHub config not available: {err:?}");
            return;
        }
    };

    let tenant_id =
        match super::admin_sync::resolve_tenant_id_from_pool(state.postgres.pool()).await {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!("Cannot resolve tenant for org sync: {err:?}");
                return;
            }
        };

    tokio::spawn({
        let pool = state.postgres.pool().clone();
        async move {
            match idp_sync::github::run_github_sync(&github_config, &pool, tenant_id).await {
                Ok(report) => {
                    tracing::info!(
                        users_created = report.users_created,
                        users_updated = report.users_updated,
                        groups_synced = report.groups_synced,
                        memberships_added = report.memberships_added,
                        "Webhook-triggered GitHub org sync completed"
                    );
                }
                Err(err) => {
                    tracing::warn!("Webhook-triggered GitHub org sync failed: {err:?}");
                }
            }
        }
    });
}

/// Derive the `TenantContext` for webhook-triggered governance events.
///
/// Webhook requests are authenticated via HMAC signature at the HTTP layer
/// (`parse_webhook` enforces signature validity), not via per-user bearer tokens.
/// The tenant for webhook events is resolved from the server's plugin auth
/// configuration in the same order as bootstrap: explicit `default_tenant_id`,
/// then the `AETERNA_DEFAULT_TENANT_ID` environment variable.
///
/// If no tenant is configured, the webhook event is published under the
/// hardcoded `"default"` tenant only as a last-resort operational fallback,
/// and a warning is emitted so operators know the deployment needs configuration.
fn webhook_tenant_context(state: &Arc<AppState>) -> TenantContext {
    let cfg = &state.plugin_auth_state.config;
    if let Some(tenant_id) = resolve_tenant_for_github_user("", cfg) {
        if let (Some(tid), Some(uid)) = (
            mk_core::types::TenantId::new(tenant_id.clone()),
            mk_core::types::UserId::new("github-webhook".to_string()),
        ) {
            return TenantContext::new(tid, uid);
        }
    }
    tracing::warn!(
        "No tenant configured for webhook events; using default tenant.          Set AETERNA_DEFAULT_TENANT_ID or configure plugin_auth.default_tenant_id."
    );
    TenantContext::default()
}

#[tracing::instrument(skip_all)]
async fn handle_event(state: &Arc<AppState>, event: WebhookEvent) {
    let ctx = webhook_tenant_context(state);

    match event {
        WebhookEvent::PullRequestOpened { pr } => {
            tracing::info!(pr_number = pr.number, "PR opened: {}", pr.title);
            let _ = state
                .governance_engine
                .publish_event(GovernanceEvent::RequestCreated {
                    request_id: pr.number.to_string(),
                    request_type: "pull_request".to_string(),
                    title: pr.title,
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await
                .map_err(|e| tracing::warn!("Failed to publish RequestCreated: {:?}", e));
        }
        WebhookEvent::PullRequestMerged {
            pr,
            merge_commit_sha,
        } => {
            tracing::info!(
                pr_number = pr.number,
                merge_sha = %merge_commit_sha,
                "PR merged: {}",
                pr.title
            );

            let _ = state
                .governance_engine
                .publish_event(GovernanceEvent::RequestApproved {
                    request_id: pr.number.to_string(),
                    approver_id: "github-webhook".to_string(),
                    fully_approved: true,
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await
                .map_err(|e| tracing::warn!("Failed to publish RequestApproved: {:?}", e));

            let trigger = sync::state::SyncTrigger::CommitMismatch {
                last_commit: "unknown".to_string(),
                head_commit: merge_commit_sha,
            };
            tracing::info!("Webhook requested sync trigger: {:?}", trigger);

            if let Err(err) = state.sync_manager.run_sync_cycle(ctx, 0).await {
                tracing::warn!("Failed to run sync cycle after PR merge: {:?}", err);
            }
        }
        WebhookEvent::PullRequestClosed { pr } => {
            tracing::info!(
                pr_number = pr.number,
                "PR closed without merge: {}",
                pr.title
            );
            let _ = state
                .governance_engine
                .publish_event(GovernanceEvent::RequestRejected {
                    request_id: pr.number.to_string(),
                    rejector_id: "github-webhook".to_string(),
                    reason: "pull request closed without merge".to_string(),
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await
                .map_err(|e| tracing::warn!("Failed to publish RequestRejected: {:?}", e));
        }
        WebhookEvent::Unknown { event_type } => {
            tracing::debug!("Ignored webhook event type: {}", event_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginAuthState;
    use crate::server::plugin_auth::{RefreshTokenStore, RefreshTokenStoreBackend};
    use agent_a2a::config::TrustedIdentityConfig;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::{GitRepository, RepositoryError};
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, ReasoningTrace, Role, RoleIdentifier, UserId,
    };
    use std::collections::HashMap;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::state_persister::FilePersister;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
    use tower::ServiceExt;

    use crate::server::AppState;

    struct MockGitProvider;

    #[async_trait]
    impl knowledge::git_provider::GitProvider for MockGitProvider {
        async fn create_branch(
            &self,
            _name: &str,
            _from_sha: &str,
        ) -> Result<(), knowledge::git_provider::GitProviderError> {
            Err(knowledge::git_provider::GitProviderError::Api(
                "not implemented".to_string(),
            ))
        }

        async fn commit_to_branch(
            &self,
            _branch: &str,
            _path: &str,
            _content: &[u8],
            _message: &str,
        ) -> Result<String, knowledge::git_provider::GitProviderError> {
            Err(knowledge::git_provider::GitProviderError::Api(
                "not implemented".to_string(),
            ))
        }

        async fn create_pull_request(
            &self,
            _title: &str,
            _body: &str,
            _head: &str,
            _base: &str,
        ) -> Result<
            knowledge::git_provider::PullRequestInfo,
            knowledge::git_provider::GitProviderError,
        > {
            Err(knowledge::git_provider::GitProviderError::Api(
                "not implemented".to_string(),
            ))
        }

        async fn merge_pull_request(
            &self,
            _pr_number: u64,
            _merge_method: knowledge::git_provider::MergeMethod,
        ) -> Result<String, knowledge::git_provider::GitProviderError> {
            Err(knowledge::git_provider::GitProviderError::Api(
                "not implemented".to_string(),
            ))
        }

        async fn list_open_prs(
            &self,
            _head_prefix: Option<&str>,
        ) -> Result<
            Vec<knowledge::git_provider::PullRequestInfo>,
            knowledge::git_provider::GitProviderError,
        > {
            Ok(Vec::new())
        }

        async fn parse_webhook(
            &self,
            event_type: &str,
            signature: Option<&str>,
            _body: &[u8],
        ) -> Result<knowledge::git_provider::WebhookEvent, knowledge::git_provider::GitProviderError>
        {
            if signature != Some("sha256=valid") {
                return Err(knowledge::git_provider::GitProviderError::InvalidSignature);
            }

            if event_type != "pull_request" {
                return Ok(knowledge::git_provider::WebhookEvent::Unknown {
                    event_type: event_type.to_string(),
                });
            }

            Ok(knowledge::git_provider::WebhookEvent::PullRequestOpened {
                pr: knowledge::git_provider::PullRequestInfo {
                    number: 42,
                    title: "Test PR".to_string(),
                    body: None,
                    head_branch: "governance/test".to_string(),
                    base_branch: "main".to_string(),
                    state: knowledge::git_provider::PullRequestState::Open,
                    html_url: "https://example.invalid/pull/42".to_string(),
                    merged: false,
                    merge_commit_sha: None,
                },
            })
        }

        async fn get_default_branch_sha(
            &self,
        ) -> Result<String, knowledge::git_provider::GitProviderError> {
            Ok("sha".to_string())
        }

        async fn get_installation_token(
            &self,
        ) -> Result<String, knowledge::git_provider::GitProviderError> {
            Ok("mock-token".to_string())
        }
    }

    struct MockRepo;

    #[async_trait]
    impl KnowledgeRepository for MockRepo {
        type Error = RepositoryError;

        async fn get(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
        ) -> Result<Option<KnowledgeEntry>, Self::Error> {
            Ok(None)
        }

        async fn store(
            &self,
            _ctx: TenantContext,
            _entry: KnowledgeEntry,
            _message: &str,
        ) -> Result<String, Self::Error> {
            Ok("mock-commit".to_string())
        }

        async fn list(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _prefix: &str,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(Vec::new())
        }

        async fn delete(
            &self,
            _ctx: TenantContext,
            _layer: KnowledgeLayer,
            _path: &str,
            _message: &str,
        ) -> Result<String, Self::Error> {
            Ok("mock-commit".to_string())
        }

        async fn get_head_commit(
            &self,
            _ctx: TenantContext,
        ) -> Result<Option<String>, Self::Error> {
            Ok(None)
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

    struct MockAuth;

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
            Ok(vec![Role::Admin.into()])
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

    struct TestNoopReasoner;

    #[async_trait]
    impl ReflectiveReasoner for TestNoopReasoner {
        async fn reason(
            &self,
            query: &str,
            _context_summary: Option<&str>,
        ) -> anyhow::Result<ReasoningTrace> {
            let now = chrono::Utc::now();
            Ok(ReasoningTrace {
                strategy: mk_core::types::ReasoningStrategy::SemanticOnly,
                thought_process: "test".to_string(),
                refined_query: Some(query.to_string()),
                start_time: now,
                end_time: now,
                timed_out: false,
                duration_ms: 0,
                metadata: HashMap::new(),
            })
        }
    }

    struct MockTokenValidator;

    #[async_trait]
    impl TokenValidator for MockTokenValidator {
        async fn validate(&self, token: &str) -> WsResult<AuthToken> {
            Ok(AuthToken {
                user_id: token.to_string(),
                tenant_id: "default".to_string(),
                permissions: vec!["read".to_string(), "write".to_string()],
                expires_at: chrono::Utc::now().timestamp() + 3600,
            })
        }
    }

    async fn test_state(
        git_provider: Option<Arc<dyn knowledge::git_provider::GitProvider>>,
        webhook_secret: Option<String>,
    ) -> Arc<AppState> {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .expect("lazy pg");
        let postgres = Arc::new(storage::postgres::PostgresBackend::from_pool(lazy_pool));
        let governance_engine = Arc::new(GovernanceEngine::new());
        let git_repo = Arc::new(GitRepository::new(tempdir.path()).expect("git repo"));
        let knowledge_manager = Arc::new(KnowledgeManager::new(
            git_repo.clone(),
            governance_engine.clone(),
        ));
        let memory_manager = Arc::new(MemoryManager::new());
        let sync_manager = Arc::new(
            sync::bridge::SyncManager::new(
                memory_manager.clone(),
                knowledge_manager.clone(),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(FilePersister::new(std::env::temp_dir())),
                None,
            )
            .await
            .expect("sync manager"),
        );
        let auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync> =
            Arc::new(MockAuth);
        let governance_dashboard = Arc::new(knowledge::api::GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let mcp_server = Arc::new(tools::server::McpServer::new(
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
        let git_provider_connection_registry = Arc::new(
            storage::git_provider_connection_store::InMemoryGitProviderConnectionStore::new(),
        );
        let tenant_repo_resolver = Arc::new(
            TenantRepositoryResolver::new(
                tenant_repository_binding_store.clone(),
                std::env::temp_dir(),
                Arc::new(LocalSecretProvider::new(HashMap::new())),
            )
            .with_connection_registry(git_provider_connection_registry.clone()),
        );

        Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres: postgres.clone(),
            memory_manager,
            knowledge_manager,
            knowledge_repository: Arc::new(MockRepo),
            governance_engine,
            governance_dashboard,
            auth_service,
            mcp_server,
            sync_manager,
            git_provider,
            webhook_secret,
            event_publisher: None,
            graph_store: None,
            governance_storage: None,
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
                config: config::PluginAuthConfig::default(),
                postgres: Some(postgres.clone()),
                refresh_store: RefreshTokenStoreBackend::InMemory(RefreshTokenStore::new()),
                oauth_state_store: super::plugin_auth::OAuthStateStore::new(),
            }),
            k8s_auth_config: config::KubernetesAuthConfig::default(),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider: Arc::new(KubernetesTenantConfigProvider::new(
                "default".to_string(),
            )),
            provider_registry: Arc::new(memory::provider_registry::TenantProviderRegistry::new(
                None, None,
            )),
            git_provider_connection_registry,
            redis_conn: None,
        })
    }

    #[tokio::test]
    async fn valid_signature_returns_200() {
        let state = test_state(
            Some(Arc::new(MockGitProvider)),
            Some("configured-secret".to_string()),
        )
        .await;
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/github")
                    .header("X-GitHub-Event", "pull_request")
                    .header("X-Hub-Signature-256", "sha256=valid")
                    .body(Body::from(r#"{"action":"opened"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_signature_returns_401() {
        let state = test_state(
            Some(Arc::new(MockGitProvider)),
            Some("configured-secret".to_string()),
        )
        .await;
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/github")
                    .header("X-GitHub-Event", "pull_request")
                    .header("X-Hub-Signature-256", "sha256=invalid")
                    .body(Body::from(r#"{"action":"opened"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_event_header_returns_400() {
        let state = test_state(
            Some(Arc::new(MockGitProvider)),
            Some("configured-secret".to_string()),
        )
        .await;
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/github")
                    .header("X-Hub-Signature-256", "sha256=valid")
                    .body(Body::from(r#"{"action":"opened"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn webhook_not_configured_returns_404() {
        let state = test_state(Some(Arc::new(MockGitProvider)), None).await;
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/github")
                    .header("X-GitHub-Event", "pull_request")
                    .header("X-Hub-Signature-256", "sha256=valid")
                    .body(Body::from(r#"{"action":"opened"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
