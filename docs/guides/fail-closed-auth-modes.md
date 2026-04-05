# Fail-Closed Auth Modes: Dev vs Production

This guide explains Aeterna's two auth modes — **dev/permissive** and **production/fail-closed** — and the expectations for each.

## Overview

Aeterna enforces a strict separation between development and production authentication behaviour.  In production, every request to a tenant-scoped surface MUST carry a validated identity.  In development, explicit opt-in relaxes this requirement to allow local testing without a full identity provider.

The guiding principle is **fail closed**: when in doubt, reject the request rather than fall back to a synthetic default tenant.

---

## Production (Fail-Closed) Mode

**Enabled by default.** Active when `AETERNA_PLUGIN_AUTH_ENABLED=true` (or `plugin_auth.enabled = true` in config).

### Behaviour

| Surface | With valid bearer | Without bearer |
|---------|------------------|----------------|
| `/sync/push`, `/sync/pull` | ✅ Proceeds with validated `TenantContext` | ❌ 401 Unauthorized |
| `/knowledge/*` | ✅ Proceeds with validated `TenantContext` | ❌ 401 Unauthorized |
| `/auth/bootstrap` | ✅ Issues token for resolved tenant | ❌ 500 if tenant not configured |
| MCP `tools/call` | ✅ Payload `tenantContext` validated or injected | ❌ -32003 if payload tenant ≠ caller |
| Webhook events | ✅ Tenant derived from config | ⚠️ Warning logged, falls back to default |
| A2A agent | ✅ API key or trusted-identity validated | ❌ 401 if auth enabled and no credential |

### Required configuration

```toml
[plugin_auth]
enabled = true
jwt_secret = "<at-least-32-char secret>"
default_tenant_id = "<your-tenant-id>"   # required for bootstrap and webhooks
```

Or via environment:
```bash
AETERNA_PLUGIN_AUTH_ENABLED=true
AETERNA_PLUGIN_AUTH_JWT_SECRET=<secret>
AETERNA_PLUGIN_AUTH_TENANT=<tenant-id>
```

### Tenant resolution priority (bootstrap / webhooks)

1. `plugin_auth.default_tenant_id` config field
2. `AETERNA_PLUGIN_AUTH_TENANT` environment variable
3. → If neither is set: bootstrap returns **500**, webhook logs a warning

---

## Development / Permissive Mode

**Explicit opt-in required.** Active when `plugin_auth.enabled = false` (the default).

### Enabling

```bash
# In your .env or shell
AETERNA_PLUGIN_AUTH_ENABLED=false   # default — dev mode
```

To suppress the startup warning about permissive auth set:
```bash
AETERNA_ALLOW_PERMISSIVE_AUTH=dev
```

Without this flag, the server logs a `WARN`-level message at startup:
```
WARN aeterna::server::bootstrap: Running with permissive (allow-all) auth.
     This MUST NOT be used in production. Set AETERNA_ALLOW_PERMISSIVE_AUTH=dev to silence this warning.
```

### Behaviour in dev mode

- Requests without a bearer token fall back to the `default/system` synthetic context.
- The `X-User-ID` and `X-Tenant-ID` headers are trusted directly (no signature validation).
- MCP `tenantContext` payloads are accepted verbatim; no caller verification is performed.

### What dev mode MUST NOT do

- Dev mode MUST NOT be used with a publicly accessible Aeterna instance.
- Dev mode MUST NOT be deployed to any environment that holds real tenant data.
- Dev mode MUST NOT be enabled when `AETERNA_PLUGIN_AUTH_JWT_SECRET` is set to a production secret.

---

## MCP Tenant Scope Enforcement

When plugin auth is enabled, the MCP HTTP transport enforces the following on every `tools/call` request:

| Scenario | Result |
|----------|--------|
| Payload `tenantContext.tenant_id` matches authenticated caller tenant | ✅ Request proceeds |
| Payload `tenantContext.tenant_id` is **different** from caller tenant | ❌ JSON-RPC error `-32003` |
| No `tenantContext` in payload | ✅ Caller's tenant is **injected** automatically |
| `caller_tenant` is `None` (dev mode / auth disabled) | ✅ Payload accepted verbatim |

This prevents any caller from self-asserting an arbitrary tenant scope in the JSON-RPC payload.

---

## Row-Level Security (RLS)

All `memory_entries`, `sync_state`, and `knowledge_items` tables have Postgres RLS policies enabled:

```sql
CREATE POLICY memory_entries_tenant_isolation ON memory_entries
  FOR ALL USING (tenant_id = current_setting('app.tenant_id', true)::text);
```

Hot-path connections (sync push/pull) call `set_config('app.tenant_id', <tenant>, false)` before executing queries, arming the RLS filter.  Explicit `WHERE tenant_id = $n` clauses are also present as belt-and-suspenders.

---

## Checklist: Hardening a New Endpoint

When adding a new tenant-scoped HTTP handler:

- [ ] Call `tenant_context_from_request` (sync/knowledge) or `authenticated_tenant_context` (org/team/user/tenant APIs) — never construct `TenantContext::default()` directly.
- [ ] When operating on the `memory_entries` or `sync_state` tables, acquire a connection and call `PostgresBackend::activate_tenant_context` before any query.
- [ ] Add a negative test: assert the handler returns 401 when `plugin_auth.enabled = true` and no bearer is present.
