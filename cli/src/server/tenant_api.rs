use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use knowledge::tenant_repo_resolver::RepoResolutionError;
use mk_core::traits::{StorageBackend, TenantConfigProvider};
use mk_core::types::{
    BranchPolicy, CredentialKind, GitProviderConnection, GitProviderKind, GovernanceEvent,
    OrganizationalUnit, PersistentEvent, RecordSource, RepositoryKind, Role, TenantConfigDocument,
    TenantConfigField, TenantConfigOwnership, TenantContext, TenantSecretEntry,
    TenantSecretReference, UnitType,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::PrincipalType;
use storage::git_provider_connection_store::GitProviderConnectionError;
use storage::tenant_config_provider::TenantConfigProviderError;
use storage::tenant_store::UpsertTenantRepositoryBinding;
use uuid::Uuid;

use super::{AppState, authenticated_tenant_context, tenant_scoped_context};

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    pub slug: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TenantListQuery {
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantDomainMappingRequest {
    pub domain: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTenantConfigRequest {
    #[serde(default)]
    pub fields: BTreeMap<String, TenantConfigField>,
    #[serde(default)]
    pub secret_references: BTreeMap<String, TenantSecretReference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTenantSecretRequest {
    #[serde(default = "default_tenant_ownership")]
    pub ownership: TenantConfigOwnership,
    pub secret_value: String,
}

// ---------------------------------------------------------------------------
// Git provider connection request/response types (task 3.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGitProviderConnectionRequest {
    pub name: String,
    pub provider_kind: GitProviderKind,
    pub app_id: u64,
    pub installation_id: u64,
    /// Secret-provider reference to the PEM private key (must use local/, secret/, arn:aws: prefix).
    pub pem_secret_ref: String,
    /// Optional secret-provider reference to the webhook secret.
    pub webhook_secret_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTenantRepositoryBindingRequest {
    pub kind: RepositoryKind,
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
    pub branch: String,
    pub branch_policy: BranchPolicy,
    pub credential_kind: CredentialKind,
    pub credential_ref: Option<String>,
    pub github_owner: Option<String>,
    pub github_repo: Option<String>,
    #[serde(default)]
    pub source_owner: RecordSource,
    /// Reference a platform-owned Git provider connection by ID.
    /// When set, `credential_ref` is not required for GitHubApp bindings.
    pub git_provider_connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HierarchyListQuery {
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateHierarchyUnitRequest {
    pub name: String,
    pub unit_type: UnitType,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Source ownership: defaults to `admin` for API-created units.
    #[serde(default)]
    pub source_owner: RecordSource,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHierarchyUnitRequest {
    pub name: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUnitRoleRequest {
    pub user_id: String,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct UserRoleListQuery {
    pub user_id: String,
}

#[derive(Debug, Serialize)]
struct UnitMemberRoleResponse {
    user_id: String,
    role: Role,
}

#[derive(Debug, Serialize)]
struct UserScopedRoleResponse {
    unit_id: String,
    role: Role,
}

#[derive(Debug, Serialize)]
struct TenantResponse<T> {
    success: bool,
    tenant: T,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/tenants", get(list_tenants).post(create_tenant))
        .route(
            "/admin/tenants/{tenant}",
            get(show_tenant).patch(update_tenant),
        )
        .route(
            "/admin/tenants/{tenant}/deactivate",
            post(deactivate_tenant),
        )
        .route(
            "/admin/tenants/{tenant}/domain-mappings",
            post(add_domain_mapping),
        )
        .route(
            "/admin/tenants/{tenant}/repository-binding",
            get(show_tenant_repository_binding).put(set_tenant_repository_binding),
        )
        .route(
            "/admin/tenants/{tenant}/repository-binding/validate",
            post(validate_tenant_repository_binding),
        )
        .route(
            "/admin/tenants/{tenant}/config",
            get(inspect_tenant_config).put(upsert_tenant_config),
        )
        .route(
            "/admin/tenants/{tenant}/config/validate",
            post(validate_tenant_config),
        )
        .route(
            "/admin/tenants/{tenant}/secrets/{logical_name}",
            put(set_tenant_secret).delete(delete_tenant_secret),
        )
        .route(
            "/admin/tenant-config",
            get(inspect_my_tenant_config).put(upsert_my_tenant_config),
        )
        .route(
            "/admin/tenant-config/validate",
            post(validate_my_tenant_config),
        )
        .route(
            "/admin/tenant-config/secrets/{logical_name}",
            put(set_my_tenant_secret).delete(delete_my_tenant_secret),
        )
        .route(
            "/admin/hierarchy",
            get(list_hierarchy_units).post(create_hierarchy_unit),
        )
        .route(
            "/admin/hierarchy/{unit}",
            get(show_hierarchy_unit).patch(update_hierarchy_unit),
        )
        .route(
            "/admin/hierarchy/{unit}/ancestors",
            get(list_hierarchy_ancestors),
        )
        .route(
            "/admin/hierarchy/{unit}/descendants",
            get(list_hierarchy_descendants),
        )
        .route(
            "/admin/hierarchy/{unit}/members",
            get(list_unit_members).post(assign_unit_role),
        )
        .route(
            "/admin/hierarchy/{unit}/members/{user_id}/roles/{role}",
            delete(remove_unit_role),
        )
        .route("/admin/memberships", get(list_user_memberships))
        .route("/admin/permissions/matrix", get(get_permission_matrix))
        .route(
            "/admin/permissions/effective",
            get(get_effective_permissions),
        )
        // Git provider connection routes (task 3.4)
        .route(
            "/admin/git-provider-connections",
            get(list_git_provider_connections).post(create_git_provider_connection),
        )
        .route(
            "/admin/git-provider-connections/{connection_id}",
            get(show_git_provider_connection),
        )
        .route(
            "/admin/git-provider-connections/{connection_id}/tenants/{tenant}",
            post(grant_git_provider_connection_to_tenant)
                .delete(revoke_git_provider_connection_from_tenant),
        )
        .route(
            "/admin/tenants/{tenant}/git-provider-connections",
            get(list_tenant_git_provider_connections),
        )
        .with_state(state)
}

fn default_tenant_ownership() -> TenantConfigOwnership {
    TenantConfigOwnership::Tenant
}

fn tenant_config_from_request(
    tenant_id: mk_core::types::TenantId,
    req: UpsertTenantConfigRequest,
) -> TenantConfigDocument {
    TenantConfigDocument {
        tenant_id,
        fields: req.fields,
        secret_references: req.secret_references,
    }
}

fn reject_non_tenant_owned_config(
    doc: &TenantConfigDocument,
) -> Result<(), axum::response::Response> {
    let platform_field = doc
        .fields
        .iter()
        .find(|(_, field)| field.ownership == TenantConfigOwnership::Platform)
        .map(|(key, _)| key.clone());
    if let Some(key) = platform_field {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            &format!("TenantAdmin cannot mutate platform-owned config field '{key}'"),
        ));
    }

    let platform_secret_ref = doc
        .secret_references
        .iter()
        .find(|(_, value)| value.ownership == TenantConfigOwnership::Platform)
        .map(|(key, _)| key.clone());
    if let Some(key) = platform_secret_ref {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            &format!("TenantAdmin cannot mutate platform-owned secret reference '{key}'"),
        ));
    }

    Ok(())
}

fn map_tenant_config_provider_error(
    operation: &str,
    err: TenantConfigProviderError,
) -> axum::response::Response {
    match err {
        TenantConfigProviderError::Validation(message) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("tenant_config_{operation}_invalid"),
            &message,
        ),
        TenantConfigProviderError::InvalidTenantId(message) => error_response(
            StatusCode::BAD_REQUEST,
            &format!("tenant_config_{operation}_failed"),
            &message,
        ),
    }
}

async fn resolve_tenant_record_or_404(
    state: &AppState,
    tenant_ref: &str,
    operation_error: &str,
) -> Result<mk_core::types::TenantRecord, axum::response::Response> {
    match state.tenant_store.get_tenant(tenant_ref).await {
        Ok(Some(record)) => Ok(record),
        Ok(None) => Err(error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_found",
            "Tenant not found",
        )),
        Err(err) => Err(error_response(
            StatusCode::BAD_REQUEST,
            operation_error,
            &err.to_string(),
        )),
    }
}

async fn inspect_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "tenant_config_inspect_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    match state
        .tenant_config_provider
        .get_config(&tenant_record.id)
        .await
    {
        Ok(config) => (
            StatusCode::OK,
            Json(json!({ "success": true, "config": config })),
        )
            .into_response(),
        Err(err) => map_tenant_config_provider_error("inspect", err),
    }
}

async fn upsert_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<UpsertTenantConfigRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "tenant_config_upsert_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };
    let document = tenant_config_from_request(tenant_record.id.clone(), req);

    match state.tenant_config_provider.upsert_config(document).await {
        Ok(config) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_upsert",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "fieldCount": config.fields.len(),
                    "secretReferenceCount": config.secret_references.len()
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "config": config })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("upsert", err),
    }
}

async fn validate_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<UpsertTenantConfigRequest>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    let tenant_record = match resolve_tenant_record_or_404(
        &state,
        &tenant,
        "tenant_config_validate_failed",
    )
    .await
    {
        Ok(record) => record,
        Err(response) => return response,
    };
    let document = tenant_config_from_request(tenant_record.id.clone(), req);

    match state.tenant_config_provider.validate(&document).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "success": true, "valid": true, "config": document })),
        )
            .into_response(),
        Err(TenantConfigProviderError::Validation(message)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "valid": false,
                "error": "tenant_config_invalid",
                "message": message,
            })),
        )
            .into_response(),
        Err(err) => map_tenant_config_provider_error("validate", err),
    }
}

async fn set_tenant_secret(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((tenant, logical_name)): Path<(String, String)>,
    Json(req): Json<SetTenantSecretRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "tenant_secret_set_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    let secret = TenantSecretEntry {
        logical_name: logical_name.clone(),
        ownership: req.ownership,
        secret_value: req.secret_value,
    };
    match state
        .tenant_config_provider
        .set_secret_entry(&tenant_record.id, secret)
        .await
    {
        Ok(reference) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_secret_set",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "logicalName": reference.logical_name,
                    "ownership": reference.ownership,
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "secretReference": reference })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("secret_set", err),
    }
}

async fn delete_tenant_secret(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((tenant, logical_name)): Path<(String, String)>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "tenant_secret_delete_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    // Guard: TenantAdmin cannot delete a platform-owned secret entry.
    // Resolve the existing config to check ownership before proceeding.
    if ctx.role != Some(Role::PlatformAdmin) {
        match state
            .tenant_config_provider
            .get_config(&tenant_record.id)
            .await
        {
            Ok(Some(existing)) => {
                if let Some(ref_entry) = existing.secret_references.get(&logical_name) {
                    if ref_entry.ownership == TenantConfigOwnership::Platform {
                        return error_response(
                            StatusCode::FORBIDDEN,
                            "forbidden",
                            "TenantAdmin cannot delete a platform-owned secret entry",
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(err) => return map_tenant_config_provider_error("secret_delete", err),
        }
    }

    match state
        .tenant_config_provider
        .delete_secret_entry(&tenant_record.id, &logical_name)
        .await
    {
        Ok(deleted) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_secret_delete",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "logicalName": logical_name,
                    "deleted": deleted,
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "deleted": deleted })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("secret_delete", err),
    }
}

async fn inspect_my_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    let tenant_ref = ctx
        .target_tenant_id
        .as_ref()
        .unwrap_or(&ctx.tenant_id)
        .as_str();
    let tenant_record = match resolve_tenant_record_or_404(
        &state,
        tenant_ref,
        "tenant_config_inspect_failed",
    )
    .await
    {
        Ok(record) => record,
        Err(response) => return response,
    };

    match state
        .tenant_config_provider
        .get_config(&tenant_record.id)
        .await
    {
        Ok(config) => (
            StatusCode::OK,
            Json(json!({ "success": true, "config": config })),
        )
            .into_response(),
        Err(err) => map_tenant_config_provider_error("inspect", err),
    }
}

async fn upsert_my_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpsertTenantConfigRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    let tenant_ref = ctx
        .target_tenant_id
        .as_ref()
        .unwrap_or(&ctx.tenant_id)
        .as_str();
    let tenant_record =
        match resolve_tenant_record_or_404(&state, tenant_ref, "tenant_config_upsert_failed").await
        {
            Ok(record) => record,
            Err(response) => return response,
        };
    let document = tenant_config_from_request(tenant_record.id.clone(), req);
    if let Err(response) = reject_non_tenant_owned_config(&document) {
        return response;
    }

    match state.tenant_config_provider.upsert_config(document).await {
        Ok(config) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_upsert",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "fieldCount": config.fields.len(),
                    "secretReferenceCount": config.secret_references.len()
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "config": config })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("upsert", err),
    }
}

async fn validate_my_tenant_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpsertTenantConfigRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_ref = ctx
        .target_tenant_id
        .as_ref()
        .unwrap_or(&ctx.tenant_id)
        .as_str();
    let tenant_record =
        match resolve_tenant_record_or_404(&state, tenant_ref, "tenant_config_validate_failed")
            .await
        {
            Ok(record) => record,
            Err(response) => return response,
        };
    let document = tenant_config_from_request(tenant_record.id.clone(), req);
    if let Err(response) = reject_non_tenant_owned_config(&document) {
        return response;
    }

    match state.tenant_config_provider.validate(&document).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "success": true, "valid": true, "config": document })),
        )
            .into_response(),
        Err(TenantConfigProviderError::Validation(message)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "valid": false,
                "error": "tenant_config_invalid",
                "message": message,
            })),
        )
            .into_response(),
        Err(err) => map_tenant_config_provider_error("validate", err),
    }
}

async fn set_my_tenant_secret(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(logical_name): Path<String>,
    Json(req): Json<SetTenantSecretRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if req.ownership != TenantConfigOwnership::Tenant {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "TenantAdmin cannot mutate platform-owned secret entries",
        );
    }

    let tenant_ref = ctx
        .target_tenant_id
        .as_ref()
        .unwrap_or(&ctx.tenant_id)
        .as_str();
    let tenant_record =
        match resolve_tenant_record_or_404(&state, tenant_ref, "tenant_secret_set_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    let secret = TenantSecretEntry {
        logical_name: logical_name.clone(),
        ownership: req.ownership,
        secret_value: req.secret_value,
    };

    match state
        .tenant_config_provider
        .set_secret_entry(&tenant_record.id, secret)
        .await
    {
        Ok(reference) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_secret_set",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "logicalName": reference.logical_name,
                    "ownership": reference.ownership,
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "secretReference": reference })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("secret_set", err),
    }
}

async fn delete_my_tenant_secret(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(logical_name): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_ref = ctx
        .target_tenant_id
        .as_ref()
        .unwrap_or(&ctx.tenant_id)
        .as_str();
    let tenant_record =
        match resolve_tenant_record_or_404(&state, tenant_ref, "tenant_secret_delete_failed").await
        {
            Ok(record) => record,
            Err(response) => return response,
        };

    let existing_config = match state
        .tenant_config_provider
        .get_config(&tenant_record.id)
        .await
    {
        Ok(config) => config,
        Err(err) => return map_tenant_config_provider_error("secret_delete", err),
    };

    if let Some(reference) = existing_config
        .as_ref()
        .and_then(|config| config.secret_references.get(&logical_name))
    {
        if reference.ownership == TenantConfigOwnership::Platform
            && ctx.role != Some(Role::PlatformAdmin)
        {
            return error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "TenantAdmin cannot delete platform-owned secret entries",
            );
        }
    }

    match state
        .tenant_config_provider
        .delete_secret_entry(&tenant_record.id, &logical_name)
        .await
    {
        Ok(deleted) => {
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_config_secret_delete",
                Some(tenant_record.id.as_str()),
                json!({
                    "tenantId": tenant_record.id.as_str(),
                    "logicalName": logical_name,
                    "deleted": deleted,
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({ "success": true, "deleted": deleted })),
            )
                .into_response()
        }
        Err(err) => map_tenant_config_provider_error("secret_delete", err),
    }
}

async fn create_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.tenant_store.create_tenant(&req.slug, &req.name).await {
        Ok(record) => {
            let now = chrono::Utc::now().timestamp();
            let tenant_event = GovernanceEvent::TenantCreated {
                record_id: record.id.as_str().to_string(),
                slug: record.slug.clone(),
                tenant_id: record.id.clone(),
                timestamp: now,
            };
            let _ = state.postgres.log_event(&tenant_event).await;
            let _ = state
                .postgres
                .persist_event(PersistentEvent::new(tenant_event))
                .await;
            audit_tenant_action(
                &state,
                &ctx,
                "tenant_create",
                Some(record.id.as_str()),
                json!({ "slug": record.slug, "name": record.name }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(TenantResponse {
                    success: true,
                    tenant: record,
                }),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_create_failed",
            &err.to_string(),
        ),
    }
}

async fn list_tenants(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<TenantListQuery>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    match state
        .tenant_store
        .list_tenants(query.include_inactive)
        .await
    {
        Ok(tenants) => (
            StatusCode::OK,
            Json(json!({ "success": true, "tenants": tenants })),
        )
            .into_response(),
        Err(err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_list_failed",
            &err.to_string(),
        ),
    }
}

async fn show_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    match state.tenant_store.get_tenant(&tenant).await {
        Ok(Some(record)) => (
            StatusCode::OK,
            Json(TenantResponse {
                success: true,
                tenant: record,
            }),
        )
            .into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_found",
            "Tenant not found",
        ),
        Err(err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "tenant_show_failed",
            &err.to_string(),
        ),
    }
}

async fn update_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<UpdateTenantRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state
        .tenant_store
        .update_tenant(&tenant, req.slug.as_deref(), req.name.as_deref())
        .await
    {
        Ok(Some(record)) => {
            let now = chrono::Utc::now().timestamp();
            let tenant_event = GovernanceEvent::TenantUpdated {
                record_id: record.id.as_str().to_string(),
                tenant_id: record.id.clone(),
                timestamp: now,
            };
            let _ = state.postgres.log_event(&tenant_event).await;
            let _ = state
                .postgres
                .persist_event(PersistentEvent::new(tenant_event))
                .await;
            audit_tenant_action(
                &state,
                &ctx,
                "tenant_update",
                Some(record.id.as_str()),
                json!({ "slug": record.slug, "name": record.name }),
            )
            .await;
            (
                StatusCode::OK,
                Json(TenantResponse {
                    success: true,
                    tenant: record,
                }),
            )
                .into_response()
        }
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_found",
            "Tenant not found",
        ),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_update_failed",
            &err.to_string(),
        ),
    }
}

async fn deactivate_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.tenant_store.deactivate_tenant(&tenant).await {
        Ok(Some(record)) => {
            let now = chrono::Utc::now().timestamp();
            let tenant_event = GovernanceEvent::TenantDeactivated {
                record_id: record.id.as_str().to_string(),
                tenant_id: record.id.clone(),
                timestamp: now,
            };
            let _ = state.postgres.log_event(&tenant_event).await;
            let _ = state
                .postgres
                .persist_event(PersistentEvent::new(tenant_event))
                .await;
            audit_tenant_action(
                &state,
                &ctx,
                "tenant_deactivate",
                Some(record.id.as_str()),
                json!({ "slug": record.slug, "status": record.status }),
            )
            .await;
            (
                StatusCode::OK,
                Json(TenantResponse {
                    success: true,
                    tenant: record,
                }),
            )
                .into_response()
        }
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "tenant_not_found",
            "Tenant not found",
        ),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_deactivate_failed",
            &err.to_string(),
        ),
    }
}

async fn add_domain_mapping(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<CreateTenantDomainMappingRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state
        .tenant_store
        .add_verified_domain_mapping(&tenant, &req.domain)
        .await
    {
        Ok(record) => {
            audit_tenant_action(
                &state,
                &ctx,
                "tenant_domain_mapping_add",
                Some(record.id.as_str()),
                json!({
                    "domain": req.domain,
                    "tenantId": record.id.as_str(),
                    "selectedTargetTenantId": ctx.target_tenant_id.as_ref().map(|value| value.as_str())
                }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "tenant": record,
                    "domain": req.domain,
                    "verified": true,
                })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_domain_mapping_failed",
            &err.to_string(),
        ),
    }
}

async fn show_tenant_repository_binding(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    let tenant_record = match state.tenant_store.get_tenant(&tenant).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                "Tenant not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "tenant_binding_show_failed",
                &err.to_string(),
            );
        }
    };

    match state
        .tenant_repository_binding_store
        .get_binding(&tenant_record.id)
        .await
    {
        Ok(Some(binding)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "binding": binding.redacted() })),
        )
            .into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "binding_not_found",
            "No repository binding configured for tenant",
        ),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_binding_show_failed",
            &err.to_string(),
        ),
    }
}

async fn set_tenant_repository_binding(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<SetTenantRepositoryBindingRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record = match state.tenant_store.get_tenant(&tenant).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                "Tenant not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "tenant_binding_set_failed",
                &err.to_string(),
            );
        }
    };

    let previous_binding = match state
        .tenant_repository_binding_store
        .get_binding(&tenant_record.id)
        .await
    {
        Ok(binding) => binding,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "tenant_binding_set_failed",
                &err.to_string(),
            );
        }
    };

    // Validate that the credential_ref is a supported secret reference (not raw
    // secret material) before persisting (task 3.5).
    {
        let candidate_ref = mk_core::types::TenantRepositoryBinding {
            id: String::new(),
            tenant_id: tenant_record.id.clone(),
            kind: req.kind.clone(),
            local_path: req.local_path.clone(),
            remote_url: req.remote_url.clone(),
            branch: req.branch.clone(),
            branch_policy: req.branch_policy.clone(),
            credential_kind: req.credential_kind.clone(),
            credential_ref: req.credential_ref.clone(),
            github_owner: req.github_owner.clone(),
            github_repo: req.github_repo.clone(),
            source_owner: req.source_owner.clone(),
            git_provider_connection_id: req.git_provider_connection_id.clone(),
            created_at: 0,
            updated_at: 0,
        };
        if let Err(reason) = candidate_ref.validate_credential_ref() {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_credential_ref",
                &reason,
            );
        }
    }

    // When a connection_id is provided, validate that the tenant has visibility
    // before persisting (fail-closed: if the registry is absent, reject).
    if let Some(ref conn_id) = req.git_provider_connection_id {
        let allowed = state
            .git_provider_connection_registry
            .tenant_can_use(conn_id, &tenant_record.id)
            .await
            .map_err(|e: GitProviderConnectionError| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "connection_registry_error",
                    &e.to_string(),
                )
            });
        match allowed {
            Err(response) => return response,
            Ok(false) => {
                return error_response(
                    StatusCode::FORBIDDEN,
                    "connection_not_allowed",
                    &format!(
                        "Tenant '{}' is not in the allow-list for Git provider connection '{conn_id}'",
                        tenant_record.id.as_str()
                    ),
                );
            }
            Ok(true) => {}
        }
    }

    let binding_request = UpsertTenantRepositoryBinding {
        tenant_id: tenant_record.id.clone(),
        kind: req.kind,
        local_path: req.local_path,
        remote_url: req.remote_url,
        branch: req.branch,
        branch_policy: req.branch_policy,
        credential_kind: req.credential_kind,
        credential_ref: req.credential_ref,
        github_owner: req.github_owner,
        github_repo: req.github_repo,
        source_owner: req.source_owner,
        git_provider_connection_id: req.git_provider_connection_id,
    };

    match state
        .tenant_repository_binding_store
        .upsert_binding(binding_request)
        .await
    {
        Ok(binding) => {
            let now = chrono::Utc::now().timestamp();
            let event = if previous_binding.is_some() {
                GovernanceEvent::RepositoryBindingUpdated {
                    binding_id: binding.id.clone(),
                    tenant_id: binding.tenant_id.clone(),
                    timestamp: now,
                }
            } else {
                GovernanceEvent::RepositoryBindingCreated {
                    binding_id: binding.id.clone(),
                    tenant_id: binding.tenant_id.clone(),
                    timestamp: now,
                }
            };
            persist_governance_event(state.as_ref(), &event).await;
            state.tenant_repo_resolver.invalidate(&binding.tenant_id);
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "tenant_repository_binding_set",
                Some(binding.id.as_str()),
                json!({
                    "tenantId": binding.tenant_id.as_str(),
                    "kind": binding.kind,
                    "branch": binding.branch,
                    "branchPolicy": binding.branch_policy,
                    "credentialKind": binding.credential_kind,
                    "sourceOwner": binding.source_owner,
                }),
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({ "success": true, "binding": binding.redacted() })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_binding_set_failed",
            &err.to_string(),
        ),
    }
}

async fn validate_tenant_repository_binding(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
    Json(req): Json<SetTenantRepositoryBindingRequest>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    let tenant_record = match state.tenant_store.get_tenant(&tenant).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                "Tenant not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "tenant_binding_validate_failed",
                &err.to_string(),
            );
        }
    };

    let candidate = mk_core::types::TenantRepositoryBinding {
        id: "validation".to_string(),
        tenant_id: tenant_record.id.clone(),
        kind: req.kind,
        local_path: req.local_path,
        remote_url: req.remote_url,
        branch: req.branch,
        branch_policy: req.branch_policy,
        credential_kind: req.credential_kind,
        credential_ref: req.credential_ref,
        github_owner: req.github_owner,
        github_repo: req.github_repo,
        source_owner: req.source_owner,
        git_provider_connection_id: None,
        created_at: 0,
        updated_at: 0,
    };

    match state
        .tenant_repo_resolver
        .validate_binding(&tenant_record.id, &candidate)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "success": true, "valid": true, "binding": candidate.redacted() })),
        )
            .into_response(),
        Err(RepoResolutionError::InvalidBinding { reason, .. }) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "success": false,
                "valid": false,
                "error": "repository_binding_invalid",
                "message": reason,
            })),
        )
            .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "tenant_binding_validate_failed",
            &err.to_string(),
        ),
    }
}

async fn list_hierarchy_units(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<HierarchyListQuery>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let result = if let Some(parent_id) = query.parent_id.as_deref() {
        state.postgres.list_children(&ctx, parent_id).await
    } else {
        state.postgres.list_all_units().await.map(|units| {
            let mut units: Vec<_> = units
                .into_iter()
                .filter(|unit| unit.tenant_id == ctx.tenant_id && unit.parent_id.is_none())
                .collect();
            units.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
            units
        })
    };

    match result {
        Ok(units) => (
            StatusCode::OK,
            Json(json!({ "success": true, "units": units })),
        )
            .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_list_failed",
            &err.to_string(),
        ),
    }
}

async fn create_hierarchy_unit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateHierarchyUnitRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let now = chrono::Utc::now().timestamp();
    let unit = OrganizationalUnit {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        unit_type: req.unit_type,
        parent_id: req.parent_id,
        tenant_id: ctx.tenant_id.clone(),
        metadata: req.metadata,
        source_owner: req.source_owner,
        created_at: now,
        updated_at: now,
    };

    match state.postgres.create_unit(&unit).await {
        Ok(()) => {
            persist_governance_event(
                state.as_ref(),
                &GovernanceEvent::UnitCreated {
                    unit_id: unit.id.clone(),
                    unit_type: unit.unit_type,
                    tenant_id: ctx.tenant_id.clone(),
                    parent_id: unit.parent_id.clone(),
                    timestamp: now,
                },
            )
            .await;
            audit_hierarchy_action(
                state.as_ref(),
                &ctx,
                "hierarchy_unit_create",
                Some(unit.id.as_str()),
                json!({
                    "name": unit.name,
                    "unitType": unit.unit_type,
                    "parentId": unit.parent_id,
                }),
            )
            .await;

            (
                StatusCode::CREATED,
                Json(json!({ "success": true, "unit": unit })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_unit_create_failed",
            &err.to_string(),
        ),
    }
}

async fn show_hierarchy_unit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.postgres.get_unit(&ctx, &unit).await {
        Ok(Some(unit)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "unit": unit })),
        )
            .into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "hierarchy_unit_not_found",
            "Hierarchy unit not found",
        ),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_unit_show_failed",
            &err.to_string(),
        ),
    }
}

async fn update_hierarchy_unit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
    Json(req): Json<UpdateHierarchyUnitRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let Some(mut existing) = (match state.postgres.get_unit(&ctx, &unit).await {
        Ok(unit) => unit,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "hierarchy_unit_update_failed",
                &err.to_string(),
            );
        }
    }) else {
        return error_response(
            StatusCode::NOT_FOUND,
            "hierarchy_unit_not_found",
            "Hierarchy unit not found",
        );
    };

    if let Some(name) = req.name {
        existing.name = name;
    }
    if let Some(metadata) = req.metadata {
        existing.metadata = metadata;
    }
    existing.updated_at = chrono::Utc::now().timestamp();

    match state.postgres.update_unit(&ctx, &existing).await {
        Ok(()) => {
            persist_governance_event(
                state.as_ref(),
                &GovernanceEvent::UnitUpdated {
                    unit_id: existing.id.clone(),
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: existing.updated_at,
                },
            )
            .await;
            audit_hierarchy_action(
                state.as_ref(),
                &ctx,
                "hierarchy_unit_update",
                Some(existing.id.as_str()),
                json!({
                    "name": existing.name,
                    "unitType": existing.unit_type,
                    "parentId": existing.parent_id,
                }),
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({ "success": true, "unit": existing })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_unit_update_failed",
            &err.to_string(),
        ),
    }
}

async fn list_hierarchy_ancestors(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.postgres.get_ancestors(&ctx, &unit).await {
        Ok(units) => (
            StatusCode::OK,
            Json(json!({ "success": true, "units": units })),
        )
            .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_ancestors_failed",
            &err.to_string(),
        ),
    }
}

async fn list_hierarchy_descendants(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.postgres.get_unit_descendants(&ctx, &unit).await {
        Ok(units) => (
            StatusCode::OK,
            Json(json!({ "success": true, "units": units })),
        )
            .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "hierarchy_descendants_failed",
            &err.to_string(),
        ),
    }
}

async fn list_unit_members(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match state.postgres.get_unit(&ctx, &unit).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "hierarchy_unit_not_found",
                "Hierarchy unit not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "membership_list_failed",
                &err.to_string(),
            );
        }
    }

    match state.postgres.list_unit_roles(&ctx.tenant_id, &unit).await {
        Ok(entries) => {
            let members: Vec<_> = entries
                .into_iter()
                .map(|(user_id, role)| UnitMemberRoleResponse {
                    user_id: user_id.into_inner(),
                    role,
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!({ "success": true, "unitId": unit, "members": members })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "membership_list_failed",
            &err.to_string(),
        ),
    }
}

async fn assign_unit_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(unit): Path<String>,
    Json(req): Json<AssignUnitRoleRequest>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    // Gate: caller must have AssignRoles permission on the tenant company resource.
    let authz_resource = format!("Aeterna::Company::\"{}\"", ctx.tenant_id.as_str());
    match state
        .auth_service
        .check_permission(&ctx, "AssignRoles", &authz_resource)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "AssignRoles permission required for this tenant",
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authz_check_failed",
                &e.to_string(),
            );
        }
    }

    if req.role == Role::PlatformAdmin {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_role_assignment",
            "PlatformAdmin cannot be assigned as a tenant-scoped hierarchy role",
        );
    }

    match state.postgres.get_unit(&ctx, &unit).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "hierarchy_unit_not_found",
                "Hierarchy unit not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "membership_assign_failed",
                &err.to_string(),
            );
        }
    }

    let user_id = match mk_core::types::UserId::new(req.user_id.clone()) {
        Some(user_id) => user_id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid user id",
            );
        }
    };

    match state
        .postgres
        .assign_role(&user_id, &ctx.tenant_id, &unit, req.role.clone())
        .await
    {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp();
            persist_governance_event(
                state.as_ref(),
                &GovernanceEvent::RoleAssigned {
                    user_id: user_id.clone(),
                    unit_id: unit.clone(),
                    role: req.role.clone(),
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: now,
                },
            )
            .await;
            audit_membership_action(
                state.as_ref(),
                &ctx,
                "membership_role_assign",
                Some(unit.as_str()),
                json!({
                    "unitId": unit,
                    "userId": user_id.as_str(),
                    "role": req.role,
                }),
            )
            .await;

            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "membership": {
                        "unitId": unit,
                        "userId": user_id.as_str(),
                        "role": req.role,
                    }
                })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "membership_assign_failed",
            &err.to_string(),
        ),
    }
}

async fn remove_unit_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((unit, user_id, role)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    // Gate: caller must have AssignRoles permission on the tenant company resource.
    let authz_resource = format!("Aeterna::Company::\"{}\"", ctx.tenant_id.as_str());
    match state
        .auth_service
        .check_permission(&ctx, "AssignRoles", &authz_resource)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "AssignRoles permission required for this tenant",
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "authz_check_failed",
                &e.to_string(),
            );
        }
    }

    match state.postgres.get_unit(&ctx, &unit).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "hierarchy_unit_not_found",
                "Hierarchy unit not found",
            );
        }
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "membership_remove_failed",
                &err.to_string(),
            );
        }
    }

    let user_id = match mk_core::types::UserId::new(user_id) {
        Some(user_id) => user_id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid user id",
            );
        }
    };
    let role: Role = match role.parse() {
        Ok(role) => role,
        Err(_) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_role_assignment",
                "Unsupported role",
            );
        }
    };

    if role == Role::PlatformAdmin {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_role_assignment",
            "PlatformAdmin cannot be assigned as a tenant-scoped hierarchy role",
        );
    }

    match state
        .postgres
        .remove_role(&user_id, &ctx.tenant_id, &unit, role.clone())
        .await
    {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp();
            persist_governance_event(
                state.as_ref(),
                &GovernanceEvent::RoleRemoved {
                    user_id: user_id.clone(),
                    unit_id: unit.clone(),
                    role: role.clone(),
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: now,
                },
            )
            .await;
            audit_membership_action(
                state.as_ref(),
                &ctx,
                "membership_role_remove",
                Some(unit.as_str()),
                json!({
                    "unitId": unit,
                    "userId": user_id.as_str(),
                    "role": role,
                }),
            )
            .await;

            (StatusCode::OK, Json(json!({ "success": true }))).into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "membership_remove_failed",
            &err.to_string(),
        ),
    }
}

async fn list_user_memberships(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<UserRoleListQuery>,
) -> impl IntoResponse {
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let user_id = match mk_core::types::UserId::new(query.user_id.clone()) {
        Some(user_id) => user_id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid user id",
            );
        }
    };

    match state
        .postgres
        .get_user_roles(&user_id, &ctx.tenant_id)
        .await
    {
        Ok(entries) => {
            let memberships: Vec<_> = entries
                .into_iter()
                .map(|(unit_id, role)| UserScopedRoleResponse { unit_id, role })
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "userId": user_id.as_str(),
                    "memberships": memberships,
                })),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "membership_list_failed",
            &err.to_string(),
        ),
    }
}

fn require_platform_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let ctx = authenticated_tenant_context(state, headers)?;
    if ctx.role != Some(Role::PlatformAdmin) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "PlatformAdmin role required",
        ));
    }

    Ok(ctx)
}

async fn require_tenant_admin_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let ctx = tenant_scoped_context(state, headers).await?;
    match ctx.role {
        Some(Role::PlatformAdmin) => Ok(ctx),
        Some(Role::Admin) => {
            // TenantAdmin is self-scoped: they MUST NOT target a different tenant.
            if let Some(ref target) = ctx.target_tenant_id {
                if target != &ctx.tenant_id {
                    return Err(error_response(
                        StatusCode::FORBIDDEN,
                        "forbidden",
                        "TenantAdmin cannot target a different tenant",
                    ));
                }
            }
            Ok(ctx)
        }
        _ => Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin or PlatformAdmin role required",
        )),
    }
}

async fn audit_tenant_action(
    state: &AppState,
    ctx: &TenantContext,
    action: &str,
    target_id: Option<&str>,
    details: serde_json::Value,
) {
    let Some(storage) = &state.governance_storage else {
        return;
    };

    let actor_id = uuid::Uuid::parse_str(ctx.user_id.as_str()).ok();
    let _ = storage
        .log_audit(
            action,
            None,
            Some("tenant"),
            target_id,
            PrincipalType::User,
            actor_id,
            None,
            serde_json::json!({
                "actorTenantId": ctx.tenant_id.as_str(),
                "selectedTargetTenantId": ctx.target_tenant_id.as_ref().map(|value| value.as_str()),
                "details": details,
            }),
        )
        .await;
}

async fn audit_hierarchy_action(
    state: &AppState,
    ctx: &TenantContext,
    action: &str,
    target_id: Option<&str>,
    details: serde_json::Value,
) {
    let Some(storage) = &state.governance_storage else {
        return;
    };

    let actor_id = uuid::Uuid::parse_str(ctx.user_id.as_str()).ok();
    let _ = storage
        .log_audit(
            action,
            None,
            Some("organizational_unit"),
            target_id,
            PrincipalType::User,
            actor_id,
            None,
            serde_json::json!({
                "actorTenantId": ctx.tenant_id.as_str(),
                "selectedTargetTenantId": ctx.target_tenant_id.as_ref().map(|value| value.as_str()),
                "details": details,
            }),
        )
        .await;
}

async fn audit_membership_action(
    state: &AppState,
    ctx: &TenantContext,
    action: &str,
    target_id: Option<&str>,
    details: serde_json::Value,
) {
    let Some(storage) = &state.governance_storage else {
        return;
    };

    let actor_id = uuid::Uuid::parse_str(ctx.user_id.as_str()).ok();
    let _ = storage
        .log_audit(
            action,
            None,
            Some("membership"),
            target_id,
            PrincipalType::User,
            actor_id,
            None,
            serde_json::json!({
                "actorTenantId": ctx.tenant_id.as_str(),
                "selectedTargetTenantId": ctx.target_tenant_id.as_ref().map(|value| value.as_str()),
                "details": details,
            }),
        )
        .await;
}

async fn persist_governance_event(state: &AppState, event: &GovernanceEvent) {
    let _ = state.postgres.log_event(event).await;
    let _ = state
        .postgres
        .persist_event(PersistentEvent::new(event.clone()))
        .await;
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (
        status,
        Json(json!({
            "error": error,
            "message": message,
        })),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Task 2.3 — Permission inspection
// ─────────────────────────────────────────────────────────────────────────────

/// All actions recognised by the active Cedar policy bundle.
const ALL_ACTIONS: &[&str] = &[
    "ViewMemory",
    "CreateMemory",
    "UpdateMemory",
    "DeleteMemory",
    "PromoteMemory",
    "ViewKnowledge",
    "ProposeKnowledge",
    "EditKnowledge",
    "ApproveKnowledge",
    "DeprecateKnowledge",
    "ViewPolicy",
    "CreatePolicy",
    "EditPolicy",
    "ApprovePolicy",
    "SimulatePolicy",
    "ViewGovernanceRequest",
    "SubmitGovernanceRequest",
    "ApproveGovernanceRequest",
    "RejectGovernanceRequest",
    "ViewOrganization",
    "CreateOrganization",
    "CreateTeam",
    "CreateProject",
    "ManageMembers",
    "AssignRoles",
    "RegisterAgent",
    "RevokeAgent",
    "DelegateToAgent",
    "ViewAuditLog",
    "ExportData",
    "ImportData",
    "ConfigureGovernance",
];

/// Role → permitted actions map derived from the Cedar RBAC policy file.
///
/// This is a static, authoritative matrix that mirrors `policies/cedar/rbac.cedar`.
/// It is intentionally kept in sync with the Cedar file; when the Cedar file
/// changes this map must be updated in the same commit.
fn role_permission_matrix() -> std::collections::HashMap<String, Vec<String>> {
    use std::collections::HashMap;

    let platform_admin: Vec<&str> = ALL_ACTIONS.to_vec(); // full access

    let admin: Vec<&str> = ALL_ACTIONS.to_vec(); // full access

    let architect: Vec<&str> = vec![
        "ViewMemory",
        "CreateMemory",
        "UpdateMemory",
        "DeleteMemory",
        "PromoteMemory",
        "ViewKnowledge",
        "ProposeKnowledge",
        "EditKnowledge",
        "ApproveKnowledge",
        "DeprecateKnowledge",
        "ViewPolicy",
        "CreatePolicy",
        "EditPolicy",
        "ApprovePolicy",
        "SimulatePolicy",
        "ViewGovernanceRequest",
        "SubmitGovernanceRequest",
        "ApproveGovernanceRequest",
        "RejectGovernanceRequest",
        "ViewOrganization",
        "CreateTeam",
        "CreateProject",
        "ManageMembers",
        "RegisterAgent",
        "RevokeAgent",
        "DelegateToAgent",
        "ViewAuditLog",
    ];

    let tech_lead: Vec<&str> = vec![
        "ViewMemory",
        "CreateMemory",
        "UpdateMemory",
        "DeleteMemory",
        "PromoteMemory",
        "ViewKnowledge",
        "ProposeKnowledge",
        "EditKnowledge",
        "ApproveKnowledge",
        "DeprecateKnowledge",
        "ViewPolicy",
        "CreatePolicy",
        "SimulatePolicy",
        "ViewGovernanceRequest",
        "SubmitGovernanceRequest",
        "ApproveGovernanceRequest",
        "RejectGovernanceRequest",
        "ViewOrganization",
        "CreateProject",
        "ManageMembers",
        "RegisterAgent",
        "RevokeAgent",
        "DelegateToAgent",
        "ViewAuditLog",
    ];

    let developer: Vec<&str> = vec![
        "ViewMemory",
        "CreateMemory",
        "UpdateMemory",
        "ViewKnowledge",
        "ProposeKnowledge",
        "ViewPolicy",
        "SimulatePolicy",
        "ViewGovernanceRequest",
        "SubmitGovernanceRequest",
        "ViewOrganization",
        "RegisterAgent",
        "DelegateToAgent",
    ];

    let viewer: Vec<&str> = vec![
        "ViewMemory",
        "ViewKnowledge",
        "ViewPolicy",
        "SimulatePolicy",
        "ViewGovernanceRequest",
        "ViewOrganization",
    ];

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    map.insert(
        "platformAdmin".to_string(),
        platform_admin.iter().map(|s| s.to_string()).collect(),
    );
    map.insert(
        "admin".to_string(),
        admin.iter().map(|s| s.to_string()).collect(),
    );
    map.insert(
        "architect".to_string(),
        architect.iter().map(|s| s.to_string()).collect(),
    );
    map.insert(
        "techLead".to_string(),
        tech_lead.iter().map(|s| s.to_string()).collect(),
    );
    map.insert(
        "developer".to_string(),
        developer.iter().map(|s| s.to_string()).collect(),
    );
    map.insert(
        "viewer".to_string(),
        viewer.iter().map(|s| s.to_string()).collect(),
    );
    map
}

/// `GET /api/v1/admin/permissions/matrix`
///
/// Returns the role-to-permission matrix derived from the active Cedar RBAC policy.
/// Requires PlatformAdmin or Admin role.
async fn get_permission_matrix(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = require_platform_or_admin(&state, &headers).await {
        return response;
    }

    let matrix = role_permission_matrix();
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "matrix": matrix,
            "actions": ALL_ACTIONS,
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct EffectivePermissionsQuery {
    user_id: String,
    resource: Option<String>,
    actions: Option<String>,
    role: Option<Role>,
}

/// `GET /api/v1/admin/permissions/effective?role=<role>`
///
/// Returns the set of permitted actions for the given role in the active policy bundle.
/// Requires PlatformAdmin or Admin role.
async fn get_effective_permissions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EffectivePermissionsQuery>,
) -> impl IntoResponse {
    let admin_ctx = match require_platform_or_admin(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let user_id = match mk_core::types::UserId::new(query.user_id.clone()) {
        Some(user_id) => user_id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invalid user id",
            );
        }
    };

    let resource = query
        .resource
        .unwrap_or_else(|| format!("Aeterna::Company::\"{}\"", admin_ctx.tenant_id.as_str()));

    let actions: Vec<String> = query
        .actions
        .as_deref()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_else(|| {
            ALL_ACTIONS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        });

    let mut principal_ctx = TenantContext::new(admin_ctx.tenant_id.clone(), user_id.clone());

    let roles = if let Some(role) = query.role.clone() {
        principal_ctx.role = Some(role.clone());
        vec![role]
    } else {
        match state.auth_service.get_user_roles(&principal_ctx).await {
            Ok(roles) => roles,
            Err(err) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "permission_inspection_failed",
                    &err.to_string(),
                );
            }
        }
    };

    let mut granted = Vec::new();
    let mut denied = Vec::new();

    for action in &actions {
        let allowed = if roles.is_empty() {
            match state
                .auth_service
                .check_permission(&principal_ctx, action, &resource)
                .await
            {
                Ok(value) => value,
                Err(err) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "permission_inspection_failed",
                        &err.to_string(),
                    );
                }
            }
        } else {
            let mut allowed = false;
            for role in &roles {
                let mut scoped_ctx = principal_ctx.clone();
                scoped_ctx.role = Some(role.clone());
                match state
                    .auth_service
                    .check_permission(&scoped_ctx, action, &resource)
                    .await
                {
                    Ok(true) => {
                        allowed = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(err) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "permission_inspection_failed",
                            &err.to_string(),
                        );
                    }
                }
            }
            allowed
        };

        if allowed {
            granted.push(action.clone());
        } else {
            denied.push(action.clone());
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "userId": user_id.as_str(),
            "resource": resource,
            "roles": roles,
            "granted": granted,
            "denied": denied,
        })),
    )
        .into_response()
}

/// Require PlatformAdmin **or** Admin (tenant-scoped) role.
async fn require_platform_or_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let ctx = match super::tenant_scoped_context(state, headers).await {
        Ok(ctx) => ctx,
        Err(response) => return Err(response),
    };
    if matches!(ctx.role, Some(Role::PlatformAdmin | Role::Admin)) {
        Ok(ctx)
    } else {
        Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin or PlatformAdmin role required",
        ))
    }
}

// ---------------------------------------------------------------------------
// Git provider connection handlers (task 3.4)
// ---------------------------------------------------------------------------

fn map_connection_error(
    operation: &str,
    err: GitProviderConnectionError,
) -> axum::response::Response {
    match &err {
        GitProviderConnectionError::NotFound(id) => error_response(
            StatusCode::NOT_FOUND,
            &format!("git_provider_connection_{operation}_not_found"),
            &format!("Git provider connection '{id}' not found"),
        ),
        GitProviderConnectionError::Validation(msg) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("git_provider_connection_{operation}_invalid"),
            msg,
        ),
    }
}

async fn create_git_provider_connection(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateGitProviderConnectionRequest>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let now = chrono::Utc::now().timestamp();
    let connection = GitProviderConnection {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        provider_kind: req.provider_kind,
        app_id: req.app_id,
        installation_id: req.installation_id,
        pem_secret_ref: req.pem_secret_ref,
        webhook_secret_ref: req.webhook_secret_ref,
        allowed_tenant_ids: Vec::new(),
        created_at: now,
        updated_at: now,
    };

    match state
        .git_provider_connection_registry
        .create_connection(connection)
        .await
    {
        Ok(created) => {
            let event = GovernanceEvent::GitProviderConnectionCreated {
                connection_id: created.id.clone(),
                tenant_id: ctx.tenant_id.clone(),
                timestamp: now,
            };
            persist_governance_event(state.as_ref(), &event).await;
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "git_provider_connection_create",
                Some(created.id.as_str()),
                json!({ "connectionId": created.id, "name": created.name }),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({ "success": true, "connection": created.redacted() })),
            )
                .into_response()
        }
        Err(err) => map_connection_error("create", err),
    }
}

async fn list_git_provider_connections(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    match state
        .git_provider_connection_registry
        .list_connections()
        .await
    {
        Ok(connections) => {
            let redacted: Vec<_> = connections.iter().map(|c| c.redacted()).collect();
            (
                StatusCode::OK,
                Json(json!({ "success": true, "connections": redacted })),
            )
                .into_response()
        }
        Err(err) => map_connection_error("list", err),
    }
}

async fn show_git_provider_connection(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(connection_id): Path<String>,
) -> impl IntoResponse {
    if let Err(response) = require_platform_admin(&state, &headers) {
        return response;
    }

    match state
        .git_provider_connection_registry
        .get_connection(&connection_id)
        .await
    {
        Ok(Some(conn)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "connection": conn.redacted() })),
        )
            .into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "git_provider_connection_not_found",
            &format!("Git provider connection '{connection_id}' not found"),
        ),
        Err(err) => map_connection_error("show", err),
    }
}

async fn grant_git_provider_connection_to_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((connection_id, tenant)): Path<(String, String)>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "connection_grant_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    match state
        .git_provider_connection_registry
        .grant_tenant_visibility(&connection_id, &tenant_record.id)
        .await
    {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp();
            let event = GovernanceEvent::GitProviderConnectionTenantGranted {
                connection_id: connection_id.clone(),
                tenant_id: tenant_record.id.clone(),
                timestamp: now,
            };
            persist_governance_event(state.as_ref(), &event).await;
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "git_provider_connection_grant",
                Some(connection_id.as_str()),
                json!({
                    "connectionId": connection_id,
                    "tenantId": tenant_record.id.as_str(),
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "connectionId": connection_id,
                    "tenantId": tenant_record.id.as_str(),
                })),
            )
                .into_response()
        }
        Err(err) => map_connection_error("grant", err),
    }
}

async fn revoke_git_provider_connection_from_tenant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((connection_id, tenant)): Path<(String, String)>,
) -> impl IntoResponse {
    let ctx = match require_platform_admin(&state, &headers) {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "connection_revoke_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    match state
        .git_provider_connection_registry
        .revoke_tenant_visibility(&connection_id, &tenant_record.id)
        .await
    {
        Ok(()) => {
            let now = chrono::Utc::now().timestamp();
            let event = GovernanceEvent::GitProviderConnectionTenantRevoked {
                connection_id: connection_id.clone(),
                tenant_id: tenant_record.id.clone(),
                timestamp: now,
            };
            persist_governance_event(state.as_ref(), &event).await;
            audit_tenant_action(
                state.as_ref(),
                &ctx,
                "git_provider_connection_revoke",
                Some(connection_id.as_str()),
                json!({
                    "connectionId": connection_id,
                    "tenantId": tenant_record.id.as_str(),
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "connectionId": connection_id,
                    "tenantId": tenant_record.id.as_str(),
                })),
            )
                .into_response()
        }
        Err(err) => map_connection_error("revoke", err),
    }
}

async fn list_tenant_git_provider_connections(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    // Both PlatformAdmin (full list) and TenantAdmin (own-tenant view) are allowed.
    let ctx = match require_tenant_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let tenant_record =
        match resolve_tenant_record_or_404(&state, &tenant, "connection_list_failed").await {
            Ok(record) => record,
            Err(response) => return response,
        };

    // TenantAdmin can only list for their own tenant.
    if ctx.role != Some(Role::PlatformAdmin) && tenant_record.id != ctx.tenant_id {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "TenantAdmin can only list connections for their own tenant",
        );
    }

    match state
        .git_provider_connection_registry
        .list_connections_for_tenant(&tenant_record.id)
        .await
    {
        Ok(connections) => {
            let redacted: Vec<_> = connections.iter().map(|c| c.redacted()).collect();
            (
                StatusCode::OK,
                Json(json!({ "success": true, "connections": redacted })),
            )
                .into_response()
        }
        Err(err) => map_connection_error("list_for_tenant", err),
    }
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
        KnowledgeEntry, KnowledgeLayer, ReasoningStrategy, ReasoningTrace, UserId,
    };
    use std::path::PathBuf;
    use storage::secret_provider::LocalSecretProvider;
    use storage::tenant_config_provider::KubernetesTenantConfigProvider;
    use storage::tenant_store::{TenantRepositoryBindingStore, TenantStore};
    use sync::bridge::SyncManager;
    use sync::state_persister::FilePersister;
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

        fn root_path(&self) -> Option<PathBuf> {
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
                thought_process: "tenant_api test noop".to_string(),
                refined_query: Some(query.to_string()),
                start_time: now,
                end_time: now,
                timed_out: false,
                duration_ms: 0,
                metadata: HashMap::new(),
            })
        }
    }

    async fn app_with_tenant() -> Option<(Router, mk_core::types::TenantRecord)> {
        let fixture = postgres().await?;
        let postgres = Arc::new(
            storage::postgres::PostgresBackend::new(fixture.url())
                .await
                .ok()?,
        );
        postgres.initialize_schema().await.ok()?;

        let auth_service: Arc<dyn AuthorizationService<Error = anyhow::Error> + Send + Sync> =
            Arc::new(MockAuth);
        let git_repo = Arc::new(
            knowledge::repository::GitRepository::new(tempfile::tempdir().ok()?.path()).ok()?,
        );
        let governance_engine = Arc::new(GovernanceEngine::new());
        let knowledge_manager =
            Arc::new(KnowledgeManager::new(git_repo, governance_engine.clone()));
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
            .ok()?,
        );
        let governance_dashboard = Arc::new(GovernanceDashboardApi::new(
            governance_engine.clone(),
            postgres.clone(),
            config::config::DeploymentConfig::default(),
        ));
        let mcp_server = Arc::new(McpServer::new(
            memory_manager.clone(),
            sync_manager.clone(),
            Arc::new(MockRepo),
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
        let tenant_config_provider =
            Arc::new(KubernetesTenantConfigProvider::new("default".to_string()));
        let tenant = tenant_store.create_tenant("acme", "Acme Corp").await.ok()?;

        let state = Arc::new(AppState {
            config: Arc::new(config::Config::default()),
            postgres,
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
                refresh_store: RefreshTokenStore::new(),
            }),
            idp_config: None,
            idp_sync_service: None,
            idp_client: None,
            shutdown_tx: Arc::new(shutdown_tx),
            tenant_store,
            tenant_repository_binding_store,
            tenant_repo_resolver,
            tenant_config_provider,
            git_provider_connection_registry,
        });

        Some((router(state), tenant))
    }

    fn request_with_headers(method: &str, uri: &str, role: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", role)
            .header("x-tenant-id", "default")
            .body(body)
            .unwrap()
    }

    #[tokio::test]
    async fn tenant_config_happy_path_and_secret_redaction() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        let upsert_config = serde_json::json!({
            "fields": {
                "runtime.logLevel": {
                    "ownership": "tenant",
                    "value": "info"
                }
            },
            "secretReferences": {}
        });
        let upsert_response = app
            .clone()
            .oneshot(request_with_headers(
                "PUT",
                &format!("/admin/tenants/{}/config", tenant.id.as_str()),
                "platformAdmin",
                Body::from(serde_json::to_vec(&upsert_config).unwrap()),
            ))
            .await
            .unwrap();
        assert_eq!(upsert_response.status(), StatusCode::OK);

        let secret_value = "super-secret-value";
        let set_secret_response = app
            .clone()
            .oneshot(request_with_headers(
                "PUT",
                &format!("/admin/tenants/{}/secrets/repo.token", tenant.id.as_str()),
                "platformAdmin",
                Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "ownership": "tenant",
                        "secretValue": secret_value
                    }))
                    .unwrap(),
                ),
            ))
            .await
            .unwrap();
        assert_eq!(set_secret_response.status(), StatusCode::OK);
        let set_body = axum::body::to_bytes(set_secret_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let set_json: serde_json::Value = serde_json::from_slice(&set_body).unwrap();
        assert_eq!(set_json["secretReference"]["logicalName"], "repo.token");
        assert!(!String::from_utf8_lossy(&set_body).contains(secret_value));

        let inspect_response = app
            .clone()
            .oneshot(request_with_headers(
                "GET",
                &format!("/admin/tenants/{}/config", tenant.id.as_str()),
                "platformAdmin",
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(inspect_response.status(), StatusCode::OK);
        let inspect_body = axum::body::to_bytes(inspect_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let inspect_text = String::from_utf8_lossy(&inspect_body);
        assert!(!inspect_text.contains(secret_value));
        let inspect_json: serde_json::Value = serde_json::from_slice(&inspect_body).unwrap();
        assert_eq!(
            inspect_json["config"]["fields"]["runtime.logLevel"]["value"],
            "info"
        );
        assert_eq!(
            inspect_json["config"]["secretReferences"]["repo.token"]["secretKey"],
            "repo.token"
        );

        let delete_response = app
            .oneshot(request_with_headers(
                "DELETE",
                &format!("/admin/tenants/{}/secrets/repo.token", tenant.id.as_str()),
                "platformAdmin",
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(delete_response.status(), StatusCode::OK);
        let delete_body = axum::body::to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let delete_json: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
        assert_eq!(delete_json["deleted"], true);
    }

    #[tokio::test]
    async fn tenant_config_validate_rejects_raw_secret_material() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        let response = app
            .oneshot(request_with_headers(
                "POST",
                &format!("/admin/tenants/{}/config/validate", tenant.id.as_str()),
                "platformAdmin",
                Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "fields": {
                            "database.password": {
                                "ownership": "tenant",
                                "value": "plain-text-secret"
                            }
                        },
                        "secretReferences": {}
                    }))
                    .unwrap(),
                ),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["valid"], false);
        assert_eq!(json["error"], "tenant_config_invalid");
    }

    #[tokio::test]
    async fn tenant_admin_secret_mutation_rejects_platform_owned_entries() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        let request = Request::builder()
            .method("PUT")
            .uri("/admin/tenant-config/secrets/platform.token")
            .header("content-type", "application/json")
            .header("x-user-id", "tenant-admin-user")
            .header("x-user-role", "admin")
            .header("x-tenant-id", "default")
            .header("x-target-tenant-id", tenant.id.as_str())
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "ownership": "platform",
                    "secretValue": "secret"
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn default_tenant_secret_ownership_is_tenant() {
        let request: SetTenantSecretRequest = serde_json::from_value(serde_json::json!({
            "secretValue": "value"
        }))
        .unwrap();
        assert_eq!(request.ownership, TenantConfigOwnership::Tenant);
    }

    #[test]
    fn tenant_config_request_deserializes_with_camel_case() {
        let request: UpsertTenantConfigRequest = serde_json::from_value(serde_json::json!({
            "fields": {
                "runtime.logLevel": {
                    "ownership": "tenant",
                    "value": "info"
                }
            },
            "secretReferences": {
                "repo.token": {
                    "logicalName": "repo.token",
                    "ownership": "tenant",
                    "secretName": "aeterna-tenant-11111111-1111-1111-1111-111111111111-secret",
                    "secretKey": "repo.token"
                }
            }
        }))
        .unwrap();

        assert_eq!(request.fields.len(), 1);
        assert_eq!(request.secret_references.len(), 1);
    }

    #[test]
    fn tenant_admin_guard_rejects_platform_owned_fields() {
        let tenant_id =
            mk_core::types::TenantId::new("11111111-1111-1111-1111-111111111111".to_string())
                .unwrap();
        let mut fields = BTreeMap::new();
        fields.insert(
            "platform.control".to_string(),
            TenantConfigField {
                ownership: TenantConfigOwnership::Platform,
                value: serde_json::json!("x"),
            },
        );

        let document = TenantConfigDocument {
            tenant_id,
            fields,
            secret_references: BTreeMap::new(),
        };

        let response = reject_non_tenant_owned_config(&document).unwrap_err();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn map_validation_error_to_unprocessable_entity() {
        let response = map_tenant_config_provider_error(
            "validate",
            TenantConfigProviderError::Validation("broken".to_string()),
        );
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn tenant_admin_cannot_target_different_tenant() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        // A second tenant to try to cross into
        let second_tenant_id = tenant.id.as_str().to_string();

        // TenantAdmin whose own tenant is "default" tries to inspect another tenant's config
        let request = Request::builder()
            .method("GET")
            .uri("/admin/tenant-config")
            .header("x-user-id", "tenant-admin-user")
            .header("x-user-role", "admin")
            .header("x-tenant-id", "default")
            .header("x-target-tenant-id", second_tenant_id.as_str())
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // "default" != second_tenant_id, so FORBIDDEN
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "forbidden");
    }

    #[tokio::test]
    async fn platform_admin_can_target_any_tenant() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        // PlatformAdmin is allowed to target any tenant
        let request = Request::builder()
            .method("GET")
            .uri("/admin/tenant-config")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .header("x-target-tenant-id", tenant.id.as_str())
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // No config stored yet — 200 with null config is acceptable
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tenant_admin_cannot_delete_platform_owned_secret() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        // First: GlobalAdmin plants a platform-owned secret via the global route
        let set_request = Request::builder()
            .method("PUT")
            .uri(&format!(
                "/admin/tenants/{}/secrets/platform.token",
                tenant.id.as_str()
            ))
            .header("content-type", "application/json")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "ownership": "platform",
                    "secretValue": "platform-secret"
                }))
                .unwrap(),
            ))
            .unwrap();
        let set_response = app.clone().oneshot(set_request).await.unwrap();
        assert_eq!(set_response.status(), StatusCode::OK);

        // Now: TenantAdmin tries to delete it via the self-scoped route
        let delete_request = Request::builder()
            .method("DELETE")
            .uri("/admin/tenant-config/secrets/platform.token")
            .header("x-user-id", "tenant-admin-user")
            .header("x-user-role", "admin")
            .header("x-tenant-id", tenant.id.as_str())
            .body(Body::empty())
            .unwrap();

        let delete_response = app.oneshot(delete_request).await.unwrap();
        assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);
        let body = axum::body::to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "forbidden");
    }

    #[tokio::test]
    async fn tenant_admin_validate_rejects_platform_owned_fields() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        let request = Request::builder()
            .method("POST")
            .uri("/admin/tenant-config/validate")
            .header("content-type", "application/json")
            .header("x-user-id", "tenant-admin-user")
            .header("x-user-role", "admin")
            .header("x-tenant-id", tenant.id.as_str())
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fields": {
                        "platform.control": {
                            "ownership": "platform",
                            "value": "blocked"
                        }
                    },
                    "secretReferences": {}
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn validate_my_tenant_config_rejects_unauthenticated() {
        let Some((app, _tenant)) = app_with_tenant().await else {
            eprintln!("Skipping tenant_api test: Docker not available");
            return;
        };

        // No role header → require_tenant_admin_context must return FORBIDDEN
        let request = Request::builder()
            .method("POST")
            .uri("/admin/tenant-config/validate")
            .header("content-type", "application/json")
            .header("x-user-id", "anonymous")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "fields": {},
                    "secretReferences": {}
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    // -----------------------------------------------------------------------
    // Task 4.3: End-to-end coverage — tenant config provisioning, secret
    // administration, deployment materialization, and tenant bootstrap flows.
    // -----------------------------------------------------------------------

    // Helper: create a Git provider connection and return its id.
    async fn create_connection(app: axum::Router, name: &str) -> String {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;
        let req = Request::builder()
            .method("POST")
            .uri("/admin/git-provider-connections")
            .header("content-type", "application/json")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "name": name,
                    "providerKind": "GitHubApp",
                    "appId": 123456u64,
                    "installationId": 9876543u64,
                    "pemSecretRef": "secret/aeterna-github-app-pem/pem-key",
                    "webhookSecretRef": null
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "connection create should return 201"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        json["connection"]["id"]
            .as_str()
            .expect("connection id should be present")
            .to_string()
    }

    #[tokio::test]
    async fn git_provider_connection_create_and_list() {
        let Some((app, _tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };

        // Create two connections
        let id1 = create_connection(app.clone(), "Acme GitHub App").await;
        let id2 = create_connection(app.clone(), "Platform GitHub App").await;
        assert_ne!(id1, id2, "connection ids must be unique");

        // List connections
        let list_req = Request::builder()
            .method("GET")
            .uri("/admin/git-provider-connections")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let list_resp = app.oneshot(list_req).await.unwrap();
        assert_eq!(list_resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let connections = json["connections"].as_array().unwrap();
        assert!(
            connections.len() >= 2,
            "should list at least 2 connections, got {}",
            connections.len()
        );
        // PEM refs must be redacted (REDACTED token) in list output
        for conn in connections {
            assert!(
                conn["pemSecretRef"]
                    .as_str()
                    .unwrap_or("")
                    .contains("REDACTED"),
                "pemSecretRef must be redacted in list response"
            );
        }
    }

    #[tokio::test]
    async fn git_provider_connection_show_redacts_pem() {
        let Some((app, _tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let conn_id = create_connection(app.clone(), "Show Redaction Test").await;

        let show_req = Request::builder()
            .method("GET")
            .uri(&format!("/admin/git-provider-connections/{}", conn_id))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let show_resp = app.oneshot(show_req).await.unwrap();
        assert_eq!(show_resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(show_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // PEM ref must be redacted
        assert!(
            json["connection"]["pemSecretRef"]
                .as_str()
                .unwrap_or("")
                .contains("REDACTED"),
            "show response must redact pemSecretRef"
        );
        // Raw PEM material must never appear
        assert!(
            !String::from_utf8_lossy(&body).contains("pem-key"),
            "raw pem key reference must not appear in show response"
        );
    }

    #[tokio::test]
    async fn git_provider_connection_show_404_for_unknown() {
        let Some((app, _tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let show_req = Request::builder()
            .method("GET")
            .uri("/admin/git-provider-connections/does-not-exist")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(show_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn git_provider_connection_create_denied_for_non_platform_admin() {
        let Some((app, _tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let req = Request::builder()
            .method("POST")
            .uri("/admin/git-provider-connections")
            .header("content-type", "application/json")
            .header("x-user-id", "tenant-admin-user")
            .header("x-user-role", "admin")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "name": "Unauthorized",
                    "providerKind": "GitHubApp",
                    "appId": 1u64,
                    "installationId": 2u64,
                    "pemSecretRef": "secret/foo/pem-key"
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "non-platform-admin must be denied connection creation"
        );
    }

    #[tokio::test]
    async fn git_provider_connection_grant_and_revoke_tenant_visibility() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let conn_id = create_connection(app.clone(), "Grant-Revoke Test").await;
        let tid = tenant.id.as_str().to_string();

        // Initially tenant cannot see connection
        let list_before = Request::builder()
            .method("GET")
            .uri(&format!("/admin/tenants/{}/git-provider-connections", tid))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let resp_before = app.clone().oneshot(list_before).await.unwrap();
        assert_eq!(resp_before.status(), StatusCode::OK);
        let body_before = axum::body::to_bytes(resp_before.into_body(), usize::MAX)
            .await
            .unwrap();
        let json_before: serde_json::Value = serde_json::from_slice(&body_before).unwrap();
        assert_eq!(
            json_before["connections"].as_array().unwrap().len(),
            0,
            "tenant should see 0 connections before grant"
        );

        // Grant visibility
        let grant_req = Request::builder()
            .method("POST")
            .uri(&format!(
                "/admin/git-provider-connections/{}/tenants/{}",
                conn_id, tid
            ))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let grant_resp = app.clone().oneshot(grant_req).await.unwrap();
        assert_eq!(
            grant_resp.status(),
            StatusCode::OK,
            "grant should return 200"
        );

        // Tenant now sees the connection
        let list_after = Request::builder()
            .method("GET")
            .uri(&format!("/admin/tenants/{}/git-provider-connections", tid))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let resp_after = app.clone().oneshot(list_after).await.unwrap();
        assert_eq!(resp_after.status(), StatusCode::OK);
        let body_after = axum::body::to_bytes(resp_after.into_body(), usize::MAX)
            .await
            .unwrap();
        let json_after: serde_json::Value = serde_json::from_slice(&body_after).unwrap();
        let conns = json_after["connections"].as_array().unwrap();
        assert_eq!(conns.len(), 1, "tenant should see 1 connection after grant");
        assert_eq!(
            conns[0]["id"].as_str().unwrap(),
            conn_id,
            "granted connection id must match"
        );

        // Revoke visibility
        let revoke_req = Request::builder()
            .method("DELETE")
            .uri(&format!(
                "/admin/git-provider-connections/{}/tenants/{}",
                conn_id, tid
            ))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let revoke_resp = app.clone().oneshot(revoke_req).await.unwrap();
        assert_eq!(
            revoke_resp.status(),
            StatusCode::OK,
            "revoke should return 200"
        );

        // Tenant no longer sees the connection
        let list_final = Request::builder()
            .method("GET")
            .uri(&format!("/admin/tenants/{}/git-provider-connections", tid))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let resp_final = app.oneshot(list_final).await.unwrap();
        assert_eq!(resp_final.status(), StatusCode::OK);
        let body_final = axum::body::to_bytes(resp_final.into_body(), usize::MAX)
            .await
            .unwrap();
        let json_final: serde_json::Value = serde_json::from_slice(&body_final).unwrap();
        assert_eq!(
            json_final["connections"].as_array().unwrap().len(),
            0,
            "tenant should see 0 connections after revoke"
        );
    }

    #[tokio::test]
    async fn set_repository_binding_rejects_unvisible_connection() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let tid = tenant.id.as_str().to_string();

        // Create a connection but do NOT grant it to the tenant
        let conn_id = create_connection(app.clone(), "Ungranted Connection").await;

        // Tenant binding with ungranted connection id must be rejected
        let bind_req = Request::builder()
            .method("POST")
            .uri(&format!("/admin/tenants/{}/repository-binding", tid))
            .header("content-type", "application/json")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "repoUrl": "https://github.com/acme/repo.git",
                    "credentialRef": null,
                    "gitProviderConnectionId": conn_id
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(bind_req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "binding with ungranted connection must be rejected with 400"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["error"]
                .as_str()
                .unwrap_or("")
                .contains("git_provider_connection"),
            "error should indicate connection visibility problem, got: {}",
            json
        );
    }

    #[tokio::test]
    async fn set_repository_binding_accepts_granted_connection() {
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let tid = tenant.id.as_str().to_string();

        // Create and grant a connection
        let conn_id = create_connection(app.clone(), "Granted Connection").await;
        let grant_req = Request::builder()
            .method("POST")
            .uri(&format!(
                "/admin/git-provider-connections/{}/tenants/{}",
                conn_id, tid
            ))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let grant_resp = app.clone().oneshot(grant_req).await.unwrap();
        assert_eq!(grant_resp.status(), StatusCode::OK);

        // Now the binding should succeed
        let bind_req = Request::builder()
            .method("POST")
            .uri(&format!("/admin/tenants/{}/repository-binding", tid))
            .header("content-type", "application/json")
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "repoUrl": "https://github.com/acme/repo.git",
                    "credentialRef": null,
                    "gitProviderConnectionId": conn_id
                }))
                .unwrap(),
            ))
            .unwrap();
        let resp = app.oneshot(bind_req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "binding with granted connection must succeed"
        );
    }

    #[tokio::test]
    async fn full_tenant_bootstrap_flow() {
        // End-to-end: provision tenant config, set secret, inspect (redacted),
        // create + grant git provider connection, then verify tenant-side list.
        let Some((app, tenant)) = app_with_tenant().await else {
            eprintln!("Skipping: Docker not available");
            return;
        };
        let tid = tenant.id.as_str().to_string();

        // 1. GlobalAdmin upserts tenant config (deployment materialization step)
        let config_body = serde_json::json!({
            "fields": {
                "runtime.logLevel": { "ownership": "tenant", "value": "warn" },
                "deployment.namespace": { "ownership": "platform", "value": "aeterna-prod" }
            },
            "secretReferences": {}
        });
        let upsert = app
            .clone()
            .oneshot(request_with_headers(
                "PUT",
                &format!("/admin/tenants/{}/config", tid),
                "platformAdmin",
                Body::from(serde_json::to_vec(&config_body).unwrap()),
            ))
            .await
            .unwrap();
        assert_eq!(
            upsert.status(),
            StatusCode::OK,
            "config upsert must succeed"
        );

        // 2. GlobalAdmin provisions a secret reference
        let set_secret = app
            .clone()
            .oneshot(request_with_headers(
                "PUT",
                &format!("/admin/tenants/{}/secrets/github.token", tid),
                "platformAdmin",
                Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "ownership": "platform",
                        "secretValue": "ghp_bootstrap_secret_value"
                    }))
                    .unwrap(),
                ),
            ))
            .await
            .unwrap();
        assert_eq!(
            set_secret.status(),
            StatusCode::OK,
            "secret set must succeed"
        );
        let secret_body = axum::body::to_bytes(set_secret.into_body(), usize::MAX)
            .await
            .unwrap();
        // Secret value must never appear in response
        assert!(
            !String::from_utf8_lossy(&secret_body).contains("ghp_bootstrap_secret_value"),
            "secret value must not appear in set-secret response"
        );

        // 3. Inspect tenant config — must be fully redacted
        let inspect = app
            .clone()
            .oneshot(request_with_headers(
                "GET",
                &format!("/admin/tenants/{}/config", tid),
                "platformAdmin",
                Body::empty(),
            ))
            .await
            .unwrap();
        assert_eq!(inspect.status(), StatusCode::OK);
        let inspect_body = axum::body::to_bytes(inspect.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&inspect_body).contains("ghp_bootstrap_secret_value"),
            "secret value must not appear in config inspect response"
        );
        let inspect_json: serde_json::Value = serde_json::from_slice(&inspect_body).unwrap();
        assert_eq!(
            inspect_json["config"]["fields"]["runtime.logLevel"]["value"], "warn",
            "tenant config field must be preserved"
        );
        assert!(
            inspect_json["config"]["secretReferences"]
                .get("github.token")
                .is_some(),
            "secret reference must be present in config inspect"
        );

        // 4. Platform admin creates + grants Git provider connection
        let conn_id = create_connection(app.clone(), "Bootstrap Test Connection").await;
        let grant = Request::builder()
            .method("POST")
            .uri(&format!(
                "/admin/git-provider-connections/{}/tenants/{}",
                conn_id, tid
            ))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let grant_resp = app.clone().oneshot(grant).await.unwrap();
        assert_eq!(grant_resp.status(), StatusCode::OK, "grant must succeed");

        // 5. Verify tenant-side connection list reflects the grant
        let tenant_conns = Request::builder()
            .method("GET")
            .uri(&format!("/admin/tenants/{}/git-provider-connections", tid))
            .header("x-user-id", "platform-admin-user")
            .header("x-user-role", "platformAdmin")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        let tenant_conns_resp = app.oneshot(tenant_conns).await.unwrap();
        assert_eq!(tenant_conns_resp.status(), StatusCode::OK);
        let tc_body = axum::body::to_bytes(tenant_conns_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let tc_json: serde_json::Value = serde_json::from_slice(&tc_body).unwrap();
        let visible = tc_json["connections"].as_array().unwrap();
        assert_eq!(
            visible.len(),
            1,
            "bootstrap flow: tenant should see exactly 1 connection"
        );
        assert_eq!(
            visible[0]["id"].as_str().unwrap(),
            conn_id,
            "visible connection id must match the granted one"
        );
    }
}
