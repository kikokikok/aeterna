//! Service-token validation pipeline (B2 §10.3 + §10.5).
//!
//! Service tokens are minted by §10.2 (`POST /api/v1/auth/tokens`) as
//! `agents` rows with `agent_type='service'`. This module is the READ
//! side of that lifecycle: given a decoded [`PluginIdentity`] whose
//! `token_type == "service"`, it converts the identity into a
//! [`ServicePrincipal`] that downstream middleware (§10.5,
//! [`require_capability`]) can use for capability checks.
//!
//! ## Validation pipeline
//!
//! 1. Short-circuit on non-service tokens (`token_type != "service"`).
//! 2. Extract the agent UUID from the `sub` claim.
//! 3. Consult the [`RevocationCache`] (§10.3: 60s TTL).
//!    - Cache hit + fresh + `status='active'` → build principal.
//!    - Cache hit + `status='revoked'` → reject 401 `token_revoked`.
//!    - Cache miss OR stale entry → fall through to Postgres.
//! 4. Postgres read of `agents` row; rehydrate cache; branch on status.
//!
//! ## §10.3 cross-instance revocation cache
//!
//! The cache is backed by Redis/Dragonfly when `AppState.redis_conn`
//! is `Some` — the production multi-instance mode. Revocation from
//! any instance is a single `DEL revocation:agent:<uuid>` and is
//! observed by every other instance on its next lookup, with zero
//! staleness. The 60s TTL (`EX 60`) on the key is a second line of
//! defence: even without an explicit revoke, any Postgres-side state
//! drift (e.g. an admin updating `agents.status` via SQL) becomes
//! visible within one TTL window.
//!
//! When `redis_conn` is `None` — local dev, single-instance tests —
//! the cache falls back to an in-process `DashMap` with the same 60s
//! TTL semantics, and the "cross-instance" guarantee degrades to
//! "within this process". This matches the pattern used by
//! `refresh_store` and `backup_api::init_job_stores`.
//!
//! ## What this module does NOT do
//!
//! - JWT decode — that's `plugin_auth::validate_plugin_bearer`; we
//!   accept already-decoded [`PluginIdentity`].
//! - Capability enforcement is right here: [`require_capability`].
//! - Issuance — that's `service_tokens::mint_handler`, which warms
//!   this cache on a successful mint.
//! - Revocation write — that's `service_tokens::revoke_handler`,
//!   which calls [`RevocationCache::invalidate`] after the DB update.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::server::AppState;
use crate::server::plugin_auth::{PluginIdentity, PluginTokenClaims};

/// 60s cross-instance bounded-staleness window (B2 §10.3).
pub const REVOCATION_CACHE_TTL: Duration = Duration::from_secs(60);

/// Redis key namespace for the revocation cache. Kept narrow so a
/// `SCAN revocation:agent:*` surfaces exactly these entries during
/// ops debugging.
const REDIS_KEY_PREFIX: &str = "revocation:agent:";

fn redis_key(agent_id: Uuid) -> String {
    format!("{REDIS_KEY_PREFIX}{agent_id}")
}

/// Cached snapshot of the `agents` columns that gate a request.
///
/// Kept intentionally narrow — only the fields that drive the
/// validation branches — so a cache entry stays under ~200 bytes
/// when serialized and we never fall into the "cache the whole ORM
/// object" trap.
///
/// `Serialize`/`Deserialize` are for the Redis payload; the
/// in-memory fallback wraps this in [`InMemoryEntry`] alongside an
/// `Instant` for local TTL tracking (Redis's `EX` handles TTL
/// natively, so the Redis payload carries no timestamp).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAgent {
    pub status: AgentStatus,
    pub tenant_id: Uuid,
    pub capabilities: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

/// Normalized agent status. `agents.status` is a `TEXT` column in
/// Postgres; we only care about the two terminal values for gating
/// and collapse anything else into `Unknown` so a bad write cannot
/// accidentally grant access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Revoked,
    Unknown,
}

impl AgentStatus {
    pub fn from_db(s: &str) -> Self {
        match s {
            "active" => AgentStatus::Active,
            "revoked" => AgentStatus::Revoked,
            _ => AgentStatus::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
struct InMemoryEntry {
    data: CachedAgent,
    cached_at: Instant,
}

/// TTL-bounded cache of service-agent validation state.
///
/// Prefers Redis/Dragonfly (multi-instance, zero-staleness revokes);
/// falls back to a process-local `DashMap` when no Redis connection
/// is configured.
#[derive(Debug)]
pub struct RevocationCache {
    /// Multi-instance backing store. When `Some`, wins over `local`.
    redis: Option<Arc<redis::aio::ConnectionManager>>,
    /// Single-instance fallback. Always populated — unused in HA
    /// mode, drives everything in dev/tests.
    local: DashMap<Uuid, InMemoryEntry>,
}

impl Default for RevocationCache {
    fn default() -> Self {
        Self {
            redis: None,
            local: DashMap::new(),
        }
    }
}

impl RevocationCache {
    /// Construct a cache that uses Redis/Dragonfly when `redis_conn`
    /// is `Some`, falling back to process-local state otherwise.
    pub fn new(redis_conn: Option<Arc<redis::aio::ConnectionManager>>) -> Self {
        Self {
            redis: redis_conn,
            local: DashMap::new(),
        }
    }

    /// Insert a fresh snapshot, overwriting any existing entry, and
    /// set the 60s TTL.
    ///
    /// Called by `mint_handler` (avoid cold-read on first request)
    /// and by [`validate_service_token`] on a cache miss after the
    /// Postgres fallback.
    pub async fn warm(&self, agent_id: Uuid, entry: CachedAgent) {
        if let Some(conn) = self.redis.as_ref() {
            let mut conn = conn.as_ref().clone();
            let payload = match serde_json::to_string(&entry) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(error = %e, agent_id = %agent_id, "Failed to serialize cached agent; skipping warm");
                    return;
                }
            };
            let result: Result<(), redis::RedisError> = redis::cmd("SET")
                .arg(redis_key(agent_id))
                .arg(payload)
                .arg("EX")
                .arg(REVOCATION_CACHE_TTL.as_secs())
                .query_async(&mut conn)
                .await;
            if let Err(e) = result {
                // Redis hiccup: log and fall through to local cache
                // so subsequent requests still see the warmed entry
                // on this instance. DB re-read covers the rest.
                tracing::warn!(error = %e, agent_id = %agent_id, "Redis SET failed; warming local cache only");
                self.local.insert(
                    agent_id,
                    InMemoryEntry {
                        data: entry,
                        cached_at: Instant::now(),
                    },
                );
            }
        } else {
            self.local.insert(
                agent_id,
                InMemoryEntry {
                    data: entry,
                    cached_at: Instant::now(),
                },
            );
        }
    }

    /// Remove an entry across every instance that shares the Redis
    /// backend.
    ///
    /// `DEL` is synchronous w.r.t. other clients: once this returns,
    /// no other instance's `GET revocation:agent:<uuid>` will hit.
    /// The local fallback is also cleared so a single-instance
    /// deployment observes the same semantics.
    pub async fn invalidate(&self, agent_id: Uuid) {
        if let Some(conn) = self.redis.as_ref() {
            let mut conn = conn.as_ref().clone();
            let result: Result<i64, redis::RedisError> = redis::cmd("DEL")
                .arg(redis_key(agent_id))
                .query_async(&mut conn)
                .await;
            if let Err(e) = result {
                tracing::error!(error = %e, agent_id = %agent_id, "Redis DEL failed during revocation cache invalidate");
            }
        }
        self.local.remove(&agent_id);
    }

    /// Read the snapshot if it exists and is fresh.
    ///
    /// Returns `None` on miss, on aged-out local entries, and on
    /// Redis errors (fail-open to the Postgres path, never grant
    /// access on infra hiccup). Aged-out local entries are left in
    /// place — the next DB-read path overwrites them — to keep the
    /// hot path lock-free.
    pub async fn get(&self, agent_id: Uuid) -> Option<CachedAgent> {
        if let Some(conn) = self.redis.as_ref() {
            let mut conn = conn.as_ref().clone();
            let payload: Result<Option<String>, redis::RedisError> = redis::cmd("GET")
                .arg(redis_key(agent_id))
                .query_async(&mut conn)
                .await;
            match payload {
                Ok(Some(s)) => match serde_json::from_str::<CachedAgent>(&s) {
                    Ok(entry) => return Some(entry),
                    Err(e) => {
                        // Poisoned entry: delete it so the next
                        // request re-reads from Postgres cleanly.
                        tracing::error!(
                            error = %e,
                            agent_id = %agent_id,
                            "Corrupt revocation cache entry in Redis; deleting",
                        );
                        let _: Result<i64, _> = redis::cmd("DEL")
                            .arg(redis_key(agent_id))
                            .query_async(&mut conn)
                            .await;
                        return None;
                    }
                },
                Ok(None) => return None,
                Err(e) => {
                    tracing::warn!(error = %e, agent_id = %agent_id, "Redis GET failed; falling through to Postgres");
                    return None;
                }
            }
        }

        let entry = self.local.get(&agent_id)?;
        if entry.cached_at.elapsed() > REVOCATION_CACHE_TTL {
            return None;
        }
        Some(entry.data.clone())
    }

    /// `true` when the cache is backed by Redis/Dragonfly. Exposed
    /// for /status-style introspection.
    pub fn is_distributed(&self) -> bool {
        self.redis.is_some()
    }

    #[cfg(test)]
    fn local_len(&self) -> usize {
        self.local.len()
    }
}

/// Fully-validated service principal. Produced only when the token
/// passed every §10.5 check: signature, `token_type='service'`,
/// agent row exists, `status='active'`, not expired, and the cached
/// `tenant_id` matches the claim's `tenant_id`.
#[derive(Debug, Clone)]
pub struct ServicePrincipal {
    pub agent_id: Uuid,
    pub tenant_id: Uuid,
    pub scopes: Vec<String>,
}

/// Validate a pre-decoded service-token identity.
///
/// Returns `Ok(Some(principal))` for a live service token, `Ok(None)`
/// when the identity describes a non-service token (caller should
/// fall through to the user pipeline), and `Err(response)` for any
/// rejection — revoked, expired, missing, tenant mismatch, malformed
/// `sub`, or a DB error.
///
/// For service tokens the `sub` claim \[carried as `identity.github_login`\]
/// is the agent UUID string set by `service_tokens::mint_handler`.
pub async fn validate_service_token(
    state: &AppState,
    identity: &PluginIdentity,
) -> Result<Option<ServicePrincipal>, Response> {
    if identity.token_type != PluginTokenClaims::TOKEN_TYPE_SERVICE {
        return Ok(None);
    }

    let agent_id = Uuid::parse_str(&identity.github_login).map_err(|_| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_service_token",
            "service token `sub` is not a valid agent id",
        )
    })?;

    let tenant_claim = Uuid::parse_str(&identity.tenant_id).map_err(|_| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_service_token",
            "service token `tenant_id` is not a valid uuid",
        )
    })?;

    let cache = &state.revocation_cache;
    let snapshot = match cache.get(agent_id).await {
        Some(cached) => cached,
        None => {
            let fresh = read_agent_from_db(state, agent_id).await?;
            cache.warm(agent_id, fresh.clone()).await;
            fresh
        }
    };

    match snapshot.status {
        AgentStatus::Revoked => Err(error_response(
            StatusCode::UNAUTHORIZED,
            "token_revoked",
            "service token has been revoked",
        )),
        AgentStatus::Unknown => Err(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_service_token",
            "service token agent is in an unexpected state",
        )),
        AgentStatus::Active => {
            if snapshot.expires_at <= Utc::now() {
                return Err(error_response(
                    StatusCode::UNAUTHORIZED,
                    "token_expired",
                    "service token has expired",
                ));
            }
            if snapshot.tenant_id != tenant_claim {
                return Err(error_response(
                    StatusCode::UNAUTHORIZED,
                    "invalid_service_token",
                    "service token `tenant_id` does not match the agent record",
                ));
            }
            Ok(Some(ServicePrincipal {
                agent_id,
                tenant_id: snapshot.tenant_id,
                scopes: snapshot.capabilities,
            }))
        }
    }
}

/// Header-based entry point used by request handlers.
///
/// Returns:
/// - `Ok(Some(principal))` — valid live service token.
/// - `Ok(None)` — no bearer token, or token is not a service token;
///   caller should fall through to the existing user-auth pipeline
///   (`authenticated_tenant_context`).
/// - `Err(response)` — token claimed to be a service token but
///   failed validation. Fail-closed; caller propagates.
pub async fn validate_service_token_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<ServicePrincipal>, Response> {
    let secret = match state.plugin_auth_state.config.jwt_secret.as_deref() {
        Some(s) => s,
        None => return Ok(None),
    };

    let identity = match crate::server::plugin_auth::validate_plugin_bearer(headers, secret) {
        Some(id) => id,
        None => return Ok(None),
    };

    validate_service_token(state, &identity).await
}

/// Enforce the route's required capability against a service principal.
///
/// - `Some(principal)` + scope present → `Ok(())`
/// - `Some(principal)` + scope missing → `Err(403 missing_capability)`
/// - `None` (user caller) → `Ok(())` \[B2 §10.5: "fall back to role
///   check for user callers" — the role check is the existing Cedar
///   enforcement inside each handler, not this function's job\]
pub fn require_capability(principal: Option<&ServicePrincipal>, cap: &str) -> Result<(), Response> {
    let Some(principal) = principal else {
        return Ok(());
    };
    if principal.scopes.iter().any(|s| s == cap) {
        Ok(())
    } else {
        Err(error_response(
            StatusCode::FORBIDDEN,
            "missing_capability",
            &format!("service token lacks required capability `{cap}`"),
        ))
    }
}

async fn read_agent_from_db(state: &AppState, agent_id: Uuid) -> Result<CachedAgent, Response> {
    let row = sqlx::query(
        r#"
        SELECT status, tenant_id, capabilities, expires_at
        FROM agents
        WHERE id = $1 AND agent_type = 'service'
        "#,
    )
    .bind(agent_id)
    .fetch_optional(state.postgres.pool())
    .await
    .map_err(|e| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent_lookup_failed",
            &e.to_string(),
        )
    })?;

    let row = row.ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_service_token",
            "service token references an unknown agent",
        )
    })?;

    let status_str: String = row.try_get("status").unwrap_or_default();
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(|e| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent_row_malformed",
            &e.to_string(),
        )
    })?;
    let capabilities_json: Value = row.try_get("capabilities").unwrap_or(Value::Null);
    let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(|e| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "agent_row_malformed",
            &e.to_string(),
        )
    })?;

    let capabilities = match capabilities_json {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };

    Ok(CachedAgent {
        status: AgentStatus::from_db(&status_str),
        tenant_id,
        capabilities,
        expires_at,
    })
}

fn error_response(status: StatusCode, error: &str, message: &str) -> Response {
    let body = json!({ "error": error, "message": message });
    (status, Json(body)).into_response()
}

// ============================================================================
// Tests — in-memory fallback path only. Redis-backed behaviour is
// covered by integration tests that stand up a real Dragonfly.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(status: AgentStatus) -> CachedAgent {
        CachedAgent {
            status,
            tenant_id: Uuid::nil(),
            capabilities: vec!["memory:read".to_string()],
            expires_at: Utc::now() + chrono::Duration::hours(1),
        }
    }

    #[test]
    fn agent_status_from_db_is_strict() {
        assert_eq!(AgentStatus::from_db("active"), AgentStatus::Active);
        assert_eq!(AgentStatus::from_db("revoked"), AgentStatus::Revoked);
        // Fail-closed: anything else — typos, future values, empty,
        // different casing — must NOT be treated as active.
        assert_eq!(AgentStatus::from_db(""), AgentStatus::Unknown);
        assert_eq!(AgentStatus::from_db("pending"), AgentStatus::Unknown);
        assert_eq!(AgentStatus::from_db("Active"), AgentStatus::Unknown);
        assert_eq!(AgentStatus::from_db("disabled"), AgentStatus::Unknown);
    }

    #[test]
    fn cached_agent_round_trips_through_json() {
        // Redis payload is JSON — any change to `CachedAgent` must
        // keep ser/de symmetric or live cache entries become
        // undecodable on deploy.
        let e = make_entry(AgentStatus::Active);
        let s = serde_json::to_string(&e).unwrap();
        let back: CachedAgent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.status, AgentStatus::Active);
        assert_eq!(back.tenant_id, e.tenant_id);
        assert_eq!(back.capabilities, e.capabilities);
    }

    #[test]
    fn agent_status_serializes_lowercase() {
        // Redis keys are long-lived; if we renamed the enum and let
        // serde emit PascalCase, every live cache entry from the
        // previous deploy would become un-parseable. Pin the
        // rename_all contract.
        assert_eq!(
            serde_json::to_string(&AgentStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Revoked).unwrap(),
            "\"revoked\""
        );
    }

    #[tokio::test]
    async fn local_cache_warm_and_get_round_trip() {
        let cache = RevocationCache::new(None);
        assert!(!cache.is_distributed());
        let id = Uuid::new_v4();
        assert!(cache.get(id).await.is_none(), "cold cache must miss");
        cache.warm(id, make_entry(AgentStatus::Active)).await;
        let got = cache.get(id).await.expect("warm entry must be visible");
        assert_eq!(got.status, AgentStatus::Active);
        assert_eq!(cache.local_len(), 1);
    }

    #[tokio::test]
    async fn local_cache_honours_ttl_window() {
        let cache = RevocationCache::new(None);
        let id = Uuid::new_v4();
        // Backdate the in-memory entry to simulate an aged-out
        // record without sleeping 60s in tests.
        cache.warm(id, make_entry(AgentStatus::Active)).await;
        if let Some(mut entry) = cache.local.get_mut(&id) {
            entry.cached_at = Instant::now() - Duration::from_secs(61);
        }
        assert!(
            cache.get(id).await.is_none(),
            "stale entry beyond TTL must return None"
        );
    }

    #[tokio::test]
    async fn local_cache_invalidate_removes_entry() {
        let cache = RevocationCache::new(None);
        let id = Uuid::new_v4();
        cache.warm(id, make_entry(AgentStatus::Active)).await;
        assert!(cache.get(id).await.is_some());
        cache.invalidate(id).await;
        assert!(
            cache.get(id).await.is_none(),
            "invalidated entry must not resurface"
        );
        assert_eq!(cache.local_len(), 0);
    }

    #[tokio::test]
    async fn local_cache_revoked_status_survives_lookup() {
        // A revoked snapshot still resolves by get() — the status is
        // the signal. The validator uses this to 401 without a DB
        // re-read.
        let cache = RevocationCache::new(None);
        let id = Uuid::new_v4();
        cache.warm(id, make_entry(AgentStatus::Revoked)).await;
        let got = cache
            .get(id)
            .await
            .expect("revoked entry must be retrievable");
        assert_eq!(got.status, AgentStatus::Revoked);
    }

    #[test]
    fn redis_key_prefix_is_stable() {
        // The key prefix is operator-visible via `SCAN`. Changing it
        // orphans every live cache entry on deploy — a deliberate
        // edit, not an accident.
        assert_eq!(REDIS_KEY_PREFIX, "revocation:agent:");
        let id = Uuid::nil();
        assert_eq!(
            redis_key(id),
            "revocation:agent:00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn cache_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RevocationCache>();
        assert_send_sync::<std::sync::Arc<RevocationCache>>();
    }

    #[test]
    fn revocation_cache_ttl_is_sixty_seconds() {
        // §10.3 contract: pin the TTL so any future change is a
        // deliberate, reviewable edit, not a silent widening of the
        // staleness window.
        assert_eq!(REVOCATION_CACHE_TTL, Duration::from_secs(60));
    }

    fn make_principal(scopes: Vec<&str>) -> ServicePrincipal {
        ServicePrincipal {
            agent_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            scopes: scopes.into_iter().map(str::to_owned).collect(),
        }
    }

    #[test]
    fn require_capability_passes_for_user_caller() {
        // `None` principal means "no service token detected" —
        // user flow. §10.5 fallback: role check is elsewhere.
        assert!(require_capability(None, "memory:write").is_ok());
        assert!(require_capability(None, "anything:at:all").is_ok());
    }

    #[test]
    fn require_capability_passes_when_scope_is_granted() {
        let p = make_principal(vec!["memory:read", "memory:write"]);
        assert!(require_capability(Some(&p), "memory:write").is_ok());
        assert!(require_capability(Some(&p), "memory:read").is_ok());
    }

    #[test]
    fn require_capability_rejects_missing_scope() {
        let p = make_principal(vec!["memory:read"]);
        let err =
            require_capability(Some(&p), "memory:write").expect_err("missing scope must deny");
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn require_capability_rejects_empty_scopes() {
        let p = make_principal(vec![]);
        let err = require_capability(Some(&p), "memory:write")
            .expect_err("empty scopes must deny everything");
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn require_capability_is_case_sensitive() {
        // Scope strings are exact-match — "Memory:Write" does NOT
        // satisfy "memory:write". Matches the mint-side allow-list.
        let p = make_principal(vec!["Memory:Write"]);
        assert!(require_capability(Some(&p), "memory:write").is_err());
    }
}
