//! Plugin authentication endpoints for the OpenCode plugin flow.
//!
//! Provides:
//! - `POST /api/v1/auth/plugin/bootstrap` — Exchange a GitHub OAuth access token
//!   for Aeterna-issued plugin session credentials (access + refresh tokens).
//! - `POST /api/v1/auth/plugin/refresh` — Refresh an expired access token.
//! - `POST /api/v1/auth/plugin/logout` — Revoke a refresh token.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use mk_core::types::{
    DEFAULT_TENANT_SLUG, PROVIDER_GITHUB, RoleIdentifier, SYSTEM_USER_ID, TenantContext, TenantId,
    UserId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::AppState;

const DEFAULT_REFRESH_TOKEN_TTL_SECS: u64 = 30 * 24 * 3600;
const DEFAULT_GITHUB_API_BASE: &str = "https://api.github.com";

// ---------------------------------------------------------------------------
// Refresh token store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RefreshEntry {
    tenant_id: String,
    github_login: String,
    github_id: u64,
    email: Option<String>,
    expires_at: i64,
}

/// In-process refresh-token store (single-use, rotation on refresh).
///
/// For HA deployments swap this for a Redis- or Postgres-backed store.
#[derive(Debug, Default)]
pub struct RefreshTokenStore {
    tokens: RwLock<HashMap<String, RefreshEntry>>,
}

impl RefreshTokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(
        &self,
        token: String,
        tenant_id: String,
        github_login: String,
        github_id: u64,
        email: Option<String>,
        ttl_seconds: u64,
    ) {
        let expires_at = Utc::now().timestamp() + ttl_seconds as i64;
        self.tokens.write().await.insert(
            token,
            RefreshEntry {
                tenant_id,
                github_login,
                github_id,
                email,
                expires_at,
            },
        );
    }

    /// Consume a refresh token (single-use). Returns `None` if missing or
    /// expired.
    pub(super) async fn take(&self, token: &str) -> Option<RefreshEntry> {
        let mut guard = self.tokens.write().await;
        let entry = guard.remove(token)?;
        if entry.expires_at <= Utc::now().timestamp() {
            return None;
        }
        Some(entry)
    }

    pub async fn revoke(&self, token: &str) {
        self.tokens.write().await.remove(token);
    }
}

/// Redis-backed refresh-token store for HA / multi-instance deployments.
///
/// Uses [`storage::RedisStore`] with prefix `aeterna:refresh_tokens`.
/// Tokens are stored with a TTL and consumed atomically via `GETDEL`.
pub struct RedisRefreshTokenStore {
    store: storage::RedisStore,
}

impl RedisRefreshTokenStore {
    /// Create a new Redis-backed refresh token store.
    pub fn new(store: storage::RedisStore) -> Self {
        Self { store }
    }

    /// Store a refresh token with the given TTL.
    pub async fn insert(
        &self,
        token: String,
        tenant_id: String,
        github_login: String,
        github_id: u64,
        email: Option<String>,
        ttl_seconds: u64,
    ) {
        let expires_at = Utc::now().timestamp() + ttl_seconds as i64;
        let entry = RefreshEntry {
            tenant_id,
            github_login,
            github_id,
            email,
            expires_at,
        };
        if let Err(e) = self.store.set(&token, &entry, Some(ttl_seconds)).await {
            tracing::error!("Failed to store refresh token in Redis: {e}");
        }
    }

    /// Consume a refresh token (single-use). Returns `None` if missing or
    /// expired.
    pub(super) async fn take(&self, token: &str) -> Option<RefreshEntry> {
        match self.store.take::<RefreshEntry>(token).await {
            Ok(Some(entry)) => {
                if entry.expires_at <= Utc::now().timestamp() {
                    return None;
                }
                Some(entry)
            }
            Ok(None) => None,
            Err(e) => {
                tracing::error!("Failed to take refresh token from Redis: {e}");
                None
            }
        }
    }

    /// Revoke (delete) a refresh token.
    pub async fn revoke(&self, token: &str) {
        if let Err(e) = self.store.delete(token).await {
            tracing::error!("Failed to revoke refresh token in Redis: {e}");
        }
    }
}

impl std::fmt::Debug for RedisRefreshTokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisRefreshTokenStore").finish()
    }
}

/// Refresh token store that can be either in-memory or Redis-backed.
///
/// In unit tests and single-instance deployments the in-memory variant is used.
/// For HA deployments (Kubernetes ReplicaSet), the Redis variant ensures all
/// replicas share the same token state.
#[derive(Debug)]
pub enum RefreshTokenStoreBackend {
    /// In-memory store (single-instance only).
    InMemory(RefreshTokenStore),
    /// Redis-backed store (multi-instance safe).
    Redis(RedisRefreshTokenStore),
}

impl RefreshTokenStoreBackend {
    /// Store a refresh token.
    pub async fn insert(
        &self,
        token: String,
        tenant_id: String,
        github_login: String,
        github_id: u64,
        email: Option<String>,
        ttl_seconds: u64,
    ) {
        match self {
            Self::InMemory(s) => {
                s.insert(
                    token,
                    tenant_id,
                    github_login,
                    github_id,
                    email,
                    ttl_seconds,
                )
                .await;
            }
            Self::Redis(s) => {
                s.insert(
                    token,
                    tenant_id,
                    github_login,
                    github_id,
                    email,
                    ttl_seconds,
                )
                .await;
            }
        }
    }

    /// Consume a refresh token (single-use).
    pub(super) async fn take(&self, token: &str) -> Option<RefreshEntry> {
        match self {
            Self::InMemory(s) => s.take(token).await,
            Self::Redis(s) => s.take(token).await,
        }
    }

    /// Revoke a refresh token.
    pub async fn revoke(&self, token: &str) {
        match self {
            Self::InMemory(s) => s.revoke(token).await,
            Self::Redis(s) => s.revoke(token).await,
        }
    }
}

impl Default for RefreshTokenStoreBackend {
    fn default() -> Self {
        Self::InMemory(RefreshTokenStore::new())
    }
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PluginAuthBootstrapRequest {
    pub provider: String,
    pub github_access_token: String,
}

#[derive(Debug, Deserialize)]
pub struct PluginAuthRefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct PluginAuthLogoutRequest {
    pub refresh_token: Option<String>,
}

// ---------------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginTokenClaims {
    pub sub: String,
    pub idp_provider: String,
    pub tenant_id: String,
    pub iss: String,
    pub aud: Vec<String>,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub github_id: u64,
    pub email: Option<String>,
    pub kind: String,
}

impl PluginTokenClaims {
    pub const AUDIENCE: &'static str = "aeterna-plugin";
    pub const KIND: &'static str = "plugin-access";
}

// ---------------------------------------------------------------------------
// Validated identity
// ---------------------------------------------------------------------------

/// Validated identity extracted from a plugin bearer token.
#[derive(Debug, Clone)]
pub struct PluginIdentity {
    pub idp_provider: String,
    pub tenant_id: String,
    pub github_login: String,
    pub github_id: u64,
    pub email: Option<String>,
    pub jti: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/plugin/bootstrap", post(bootstrap_handler))
        .route("/auth/plugin/refresh", post(refresh_handler))
        .route("/auth/plugin/logout", post(logout_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[tracing::instrument(skip_all, fields(provider = %req.provider))]
async fn bootstrap_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PluginAuthBootstrapRequest>,
) -> impl IntoResponse {
    let cfg = &state.plugin_auth_state.config;

    if !cfg.enabled {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "plugin_auth_disabled",
                "message": "Plugin authentication is not enabled on this server"
            })),
        );
    }

    if !cfg.allowed_providers.contains(&req.provider) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_provider",
                "message": "Provider not supported"
            })),
        );
    }

    let jwt_secret = match &cfg.jwt_secret {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "configuration_error",
                    "message": "JWT secret is not configured"
                })),
            );
        }
    };

    let github_user = match fetch_github_user(&req.github_access_token, cfg).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("GitHub user fetch failed: {e}");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "github_user_fetch_failed",
                    "message": format!("Failed to fetch GitHub user: {e}")
                })),
            );
        }
    };

    let access_ttl = cfg.access_token_ttl_seconds.unwrap_or(3600);
    let refresh_ttl = cfg
        .refresh_token_ttl_seconds
        .unwrap_or(DEFAULT_REFRESH_TOKEN_TTL_SECS);
    let issuer = cfg
        .token_issuer
        .clone()
        .unwrap_or_else(|| "aeterna".to_string());
    let tenant_id = match resolve_tenant_for_github_user(&github_user.login, cfg) {
        Some(t) => t,
        None => {
            tracing::error!(login = %github_user.login, "No tenant configured for plugin auth bootstrap");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "tenant_not_configured",
                    "message": "No tenant configured for plugin authentication. Set AETERNA_DEFAULT_TENANT_ID or configure default_tenant_id."
                })),
            );
        }
    };

    let access_token =
        match mint_access_token(&jwt_secret, &issuer, &tenant_id, &github_user, access_ttl) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to mint access token: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "token_mint_failed",
                        "message": "Failed to issue access token"
                    })),
                );
            }
        };

    let refresh_token = Uuid::new_v4().to_string();
    state
        .plugin_auth_state
        .refresh_store
        .insert(
            refresh_token.clone(),
            tenant_id,
            github_user.login.clone(),
            github_user.id,
            github_user.email.clone(),
            refresh_ttl,
        )
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": access_token,
            "refresh_token": refresh_token,
            "expires_in": access_ttl,
            "token_type": "Bearer",
            "github_login": github_user.login,
            "github_email": github_user.email,
        })),
    )
}

#[tracing::instrument(skip_all)]
async fn refresh_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PluginAuthRefreshRequest>,
) -> impl IntoResponse {
    let cfg = &state.plugin_auth_state.config;

    if !cfg.enabled {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "plugin_auth_disabled",
                "message": "Plugin authentication is not enabled on this server"
            })),
        );
    }

    let jwt_secret = match &cfg.jwt_secret {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "configuration_error",
                    "message": "JWT secret is not configured"
                })),
            );
        }
    };

    let entry = match state
        .plugin_auth_state
        .refresh_store
        .take(&req.refresh_token)
        .await
    {
        Some(e) => e,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "invalid_refresh_token",
                    "message": "Refresh token is invalid, expired, or already consumed"
                })),
            );
        }
    };

    let access_ttl = cfg.access_token_ttl_seconds.unwrap_or(3600);
    let refresh_ttl = cfg
        .refresh_token_ttl_seconds
        .unwrap_or(DEFAULT_REFRESH_TOKEN_TTL_SECS);
    let issuer = cfg
        .token_issuer
        .clone()
        .unwrap_or_else(|| "aeterna".to_string());

    let user_info = GitHubUser {
        id: entry.github_id,
        login: entry.github_login,
        email: entry.email,
    };

    let access_token = match mint_access_token(
        &jwt_secret,
        &issuer,
        &entry.tenant_id,
        &user_info,
        access_ttl,
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to mint access token on refresh: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "token_mint_failed",
                    "message": "Failed to issue access token"
                })),
            );
        }
    };

    // Rotate refresh token
    let new_refresh = Uuid::new_v4().to_string();
    state
        .plugin_auth_state
        .refresh_store
        .insert(
            new_refresh.clone(),
            entry.tenant_id,
            user_info.login.clone(),
            user_info.id,
            user_info.email.clone(),
            refresh_ttl,
        )
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": access_token,
            "refresh_token": new_refresh,
            "expires_in": access_ttl,
            "token_type": "Bearer",
            "github_login": user_info.login,
            "github_email": user_info.email,
        })),
    )
}

#[tracing::instrument(skip_all)]
async fn logout_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PluginAuthLogoutRequest>,
) -> impl IntoResponse {
    if let Some(token) = &req.refresh_token {
        state.plugin_auth_state.refresh_store.revoke(token).await;
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "message": "Logged out successfully" })),
    )
}

// ---------------------------------------------------------------------------
// Admin session endpoint (returns user profile + roles + tenant memberships)
// ---------------------------------------------------------------------------

/// Router for the admin session convenience endpoint.
///
/// This is registered inside the protected API route group (requires bearer
/// token). It returns the authenticated user's profile, roles across all
/// tenants (including `__root__` PlatformAdmin grants), and tenant memberships
/// in a single response for efficient admin UI bootstrap.
pub fn admin_session_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/admin/session", post(admin_session_handler))
        .with_state(state)
}

#[tracing::instrument(skip_all)]
async fn admin_session_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let cfg = &state.plugin_auth_state.config;

    let jwt_secret = match &cfg.jwt_secret {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "configuration_error",
                    "message": "JWT secret is not configured"
                })),
            );
        }
    };

    let identity = match validate_plugin_bearer(&headers, &jwt_secret) {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "unauthorized",
                    "message": "Valid bearer token required"
                })),
            );
        }
    };

    let user_id = match state
        .postgres
        .resolve_user_id_by_idp(&identity.idp_provider, &identity.github_login)
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "identity_not_provisioned",
                    "message": "GitHub identity is not provisioned in this Aeterna instance"
                })),
            );
        }
        Err(e) => {
            tracing::error!("Failed to resolve user identity: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal_error",
                    "message": "Failed to resolve user identity"
                })),
            );
        }
    };

    let roles = match state
        .postgres
        .get_user_roles_for_auth(&user_id, &identity.tenant_id)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to fetch user roles: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal_error",
                    "message": "Failed to fetch user roles"
                })),
            );
        }
    };

    let is_platform_admin = roles
        .iter()
        .any(|r| r == &RoleIdentifier::Known(mk_core::types::Role::PlatformAdmin));
    let is_tenant_admin = roles
        .iter()
        .any(|r| r == &RoleIdentifier::Known(mk_core::types::Role::TenantAdmin));

    let tenants = if is_platform_admin {
        match state.tenant_store.list_tenants(false).await {
            Ok(t) => t
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id.as_str(),
                        "slug": t.slug,
                        "name": t.name,
                        "status": format!("{:?}", t.status),
                    })
                })
                .collect::<Vec<_>>(),
            Err(_) => vec![],
        }
    } else {
        match state.postgres.get_user_tenant_ids(&user_id).await {
            Ok(ids) => ids
                .into_iter()
                .map(|id| {
                    serde_json::json!({
                        "id": id,
                        "slug": id,
                        "name": id,
                        "status": "Active",
                    })
                })
                .collect::<Vec<_>>(),
            Err(_) => vec![],
        }
    };

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "user": {
                "user_id": user_id,
                "github_login": identity.github_login,
                "github_id": identity.github_id,
                "email": identity.email,
            },
            "roles": roles,
            "tenants": tenants,
            "is_platform_admin": is_platform_admin,
            "is_tenant_admin": is_tenant_admin,
            "active_tenant_id": identity.tenant_id,
        })),
    )
}

// ---------------------------------------------------------------------------
// Token validation (used by request extractors elsewhere)
// ---------------------------------------------------------------------------

/// Validate a plugin bearer token from an Authorization header.
pub fn validate_plugin_bearer(headers: &HeaderMap, jwt_secret: &str) -> Option<PluginIdentity> {
    let auth_header = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let token = auth_header.strip_prefix("Bearer ")?;
    validate_plugin_token(token, jwt_secret)
}

/// Validate a raw plugin JWT and return the embedded identity.
pub fn validate_plugin_token(token: &str, jwt_secret: &str) -> Option<PluginIdentity> {
    let key = DecodingKey::from_secret(jwt_secret.as_bytes());
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_audience(&[PluginTokenClaims::AUDIENCE]);
    // Accept any registered issuer; caller can restrict further if needed.
    validation.iss = None;

    match decode::<PluginTokenClaims>(token, &key, &validation) {
        Ok(data) if data.claims.kind == PluginTokenClaims::KIND => Some(PluginIdentity {
            idp_provider: data.claims.idp_provider,
            tenant_id: data.claims.tenant_id,
            github_login: data.claims.sub,
            github_id: data.claims.github_id,
            email: data.claims.email,
            jti: data.claims.jti,
        }),
        Ok(_) => {
            tracing::debug!("Plugin token rejected: unexpected kind");
            None
        }
        Err(e) => {
            tracing::debug!("Plugin token validation failed: {e}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tenant context derivation
// ---------------------------------------------------------------------------

/// Derive a `TenantContext` from a plugin bearer token.
///
/// If the `Authorization: Bearer <token>` header is present and the token
/// validates, the GitHub login and tenant claim are used as the caller identity.
///
/// Returns `None` when no valid bearer token is present, allowing callers to
/// enforce fail-closed behaviour rather than silently falling back to a synthetic
/// default context.  Use [`tenant_context_from_plugin_bearer_or_default`] only in
/// contexts where an unauthenticated / development fallback is explicitly intended.
pub fn tenant_context_from_plugin_bearer(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> Option<TenantContext> {
    let identity = validate_plugin_bearer(headers, jwt_secret)?;
    let tenant = sanitize_identifier(&identity.tenant_id, "");
    let login = identity.github_login.chars().take(100).collect::<String>();
    match (TenantId::new(tenant), UserId::new(login)) {
        (Some(tenant_id), Some(user_id)) => Some(TenantContext::new(tenant_id, user_id)),
        _ => {
            tracing::warn!("Plugin token carried invalid tenant_id or user_id; rejecting context");
            None
        }
    }
}

/// Derive a `TenantContext` from a plugin bearer token, falling back to a
/// synthetic `"default" / "system"` context when no valid token is present.
///
/// **This fallback is intentional for development/service-to-service mode only.**
/// It MUST NOT be used when plugin auth is enabled in a production-capable
/// deployment; use [`tenant_context_from_plugin_bearer`] and enforce the `None`
/// case explicitly.
pub fn tenant_context_from_plugin_bearer_or_default(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> TenantContext {
    if let Some(ctx) = tenant_context_from_plugin_bearer(headers, jwt_secret) {
        return ctx;
    }
    tracing::debug!(
        "No valid plugin bearer token found; using unauthenticated dev context (default/system).          This fallback must only occur in development or service-to-service mode."
    );
    TenantContext::new(
        TenantId::new(DEFAULT_TENANT_SLUG.to_string()).expect("static tenant id"),
        UserId::new(SYSTEM_USER_ID.to_string()).expect("static user id"),
    )
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct GitHubUser {
    id: u64,
    login: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct GitHubOAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUserResponse {
    id: u64,
    login: String,
    email: Option<String>,
}

async fn fetch_github_user(
    github_token: &str,
    cfg: &config::PluginAuthConfig,
) -> anyhow::Result<GitHubUser> {
    let client = reqwest::Client::new();
    let api_base = cfg
        .github_api_base_url
        .as_deref()
        .unwrap_or(DEFAULT_GITHUB_API_BASE);
    let resp = client
        .get(format!("{api_base}/user"))
        .header("Authorization", format!("Bearer {github_token}"))
        .header("User-Agent", "aeterna-plugin-auth/1.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?;

    let user: GitHubUserResponse = resp.json().await?;
    let email = match user.email {
        Some(email) => Some(email),
        None => fetch_github_primary_email(&client, github_token, cfg).await?,
    };

    Ok(GitHubUser {
        id: user.id,
        login: user.login,
        email,
    })
}

fn mint_access_token(
    jwt_secret: &str,
    issuer: &str,
    tenant_id: &str,
    user: &GitHubUser,
    ttl_seconds: u64,
) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    let claims = PluginTokenClaims {
        sub: user.login.clone(),
        idp_provider: PROVIDER_GITHUB.to_string(),
        tenant_id: tenant_id.to_string(),
        iss: issuer.to_string(),
        aud: vec![PluginTokenClaims::AUDIENCE.to_string()],
        iat: now,
        exp: now + ttl_seconds as i64,
        jti: Uuid::new_v4().to_string(),
        github_id: user.id,
        email: user.email.clone(),
        kind: PluginTokenClaims::KIND.to_string(),
    };

    let key = EncodingKey::from_secret(jwt_secret.as_bytes());
    Ok(encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &key,
    )?)
}

#[derive(Debug, Deserialize)]
struct GitHubEmailResponse {
    email: String,
    primary: bool,
    verified: bool,
}

async fn fetch_github_primary_email(
    client: &reqwest::Client,
    github_token: &str,
    cfg: &config::PluginAuthConfig,
) -> anyhow::Result<Option<String>> {
    let api_base = cfg
        .github_api_base_url
        .as_deref()
        .unwrap_or(DEFAULT_GITHUB_API_BASE);
    let resp = client
        .get(format!("{api_base}/user/emails"))
        .header("Authorization", format!("Bearer {github_token}"))
        .header("User-Agent", "aeterna-plugin-auth/1.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?;

    let emails: Vec<GitHubEmailResponse> = resp.json().await?;
    Ok(emails
        .into_iter()
        .find(|email| email.primary && email.verified)
        .map(|email| email.email))
}

/// Resolve a tenant ID for the given GitHub login using the plugin auth config.
///
/// Returns  if no tenant mapping is configured, which callers MUST treat
/// as a fail-closed condition (reject the request).
///
/// Resolution order:
/// 1.  map entry for this specific login (not yet supported in
///    config schema; reserved for future per-login routing).
/// 2.  from the plugin auth config.
/// 3.  environment variable (operator override for
///    simple single-tenant deployments without full config reload).
///
/// The synthetic hardcoded  string is intentionally NOT in this list.
pub(crate) fn resolve_tenant_for_github_user(
    _github_login: &str,
    cfg: &config::PluginAuthConfig,
) -> Option<String> {
    // Priority 1: explicit per-login mapping (config schema extension point).
    // Not yet wired; reserved.

    // Priority 2: default_tenant_id from config.
    if let Some(ref tenant_id) = cfg.default_tenant_id {
        let trimmed = tenant_id.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Priority 3: env override for operator convenience.
    if let Ok(env_tenant) = std::env::var("AETERNA_DEFAULT_TENANT_ID") {
        let trimmed = env_tenant.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    None
}

fn sanitize_identifier(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(100).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> String {
        "super-secret-test-key-at-least-32-chars".to_string()
    }

    fn user() -> GitHubUser {
        GitHubUser {
            id: 42,
            login: "testuser".to_string(),
            email: Some("testuser@example.com".to_string()),
        }
    }

    #[test]
    fn mint_and_validate_roundtrip() {
        let token = mint_access_token(&secret(), "aeterna", "default", &user(), 3600).unwrap();
        let identity = validate_plugin_token(&token, &secret()).unwrap();
        assert_eq!(identity.tenant_id, "default");
        assert_eq!(identity.github_login, "testuser");
        assert_eq!(identity.github_id, 42);
        assert_eq!(identity.email.as_deref(), Some("testuser@example.com"));
        assert!(!identity.jti.is_empty());
    }

    #[test]
    fn validate_rejects_wrong_secret() {
        let token = mint_access_token(&secret(), "aeterna", "default", &user(), 3600).unwrap();
        assert!(validate_plugin_token(&token, "wrong-secret").is_none());
    }

    #[test]
    fn validate_rejects_tampered_token() {
        let token = mint_access_token(&secret(), "aeterna", "default", &user(), 3600).unwrap();
        let tampered = format!("{token}x");
        assert!(validate_plugin_token(&tampered, &secret()).is_none());
    }

    #[test]
    fn validate_bearer_header_missing_returns_none() {
        let headers = HeaderMap::new();
        assert!(validate_plugin_bearer(&headers, &secret()).is_none());
    }

    #[test]
    fn validate_bearer_header_extracts_identity() {
        let token = mint_access_token(&secret(), "aeterna", "default", &user(), 3600).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        let identity = validate_plugin_bearer(&headers, &secret()).unwrap();
        assert_eq!(identity.github_login, "testuser");
    }

    #[test]
    fn bootstrap_request_uses_github_access_token_contract() {
        let req: PluginAuthBootstrapRequest = serde_json::from_value(serde_json::json!({
            "provider": "github",
            "github_access_token": "gho_123"
        }))
        .unwrap();

        assert_eq!(req.provider, "github");
        assert_eq!(req.github_access_token, "gho_123");
    }

    #[test]
    fn bootstrap_request_rejects_legacy_code_shape() {
        let req = serde_json::from_value::<PluginAuthBootstrapRequest>(serde_json::json!({
            "provider": "github",
            "code": "legacy-code"
        }));

        assert!(req.is_err());
    }

    #[test]
    fn tenant_context_uses_tenant_claim_from_bearer() {
        let token = mint_access_token(&secret(), "aeterna", "tenant-42", &user(), 3600).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());

        let ctx = tenant_context_from_plugin_bearer(&headers, &secret()).unwrap();
        assert_eq!(ctx.tenant_id.as_str(), "tenant-42");
        assert_eq!(ctx.user_id.as_str(), "testuser");
    }

    #[tokio::test]
    async fn refresh_store_roundtrip() {
        let store = RefreshTokenStore::new();
        store
            .insert(
                "tok".to_string(),
                "default".to_string(),
                "alice".to_string(),
                1,
                None,
                3600,
            )
            .await;

        let entry = store.take("tok").await.unwrap();
        assert_eq!(entry.tenant_id, "default");
        assert_eq!(entry.github_login, "alice");

        // Single-use: second take must be None
        assert!(store.take("tok").await.is_none());
    }

    #[tokio::test]
    async fn refresh_store_revoke() {
        let store = RefreshTokenStore::new();
        store
            .insert(
                "tok".to_string(),
                "default".to_string(),
                "bob".to_string(),
                2,
                None,
                3600,
            )
            .await;
        store.revoke("tok").await;
        assert!(store.take("tok").await.is_none());
    }

    #[tokio::test]
    async fn refresh_store_expired_returns_none() {
        let store = RefreshTokenStore::new();
        // TTL 0 → expires_at == now → already expired
        store
            .insert(
                "tok".to_string(),
                "default".to_string(),
                "carol".to_string(),
                3,
                None,
                0,
            )
            .await;
        assert!(store.take("tok").await.is_none());
    }

    // ── Task 4.1: bootstrap fail-closed – resolve_tenant_for_github_user ──────

    #[test]
    fn resolve_tenant_returns_none_when_no_config_and_no_env() {
        // SAFETY: test-only env manipulation, tests run single-threaded via --test-threads=1
        unsafe { std::env::remove_var("AETERNA_DEFAULT_TENANT_ID") };
        let cfg = config::PluginAuthConfig {
            enabled: true,
            jwt_secret: Some("s".to_string()),
            default_tenant_id: None,
            ..Default::default()
        };
        assert!(
            resolve_tenant_for_github_user("anyuser", &cfg).is_none(),
            "MUST return None when no config field and no env var are set"
        );
    }

    #[test]
    fn resolve_tenant_prefers_config_field_over_env() {
        // SAFETY: test-only env manipulation, tests run single-threaded via --test-threads=1
        unsafe { std::env::set_var("AETERNA_DEFAULT_TENANT_ID", "env-tenant") };
        let cfg = config::PluginAuthConfig {
            enabled: true,
            jwt_secret: Some("s".to_string()),
            default_tenant_id: Some("config-tenant".to_string()),
            ..Default::default()
        };
        let result = resolve_tenant_for_github_user("alice", &cfg);
        unsafe { std::env::remove_var("AETERNA_DEFAULT_TENANT_ID") };
        assert_eq!(result.as_deref(), Some("config-tenant"));
    }

    #[test]
    fn resolve_tenant_falls_back_to_env_when_config_absent() {
        // SAFETY: test-only env manipulation, tests run single-threaded via --test-threads=1
        unsafe { std::env::set_var("AETERNA_DEFAULT_TENANT_ID", "env-tenant") };
        let cfg = config::PluginAuthConfig {
            enabled: true,
            jwt_secret: Some("s".to_string()),
            default_tenant_id: None,
            ..Default::default()
        };
        let result = resolve_tenant_for_github_user("alice", &cfg);
        unsafe { std::env::remove_var("AETERNA_DEFAULT_TENANT_ID") };
        assert_eq!(result.as_deref(), Some("env-tenant"));
    }

    #[test]
    fn resolve_tenant_ignores_blank_config_field() {
        // SAFETY: test-only env manipulation, tests run single-threaded via --test-threads=1
        unsafe { std::env::remove_var("AETERNA_DEFAULT_TENANT_ID") };
        let cfg = config::PluginAuthConfig {
            enabled: true,
            jwt_secret: Some("s".to_string()),
            default_tenant_id: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(
            resolve_tenant_for_github_user("alice", &cfg).is_none(),
            "Whitespace-only tenant id MUST be treated as absent"
        );
    }

    // ── Task 4.1: tenant_context_from_plugin_bearer returns None on bad token ──

    #[test]
    fn tenant_context_from_plugin_bearer_returns_none_without_token() {
        let headers = HeaderMap::new();
        assert!(
            tenant_context_from_plugin_bearer(&headers, &secret()).is_none(),
            "MUST return None when no Authorization header is present"
        );
    }

    #[test]
    fn tenant_context_from_plugin_bearer_returns_none_on_wrong_secret() {
        let token = mint_access_token(&secret(), "aeterna", "t1", &user(), 3600).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        assert!(
            tenant_context_from_plugin_bearer(&headers, "wrong-secret").is_none(),
            "MUST return None when token was signed with a different secret"
        );
    }
}
