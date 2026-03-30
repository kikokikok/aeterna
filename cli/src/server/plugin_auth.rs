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
use mk_core::types::{TenantContext, TenantId, UserId};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::AppState;

// ---------------------------------------------------------------------------
// Refresh token store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
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

    if req.provider != "github" {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_provider",
                "message": "Only 'github' is supported as a plugin auth provider"
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

    let github_user = match fetch_github_user(&req.github_access_token).await {
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
    let refresh_ttl = cfg.refresh_token_ttl_seconds.unwrap_or(30 * 24 * 3600);
    let issuer = cfg
        .token_issuer
        .clone()
        .unwrap_or_else(|| "aeterna".to_string());
    let tenant_id = default_tenant_claim();

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
    let refresh_ttl = cfg.refresh_token_ttl_seconds.unwrap_or(30 * 24 * 3600);
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
/// validates, the GitHub login is used as the user identifier.  Falls back
/// to a default `"default" / "system"` context when no valid token is found.
pub fn tenant_context_from_plugin_bearer(headers: &HeaderMap, jwt_secret: &str) -> TenantContext {
    if let Some(identity) = validate_plugin_bearer(headers, jwt_secret) {
        let tenant = sanitize_identifier(&identity.tenant_id, "default");
        let login = identity.github_login.chars().take(100).collect::<String>();
        if let (Some(tenant_id), Some(user_id)) = (TenantId::new(tenant), UserId::new(login)) {
            return TenantContext::new(tenant_id, user_id);
        }
    }

    // Fallback: unauthenticated / service context
    TenantContext::new(
        TenantId::new("default".to_string()).expect("static tenant id"),
        UserId::new("system".to_string()).expect("static user id"),
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

async fn fetch_github_user(github_token: &str) -> anyhow::Result<GitHubUser> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {github_token}"))
        .header("User-Agent", "aeterna-plugin-auth/1.0")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?;

    let user: GitHubUserResponse = resp.json().await?;
    let email = match user.email {
        Some(email) => Some(email),
        None => fetch_github_primary_email(&client, github_token).await?,
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
) -> anyhow::Result<Option<String>> {
    let resp = client
        .get("https://api.github.com/user/emails")
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

fn default_tenant_claim() -> String {
    "default".to_string()
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

        let ctx = tenant_context_from_plugin_bearer(&headers, &secret());
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
}
