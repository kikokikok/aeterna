# HA Auth & Session Backing Store Requirements

> **Scope**: This guide covers the plugin-auth refresh-token store and any
> future session-state paths that must be durable and shared across server
> replicas for High-Availability (HA) deployments.

---

## Problem Statement

Aeterna's plugin authentication flow (`/api/v1/auth/plugin/*`) issues
single-use, rotating refresh tokens via the `RefreshTokenStore` type in
`cli/src/server/plugin_auth.rs`.  In its current implementation the store
is **in-process only** (a `tokio::sync::RwLock<HashMap<…>>`).

This causes two failure modes in multi-replica deployments:

| Scenario | Effect |
|----------|--------|
| Process restart | All refresh tokens are lost; active plugin sessions are terminated |
| Load-balanced request to a different replica | The refresh token is unknown to that replica; the client receives `401 invalid_refresh_token` |

---

## Affected Components

| Component | File | Store type | Limitation |
|-----------|------|------------|------------|
| Plugin refresh tokens | `cli/src/server/plugin_auth.rs` | `RefreshTokenStore` (in-memory HashMap) | Not shared across replicas |

---

## Runtime Indicator

When `plugin_auth.enabled = true` and the deployment mode is not `"local"`,
the server emits a structured warning at the start of every bootstrap call:

```
WARN aeterna::server::plugin_auth: Plugin auth is enabled with an in-memory
     refresh-token store in a non-local deployment mode. Refresh tokens are NOT
     shared across replicas and will be lost on restart. Migrate to a Redis- or
     Postgres-backed store before running multiple server instances.
     deployment_mode="remote"
```

This warning is **not** fatal in order to avoid breaking single-node
non-local deployments during transition.  Operators MUST act on it before
running multiple replicas.

---

## Required Action for HA Deployments

### Option A — Redis-backed refresh token store (recommended)

1. Implement a `RedisRefreshTokenStore` that satisfies the same insert/take/revoke
   contract as `RefreshTokenStore`.
2. Persist each token entry as a Redis key:
   ```
   plugin:refresh:<token> → JSON { tenant_id, github_login, github_id, email, expires_at }
   ```
   with a Redis TTL equal to `refresh_token_ttl_seconds`.
3. Implement single-use semantics using a Redis `GETDEL` or Lua CAS script:
   ```lua
   local v = redis.call("GET", KEYS[1])
   if v then redis.call("DEL", KEYS[1]) end
   return v
   ```
4. Wire `RedisRefreshTokenStore` into `PluginAuthState` and remove the
   `tracing::warn!` HA check once the implementation is in place.

### Option B — PostgreSQL-backed store

1. Create a `plugin_refresh_tokens` table:
   ```sql
   CREATE TABLE plugin_refresh_tokens (
       token       TEXT PRIMARY KEY,
       tenant_id   TEXT NOT NULL,
       github_login TEXT NOT NULL,
       github_id   BIGINT NOT NULL,
       email       TEXT,
       expires_at  BIGINT NOT NULL
   );
   ```
2. Use `DELETE … RETURNING` for single-use semantics:
   ```sql
   DELETE FROM plugin_refresh_tokens
   WHERE token = $1 AND expires_at > extract(epoch from now())
   RETURNING tenant_id, github_login, github_id, email;
   ```
3. Add a periodic cleanup job to remove expired rows.

---

## Testing HA Behaviour

The unit test `refresh_store_is_not_ha_safe_without_backing_store` in
`cli/src/server/plugin_auth.rs` asserts that two independent `RefreshTokenStore`
instances do **not** share state — which proves the in-memory limitation.

When a durable implementation is adopted, add an integration test that:

1. Inserts a token via store instance A.
2. Consumes it via store instance B pointed at the same Redis/Postgres.
3. Verifies the token is consumed exactly once (single-use semantics hold).

---

## Non-HA (local) Mode

When `deployment.mode = "local"` (single-replica, developer workstation):

- The in-memory `RefreshTokenStore` is fully functional.
- No warning is emitted.
- No migration is required.

---

## Session State (future)

If Aeterna introduces server-side session state (e.g., for Okta browser
flows via oauth2-proxy or direct cookie sessions), the same principle
applies: any state that must survive a restart or be shared across replicas
MUST be stored in Redis (with `scoped_key`) or PostgreSQL (with RLS-protected
tables), NOT in process memory.

See also:
- [`docs/guides/ha-deployment.md`](ha-deployment.md) — HA infrastructure setup
- [`docs/guides/okta-auth-deployment.md`](okta-auth-deployment.md) — Okta browser auth
- `storage/src/redis.rs` — `scoped_key()` and tenant key-namespace audit comment
