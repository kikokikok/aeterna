use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use mk_core::traits::StorageBackend;
use mk_core::types::{
    GovernanceEvent, OrganizationalUnit, PersistentEvent, Role, RoleIdentifier, TenantContext,
    UnitType,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use storage::PrincipalType;
use uuid::Uuid;

use super::{AppState, tenant_scoped_context};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrgListQuery {
    company: Option<String>,
    /// `?tenant=` — see #44.d RFC. Accepts `*` / `all` / `<slug>`.
    tenant: Option<String>,
    /// Retained for backward compatibility with pre-RFC experimental clients.
    #[allow(dead_code)]
    all: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrgRequest {
    name: String,
    description: Option<String>,
    company_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberBody {
    user_id: String,
    role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleBody {
    role: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberResponse {
    user_id: String,
    role: RoleIdentifier,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/org", get(list_orgs).post(create_org))
        .route("/org/{org_id}", get(show_org))
        .route("/org/{org_id}/members", get(list_members).post(add_member))
        .route("/org/{org_id}/members/{user_id}", delete(remove_member))
        .route("/org/{org_id}/members/{user_id}/role", put(set_member_role))
        .with_state(state)
}

async fn list_orgs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<OrgListQuery>,
) -> impl IntoResponse {
    // #44.d §2.4 — cross-tenant listing dispatch.
    // Historical note: this handler already had half-baked cross-tenant
    // behavior — PlatformAdmins received all-tenant rows in the bare-array
    // body when ?tenant was absent. That behavior is preserved for
    // backward compat; the new `?tenant=*` path adds the RFC envelope.
    match super::context::resolve_list_scope(&state, &headers, query.tenant.as_deref(), "/org")
        .await
    {
        super::context::ListDispatch::TenantScoped => { /* fall through */ }
        super::context::ListDispatch::CrossTenant => {
            return list_orgs_cross_tenant(&state, &query).await;
        }
        super::context::ListDispatch::Response(r) => return r,
    }

    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let mut units = match state.postgres.list_all_units().await {
        Ok(units) => units,
        Err(err) => {
            return error_response(StatusCode::BAD_REQUEST, "org_list_failed", &err.to_string());
        }
    };
    units.retain(|unit| unit.unit_type == UnitType::Organization);
    if !ctx.has_known_role(&Role::PlatformAdmin) {
        units.retain(|unit| unit.tenant_id == ctx.tenant_id);
    }
    if let Some(company_id) = query.company.as_deref() {
        units.retain(|unit| unit.parent_id.as_deref() == Some(company_id));
    }
    units.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Json(units).into_response()
}

/// Cross-tenant organization listing (`?tenant=*`, PlatformAdmin only).
///
/// Same shape as `/project` in scope=all mode — one item per org across
/// all tenants, each decorated with `tenantId` + `tenantSlug` + `tenantName`.
async fn list_orgs_cross_tenant(
    state: &AppState,
    query: &OrgListQuery,
) -> axum::response::Response {
    let mut units = match state.postgres.list_all_units().await {
        Ok(units) => units,
        Err(err) => {
            return error_response(StatusCode::BAD_REQUEST, "org_list_failed", &err.to_string());
        }
    };
    units.retain(|unit| unit.unit_type == UnitType::Organization);
    if let Some(company_id) = query.company.as_deref() {
        units.retain(|unit| unit.parent_id.as_deref() == Some(company_id));
    }
    // Stable ordering: tenant first, then org name, then id.
    units.sort_by(|a, b| {
        a.tenant_id
            .cmp(&b.tenant_id)
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });

    let tenants = match state.tenant_store.list_tenants(true).await {
        Ok(ts) => ts,
        Err(err) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "tenant_lookup_failed",
                &err.to_string(),
            );
        }
    };
    let tenant_by_id: std::collections::HashMap<String, &mk_core::types::TenantRecord> = tenants
        .iter()
        .map(|t| (t.id.as_str().to_string(), t))
        .collect();

    let items: Vec<serde_json::Value> = units
        .into_iter()
        .map(|unit| {
            let t = tenant_by_id.get(unit.tenant_id.as_str());
            json!({
                "id":         unit.id,
                "name":       unit.name,
                "parentId":   unit.parent_id,
                "unitType":   "organization",
                "tenantId":   unit.tenant_id,
                "tenantSlug": t.map(|t| t.slug.clone()),
                "tenantName": t.map(|t| t.name.clone()),
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "scope":   "all",
            "items":   items,
        })),
    )
        .into_response()
}

async fn show_org(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    match get_unit_of_type(&state, &ctx, &org_id, UnitType::Organization).await {
        Ok(unit) => Json(unit).into_response(),
        Err(response) => response,
    }
}

async fn create_org(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateOrgRequest>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    if get_unit_of_type(&state, &ctx, &req.company_id, UnitType::Company)
        .await
        .is_err()
    {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_company",
            "companyId must reference an existing company unit in the target tenant",
        );
    }

    let now = chrono::Utc::now().timestamp();
    let mut metadata = HashMap::new();
    if let Some(description) = req.description.clone() {
        metadata.insert("description".to_string(), json!(description));
    }
    let unit = OrganizationalUnit {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        unit_type: UnitType::Organization,
        parent_id: Some(req.company_id),
        tenant_id: ctx.tenant_id.clone(),
        metadata,
        created_at: now,
        updated_at: now,
        source_owner: mk_core::types::RecordSource::Admin,
    };

    match state.postgres.create_unit(&unit).await {
        Ok(()) => {
            persist_event(
                &state,
                &GovernanceEvent::UnitCreated {
                    unit_id: unit.id.clone(),
                    unit_type: unit.unit_type,
                    tenant_id: ctx.tenant_id.clone(),
                    parent_id: unit.parent_id.clone(),
                    timestamp: now,
                },
            )
            .await;
            audit_action(
                &state,
                &ctx,
                "org_create",
                Some(unit.id.as_str()),
                json!({"name": unit.name, "parentId": unit.parent_id}),
            )
            .await;
            (StatusCode::CREATED, Json(unit)).into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "org_create_failed",
            &err.to_string(),
        ),
    }
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    if let Err(response) = get_unit_of_type(&state, &ctx, &org_id, UnitType::Organization).await {
        return response;
    }

    match state
        .postgres
        .list_unit_roles(&ctx.tenant_id, &org_id)
        .await
    {
        Ok(entries) => Json(
            entries
                .into_iter()
                .map(|(user_id, role)| MemberResponse {
                    user_id: user_id.into_inner(),
                    role,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "org_members_list_failed",
            &err.to_string(),
        ),
    }
}

async fn add_member(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(req): Json<MemberBody>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if let Err(response) = require_assign_roles_permission(&state, &ctx).await {
        return response;
    }
    if let Err(response) = get_unit_of_type(&state, &ctx, &org_id, UnitType::Organization).await {
        return response;
    }

    let Some(user_id) = mk_core::types::UserId::new(req.user_id.clone()) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid user id",
        );
    };
    let role = match parse_tenant_role(&req.role) {
        Ok(role) => role,
        Err(response) => return response,
    };

    match state
        .postgres
        .assign_role(&user_id, &ctx.tenant_id, &org_id, role.clone())
        .await
    {
        Ok(()) => {
            persist_event(
                &state,
                &GovernanceEvent::RoleAssigned {
                    user_id: user_id.clone(),
                    unit_id: org_id.clone(),
                    role: role.clone(),
                    tenant_id: ctx.tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
            )
            .await;
            audit_action(
                &state,
                &ctx,
                "org_member_add",
                Some(org_id.as_str()),
                json!({"userId": user_id.as_str(), "role": role}),
            )
            .await;
            (
                StatusCode::CREATED,
                Json(json!({"userId": user_id.as_str(), "role": role})),
            )
                .into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "org_member_add_failed",
            &err.to_string(),
        ),
    }
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((org_id, user_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if let Err(response) = require_assign_roles_permission(&state, &ctx).await {
        return response;
    }
    if let Err(response) = get_unit_of_type(&state, &ctx, &org_id, UnitType::Organization).await {
        return response;
    }

    let Some(user_id) = mk_core::types::UserId::new(user_id) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid user id",
        );
    };
    let existing = match state
        .postgres
        .list_unit_roles(&ctx.tenant_id, &org_id)
        .await
    {
        Ok(entries) => entries,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "org_member_remove_failed",
                &err.to_string(),
            );
        }
    };

    let current_roles: Vec<RoleIdentifier> = existing
        .into_iter()
        .filter(|(candidate, _)| candidate.as_str() == user_id.as_str())
        .map(|(_, role)| role)
        .collect();
    if current_roles.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            "org_member_not_found",
            "Member has no roles in this organization",
        );
    }

    for role in &current_roles {
        if let Err(err) = state
            .postgres
            .remove_role(&user_id, &ctx.tenant_id, &org_id, role.clone())
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "org_member_remove_failed",
                &err.to_string(),
            );
        }
    }

    audit_action(
        &state,
        &ctx,
        "org_member_remove",
        Some(org_id.as_str()),
        json!({"userId": user_id.as_str(), "removedRoles": current_roles}),
    )
    .await;
    Json(json!({"success": true})).into_response()
}

async fn set_member_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((org_id, user_id)): Path<(String, String)>,
    Json(req): Json<RoleBody>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if let Err(response) = require_assign_roles_permission(&state, &ctx).await {
        return response;
    }
    if let Err(response) = get_unit_of_type(&state, &ctx, &org_id, UnitType::Organization).await {
        return response;
    }

    let Some(user_id) = mk_core::types::UserId::new(user_id) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid user id",
        );
    };
    let role = match parse_tenant_role(&req.role) {
        Ok(role) => role,
        Err(response) => return response,
    };
    let existing = match state
        .postgres
        .list_unit_roles(&ctx.tenant_id, &org_id)
        .await
    {
        Ok(entries) => entries,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "org_member_set_role_failed",
                &err.to_string(),
            );
        }
    };
    for current_role in existing
        .into_iter()
        .filter(|(candidate, _)| candidate.as_str() == user_id.as_str())
        .map(|(_, role)| role)
    {
        if let Err(err) = state
            .postgres
            .remove_role(&user_id, &ctx.tenant_id, &org_id, current_role)
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "org_member_set_role_failed",
                &err.to_string(),
            );
        }
    }
    match state
        .postgres
        .assign_role(&user_id, &ctx.tenant_id, &org_id, role.clone())
        .await
    {
        Ok(()) => {
            audit_action(
                &state,
                &ctx,
                "org_member_set_role",
                Some(org_id.as_str()),
                json!({"userId": user_id.as_str(), "role": role}),
            )
            .await;
            Json(json!({"userId": user_id.as_str(), "role": role})).into_response()
        }
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "org_member_set_role_failed",
            &err.to_string(),
        ),
    }
}

async fn require_admin_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, axum::response::Response> {
    let ctx = tenant_scoped_context(state, headers).await?;
    if ctx.has_known_role(&Role::PlatformAdmin) || ctx.has_known_role(&Role::Admin) {
        Ok(ctx)
    } else {
        Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin or PlatformAdmin role required",
        ))
    }
}

async fn require_assign_roles_permission(
    state: &AppState,
    ctx: &TenantContext,
) -> Result<(), axum::response::Response> {
    let resource = format!("Aeterna::Company::\"{}\"", ctx.tenant_id.as_str());
    match state
        .auth_service
        .check_permission(ctx, "AssignRoles", &resource)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "AssignRoles permission required for this tenant",
        )),
        Err(err) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authz_check_failed",
            &err.to_string(),
        )),
    }
}

async fn get_unit_of_type(
    state: &AppState,
    ctx: &TenantContext,
    unit_id: &str,
    expected: UnitType,
) -> Result<OrganizationalUnit, axum::response::Response> {
    match state.postgres.get_unit(ctx, unit_id).await {
        Ok(Some(unit)) if unit.unit_type == expected => Ok(unit),
        Ok(Some(_)) => Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_unit_type",
            "Unit exists but has the wrong type for this operation",
        )),
        Ok(None) => Err(error_response(
            StatusCode::NOT_FOUND,
            "unit_not_found",
            "Requested unit was not found",
        )),
        Err(err) => Err(error_response(
            StatusCode::BAD_REQUEST,
            "unit_lookup_failed",
            &err.to_string(),
        )),
    }
}

fn parse_tenant_role(value: &str) -> Result<RoleIdentifier, axum::response::Response> {
    let role = RoleIdentifier::from_str_flexible(value);
    if matches!(role, RoleIdentifier::Known(Role::PlatformAdmin)) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_role",
            "PlatformAdmin cannot be assigned as a tenant-scoped role",
        ));
    }
    Ok(role)
}

async fn persist_event(state: &AppState, event: &GovernanceEvent) {
    let _ = state.postgres.log_event(event).await;
    let _ = state
        .postgres
        .persist_event(PersistentEvent::new(event.clone()))
        .await;
}

async fn audit_action(
    state: &AppState,
    ctx: &TenantContext,
    action: &str,
    target_id: Option<&str>,
    details: serde_json::Value,
) {
    let Some(storage) = &state.governance_storage else {
        return;
    };
    let actor_id = Uuid::parse_str(ctx.user_id.as_str()).ok();
    let _ = storage
        .log_audit(
            action,
            None,
            Some("organization"),
            target_id,
            PrincipalType::User,
            actor_id,
            None,
            json!({"actorTenantId": ctx.tenant_id.as_str(), "details": details}),
        )
        .await;
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}
