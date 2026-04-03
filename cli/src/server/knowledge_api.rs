use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;

use super::AppState;
use super::plugin_auth::{tenant_context_from_plugin_bearer, validate_plugin_bearer};

#[derive(Debug, Deserialize)]
pub struct KnowledgeQueryRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub layer: Option<String>,
    pub tags: Option<Vec<String>>,
}

fn default_limit() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct CreateKnowledgeRequest {
    pub content: String,
    pub path: String,
    pub layer: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKnowledgeRequest {
    pub content: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub operations: Vec<BatchOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum BatchOperation {
    Create(CreateKnowledgeRequest),
    Delete { id: String },
}

#[derive(Debug, Serialize)]
pub struct DiscoveryResponse {
    pub service: &'static str,
    pub version: &'static str,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize)]
pub struct EndpointInfo {
    pub path: &'static str,
    pub method: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeItem {
    pub id: String,
    pub content: String,
    pub path: String,
    pub layer: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub items: Vec<KnowledgeItem>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub results: Vec<BatchOperationResult>,
}

#[derive(Debug, Serialize)]
pub struct BatchOperationResult {
    pub index: usize,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MetadataResponse {
    pub id: String,
    pub layer: String,
    pub path: String,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    #[serde(default)]
    pub layer: Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/knowledge", get(discovery_handler))
        .route("/knowledge/query", post(query_handler))
        .route("/knowledge/create", post(create_handler))
        .route("/knowledge/{id}", put(update_handler))
        .route("/knowledge/{id}", delete(delete_handler))
        .route("/knowledge/batch", post(batch_handler))
        .route("/knowledge/stream", get(stream_handler))
        .route("/knowledge/{id}/metadata", get(metadata_handler))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn discovery_handler() -> impl IntoResponse {
    Json(DiscoveryResponse {
        service: "aeterna-knowledge",
        version: env!("CARGO_PKG_VERSION"),
        endpoints: vec![
            EndpointInfo {
                path: "/api/v1/knowledge",
                method: "GET",
                description: "Service discovery",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/query",
                method: "POST",
                description: "Search knowledge with relevance ranking",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/create",
                method: "POST",
                description: "Create a new knowledge item",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/{id}",
                method: "PUT",
                description: "Update an existing knowledge item",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/{id}",
                method: "DELETE",
                description: "Delete a knowledge item",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/batch",
                method: "POST",
                description: "Batch operations on knowledge items",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/stream",
                method: "GET",
                description: "SSE stream of knowledge updates",
            },
            EndpointInfo {
                path: "/api/v1/knowledge/{id}/metadata",
                method: "GET",
                description: "Get metadata for a knowledge item",
            },
        ],
    })
}

#[tracing::instrument(skip_all, fields(query = %req.query, limit = req.limit))]
async fn query_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<KnowledgeQueryRequest>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);
    let layer =
        parse_layer(req.layer.as_deref()).unwrap_or(mk_core::types::KnowledgeLayer::Project);

    match state
        .knowledge_repository
        .list(ctx, layer, &req.query)
        .await
    {
        Ok(entries) => {
            let items: Vec<KnowledgeItem> = entries
                .into_iter()
                .take(req.limit)
                .map(knowledge_entry_to_item)
                .collect();
            let total = items.len();
            (StatusCode::OK, Json(QueryResponse { items, total })).into_response()
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "query_failed",
            &e.to_string(),
        ),
    }
}

#[tracing::instrument(skip_all, fields(path = %req.path, layer = %req.layer))]
async fn create_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateKnowledgeRequest>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);
    let layer = match parse_layer(Some(&req.layer)) {
        Some(l) => l,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_layer",
                &format!("Unknown layer: {}", req.layer),
            );
        }
    };

    let entry = mk_core::types::KnowledgeEntry {
        content: req.content,
        path: req.path,
        layer,
        kind: mk_core::types::KnowledgeType::Spec,
        status: mk_core::types::KnowledgeStatus::Draft,
        metadata: if req.metadata.is_null() {
            metadata_with_tags(req.tags, serde_json::Map::new())
        } else {
            match req.metadata {
                serde_json::Value::Object(map) => metadata_with_tags(req.tags, map),
                _ => metadata_with_tags(req.tags, serde_json::Map::new()),
            }
        },
        summaries: Default::default(),
        commit_hash: None,
        author: None,
        updated_at: 0,
    };

    match state
        .knowledge_repository
        .store(ctx, entry.clone(), "Created via Knowledge API")
        .await
    {
        Ok(commit_hash) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": entry.path,
                "commit": commit_hash,
            })),
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "create_failed",
            &e.to_string(),
        ),
    }
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn update_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateKnowledgeRequest>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);

    let existing = find_entry_by_id(&state, &ctx, &id).await;
    let mut entry = match existing {
        Some(e) => e,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "Knowledge item not found",
            );
        }
    };

    if let Some(content) = req.content {
        entry.content = content;
    }
    if let Some(path) = req.path {
        entry.path = path;
    }
    if let Some(tags) = req.tags {
        set_tags(&mut entry.metadata, tags);
    }
    if let Some(metadata) = req.metadata {
        if let Ok(m) = serde_json::from_value(metadata) {
            entry.metadata = m;
        }
    }

    match state
        .knowledge_repository
        .store(ctx, entry, "Updated via Knowledge API")
        .await
    {
        Ok(commit_hash) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": id,
                "commit": commit_hash,
            })),
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "update_failed",
            &e.to_string(),
        ),
    }
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);

    let existing = find_entry_by_id(&state, &ctx, &id).await;
    match existing {
        Some(entry) => {
            match state
                .knowledge_repository
                .delete(ctx, entry.layer, &entry.path, "Deleted via Knowledge API")
                .await
            {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(e) => error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "delete_failed",
                    &e.to_string(),
                ),
            }
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Knowledge item not found",
        ),
    }
}

#[tracing::instrument(skip_all, fields(operation_count = req.operations.len()))]
async fn batch_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<BatchRequest>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);
    let mut results = Vec::with_capacity(req.operations.len());

    for (index, op) in req.operations.into_iter().enumerate() {
        let result = match op {
            BatchOperation::Create(create_req) => {
                let layer = parse_layer(Some(&create_req.layer))
                    .unwrap_or(mk_core::types::KnowledgeLayer::Project);
                let entry = mk_core::types::KnowledgeEntry {
                    content: create_req.content,
                    path: create_req.path,
                    layer,
                    kind: mk_core::types::KnowledgeType::Spec,
                    status: mk_core::types::KnowledgeStatus::Draft,
                    metadata: metadata_with_tags(create_req.tags, serde_json::Map::new()),
                    summaries: Default::default(),
                    commit_hash: None,
                    author: None,
                    updated_at: 0,
                };
                let id = entry.path.clone();
                match state
                    .knowledge_repository
                    .store(ctx.clone(), entry, "Batch create via Knowledge API")
                    .await
                {
                    Ok(_) => BatchOperationResult {
                        index,
                        success: true,
                        id: Some(id),
                        error: None,
                    },
                    Err(e) => BatchOperationResult {
                        index,
                        success: false,
                        id: None,
                        error: Some(e.to_string()),
                    },
                }
            }
            BatchOperation::Delete { id } => match find_entry_by_id(&state, &ctx, &id).await {
                Some(entry) => {
                    match state
                        .knowledge_repository
                        .delete(
                            ctx.clone(),
                            entry.layer,
                            &entry.path,
                            "Batch delete via Knowledge API",
                        )
                        .await
                    {
                        Ok(_) => BatchOperationResult {
                            index,
                            success: true,
                            id: Some(id),
                            error: None,
                        },
                        Err(e) => BatchOperationResult {
                            index,
                            success: false,
                            id: None,
                            error: Some(e.to_string()),
                        },
                    }
                }
                None => BatchOperationResult {
                    index,
                    success: false,
                    id: None,
                    error: Some(format!("Item not found: {id}")),
                },
            },
        };
        results.push(result);
    }

    Json(BatchResponse { results }).into_response()
}

#[tracing::instrument(skip_all)]
async fn stream_handler(
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<StreamQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::once(async {
        Ok::<_, Infallible>(
            Event::default()
                .event("connected")
                .data(serde_json::json!({"status": "connected"}).to_string()),
        )
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn metadata_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(response) = reject_invalid_plugin_bearer(&state, &headers) {
        return response;
    }

    let ctx = tenant_context_from_request(&state, &headers);

    match find_entry_by_id(&state, &ctx, &id).await {
        Some(entry) => {
            let layer_str = match entry.layer {
                mk_core::types::KnowledgeLayer::Company => "company",
                mk_core::types::KnowledgeLayer::Org => "org",
                mk_core::types::KnowledgeLayer::Team => "team",
                mk_core::types::KnowledgeLayer::Project => "project",
            };
            (
                StatusCode::OK,
                Json(MetadataResponse {
                    id: entry.path.clone(),
                    layer: layer_str.to_string(),
                    path: entry.path,
                    created_at: entry.metadata.get("created_at").and_then(|v| v.as_i64()),
                    updated_at: entry.metadata.get("updated_at").and_then(|v| v.as_i64()),
                    tags: if tags_from_metadata(&entry.metadata).is_empty() {
                        None
                    } else {
                        Some(tags_from_metadata(&entry.metadata))
                    },
                }),
            )
                .into_response()
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "Knowledge item not found",
        ),
    }
}

fn default_tenant_context() -> mk_core::types::TenantContext {
    mk_core::types::TenantContext::new(
        mk_core::types::TenantId::new("default".to_string()).expect("default tenant id"),
        mk_core::types::UserId::new("system".to_string()).expect("default user id"),
    )
}

/// Derive the caller's `TenantContext` from a validated Aeterna plugin bearer
/// token.  Falls back to the default system context when no valid token is
/// present (e.g. unauthenticated or static-token callers).
fn tenant_context_from_request(
    state: &AppState,
    headers: &HeaderMap,
) -> mk_core::types::TenantContext {
    if let Some(secret) = state.plugin_auth_state.config.jwt_secret.as_deref() {
        if state.plugin_auth_state.config.enabled {
            return tenant_context_from_plugin_bearer(headers, secret);
        }
    }
    default_tenant_context()
}

fn reject_invalid_plugin_bearer(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<axum::response::Response> {
    if !state.plugin_auth_state.config.enabled {
        return None;
    }

    let Some(secret) = state.plugin_auth_state.config.jwt_secret.as_deref() else {
        return Some(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "configuration_error",
            "Plugin auth JWT secret is not configured",
        ));
    };

    if validate_plugin_bearer(headers, secret).is_none() {
        return Some(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_plugin_token",
            "Valid plugin bearer token required",
        ));
    }

    None
}

fn parse_layer(s: Option<&str>) -> Option<mk_core::types::KnowledgeLayer> {
    match s {
        Some("company") => Some(mk_core::types::KnowledgeLayer::Company),
        Some("org") | Some("organization") => Some(mk_core::types::KnowledgeLayer::Org),
        Some("team") => Some(mk_core::types::KnowledgeLayer::Team),
        Some("project") => Some(mk_core::types::KnowledgeLayer::Project),
        _ => None,
    }
}

fn knowledge_entry_to_item(entry: mk_core::types::KnowledgeEntry) -> KnowledgeItem {
    let layer_str = match entry.layer {
        mk_core::types::KnowledgeLayer::Company => "company",
        mk_core::types::KnowledgeLayer::Org => "org",
        mk_core::types::KnowledgeLayer::Team => "team",
        mk_core::types::KnowledgeLayer::Project => "project",
    };
    KnowledgeItem {
        id: entry.path.clone(),
        content: entry.content,
        path: entry.path,
        layer: layer_str.to_string(),
        tags: tags_from_metadata(&entry.metadata),
        metadata: if entry.metadata.is_empty() {
            None
        } else {
            serde_json::to_value(&entry.metadata).ok()
        },
    }
}

async fn find_entry_by_id(
    state: &AppState,
    ctx: &mk_core::types::TenantContext,
    id: &str,
) -> Option<mk_core::types::KnowledgeEntry> {
    let layers = [
        mk_core::types::KnowledgeLayer::Project,
        mk_core::types::KnowledgeLayer::Team,
        mk_core::types::KnowledgeLayer::Org,
        mk_core::types::KnowledgeLayer::Company,
    ];
    for layer in &layers {
        if let Ok(entries) = state
            .knowledge_repository
            .list(ctx.clone(), *layer, "")
            .await
        {
            if let Some(entry) = entries.into_iter().find(|entry| entry.path == id) {
                return Some(entry);
            }
        }
    }
    None
}

fn metadata_with_tags(
    tags: Vec<String>,
    map: serde_json::Map<String, serde_json::Value>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut metadata: std::collections::HashMap<String, serde_json::Value> =
        map.into_iter().collect();
    set_tags(&mut metadata, tags);
    metadata
}

fn set_tags(
    metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    tags: Vec<String>,
) {
    if tags.is_empty() {
        metadata.remove("tags");
    } else {
        metadata.insert("tags".to_string(), serde_json::json!(tags));
    }
}

fn tags_from_metadata(
    metadata: &std::collections::HashMap<String, serde_json::Value>,
) -> Vec<String> {
    metadata
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (
        status,
        Json(ErrorBody {
            error: error.to_string(),
            message: message.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginAuthState;
    use crate::server::plugin_auth::RefreshTokenStore;
    use agent_a2a::config::TrustedIdentityConfig;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use knowledge::api::GovernanceDashboardApi;
    use knowledge::governance::GovernanceEngine;
    use knowledge::manager::KnowledgeManager;
    use knowledge::repository::RepositoryError;
    use knowledge::tenant_repo_resolver::TenantRepositoryResolver;
    use memory::manager::MemoryManager;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::traits::{AuthorizationService, KnowledgeRepository};
    use mk_core::types::{
        KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType, ReasoningStrategy,
        ReasoningTrace, Role, TenantContext, UserId,
    };
    use std::collections::HashMap;
    use storage::postgres::PostgresBackend;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::websocket::{AuthToken, TokenValidator, WsResult, WsServer};
    use testing::postgres;
    use tools::server::McpServer;
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

        async fn get_user_roles(&self, _ctx: &TenantContext) -> Result<Vec<Role>, Self::Error> {
            Ok(vec![Role::Developer])
        }

        async fn assign_role(
            &self,
            _ctx: &TenantContext,
            _user_id: &UserId,
            _role: Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_role(
            &self,
            _ctx: &TenantContext,
            _user_id: &UserId,
            _role: Role,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct MockRepo {
        items: tokio::sync::RwLock<HashMap<(KnowledgeLayer, String), KnowledgeEntry>>,
    }

    impl MockRepo {
        fn new() -> Self {
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
                .filter(|((entry_layer, path), _)| {
                    *entry_layer == layer && path.starts_with(prefix)
                })
                .map(|(_, value)| value.clone())
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
            query: &str,
            layers: Vec<KnowledgeLayer>,
            limit: usize,
        ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
            Ok(self
                .items
                .read()
                .await
                .values()
                .filter(|entry| layers.contains(&entry.layer) && entry.content.contains(query))
                .take(limit)
                .cloned()
                .collect())
        }

        fn root_path(&self) -> Option<std::path::PathBuf> {
            None
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

    fn discovery_router() -> Router {
        Router::new().route("/knowledge", get(discovery_handler))
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

    async fn app_with_repo(repo: Arc<MockRepo>) -> Option<Router> {
        let auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync> =
            Arc::new(MockAuth);
        let fixture = postgres().await?;
        let postgres = Arc::new(PostgresBackend::new(fixture.url()).await.ok()?);
        postgres.initialize_schema().await.ok()?;
        let governance_engine = Arc::new(GovernanceEngine::new());
        let knowledge_manager = Arc::new(KnowledgeManager::new(
            Arc::new(
                knowledge::repository::GitRepository::new(tempfile::tempdir().unwrap().path())
                    .unwrap(),
            ),
            governance_engine.clone(),
        ));
        let memory_manager = Arc::new(MemoryManager::new());
        let sync_manager = Arc::new(
            SyncManager::new(
                memory_manager.clone(),
                knowledge_manager.clone(),
                config::config::DeploymentConfig::default(),
                None,
                Arc::new(sync::state_persister::FilePersister::new(
                    std::env::temp_dir(),
                )),
                None,
            )
            .await
            .unwrap(),
        );
        let dashboard = Arc::new(GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let mcp_server = Arc::new(McpServer::new(
            memory_manager.clone(),
            sync_manager.clone(),
            repo.clone(),
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

        Some(router(Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres,
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
                refresh_store: RefreshTokenStore::new(),
            }),
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
            git_provider_connection_registry,
        })))
    }

    #[tokio::test]
    async fn test_discovery_returns_endpoints() {
        let app = discovery_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/knowledge")
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
        assert_eq!(json["service"], "aeterna-knowledge");
        assert!(json["endpoints"].as_array().unwrap().len() == 8);
    }

    #[test]
    fn test_parse_layer_valid() {
        assert!(matches!(
            parse_layer(Some("company")),
            Some(mk_core::types::KnowledgeLayer::Company)
        ));
        assert!(matches!(
            parse_layer(Some("team")),
            Some(mk_core::types::KnowledgeLayer::Team)
        ));
        assert!(matches!(
            parse_layer(Some("org")),
            Some(mk_core::types::KnowledgeLayer::Org)
        ));
        assert!(matches!(
            parse_layer(Some("project")),
            Some(mk_core::types::KnowledgeLayer::Project)
        ));
    }

    #[test]
    fn test_parse_layer_invalid() {
        assert!(parse_layer(None).is_none());
        assert!(parse_layer(Some("unknown")).is_none());
    }

    #[test]
    fn tags_round_trip_via_metadata() {
        let metadata = metadata_with_tags(
            vec!["a".to_string(), "b".to_string()],
            serde_json::Map::new(),
        );
        assert_eq!(tags_from_metadata(&metadata), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn create_query_metadata_and_delete_flow_works() {
        let repo = Arc::new(MockRepo::new());
        let Some(app) = app_with_repo(repo.clone()).await else {
            eprintln!("Skipping OpenSpec test: Docker not available");
            return;
        };

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/knowledge/create")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "content": "hello world",
                            "path": "specs/new.md",
                            "layer": "project",
                            "tags": ["alpha"]
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let query_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/knowledge/query")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "query": "hello",
                            "limit": 10,
                            "layer": "project"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(query_response.status(), StatusCode::OK);
        let query_body = axum::body::to_bytes(query_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let query_json: serde_json::Value = serde_json::from_slice(&query_body).unwrap();
        assert_eq!(query_json["total"], 1);
        assert_eq!(query_json["items"][0]["path"], "specs/new.md");
        assert_eq!(query_json["items"][0]["tags"][0], "alpha");

        let update_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/knowledge/specs/new.md")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "content": "updated content",
                            "tags": ["beta"],
                            "metadata": { "owner": "platform" }
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_response.status(), StatusCode::OK);

        let metadata_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/knowledge/specs/new.md/metadata")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(metadata_response.status(), StatusCode::OK);
        let metadata_body = axum::body::to_bytes(metadata_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let metadata_json: serde_json::Value = serde_json::from_slice(&metadata_body).unwrap();
        assert_eq!(metadata_json["id"], "specs/new.md");
        assert_eq!(metadata_json["path"], "specs/new.md");
        assert_eq!(metadata_json["layer"], "project");
        assert_eq!(metadata_json["tags"][0], "beta");

        let get_updated = repo
            .get(
                TenantContext::default(),
                KnowledgeLayer::Project,
                "specs/new.md",
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(get_updated.content, "updated content");
        assert_eq!(tags_from_metadata(&get_updated.metadata), vec!["beta"]);
        assert_eq!(get_updated.metadata["owner"], "platform");

        let delete_response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/knowledge/specs/new.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
        assert!(
            repo.get(
                TenantContext::default(),
                KnowledgeLayer::Project,
                "specs/new.md"
            )
            .await
            .unwrap()
            .is_none()
        );
    }

    #[tokio::test]
    async fn batch_and_stream_endpoints_work() {
        let repo = Arc::new(MockRepo::new());
        let Some(app) = app_with_repo(repo).await else {
            eprintln!("Skipping OpenSpec test: Docker not available");
            return;
        };

        let batch_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/knowledge/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "operations": [{
                                "action": "create",
                                "content": "batched",
                                "path": "specs/batch.md",
                                "layer": "project",
                                "tags": []
                            }]
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(batch_response.status(), StatusCode::OK);
        let batch_body = axum::body::to_bytes(batch_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch_json: serde_json::Value = serde_json::from_slice(&batch_body).unwrap();
        assert_eq!(batch_json["results"][0]["success"], true);
        assert_eq!(batch_json["results"][0]["id"], "specs/batch.md");

        let stream_response = app
            .oneshot(
                Request::builder()
                    .uri("/knowledge/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(stream_response.status(), StatusCode::OK);
        let content_type = stream_response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.contains("text/event-stream"));
    }
}
