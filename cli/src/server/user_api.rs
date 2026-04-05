use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{Json, Router};
use mk_core::traits::StorageBackend;
use mk_core::types::{Role, RoleIdentifier, TenantContext, UnitType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use super::{AppState, tenant_scoped_context};

#[derive(Debug, Deserialize)]
struct UserListQuery {
    org: Option<String>,
    team: Option<String>,
    role: Option<String>,
    #[allow(dead_code)]
    all: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterUserRequest {
    email: String,
    name: Option<String>,
    org: Option<String>,
    team: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InviteUserRequest {
    email: String,
    org: Option<String>,
    team: Option<String>,
    role: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantRoleRequest {
    role: String,
    scope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserRoleResponse {
    role: RoleIdentifier,
    scope: String,
    unit_id: String,
}

#[derive(Debug, Clone, Copy)]
enum UserTableVariant {
    Main,
    IdpSync,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/user", get(list_users).post(register_user))
        .route("/user/invite", axum::routing::post(invite_user))
        .route("/user/{user_id}", get(show_user))
        .route(
            "/user/{user_id}/roles",
            get(list_user_roles).post(grant_user_role),
        )
        .route("/user/{user_id}/roles/{role}", delete(revoke_user_role))
        .with_state(state)
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<UserListQuery>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    let variant = match detect_user_table_variant(&state).await {
        Ok(variant) => variant,
        Err(response) => return response,
    };

    let rows = match variant {
        UserTableVariant::Main => match sqlx::query(
            r#"
            SELECT
                u.id::text AS user_id,
                u.email,
                COALESCE(u.name, u.email) AS display_name,
                u.status,
                u.avatar_url,
                u.settings,
                u.created_at,
                u.updated_at
            FROM users u
            WHERE u.deleted_at IS NULL
              AND ($1::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND ur.unit_id = $1
                      AND ur.user_id IN (u.id::text, u.email)
              ))
              AND ($2::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND ur.unit_id = $2
                      AND ur.user_id IN (u.id::text, u.email)
              ))
              AND ($3::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND lower(ur.role) = lower($3)
                      AND ur.user_id IN (u.id::text, u.email)
              ))
            ORDER BY u.created_at DESC
            "#,
        )
        .bind(query.org.as_deref())
        .bind(query.team.as_deref())
        .bind(query.role.as_deref())
        .bind(ctx.tenant_id.as_str())
        .fetch_all(state.postgres.pool())
        .await {
            Ok(rows) => rows,
            Err(err) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "user_list_failed",
                    &err.to_string(),
                );
            }
        },
        UserTableVariant::IdpSync => match sqlx::query(
            r#"
            SELECT
                u.id::text AS user_id,
                u.email,
                COALESCE(u.display_name, NULLIF(CONCAT_WS(' ', u.first_name, u.last_name), ''), u.email) AS display_name,
                CASE WHEN u.is_active THEN 'active' ELSE 'inactive' END AS status,
                NULL::text AS avatar_url,
                '{}'::jsonb AS settings,
                u.created_at,
                u.updated_at
            FROM users u
            WHERE TRUE
              AND ($1::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND ur.unit_id = $1
                      AND ur.user_id IN (u.id::text, u.email)
              ))
              AND ($2::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND ur.unit_id = $2
                      AND ur.user_id IN (u.id::text, u.email)
              ))
              AND ($3::text IS NULL OR EXISTS (
                    SELECT 1 FROM user_roles ur
                    WHERE ur.tenant_id = $4
                      AND lower(ur.role) = lower($3)
                      AND ur.user_id IN (u.id::text, u.email)
              ))
            ORDER BY u.created_at DESC
            "#,
        )
        .bind(query.org.as_deref())
        .bind(query.team.as_deref())
        .bind(query.role.as_deref())
        .bind(ctx.tenant_id.as_str())
        .fetch_all(state.postgres.pool())
        .await {
            Ok(rows) => rows,
            Err(err) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "user_list_failed",
                    &err.to_string(),
                );
            }
        },
    };

    let mut users = Vec::new();
    for row in rows {
        let user_id: String = row.get("user_id");
        let email: String = row.get("email");
        let roles = match load_roles_for_user(&state, &ctx, &[user_id.clone(), email.clone()]).await
        {
            Ok(roles) => roles,
            Err(response) => return response,
        };
        users.push(json!({
            "id": user_id,
            "email": email,
            "name": row.get::<String, _>("display_name"),
            "status": row.get::<String, _>("status"),
            "avatarUrl": row.get::<Option<String>, _>("avatar_url"),
            "settings": row.get::<serde_json::Value, _>("settings"),
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            "updatedAt": row.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
            "roles": roles,
        }));
    }

    Json(users).into_response()
}

async fn show_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    let ctx = match tenant_scoped_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if !is_self_or_admin(&ctx, &user_id) {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Admin, PlatformAdmin, or the target user is required",
        );
    }

    let variant = match detect_user_table_variant(&state).await {
        Ok(variant) => variant,
        Err(response) => return response,
    };
    let row = match get_user_row(&state, variant, &user_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "user_not_found",
                "Requested user was not found",
            );
        }
        Err(response) => return response,
    };

    let db_user_id: String = row.get("user_id");
    let email: String = row.get("email");
    let roles = match load_roles_for_user(&state, &ctx, &[db_user_id.clone(), email.clone()]).await
    {
        Ok(roles) => roles,
        Err(response) => return response,
    };

    Json(json!({
        "id": db_user_id,
        "email": email,
        "name": row.get::<String, _>("display_name"),
        "status": row.get::<String, _>("status"),
        "avatarUrl": row.get::<Option<String>, _>("avatar_url"),
        "settings": row.get::<serde_json::Value, _>("settings"),
        "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "updatedAt": row.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
        "roles": roles,
    }))
    .into_response()
}

async fn register_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<RegisterUserRequest>,
) -> impl IntoResponse {
    let ctx = match tenant_scoped_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if !looks_like_email(&req.email) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_email",
            "Email must be a valid email address",
        );
    }

    let variant = match detect_user_table_variant(&state).await {
        Ok(variant) => variant,
        Err(response) => return response,
    };
    let name = req
        .name
        .clone()
        .unwrap_or_else(|| derive_name_from_email(&req.email));
    let user_id = Uuid::new_v4();
    let created =
        match insert_or_update_user(&state, variant, user_id, &req.email, &name, true).await {
            Ok(row) => row,
            Err(response) => return response,
        };

    let membership_user_id = match mk_core::types::UserId::new(req.email.clone()) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Registered email cannot be represented as a user id",
            );
        }
    };
    let mut assigned = Vec::new();
    if let Some(org) = req.org.as_deref() {
        if let Err(response) = ensure_unit_type(&state, &ctx, org, UnitType::Organization).await {
            return response;
        }
        if let Err(err) = state
            .postgres
            .assign_role(
                &membership_user_id,
                &ctx.tenant_id,
                org,
                Role::Developer.into(),
            )
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_register_failed",
                &err.to_string(),
            );
        }
        assigned.push(json!({"scope": format!("org:{org}"), "role": "developer"}));
    }
    if let Some(team) = req.team.as_deref() {
        if let Err(response) = ensure_unit_type(&state, &ctx, team, UnitType::Team).await {
            return response;
        }
        if let Err(err) = state
            .postgres
            .assign_role(
                &membership_user_id,
                &ctx.tenant_id,
                team,
                Role::Developer.into(),
            )
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_register_failed",
                &err.to_string(),
            );
        }
        assigned.push(json!({"scope": format!("team:{team}"), "role": "developer"}));
    }

    Json(json!({
        "id": created.get::<String, _>("user_id"),
        "email": created.get::<String, _>("email"),
        "name": created.get::<String, _>("display_name"),
        "status": created.get::<String, _>("status"),
        "assigned": assigned,
    }))
    .into_response()
}

async fn invite_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<InviteUserRequest>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    if !looks_like_email(&req.email) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_email",
            "Email must be a valid email address",
        );
    }
    let role = match parse_tenant_role(req.role.as_deref().unwrap_or("developer")) {
        Ok(role) => role,
        Err(response) => return response,
    };

    let variant = match detect_user_table_variant(&state).await {
        Ok(variant) => variant,
        Err(response) => return response,
    };
    let name = derive_name_from_email(&req.email);
    let user_id = Uuid::new_v4();
    let created =
        match insert_or_update_user(&state, variant, user_id, &req.email, &name, false).await {
            Ok(row) => row,
            Err(response) => return response,
        };

    let membership_user_id = match mk_core::types::UserId::new(req.email.clone()) {
        Some(id) => id,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_user_id",
                "Invited email cannot be represented as a user id",
            );
        }
    };

    let scope = if let Some(team) = req.team.as_deref() {
        if let Err(response) = ensure_unit_type(&state, &ctx, team, UnitType::Team).await {
            return response;
        }
        if let Err(err) = state
            .postgres
            .assign_role(&membership_user_id, &ctx.tenant_id, team, role.clone())
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_invite_failed",
                &err.to_string(),
            );
        }
        format!("team:{team}")
    } else if let Some(org) = req.org.as_deref() {
        if let Err(response) = ensure_unit_type(&state, &ctx, org, UnitType::Organization).await {
            return response;
        }
        if let Err(err) = state
            .postgres
            .assign_role(&membership_user_id, &ctx.tenant_id, org, role.clone())
            .await
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_invite_failed",
                &err.to_string(),
            );
        }
        format!("org:{org}")
    } else {
        "company".to_string()
    };

    if let Some(storage) = &state.governance_storage {
        let _ = storage
            .log_audit(
                "user_invite",
                None,
                Some("user"),
                Some(&created.get::<String, _>("user_id")),
                storage::governance::PrincipalType::User,
                Some(actor_uuid(ctx.user_id.as_str())),
                None,
                json!({
                    "email": req.email,
                    "scope": scope,
                    "role": role.to_string().to_lowercase(),
                    "message": req.message,
                }),
            )
            .await;
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "invitation": {
                "id": format!("invite-{}", created.get::<String, _>("user_id")),
                "email": req.email,
                "scope": scope,
                "role": role.to_string().to_lowercase(),
                "status": "pending",
                "message": req.message,
            },
            "user": {
                "id": created.get::<String, _>("user_id"),
                "email": created.get::<String, _>("email"),
                "name": created.get::<String, _>("display_name"),
                "status": created.get::<String, _>("status"),
            }
        })),
    )
        .into_response()
}

async fn list_user_roles(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    let Some(user_id) = mk_core::types::UserId::new(user_id) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid user id",
        );
    };
    let roles = match state
        .postgres
        .get_user_roles(&user_id, &ctx.tenant_id)
        .await
    {
        Ok(roles) => roles,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_roles_list_failed",
                &err.to_string(),
            );
        }
    };
    let mut out = Vec::new();
    for (unit_id, role) in roles {
        let scope = match resolve_unit_scope(&state, &ctx, &unit_id).await {
            Ok(scope) => scope,
            Err(response) => return response,
        };
        out.push(UserRoleResponse {
            role,
            scope,
            unit_id,
        });
    }
    Json(out).into_response()
}

async fn grant_user_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(req): Json<GrantRoleRequest>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
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
    let (unit_id, scope) = match resolve_scope_target(&state, &ctx, &req.scope).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    match state
        .postgres
        .assign_role(&user_id, &ctx.tenant_id, &unit_id, role.clone())
        .await
    {
        Ok(()) => Json(
            json!({"userId": user_id.as_str(), "role": role, "scope": scope, "unitId": unit_id}),
        )
        .into_response(),
        Err(err) => error_response(
            StatusCode::BAD_REQUEST,
            "user_role_grant_failed",
            &err.to_string(),
        ),
    }
}

async fn revoke_user_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((user_id, role_name)): Path<(String, String)>,
) -> impl IntoResponse {
    let ctx = match require_admin_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };
    let Some(user_id) = mk_core::types::UserId::new(user_id) else {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_user_id",
            "Invalid user id",
        );
    };
    let role = match parse_tenant_role(&role_name) {
        Ok(role) => role,
        Err(response) => return response,
    };
    let roles = match state
        .postgres
        .get_user_roles(&user_id, &ctx.tenant_id)
        .await
    {
        Ok(roles) => roles,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "user_role_revoke_failed",
                &err.to_string(),
            );
        }
    };
    let matches: Vec<String> = roles
        .into_iter()
        .filter(|(_, candidate_role)| *candidate_role == role)
        .map(|(unit_id, _)| unit_id)
        .collect();
    if matches.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            "role_assignment_not_found",
            "No matching role assignment found for this user",
        );
    }
    if matches.len() > 1 {
        let mut scopes = Vec::new();
        for unit_id in &matches {
            if let Ok(scope) = resolve_unit_scope(&state, &ctx, unit_id).await {
                scopes.push(json!({"unitId": unit_id, "scope": scope}));
            }
        }
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "ambiguous_role_assignment",
                "message": "Role exists at multiple scopes; revoke is ambiguous without a unique scope target",
                "assignments": scopes,
            })),
        )
            .into_response();
    }
    match state.postgres.remove_role(&user_id, &ctx.tenant_id, &matches[0], role.clone()).await {
        Ok(()) => Json(json!({"success": true, "userId": user_id.as_str(), "role": role, "unitId": matches[0]})).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, "user_role_revoke_failed", &err.to_string()),
    }
}

async fn detect_user_table_variant(
    state: &AppState,
) -> Result<UserTableVariant, axum::response::Response> {
    let rows = sqlx::query(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'users'
        "#,
    )
    .fetch_all(state.postgres.pool())
    .await
    .map_err(|err| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "user_schema_lookup_failed",
            &err.to_string(),
        )
    })?;

    let columns: std::collections::HashSet<String> = rows
        .into_iter()
        .map(|row| row.get::<String, _>("column_name"))
        .collect();
    if columns.contains("name") && columns.contains("status") {
        Ok(UserTableVariant::Main)
    } else if columns.contains("display_name") && columns.contains("is_active") {
        Ok(UserTableVariant::IdpSync)
    } else {
        Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "user_schema_unsupported",
            "Unsupported users table schema",
        ))
    }
}

async fn get_user_row(
    state: &AppState,
    variant: UserTableVariant,
    user_id_or_email: &str,
) -> Result<Option<sqlx::postgres::PgRow>, axum::response::Response> {
    let sql = match variant {
        UserTableVariant::Main => {
            r#"
            SELECT
                u.id::text AS user_id,
                u.email,
                COALESCE(u.name, u.email) AS display_name,
                u.status,
                u.avatar_url,
                u.settings,
                u.created_at,
                u.updated_at
            FROM users u
            WHERE u.deleted_at IS NULL AND (u.id::text = $1 OR u.email = $1)
            LIMIT 1
            "#
        }
        UserTableVariant::IdpSync => {
            r#"
            SELECT
                u.id::text AS user_id,
                u.email,
                COALESCE(u.display_name, NULLIF(CONCAT_WS(' ', u.first_name, u.last_name), ''), u.email) AS display_name,
                CASE WHEN u.is_active THEN 'active' ELSE 'inactive' END AS status,
                NULL::text AS avatar_url,
                '{}'::jsonb AS settings,
                u.created_at,
                u.updated_at
            FROM users u
            WHERE u.id::text = $1 OR u.email = $1
            LIMIT 1
            "#
        }
    };

    sqlx::query(sql)
        .bind(user_id_or_email)
        .fetch_optional(state.postgres.pool())
        .await
        .map_err(|err| {
            error_response(
                StatusCode::BAD_REQUEST,
                "user_lookup_failed",
                &err.to_string(),
            )
        })
}

async fn insert_or_update_user(
    state: &AppState,
    variant: UserTableVariant,
    user_id: Uuid,
    email: &str,
    name: &str,
    active: bool,
) -> Result<sqlx::postgres::PgRow, axum::response::Response> {
    match variant {
        UserTableVariant::Main => sqlx::query(
            r#"
            INSERT INTO users (id, email, name, status, settings, created_at, updated_at)
            VALUES ($1, $2, $3, $4, '{}'::jsonb, NOW(), NOW())
            ON CONFLICT (email) DO UPDATE SET
                name = COALESCE(EXCLUDED.name, users.name),
                status = EXCLUDED.status,
                updated_at = NOW()
            RETURNING
                id::text AS user_id,
                email,
                COALESCE(name, email) AS display_name,
                status,
                avatar_url,
                settings,
                created_at,
                updated_at
            "#,
        )
        .bind(user_id)
        .bind(email)
        .bind(name)
        .bind(if active { "active" } else { "inactive" })
        .fetch_one(state.postgres.pool())
        .await
        .map_err(|err| {
            error_response(
                StatusCode::BAD_REQUEST,
                "user_upsert_failed",
                &err.to_string(),
            )
        }),
        UserTableVariant::IdpSync => sqlx::query(
            r#"
            INSERT INTO users (
                id, email, first_name, last_name, display_name,
                idp_provider, idp_subject, is_active, created_at, updated_at
            )
            VALUES ($1, $2, NULL, NULL, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (email) DO UPDATE SET
                display_name = COALESCE(EXCLUDED.display_name, users.display_name),
                is_active = EXCLUDED.is_active,
                updated_at = NOW()
            RETURNING
                id::text AS user_id,
                email,
                COALESCE(display_name, email) AS display_name,
                CASE WHEN is_active THEN 'active' ELSE 'inactive' END AS status,
                NULL::text AS avatar_url,
                '{}'::jsonb AS settings,
                created_at,
                updated_at
            "#,
        )
        .bind(user_id)
        .bind(email)
        .bind(name)
        .bind(if active { "local" } else { "invite" })
        .bind(format!("local:{user_id}"))
        .bind(active)
        .fetch_one(state.postgres.pool())
        .await
        .map_err(|err| {
            error_response(
                StatusCode::BAD_REQUEST,
                "user_upsert_failed",
                &err.to_string(),
            )
        }),
    }
}

async fn load_roles_for_user(
    state: &AppState,
    ctx: &TenantContext,
    keys: &[String],
) -> Result<Vec<serde_json::Value>, axum::response::Response> {
    let rows = sqlx::query(
        r#"
        SELECT user_id, unit_id, role
        FROM user_roles
        WHERE tenant_id = $1 AND user_id = ANY($2)
        ORDER BY unit_id, role
        "#,
    )
    .bind(ctx.tenant_id.as_str())
    .bind(keys)
    .fetch_all(state.postgres.pool())
    .await
    .map_err(|err| {
        error_response(
            StatusCode::BAD_REQUEST,
            "user_roles_lookup_failed",
            &err.to_string(),
        )
    })?;

    let mut out = Vec::new();
    for row in rows {
        let unit_id: String = row.get("unit_id");
        let role_str: String = row.get("role");
        let role = RoleIdentifier::from_str_flexible(&role_str);
        let scope = resolve_unit_scope(state, ctx, &unit_id).await?;
        out.push(json!({
            "userId": row.get::<String, _>("user_id"),
            "unitId": unit_id,
            "scope": scope,
            "role": role,
        }));
    }
    Ok(out)
}

fn looks_like_email(value: &str) -> bool {
    value.contains('@') && value.contains('.')
}

fn derive_name_from_email(email: &str) -> String {
    email
        .split('@')
        .next()
        .unwrap_or("User")
        .replace('.', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_self_or_admin(ctx: &TenantContext, target: &str) -> bool {
    ctx.user_id.as_str() == target
        || ctx.has_known_role(&Role::PlatformAdmin)
        || ctx.has_known_role(&Role::Admin)
}

async fn ensure_unit_type(
    state: &AppState,
    ctx: &TenantContext,
    unit_id: &str,
    expected: UnitType,
) -> Result<(), axum::response::Response> {
    match state.postgres.get_unit(ctx, unit_id).await {
        Ok(Some(unit)) if unit.unit_type == expected => Ok(()),
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

async fn resolve_unit_scope(
    state: &AppState,
    ctx: &TenantContext,
    unit_id: &str,
) -> Result<String, axum::response::Response> {
    match state.postgres.get_unit(ctx, unit_id).await {
        Ok(Some(unit)) => Ok(match unit.unit_type {
            UnitType::Company => "company".to_string(),
            UnitType::Organization => format!("org:{}", unit.id),
            UnitType::Team => format!("team:{}", unit.id),
            UnitType::Project => format!("project:{}", unit.id),
        }),
        Ok(None) => Err(error_response(
            StatusCode::NOT_FOUND,
            "unit_not_found",
            "Referenced unit was not found",
        )),
        Err(err) => Err(error_response(
            StatusCode::BAD_REQUEST,
            "scope_resolution_failed",
            &err.to_string(),
        )),
    }
}

async fn resolve_scope_target(
    state: &AppState,
    ctx: &TenantContext,
    scope: &str,
) -> Result<(String, String), axum::response::Response> {
    let Some((kind, maybe_id)) = scope
        .split_once(':')
        .map(|(a, b)| (a, Some(b)))
        .or(Some((scope, None)))
    else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_scope",
            "Invalid scope",
        ));
    };
    match (kind, maybe_id) {
        ("company", None) => {
            let units = state.postgres.list_all_units().await.map_err(|err| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    "scope_resolution_failed",
                    &err.to_string(),
                )
            })?;
            let companies: Vec<_> = units
                .into_iter()
                .filter(|unit| {
                    unit.tenant_id == ctx.tenant_id && unit.unit_type == UnitType::Company
                })
                .collect();
            match companies.as_slice() {
                [company] => Ok((company.id.clone(), "company".to_string())),
                [] => Err(error_response(
                    StatusCode::NOT_FOUND,
                    "company_not_found",
                    "No company unit exists for the target tenant",
                )),
                _ => Err(error_response(
                    StatusCode::CONFLICT,
                    "ambiguous_company_scope",
                    "Multiple company units exist; use company:<unit-id>",
                )),
            }
        }
        ("company", Some(id)) => Ok((id.to_string(), format!("company:{id}"))),
        ("org", Some(id)) => Ok((id.to_string(), format!("org:{id}"))),
        ("team", Some(id)) => Ok((id.to_string(), format!("team:{id}"))),
        ("project", Some(id)) => Ok((id.to_string(), format!("project:{id}"))),
        ("org", None) | ("team", None) | ("project", None) => Err(error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "explicit_scope_target_required",
            "Use an explicit scope target such as org:<unit-id>, team:<unit-id>, or project:<unit-id>",
        )),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_scope",
            "Scope must be company, company:<id>, org:<id>, team:<id>, or project:<id>",
        )),
    }
}

fn actor_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap_or_else(|_| Uuid::new_v5(&Uuid::NAMESPACE_URL, value.as_bytes()))
}

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}
