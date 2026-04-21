use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

use super::AppState;
use super::tenant_eager_wire::is_strict_mode;
use super::tenant_runtime_state::TenantRuntimeState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
pub(crate) struct ReadinessResponse {
    status: &'static str,
    checks: ReadinessChecks,
}

#[derive(Serialize)]
pub(crate) struct ReadinessChecks {
    postgres: CheckResult,
    vector_store: CheckResult,
    /// Summary of per-tenant wiring state. See [`TenantCheck`].
    ///
    /// Added in B2 task 5.3. The field name is stable and monitored by
    /// the SRE dashboard; renaming breaks external alerting rules.
    tenants: TenantCheck,
}

#[derive(Serialize)]
pub(crate) struct CheckResult {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

/// Observable summary of the pod-local tenant runtime registry.
///
/// # Semantics
///
/// * `status == "ok"`      \u2014 every known tenant is `Available` *or* there are
///   no tenants at all (fresh cluster).
/// * `status == "pending"` \u2014 at least one tenant is `Loading`, none failed.
///   This happens during the eager-boot window and during pub/sub rewires;
///   it does **not** flip the HTTP gate (see §Gate below).
/// * `status == "degraded"`\u2014 at least one tenant is `LoadingFailed`. Flips
///   the HTTP gate iff strict mode is on.
///
/// # Gate
///
/// `/ready` returns 503 when `failed > 0 && strictMode`. All other
/// combinations return 200. In particular:
///
/// * A tenant in `Loading` does **not** cause 503 \u2014 rewires triggered by
///   `tenant:changed` pub/sub would otherwise flap traffic off the pod.
/// * Permissive mode (the default) never 503s on tenant state so
///   single-tenant misconfiguration cannot take an entire cluster out
///   of rotation. Operators opt in to strict mode per-environment via
///   `AETERNA_EAGER_WIRE_STRICT=1`.
///
/// # Field stability
///
/// `total`, `available`, `loading`, `failed`, and `strictMode` are the
/// wire contract monitored by alerts and the ops dashboard. Adding new
/// fields is fine; renaming is not.
///
/// `failedSlugs` is populated only when `failed > 0` and contains the
/// tenant slugs (never the failure reason strings, which may include
/// upstream error details the caller isn't authorised to see \u2014 reasons
/// are surfaced by the admin-only status endpoint in task 5.5).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TenantCheck {
    status: &'static str,
    total: usize,
    available: usize,
    loading: usize,
    failed: usize,
    /// Reflects the value of `AETERNA_EAGER_WIRE_STRICT` at *this request*.
    /// Read each call so an operator toggling the env var doesn't need a
    /// pod restart for the gate behaviour to change.
    strict_mode: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    failed_slugs: Vec<String>,
}

impl TenantCheck {
    /// Compute the check body from a snapshot of the registry.
    ///
    /// Sorts `failed_slugs` for deterministic output \u2014 avoids churning
    /// monitoring diffs on every scrape when the set is stable.
    fn from_snapshot(snapshot: Vec<(String, TenantRuntimeState)>, strict_mode: bool) -> Self {
        let total = snapshot.len();
        let mut available = 0usize;
        let mut loading = 0usize;
        let mut failed = 0usize;
        let mut failed_slugs: Vec<String> = Vec::new();
        for (slug, state) in snapshot {
            match state {
                TenantRuntimeState::Available { .. } => available += 1,
                TenantRuntimeState::Loading { .. } => loading += 1,
                TenantRuntimeState::LoadingFailed { .. } => {
                    failed += 1;
                    failed_slugs.push(slug);
                }
            }
        }
        failed_slugs.sort();
        let status = if failed > 0 {
            "degraded"
        } else if loading > 0 {
            "pending"
        } else {
            "ok"
        };
        Self {
            status,
            total,
            available,
            loading,
            failed,
            strict_mode,
            failed_slugs,
        }
    }

    /// True iff this check should gate the `/ready` response to 503.
    ///
    /// Intentionally narrow: only `failed > 0 && strict_mode`. See the
    /// type-level docs for the rationale.
    fn blocks_readiness(&self) -> bool {
        self.failed > 0 && self.strict_mode
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/live", get(liveness_handler))
        .route("/ready", get(readiness_handler))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tracing::instrument(skip_all)]
async fn liveness_handler() -> StatusCode {
    StatusCode::OK
}

#[tracing::instrument(skip_all)]
async fn readiness_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pg_check = check_postgres(&state).await;
    let vector_check = check_vector_store(&state).await;
    let tenant_check = check_tenants(&state).await;

    readiness_response(pg_check, vector_check, tenant_check)
}

pub(crate) fn readiness_response(
    pg_check: CheckResult,
    vector_check: CheckResult,
    tenant_check: TenantCheck,
) -> (StatusCode, Json<ReadinessResponse>) {
    // Backend checks gate unconditionally; tenant state gates only in
    // strict mode with actual failures. See `TenantCheck::blocks_readiness`.
    let backends_ok = pg_check.status == "ok" && vector_check.status == "ok";
    let all_healthy = backends_ok && !tenant_check.blocks_readiness();
    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let response = ReadinessResponse {
        status: if all_healthy { "ready" } else { "not_ready" },
        checks: ReadinessChecks {
            postgres: pg_check,
            vector_store: vector_check,
            tenants: tenant_check,
        },
    };

    (status_code, Json(response))
}

/// Build the `tenants` check body by snapshotting the runtime registry.
///
/// This is a read under the registry's `RwLock::read`; concurrent
/// `mark_*` calls during a scrape yield a point-in-time snapshot, not a
/// torn view. Duration is bounded by the number of tenants, which is
/// single-digit thousands in practice \u2014 well within a probe's budget.
#[tracing::instrument(skip_all)]
async fn check_tenants(state: &AppState) -> TenantCheck {
    let snapshot = state.tenant_runtime_state.snapshot().await;
    // `is_strict_mode` is read every request so operators can flip
    // `AETERNA_EAGER_WIRE_STRICT` without a pod restart when they need
    // to bring a cluster back into rotation fast.
    TenantCheck::from_snapshot(snapshot, is_strict_mode())
}

#[tracing::instrument(skip_all)]
async fn check_postgres(state: &AppState) -> CheckResult {
    match sqlx::query("SELECT 1").execute(state.postgres.pool()).await {
        Ok(_) => CheckResult {
            status: "ok",
            message: None,
        },
        Err(e) => CheckResult {
            status: "error",
            message: Some(e.to_string()),
        },
    }
}

#[tracing::instrument(skip_all)]
async fn check_vector_store(_state: &AppState) -> CheckResult {
    // Vector store health is verified through the memory manager's embedded
    // backend. For now, report ok — the readiness probe covers postgres which
    // is the critical dependency for startup.
    CheckResult {
        status: "ok",
        message: None,
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
    use knowledge::api::GovernanceDashboardApi;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::{GitRepository, RepositoryError};
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, ReasoningStrategy, ReasoningTrace, Role, RoleIdentifier,
        TenantContext, UserId,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use storage::postgres::PostgresBackend;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::state_persister::FilePersister;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
    use tower::ServiceExt;

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
            Ok(vec![])
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
            Ok(Some("mock-commit".to_string()))
        }

        async fn get_affected_items(
            &self,
            _ctx: TenantContext,
            _since_commit: &str,
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

    struct MockTokenValidator;

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

    async fn app_state() -> Arc<AppState> {
        let tempdir = tempfile::tempdir().unwrap();
        let lazy_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/aeterna")
            .unwrap();
        let postgres = Arc::new(PostgresBackend::from_pool(lazy_pool));
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
        let governance_dashboard = Arc::new(GovernanceDashboardApi::new(
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
            git_provider: None,
            webhook_secret: None,
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
            tenant_runtime_state: std::sync::Arc::new(
                crate::server::tenant_runtime_state::TenantRuntimeRegistry::new(),
            ),
            bootstrap_tracker: std::sync::Arc::new(crate::server::bootstrap_tracker::BootstrapTracker::new()),
        })
    }

    fn mock_router() -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/live", get(liveness_handler))
    }

    #[tokio::test]
    async fn test_health_returns_ok() {
        let app = mock_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_liveness_returns_200() {
        let app = mock_router();
        let response = app
            .oneshot(Request::builder().uri("/live").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn router_ready_endpoint_reports_not_ready_without_live_postgres() {
        let app = router(app_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "not_ready");
        assert_eq!(json["checks"]["vector_store"]["status"], "ok");
        assert_eq!(json["checks"]["postgres"]["status"], "error");
    }

    #[tokio::test]
    async fn test_health_has_version_field() {
        let app = mock_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("version").is_some());
    }

    /// Shorthand helpers so the assertions stay readable.
    fn ok_check() -> CheckResult {
        CheckResult {
            status: "ok",
            message: None,
        }
    }
    fn err_check(msg: &str) -> CheckResult {
        CheckResult {
            status: "error",
            message: Some(msg.to_string()),
        }
    }
    fn empty_tenants() -> TenantCheck {
        TenantCheck::from_snapshot(Vec::new(), false)
    }

    #[test]
    fn readiness_response_is_ready_when_all_checks_ok() {
        let (status, Json(body)) = readiness_response(ok_check(), ok_check(), empty_tenants());
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ready");
        assert_eq!(body.checks.tenants.status, "ok");
        assert_eq!(body.checks.tenants.total, 0);
    }

    #[test]
    fn readiness_response_is_not_ready_when_one_backend_fails() {
        let (status, Json(body)) =
            readiness_response(err_check("db down"), ok_check(), empty_tenants());
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.checks.postgres.message.as_deref(), Some("db down"));
    }

    #[test]
    fn readiness_response_is_not_ready_when_all_backends_fail() {
        let (status, Json(body)) = readiness_response(
            err_check("db down"),
            err_check("vector down"),
            empty_tenants(),
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(
            body.checks.vector_store.message.as_deref(),
            Some("vector down")
        );
    }

    // ── Tenant gate tests (B2 task 5.3) ─────────────────────────────────

    /// Build a snapshot from `(slug, state)` tuples. Using the concrete
    /// enum keeps the test expressive vs. constructing a full registry.
    fn snap(entries: Vec<(&str, TenantRuntimeState)>) -> Vec<(String, TenantRuntimeState)> {
        entries
            .into_iter()
            .map(|(s, st)| (s.to_string(), st))
            .collect()
    }

    #[test]
    fn tenant_check_all_available_is_ok() {
        let snapshot = snap(vec![
            ("alpha", TenantRuntimeState::available_now(3)),
            ("beta", TenantRuntimeState::available_now(1)),
        ]);
        let c = TenantCheck::from_snapshot(snapshot, true);
        assert_eq!(c.status, "ok");
        assert_eq!((c.total, c.available, c.loading, c.failed), (2, 2, 0, 0));
        assert!(c.failed_slugs.is_empty());
        assert!(!c.blocks_readiness());
    }

    #[test]
    fn tenant_check_loading_is_pending_and_not_blocking() {
        // Critical for production: a rewire puts the tenant briefly in
        // Loading; /ready MUST NOT flap to 503 during this window, even
        // in strict mode, otherwise every pub/sub invalidation would
        // kick the pod out of the LB rotation.
        let snapshot = snap(vec![("alpha", TenantRuntimeState::loading_now())]);
        let c = TenantCheck::from_snapshot(snapshot, true);
        assert_eq!(c.status, "pending");
        assert_eq!((c.total, c.available, c.loading, c.failed), (1, 0, 1, 0));
        assert!(!c.blocks_readiness());
    }

    #[test]
    fn tenant_check_failed_non_strict_is_degraded_but_not_blocking() {
        let snapshot = snap(vec![(
            "alpha",
            TenantRuntimeState::failed_now("secret missing"),
        )]);
        let c = TenantCheck::from_snapshot(snapshot, false);
        assert_eq!(c.status, "degraded");
        assert_eq!(c.failed, 1);
        assert_eq!(c.failed_slugs, vec!["alpha"]);
        assert!(
            !c.blocks_readiness(),
            "permissive mode must not take the pod out of rotation"
        );
    }

    #[test]
    fn tenant_check_failed_strict_blocks_readiness() {
        let snapshot = snap(vec![
            ("alpha", TenantRuntimeState::available_now(1)),
            ("beta", TenantRuntimeState::failed_now("dns nxdomain")),
        ]);
        let c = TenantCheck::from_snapshot(snapshot, true);
        assert_eq!(c.status, "degraded");
        assert_eq!(c.failed, 1);
        assert!(c.blocks_readiness());

        let (status, Json(body)) = readiness_response(ok_check(), ok_check(), c);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
    }

    #[test]
    fn tenant_check_failed_slugs_are_sorted() {
        // Deterministic output keeps monitoring diffs quiet on steady
        // state; HashMap snapshot order is otherwise arbitrary.
        let snapshot = snap(vec![
            ("zulu", TenantRuntimeState::failed_now("x")),
            ("alpha", TenantRuntimeState::failed_now("y")),
            ("mike", TenantRuntimeState::failed_now("z")),
        ]);
        let c = TenantCheck::from_snapshot(snapshot, true);
        assert_eq!(c.failed_slugs, vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn tenant_check_never_leaks_failure_reasons_to_wire() {
        // The reason field may contain upstream error text not fit for
        // unauthenticated probes; only slugs go on the wire.
        let snapshot = snap(vec![(
            "alpha",
            TenantRuntimeState::failed_now("upstream api key XYZ rejected"),
        )]);
        let c = TenantCheck::from_snapshot(snapshot, false);
        let wire = serde_json::to_string(&c).unwrap();
        assert!(!wire.contains("XYZ"), "reason leaked: {wire}");
        assert!(!wire.contains("rejected"), "reason leaked: {wire}");
        assert!(wire.contains("\"failedSlugs\""));
        assert!(wire.contains("alpha"));
    }

    #[test]
    fn tenant_check_wire_fields_are_camel_case() {
        // Wire contract: dashboards key on camelCase. Guard against a
        // rename sneaking in.
        let snapshot = snap(vec![("x", TenantRuntimeState::available_now(1))]);
        let c = TenantCheck::from_snapshot(snapshot, true);
        let v = serde_json::to_value(&c).unwrap();
        for key in [
            "status",
            "total",
            "available",
            "loading",
            "failed",
            "strictMode",
        ] {
            assert!(v.get(key).is_some(), "missing key {key} in {v}");
        }
    }

    #[test]
    fn tenant_check_mixed_counts_are_exact() {
        let snapshot = snap(vec![
            ("a", TenantRuntimeState::available_now(1)),
            ("b", TenantRuntimeState::available_now(2)),
            ("c", TenantRuntimeState::loading_now()),
            ("d", TenantRuntimeState::failed_now("x")),
            ("e", TenantRuntimeState::failed_now("y")),
        ]);
        let c = TenantCheck::from_snapshot(snapshot, false);
        assert_eq!((c.total, c.available, c.loading, c.failed), (5, 2, 1, 2));
        // `degraded` wins over `pending` when both apply: operator
        // attention should focus on failures first.
        assert_eq!(c.status, "degraded");
    }
}
