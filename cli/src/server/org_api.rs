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

/// Request body for `POST /api/v1/org`. Despite the legacy route name, this
/// endpoint creates *any* organizational unit type — the request used to be
/// hardcoded to `UnitType::Organization` with a mandatory `companyId`
/// pointing at the parent Company, which created a UI dead-end whenever the
/// target tenant had zero Companies (rc.9 triage item B-create-org). The
/// request shape is now type-agnostic; the handler resolves the effective
/// `unit_type` and the effective parent, then forwards to
/// [`storage::postgres::create_unit_scoped`] which enforces the hierarchy
/// rules (Company has no parent; Organization → Company; Team →
/// Organization; Project → Team).
///
/// Backward compatibility:
///
/// - `unit_type` is optional and defaults to `Organization` so admin-ui
///   builds shipped before v1.5.0 keep working unchanged.
/// - `company_id` is retained as a deprecated alias for `parent_id`. CLI
///   tools (`cli/src/commands/org.rs`) and the tools-crate JSON schemas
///   still send `companyId`. When both are present, `parent_id` wins.
///
/// The deprecated alias should be removed in 1.6.0 once those callers are
/// migrated; until then, the handler normalizes them in
/// [`CreateOrgRequest::resolve_parent_id`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrgRequest {
    name: String,
    description: Option<String>,
    /// Defaults to [`UnitType::Organization`] for back-compat with
    /// pre-v1.5.0 admin-ui clients that never sent this field.
    #[serde(default)]
    unit_type: Option<UnitType>,
    /// Parent unit id. Required for Organization/Team/Project, must be
    /// absent for Company. The handler validates the cheap rules; the
    /// storage layer validates parent existence and tenant ownership.
    #[serde(default)]
    parent_id: Option<String>,
    /// Deprecated alias for `parent_id`. Pre-v1.5.0 callers send only this
    /// field with the Company id. Remove in 1.6.0.
    #[serde(default)]
    company_id: Option<String>,
}

impl CreateOrgRequest {
    /// Returns the effective parent id, preferring the modern `parent_id`
    /// field over the deprecated `company_id` alias when both are set.
    /// Pure — no I/O — so the precedence rule is unit-testable.
    fn resolve_parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref().or(self.company_id.as_deref())
    }

    /// Returns the effective unit type, defaulting to
    /// [`UnitType::Organization`] for back-compat.
    fn resolve_unit_type(&self) -> UnitType {
        self.unit_type.unwrap_or(UnitType::Organization)
    }
}

/// Returns the [`UnitType`] that the parent of a given child type must
/// have, or `None` if the child type is a hierarchy root (only `Company`).
///
/// Mirrors the matrix in [`storage::postgres::create_unit_scoped`]
/// (lines 1095–1108 at the time of writing). Kept pure — no DB calls — so
/// the matrix is unit-testable and so any divergence from the storage
/// layer's truth surfaces at compile time when `UnitType` gains a variant
/// (the `match` is exhaustive).
fn expected_parent_type(child: UnitType) -> Option<UnitType> {
    match child {
        UnitType::Company => None,
        UnitType::Organization => Some(UnitType::Company),
        UnitType::Team => Some(UnitType::Organization),
        UnitType::Project => Some(UnitType::Team),
    }
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
    match super::context::resolve_list_scope(
        &state,
        &headers,
        query.tenant.as_deref(),
        "/org",
        true, // supports ?tenant=<slug>
    )
    .await
    {
        super::context::ListDispatch::TenantScoped => { /* fall through */ }
        super::context::ListDispatch::CrossTenant => {
            return list_orgs_cross_tenant(&state, &query, None).await;
        }
        super::context::ListDispatch::CrossTenantSingle(t) => {
            return list_orgs_cross_tenant(&state, &query, Some(&t)).await;
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
    if !ctx.has_known_role(&Role::PlatformAdmin) {
        units.retain(|unit| unit.tenant_id == ctx.tenant_id);
    }
    if let Some(company_id) = query.company.as_deref() {
        units.retain(|unit| unit.parent_id.as_deref() == Some(company_id));
    }
    units.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Json(units).into_response()
}

/// Cross-tenant organization listing — serves both `?tenant=*`
/// (`single_tenant=None`) and `?tenant=<slug>` (`single_tenant=Some(...)`).
/// Symmetric with `list_projects_cross_tenant`; see that function for the
/// rationale on shape reuse + in-memory narrowing.
async fn list_orgs_cross_tenant(
    state: &AppState,
    query: &OrgListQuery,
    single_tenant: Option<&super::context::ResolvedTenant>,
) -> axum::response::Response {
    let mut units = match state.postgres.list_all_units().await {
        Ok(units) => units,
        Err(err) => {
            return error_response(StatusCode::BAD_REQUEST, "org_list_failed", &err.to_string());
        }
    };
    if let Some(t) = single_tenant {
        units.retain(|unit| unit.tenant_id.as_str() == t.id.as_str());
    }
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
                "unitType":   unit.unit_type,
                "tenantId":   unit.tenant_id,
                "tenantSlug": t.map(|t| t.slug.clone()),
                "tenantName": t.map(|t| t.name.clone()),
            })
        })
        .collect();

    let body = match single_tenant {
        None => json!({
            "success": true,
            "scope":   "all",
            "items":   items,
        }),
        Some(t) => json!({
            "success": true,
            "scope":   "tenant",
            "tenant":  { "id": t.id, "slug": t.slug, "name": t.name },
            "items":   items,
        }),
    };
    (StatusCode::OK, Json(body)).into_response()
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
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    Json(req): Json<CreateOrgRequest>,
) -> impl IntoResponse {
    // #44.d §5.8 — block write-with-?tenant=* (see user_api for rationale).
    if let Some(resp) = super::context::reject_cross_tenant_write(raw_query.as_deref(), "/org") {
        return resp;
    }
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let unit_type = req.resolve_unit_type();
    let parent_id_str = req.resolve_parent_id().map(str::to_owned);
    let expected_parent = expected_parent_type(unit_type);

    // Cheap pre-flight on the parent rules. The storage layer also
    // enforces them, but its error message is generic; the handler can
    // surface a 422 with a more actionable error_code so the admin-ui
    // can render targeted form errors (parent picker vs. type picker).
    match (expected_parent, parent_id_str.as_deref()) {
        // Company — no parent. Anything else is a client-side bug.
        (None, None) => {}
        (None, Some(_)) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "parent_forbidden_for_company",
                "Company units must not have a parent; omit parentId.",
            );
        }
        // Organization / Team / Project — parent required and must be of
        // the matching upstream type.
        (Some(_), None) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "parent_required",
                &format!(
                    "{:?} units require a parent of type {:?}; provide parentId.",
                    unit_type,
                    expected_parent.expect("just matched Some")
                ),
            );
        }
        (Some(expected), Some(pid)) => {
            if get_unit_of_type(&state, &ctx, pid, expected).await.is_err() {
                return error_response(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_parent",
                    &format!(
                        "parentId must reference an existing {:?} unit in the target tenant",
                        expected
                    ),
                );
            }
        }
    }

    let now = chrono::Utc::now().timestamp();
    let mut metadata = HashMap::new();
    if let Some(description) = req.description.clone() {
        metadata.insert("description".to_string(), json!(description));
    }
    let unit = OrganizationalUnit {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        unit_type,
        parent_id: parent_id_str,
        tenant_id: ctx.tenant_id.clone(),
        metadata,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        source_owner: mk_core::types::RecordSource::Admin,
    };

    match state.postgres.create_unit_scoped(&ctx, &unit).await {
        Ok(()) => {
            persist_event(
                &state,
                &ctx,
                &GovernanceEvent::UnitCreated {
                    unit_id: unit.id.clone(),
                    unit_type: unit.unit_type,
                    tenant_id: ctx.tenant_id.clone(),
                    parent_id: unit.parent_id.clone(),
                    timestamp: now,
                },
            )
            .await;
            // Audit action label uses the resolved unit type so platform
            // admins can filter by exactly what was created. The legacy
            // 'org_create' action label is preserved for Organization
            // creates so existing audit dashboards keep working; other
            // types use a kebab-case 'unit_create_<type>' label.
            let action_label = match unit.unit_type {
                UnitType::Organization => "org_create".to_string(),
                other => format!("unit_create_{}", format!("{other:?}").to_lowercase()),
            };
            audit_action(
                &state,
                &ctx,
                &action_label,
                Some(unit.id.as_str()),
                json!({
                    "name": unit.name,
                    "unitType": unit.unit_type,
                    "parentId": unit.parent_id,
                }),
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

    match state.postgres.list_unit_roles_scoped(&ctx, &org_id).await {
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
        .assign_role_scoped(&ctx, &user_id, &org_id, role.clone())
        .await
    {
        Ok(()) => {
            persist_event(
                &state,
                &ctx,
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
    let existing = match state.postgres.list_unit_roles_scoped(&ctx, &org_id).await {
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
            .remove_role_scoped(&ctx, &user_id, &org_id, role.clone())
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
    let existing = match state.postgres.list_unit_roles_scoped(&ctx, &org_id).await {
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
            .remove_role_scoped(&ctx, &user_id, &org_id, current_role)
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
        .assign_role_scoped(&ctx, &user_id, &org_id, role.clone())
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
    match state.postgres.get_unit_scoped(ctx, unit_id).await {
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

async fn persist_event(state: &AppState, ctx: &TenantContext, event: &GovernanceEvent) {
    // Clone so the boxed future owns the event and satisfies the `'static`
    // bound required by `with_tenant_context`'s HRTB. See the contract in
    // `PostgresBackend::log_event`.
    let event_owned = event.clone();
    let _ = state
        .postgres
        .with_tenant_context(ctx, move |tx| {
            Box::pin(async move {
                storage::postgres::PostgresBackend::log_event(tx, &event_owned).await
            })
        })
        .await;
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
    // #44.d §2.5 — `acting_as_tenant_id` = impersonated tenant if set, else
    // the actor's own tenant.
    let acting_as = Uuid::parse_str(
        ctx.target_tenant_id
            .as_ref()
            .map_or(ctx.tenant_id.as_str(), mk_core::TenantId::as_str),
    )
    .ok();
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
            acting_as,
        )
        .await;
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure helpers introduced in the v1.5.0
    //! Create-Unit dialog generalisation. The HTTP-level behaviour of
    //! `create_org` is exercised end-to-end in `cli/tests/server_runtime_test.rs`
    //! (the existing `companyId`-based smoke tests still pass; new
    //! per-unit-type cases are added there).
    use super::*;

    // -------------------------------------------------------------------------
    // expected_parent_type — must mirror the matrix in
    // storage::postgres::create_unit_scoped.
    // -------------------------------------------------------------------------

    #[test]
    fn expected_parent_company_is_a_root() {
        assert_eq!(expected_parent_type(UnitType::Company), None);
    }

    #[test]
    fn expected_parent_organization_is_company() {
        assert_eq!(
            expected_parent_type(UnitType::Organization),
            Some(UnitType::Company)
        );
    }

    #[test]
    fn expected_parent_team_is_organization() {
        assert_eq!(
            expected_parent_type(UnitType::Team),
            Some(UnitType::Organization)
        );
    }

    #[test]
    fn expected_parent_project_is_team() {
        assert_eq!(
            expected_parent_type(UnitType::Project),
            Some(UnitType::Team)
        );
    }

    // -------------------------------------------------------------------------
    // CreateOrgRequest::resolve_parent_id — precedence between the modern
    // parent_id and the deprecated company_id alias.
    // -------------------------------------------------------------------------

    fn req(parent_id: Option<&str>, company_id: Option<&str>) -> CreateOrgRequest {
        CreateOrgRequest {
            name: "x".to_string(),
            description: None,
            unit_type: None,
            parent_id: parent_id.map(str::to_owned),
            company_id: company_id.map(str::to_owned),
        }
    }

    #[test]
    fn resolve_parent_prefers_parent_id_over_company_id() {
        let r = req(Some("p-modern"), Some("p-legacy"));
        assert_eq!(r.resolve_parent_id(), Some("p-modern"));
    }

    #[test]
    fn resolve_parent_falls_back_to_company_id() {
        let r = req(None, Some("p-legacy"));
        assert_eq!(r.resolve_parent_id(), Some("p-legacy"));
    }

    #[test]
    fn resolve_parent_returns_none_when_both_absent() {
        let r = req(None, None);
        assert_eq!(r.resolve_parent_id(), None);
    }

    #[test]
    fn resolve_parent_uses_parent_id_even_when_company_id_absent() {
        let r = req(Some("p-modern"), None);
        assert_eq!(r.resolve_parent_id(), Some("p-modern"));
    }

    // -------------------------------------------------------------------------
    // CreateOrgRequest::resolve_unit_type — back-compat default.
    // -------------------------------------------------------------------------

    #[test]
    fn resolve_unit_type_defaults_to_organization() {
        // This default exists for pre-v1.5.0 admin-ui clients which never
        // sent `unitType`. Removing or changing the default is a wire
        // contract change.
        let r = req(Some("p"), None);
        assert_eq!(r.resolve_unit_type(), UnitType::Organization);
    }

    #[test]
    fn resolve_unit_type_honours_explicit_choice() {
        for ut in [
            UnitType::Company,
            UnitType::Organization,
            UnitType::Team,
            UnitType::Project,
        ] {
            let r = CreateOrgRequest {
                name: "x".to_string(),
                description: None,
                unit_type: Some(ut),
                parent_id: None,
                company_id: None,
            };
            assert_eq!(r.resolve_unit_type(), ut);
        }
    }

    // -------------------------------------------------------------------------
    // Wire-format regression tests — these guard the camelCase contract on
    // the request body. Pre-v1.5.0 admin-ui clients send `companyId`; the
    // CLI tools crate sends both `companyId` and `name`. v1.5.0 clients
    // can additionally send `unitType` and `parentId`. ALL of the
    // following payloads MUST deserialise.
    // -------------------------------------------------------------------------

    #[test]
    fn deserialises_legacy_company_id_only_payload() {
        let json = r#"{"name":"acme-eng","companyId":"comp-1"}"#;
        let r: CreateOrgRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.name, "acme-eng");
        assert_eq!(r.resolve_parent_id(), Some("comp-1"));
        assert_eq!(r.resolve_unit_type(), UnitType::Organization);
    }

    #[test]
    fn deserialises_modern_unit_type_and_parent_id_payload() {
        let json = r#"{"name":"acme","unitType":"Company","parentId":null}"#;
        let r: CreateOrgRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.resolve_unit_type(), UnitType::Company);
        assert_eq!(r.resolve_parent_id(), None);
    }

    #[test]
    fn deserialises_team_payload() {
        let json = r#"{"name":"backend","unitType":"Team","parentId":"org-1"}"#;
        let r: CreateOrgRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.resolve_unit_type(), UnitType::Team);
        assert_eq!(r.resolve_parent_id(), Some("org-1"));
    }

    #[test]
    fn deserialises_payload_with_both_parent_id_and_company_id() {
        // When the admin-ui sends parentId but a stale CLI client also
        // sends companyId, parentId must win — this is the documented
        // precedence rule that keeps the deprecation transition safe.
        let json = r#"{"name":"x","parentId":"new","companyId":"old"}"#;
        let r: CreateOrgRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.resolve_parent_id(), Some("new"));
    }

    #[test]
    fn rejects_unknown_unit_type_strings() {
        // Defence in depth: a typo'd unitType should hard-fail at the
        // deserialise boundary, not silently fall through to the
        // Organization default. The default only applies when the field
        // is *absent*, not when it's present-but-malformed.
        let json = r#"{"name":"x","unitType":"Org","parentId":"p"}"#;
        assert!(serde_json::from_str::<CreateOrgRequest>(json).is_err());
    }
}
