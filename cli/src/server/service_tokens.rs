//! B2 §10.2 / §10.4 / §10.6 — Service token lifecycle (mint / revoke / list).
//!
//! A service token is an `Aeterna::Agent` principal with
//! `agent_type = "service"`, persisted in the `agents` table
//! (migrations 009 + 029 already provide the full schema — no new
//! migration is introduced by this module).
//!
//! Mint is **PlatformAdmin-only**. The raw JWT is returned exactly once
//! at mint time and is not re-derivable. List / revoke are also
//! PlatformAdmin-only for v1; a TenantAdmin self-service surface is a
//! deliberate follow-up.
//!
//! Endpoints:
//!
//! * `POST   /api/v1/auth/tokens`         — mint (§10.2)
//! * `GET    /api/v1/auth/tokens`         — list active tokens (§10.6)
//! * `DELETE /api/v1/auth/tokens/:id`     — revoke (§10.4)
//!
//! JWT shape (piggy-backs on §10.1 claims):
//!
//! * `sub`        = agent UUID (string form)
//! * `tenant_id`  = tenant UUID (string form)
//! * `token_type` = `"service"`   (§10.1 `PluginTokenClaims::TOKEN_TYPE_SERVICE`)
//! * `scopes`     = the granted capability list
//! * `kind`       = `"plugin-access"` (reused so existing validator works)

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use mk_core::types::{Role, RoleIdentifier};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use super::plugin_auth::PluginTokenClaims;
use super::{AppState, authenticated_platform_context};

// ─── Constants ────────────────────────────────────────────────────────────

pub const DEFAULT_EXPIRES_IN_SECONDS: u64 = 4 * 60 * 60;
pub const MIN_EXPIRES_IN_SECONDS: u64 = 60;
pub const MAX_EXPIRES_IN_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_NAME_LEN: usize = 255;

pub const KNOWN_CAPABILITIES: &[&str] = &[
    "memory:read",
    "memory:write",
    "memory:delete",
    "memory:promote",
    "knowledge:read",
    "knowledge:propose",
    "knowledge:edit",
    "policy:read",
    "policy:create",
    "policy:simulate",
    "governance:read",
    "governance:submit",
    "org:read",
    "agent:register",
    "agent:delegate",
];

// ─── Request / response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintServiceTokenRequest {
    pub name: String,
    pub tenant_id: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MintServiceTokenResponse {
    pub token_id: Uuid,
    pub token: String,
    pub name: String,
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub capabilities: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTokenSummary {
    pub token_id: Uuid,
    pub name: String,
    pub tenant_id: Uuid,
    pub capabilities: Vec<String>,
    pub status: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListTokensQuery {
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub include_revoked: Option<bool>,
}

// ─── Router ───────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    // Paths are relative — `build_router` nests the protected router
    // under `/api/v1`, so these become `/api/v1/auth/tokens{,/:id}`.
    Router::new()
        .route("/auth/tokens", post(mint_handler).get(list_handler))
        .route("/auth/tokens/:id", delete(revoke_handler))
        .with_state(state)
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "error": code, "message": message }))).into_response()
}

async fn require_platform_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<mk_core::types::UserId, Response> {
    let (user_id, roles) = authenticated_platform_context(state, headers).await?;
    let pa: RoleIdentifier = Role::PlatformAdmin.into();
    if !roles.iter().any(|r| r == &pa) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "forbidden_scope",
            "PlatformAdmin role required",
        ));
    }
    Ok(user_id)
}

pub fn validate_capabilities(caps: &[String]) -> Result<(), (StatusCode, &'static str, String)> {
    if caps.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "capabilities must be a non-empty array".to_string(),
        ));
    }
    for cap in caps {
        if cap.len() > 128 {
            return Err((
                StatusCode::BAD_REQUEST,
                "invalid_capability",
                format!("capability exceeds 128 chars: {cap}"),
            ));
        }
        if !KNOWN_CAPABILITIES.contains(&cap.as_str())
        {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_capability",
                format!("unknown capability: {cap}"),
            ));
        }
    }
    Ok(())
}

pub fn resolve_expires_in(requested: Option<u64>) -> Result<u64, (StatusCode, String)> {
    let v = requested.unwrap_or(DEFAULT_EXPIRES_IN_SECONDS);
    if !(MIN_EXPIRES_IN_SECONDS..=MAX_EXPIRES_IN_SECONDS).contains(&v) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "expiresInSeconds must be between {MIN_EXPIRES_IN_SECONDS} and {MAX_EXPIRES_IN_SECONDS}"
            ),
        ));
    }
    Ok(v)
}

pub fn validate_name(name: &str) -> Result<&str, (StatusCode, String)> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_NAME_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("name must be 1..={MAX_NAME_LEN} chars"),
        ));
    }
    Ok(trimmed)
}

pub fn mint_service_jwt(
    jwt_secret: &str,
    issuer: &str,
    agent_id: Uuid,
    tenant_id: Uuid,
    capabilities: &[String],
    ttl_seconds: u64,
    jti: Uuid,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp();
    let claims = PluginTokenClaims {
        sub: agent_id.to_string(),
        idp_provider: "aeterna-service".to_string(),
        tenant_id: tenant_id.to_string(),
        iss: issuer.to_string(),
        aud: vec![PluginTokenClaims::AUDIENCE.to_string()],
        iat: now,
        exp: now + ttl_seconds as i64,
        jti: jti.to_string(),
        github_id: 0,
        email: None,
        kind: PluginTokenClaims::KIND.to_string(),
        token_type: PluginTokenClaims::TOKEN_TYPE_SERVICE.to_string(),
        scopes: capabilities.to_vec(),
    };
    let key = EncodingKey::from_secret(jwt_secret.as_bytes());
    encode(&Header::new(jsonwebtoken::Algorithm::HS256), &claims, &key)
}

// ─── Handlers ──────────────────────────────────────────────────────────────

async fn mint_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<MintServiceTokenRequest>,
) -> Response {
    let caller_uid = match require_platform_admin(&state, &headers).await {
        Ok(uid) => uid,
        Err(resp) => return resp,
    };

    let name = match validate_name(&req.name) {
        Ok(n) => n.to_string(),
        Err((status, msg)) => return error_response(status, "invalid_request", &msg),
    };
    if let Err((status, code, msg)) = validate_capabilities(&req.capabilities) {
        return error_response(status, code, &msg);
    }
    let ttl = match resolve_expires_in(req.expires_in_seconds) {
        Ok(v) => v,
        Err((status, msg)) => return error_response(status, "invalid_request", &msg),
    };

    let tenant = match state.tenant_store.get_tenant(&req.tenant_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                &format!("tenant not found: {}", req.tenant_id),
            );
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "tenant_lookup_failed",
                &e.to_string(),
            );
        }
    };

    let tenant_uuid = match Uuid::parse_str(tenant.id.as_str()) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "tenant_uuid_malformed",
                "tenant.id is not a valid UUID",
            );
        }
    };

    let caller_uuid = match Uuid::parse_str(caller_uid.as_str()) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "caller_identity_not_uuid",
                "Service-token mint requires a UUID-addressable caller",
            );
        }
    };

    let agent_id = Uuid::new_v4();
    let jti = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(ttl as i64);
    let caps_json = match serde_json::to_value(&req.capabilities) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "capabilities_serialize_failed",
                &e.to_string(),
            );
        }
    };

    let insert_result = sqlx::query(
        r#"
        INSERT INTO agents (
            id, name, agent_type, tenant_id,
            delegated_by_user_id, delegation_depth, max_delegation_depth,
            capabilities, status, expires_at, created_at, updated_at
        ) VALUES (
            $1, $2, 'service', $3,
            $4, 1, 3,
            $5, 'active', $6, $7, $7
        )
        "#,
    )
    .bind(agent_id)
    .bind(&name)
    .bind(tenant_uuid)
    .bind(caller_uuid)
    .bind(caps_json)
    .bind(expires_at)
    .bind(now)
    .execute(state.postgres.pool())
    .await;

    if let Err(e) = insert_result {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent_insert_failed",
            &e.to_string(),
        );
    }

    let cfg = &state.plugin_auth_state.config;
    let jwt_secret = match cfg.jwt_secret.as_deref() {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "configuration_error",
                "JWT secret is not configured",
            );
        }
    };
    let issuer = cfg
        .token_issuer
        .clone()
        .unwrap_or_else(|| "aeterna".to_string());

    let token = match mint_service_jwt(
        jwt_secret,
        &issuer,
        agent_id,
        tenant_uuid,
        &req.capabilities,
        ttl,
        jti,
    ) {
        Ok(t) => t,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token_mint_failed",
                &e.to_string(),
            );
        }
    };

    // §10.3: warm the revocation cache (Redis/Dragonfly in HA mode,
    // in-memory fallback otherwise) so the first request bearing
    // this token does not pay the Postgres round-trip. Mirrors the
    // exact columns `validate_service_token` would have read on a
    // miss.
    state
        .revocation_cache
        .warm(
            agent_id,
            crate::server::service_token_validator::CachedAgent {
                status: crate::server::service_token_validator::AgentStatus::Active,
                tenant_id: tenant_uuid,
                capabilities: req.capabilities.clone(),
                expires_at,
            },
        )
        .await;

    tracing::info!(
        agent_id = %agent_id,
        tenant_id = %tenant_uuid,
        caller_uid = %caller_uuid,
        ttl_seconds = ttl,
        capabilities = ?req.capabilities,
        "Minted service token",
    );

    (
        StatusCode::CREATED,
        Json(MintServiceTokenResponse {
            token_id: agent_id,
            token,
            name,
            tenant_id: tenant_uuid,
            tenant_slug: tenant.slug.clone(),
            capabilities: req.capabilities,
            expires_at,
            created_at: now,
        }),
    )
        .into_response()
}

async fn revoke_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(token_id): Path<Uuid>,
) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let result = sqlx::query(
        r#"
        UPDATE agents
        SET status = 'revoked',
            updated_at = NOW()
        WHERE id = $1
          AND agent_type = 'service'
          AND status = 'active'
        "#,
    )
    .bind(token_id)
    .execute(state.postgres.pool())
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => error_response(
            StatusCode::NOT_FOUND,
            "token_not_found",
            "service token not found, already revoked, or not a service token",
        ),
        Ok(_) => {
            // §10.3: `DEL revocation:agent:<uuid>` — in HA mode this
            // is observed by every other instance on its next
            // lookup with zero staleness. In single-instance mode
            // this is a local-only eviction with identical
            // semantics.
            state.revocation_cache.invalidate(token_id).await;
            tracing::info!(token_id = %token_id, "Revoked service token");
            (
                StatusCode::OK,
                Json(json!({ "revoked": true, "tokenId": token_id })),
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "revoke_failed",
            &e.to_string(),
        ),
    }
}

async fn list_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ListTokensQuery>,
) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    let include_revoked = q.include_revoked.unwrap_or(false);

    let tenant_filter: Option<Uuid> = match q.tenant_id.as_ref() {
        Some(tid) => match state.tenant_store.get_tenant(tid).await {
            Ok(Some(t)) => match Uuid::parse_str(t.id.as_str()) {
                Ok(u) => Some(u),
                Err(_) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "tenant_uuid_malformed",
                        "tenant.id is not a valid UUID",
                    );
                }
            },
            Ok(None) => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "tenant_not_found",
                    &format!("tenant not found: {tid}"),
                );
            }
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "tenant_lookup_failed",
                    &e.to_string(),
                );
            }
        },
        None => None,
    };

    let rows_result = sqlx::query(
        r#"
        SELECT
            id,
            name,
            tenant_id,
            capabilities,
            status,
            expires_at,
            created_at,
            delegated_by_user_id
        FROM agents
        WHERE agent_type = 'service'
          AND deleted_at IS NULL
          AND ($1 OR status = 'active')
          AND ($2::uuid IS NULL OR tenant_id = $2)
          AND (expires_at IS NULL OR expires_at > NOW())
        ORDER BY created_at DESC
        "#,
    )
    .bind(include_revoked)
    .bind(tenant_filter)
    .fetch_all(state.postgres.pool())
    .await;

    let rows = match rows_result {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "list_failed",
                &e.to_string(),
            );
        }
    };

    let tokens: Vec<ServiceTokenSummary> = rows
        .into_iter()
        .map(|r| {
            let caps: serde_json::Value = r.get("capabilities");
            let capabilities: Vec<String> = serde_json::from_value(caps).unwrap_or_default();
            ServiceTokenSummary {
                token_id: r.get("id"),
                name: r.get("name"),
                tenant_id: r.get("tenant_id"),
                capabilities,
                status: r.get("status"),
                expires_at: r.get("expires_at"),
                created_at: r.get("created_at"),
                created_by: r.get("delegated_by_user_id"),
            }
        })
        .collect();

    Json(json!({ "tokens": tokens })).into_response()
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{DecodingKey, Validation, decode};

    fn caps(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn name_rejects_empty() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn name_rejects_over_limit() {
        let long = "x".repeat(MAX_NAME_LEN + 1);
        assert!(validate_name(&long).is_err());
    }

    #[test]
    fn name_accepts_reasonable() {
        assert_eq!(
            validate_name("ci-deployer-prod").unwrap(),
            "ci-deployer-prod"
        );
        assert_eq!(validate_name("  trimmed  ").unwrap(), "trimmed");
    }

    #[test]
    fn capabilities_rejects_empty() {
        assert!(validate_capabilities(&[]).is_err());
    }

    #[test]
    fn capabilities_rejects_unknown() {
        let err = validate_capabilities(&caps(&["memory:yolo"])).unwrap_err();
        assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.1, "invalid_capability");
    }

    #[test]
    fn capabilities_rejects_oversized() {
        let big = "x".repeat(129);
        let err = validate_capabilities(&[big]).unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn capabilities_unknown_string_rejected() {
        assert!(
            validate_capabilities(&caps(&["memory:read", "knowledge:search"])).is_err(),
            "knowledge:search is not in the allow-list"
        );
        assert!(validate_capabilities(&caps(&["memory:read", "knowledge:read"])).is_ok());
    }

    #[test]
    fn capabilities_full_known_set_is_accepted() {
        let all: Vec<String> = KNOWN_CAPABILITIES.iter().map(|s| s.to_string()).collect();
        assert!(validate_capabilities(&all).is_ok());
    }

    #[test]
    fn expires_in_default_is_four_hours() {
        assert_eq!(
            resolve_expires_in(None).unwrap(),
            DEFAULT_EXPIRES_IN_SECONDS
        );
    }

    #[test]
    fn expires_in_below_min_is_rejected() {
        assert!(resolve_expires_in(Some(MIN_EXPIRES_IN_SECONDS - 1)).is_err());
    }

    #[test]
    fn expires_in_above_max_is_rejected() {
        assert!(resolve_expires_in(Some(MAX_EXPIRES_IN_SECONDS + 1)).is_err());
    }

    #[test]
    fn expires_in_at_bounds_is_accepted() {
        assert_eq!(
            resolve_expires_in(Some(MIN_EXPIRES_IN_SECONDS)).unwrap(),
            MIN_EXPIRES_IN_SECONDS
        );
        assert_eq!(
            resolve_expires_in(Some(MAX_EXPIRES_IN_SECONDS)).unwrap(),
            MAX_EXPIRES_IN_SECONDS
        );
    }

    #[test]
    fn service_jwt_round_trip() {
        let secret = "unit-test-secret";
        let agent_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let jti = Uuid::new_v4();
        let cap_list = caps(&["memory:read", "knowledge:read"]);

        let token = mint_service_jwt(
            secret,
            "aeterna-test",
            agent_id,
            tenant_id,
            &cap_list,
            3600,
            jti,
        )
        .unwrap();

        let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_audience(&[PluginTokenClaims::AUDIENCE]);
        validation.set_issuer(&["aeterna-test"]);

        let decoded = decode::<PluginTokenClaims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .unwrap();

        assert_eq!(decoded.claims.sub, agent_id.to_string());
        assert_eq!(decoded.claims.tenant_id, tenant_id.to_string());
        assert_eq!(
            decoded.claims.token_type,
            PluginTokenClaims::TOKEN_TYPE_SERVICE
        );
        assert_eq!(decoded.claims.kind, PluginTokenClaims::KIND);
        assert_eq!(decoded.claims.scopes, cap_list);
        assert_eq!(decoded.claims.github_id, 0);
        assert_eq!(decoded.claims.email, None);
        assert_eq!(decoded.claims.jti, jti.to_string());
        assert!(decoded.claims.exp > decoded.claims.iat);
        assert_eq!(decoded.claims.exp - decoded.claims.iat, 3600);
    }

    #[test]
    fn service_jwt_wrong_secret_fails_to_decode() {
        let token = mint_service_jwt(
            "right",
            "aeterna-test",
            Uuid::new_v4(),
            Uuid::new_v4(),
            &caps(&["memory:read"]),
            60,
            Uuid::new_v4(),
        )
        .unwrap();

        let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_audience(&[PluginTokenClaims::AUDIENCE]);
        validation.set_issuer(&["aeterna-test"]);

        let decoded =
            decode::<PluginTokenClaims>(&token, &DecodingKey::from_secret(b"wrong"), &validation);
        assert!(decoded.is_err());
    }

    #[test]
    fn known_capabilities_is_non_empty_and_unique() {
        assert!(!KNOWN_CAPABILITIES.is_empty());
        let mut sorted: Vec<&&str> = KNOWN_CAPABILITIES.iter().collect();
        sorted.sort();
        let len_before = sorted.len();
        sorted.dedup();
        assert_eq!(
            len_before,
            sorted.len(),
            "KNOWN_CAPABILITIES must not contain duplicates"
        );
    }
}
