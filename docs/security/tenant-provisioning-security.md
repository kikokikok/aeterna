# Tenant provisioning — security appendix

**Audience:** security reviewers, platform operators.
**Related:** [`../tenant-provisioning.md`](../tenant-provisioning.md) · [`rbac-matrix.md`](rbac-matrix.md) · spec: `openspec/changes/harden-tenant-provisioning/`.

This document covers the three surfaces that have a non-trivial security contract in the apply-based tenant pipeline: **secret input modes**, **token scopes**, and the **readiness contract**.

---

## 1. Secret input modes

The manifest model has **two distinct channels** for sensitive material, and the distinction matters:

| Channel | JSON field | Storage | Plaintext lifetime |
|---------|------------|---------|--------------------|
| **Bound secret**     | `secrets[]`                  | `tenant_config_secrets.ciphertext` (AEAD-encrypted, DEK derived from tenant root key) | Server holds plaintext only long enough to encrypt-at-rest; request-scoped `SecretBytes`. |
| **Secret reference** | `config.secretReferences{}`  | `tenant_config_secrets.reference` (the *pointer* — e.g. `k8s:aeterna/openai#api-key`)   | Resolved on every read through the resolver; server never persists plaintext at all. |

**Rule of thumb:** prefer references. They keep plaintext out of the database entirely. Bound secrets exist for material the operator genuinely wants Aeterna to be the source of truth for (e.g. per-tenant integration tokens that rotate from the Admin UI).

### 1.1 Supported reference kinds and environment gates

Not every resolver is safe in every deployment. Cluster config (`server.secrets.allowed_reference_kinds`) gates them:

| Kind         | Dev default | Prod default | Notes |
|--------------|:-----------:|:------------:|-------|
| `k8sSecret`  | off (no k8s) | **on**      | Only usable when the server runs in-cluster with a ServiceAccount that has `get` on the target Secret. |
| `file`       | **on**      | off         | Path is resolved on the server host; gated off in prod because pod-level file access is a narrower audit boundary than k8s RBAC. |
| `env`        | **on**      | off         | Single-node only. Process environment is shared across requests; do not use in multi-tenant prod. |
| `inline`     | **on** (tests) | off      | For bootstrap/tests. Writing plaintext inside a manifest defeats the point of the reference channel; the validator emits a warning when it sees one. |

When a gated kind is used, `/provision` returns `400 unsupported_secret_reference_kind` with the kind name and the config key that disabled it.

### 1.2 Plaintext handling invariants

- `mk_core::SecretBytes` is the only type allowed to cross function boundaries for plaintext. Its `Debug` impl is redacted; `Serialize` is not implemented, so plaintext cannot accidentally round-trip through JSON.
- The manifest parser deserialises `secrets[].secretValue` **directly** into `SecretBytes`; it never exists as a `String` in process memory.
- `ManifestSecret`, `ManifestProvider`, and any downstream types that carry resolved plaintext do **not** implement `Clone` or `Serialize`. See `cli/src/server/tenant_api.rs`.
- Structured audit events (`TenantAuditEvent::Provisioned`, etc.) carry the **canonical hash** of the manifest and the list of logical secret names touched — never values.

### 1.3 Rotation

Rotation is just another apply: supply the new bytes under the same `logicalName`, `apply`. The server emits a `TenantAuditEvent::SecretRotated` with the old and new ciphertext identifiers (content-hash-addressed) so the audit trail ties the rotation to the manifest generation that caused it.

To rotate a `secretReferences` target (e.g. the k8s Secret behind a ref), rotate out-of-band; the reference string does not change and the next tenant-config read picks up the new value transparently.

---

## 2. Token scopes

### 2.1 Who can call `/provision`

`POST /api/v1/admin/tenants/provision` accepts **two principal kinds**:

| Principal      | Check                                                 | Typical caller |
|----------------|-------------------------------------------------------|----------------|
| **User**       | Cedar role check: must hold `PlatformAdmin` or `TenantAdmin` on the tenant | Admin UI, interactive CLI |
| **Service**    | JWT must carry `tenants:provision` in the `scopes` claim | CI / GitOps bots, Terraform, ArgoCD |

Either gate is sufficient. A user principal bypasses the scope check (Cedar role is authoritative for users); a service principal bypasses the role check (it has no user identity).

### 2.2 Scope vocabulary

The complete scope list for tenant lifecycle operations:

| Scope                  | Endpoints |
|------------------------|-----------|
| `tenants:read`         | `GET /admin/tenants`, `GET /admin/tenants/{slug}`, `GET /admin/tenants/{slug}/render` |
| `tenants:provision`    | `POST /admin/tenants/provision` |
| `tenants:validate`     | `POST /admin/tenants/validate` |
| `tenants:diff`         | `POST /admin/tenants/{slug}/diff` |
| `tenants:watch`        | `GET /admin/tenants/{slug}/watch` (SSE) |
| `connections:manage`   | `POST/DELETE /admin/git-connections/{id}/tenants/{slug}` |

Scopes are **additive**: a token that grants `tenants:provision` does not automatically grant `tenants:read`. Mint tokens with the minimum set required.

### 2.3 Mint → use → revoke lifecycle

```bash
# Platform admin mints a scoped service token:
curl -XPOST $SERVER/api/v1/admin/service-tokens \
  -H "Authorization: Bearer $ADMIN_USER_TOKEN" \
  -d '{
    "name":   "gitops-acme",
    "scopes": ["tenants:read", "tenants:diff", "tenants:provision"],
    "ttl_seconds": 3600
  }'

# Response includes {token_type:"Bearer", access_token:"…"}. The access_token is
# a JWT with "token_type":"service" and "scopes":["tenants:provision", …] as
# first-class claims, validated on every request by the scope middleware.
```

### 2.4 Middleware enforcement

Each protected handler calls, in order:

1. `service_token_validator::validate_service_token_from_headers(headers)` → `Option<ServicePrincipal>`
2. If `Some(principal)`: `require_capability(principal, "<scope>")?` — returns `403 insufficient_scope` on miss
3. If `None` (user call): existing Cedar `authorise(user, "<action>", resource)` path

Validator state (principal + scopes + revocation flag) is cached in Redis with a short TTL; mint and revoke operations warm/evict the cache eagerly (see `mint_handler`, `revoke_handler` in `server::auth::service_tokens`).

### 2.5 Audit

Every scoped call produces an audit event that carries:

- The **principal kind** (`user` | `service`) and identifier
- The **scope actually used** (not just the scopes the token held)
- The **canonical manifest hash** of the applied document
- The **generation** before and after

This makes it trivial to answer "show me every tenant apply done by the `gitops-acme` bot in the last 24h" as a single query on `tenant_audit_events`.

---

## 3. Readiness contract

A successful `apply` is **not** a guarantee that every downstream system is serving the new state. The readiness contract makes this explicit.

### 3.1 Semantic guarantee of `apply`

When `POST /provision` returns `200`:

- The Postgres transaction has committed.
- `tenants`, `tenant_config_fields`, `tenant_config_secrets`, `tenant_repository_bindings`, and all hierarchy tables reflect the manifest.
- A `TenantAuditEvent::Provisioned` row with the new generation is visible.

What it does **not** guarantee:

- That in-process caches in other replicas have been invalidated.
- That background workers (indexer, embedding builders) have observed the new generation.
- That Git provider credentials have been verified against the live provider.

### 3.2 Per-step readiness events

`GET /admin/tenants/{slug}/watch` streams the following SSE event types in order:

| Event                      | Emitted when |
|----------------------------|--------------|
| `manifest.validated`       | Server-side validation passed |
| `transaction.committed`    | Postgres transaction succeeded (= `apply` returned 200) |
| `cache.invalidated`        | In-process tenant-config cache evicted on every replica (via Redis pub/sub `tenant.config.invalidated`) |
| `repo_binding.verified`    | Git provider auth verified against the live provider (best-effort — failure sets `warn` level) |
| `providers.ready`          | LLM / embedding / memory-layer factories re-initialised for the tenant |
| `indexer.generation_seen`  | Indexer worker observed the new generation and scheduled work if needed |
| `ready`                    | All required steps above reached terminal state |

The `ready` event is the one most callers actually want to wait on; CI can simply `aeterna tenant apply --watch --until ready` and treat timeout as failure.

### 3.3 Steps NOT in the readiness contract

By design:

- **Knowledge re-index completion** — large repositories can take hours; decoupled from apply readiness.
- **Downstream federated caches** (API gateway, edge workers) — their own staleness budget applies.
- **Role assignments propagating to sessions** — users with active sessions need a token refresh to pick up new roles.

Clients that need one of these should poll a dedicated endpoint, not the readiness stream.

### 3.4 Timeouts and retry

- `apply` itself has a server-side 60s timeout on the transaction. Longer than that and the request fails with `504 provision_timeout` — the transaction is either committed or rolled back; caller inspects `generation` in `render` to disambiguate.
- `watch` holds the connection for up to 10 minutes; clients should reconnect with a `since` cursor on timeout.
- **`apply` is not idempotent on identical content**: re-applying the same manifest bumps `generation` and emits a new audit row, but makes no DB change beyond that (the canonical hash is identical — a noop diff). Callers relying on strict idempotence should supply `metadata.generation` and expect `409 generation_stale` on collision.
