# Design — harden-tenant-provisioning

**Status:** B1 scope locked 2026-04-20
**Target branch:** `harden-tenant-provisioning-b1`

## Decisions

### D1 — SecretReference is a sum type

Replace the flat `TenantSecretReference` struct in `mk_core/src/types.rs` with a tagged enum. Hard cut. No live data to migrate.

```rust
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SecretReference {
    /// Encrypted blob stored in our own Postgres (default path)
    Postgres { secret_id: Uuid },
}
```

Only one variant in B1. The enum shape is there so future variants (`External { provider, id }`, etc.) land as additive PRs without touching serialization of existing data.

### D2 — Secret storage: Postgres with KMS-wrapped DEK envelope encryption

Schema (migration in B1):

```sql
CREATE TABLE tenant_secrets (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    logical_name   TEXT NOT NULL,
    kms_key_id     TEXT NOT NULL,      -- CMK that wrapped the DEK
    wrapped_dek    BYTEA NOT NULL,     -- KMS(EncryptData, plaintext=DEK, keyId=CMK)
    ciphertext     BYTEA NOT NULL,     -- AES-256-GCM(DEK, secret_bytes)
    nonce          BYTEA NOT NULL,     -- 12 bytes
    generation     BIGINT NOT NULL DEFAULT 1,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, logical_name)
);
CREATE INDEX idx_tenant_secrets_tenant ON tenant_secrets(tenant_id);
```

**Envelope encryption** (industry standard):
1. Generate a random 32-byte DEK per write.
2. Encrypt secret bytes with DEK using AES-256-GCM → `ciphertext` + `nonce`.
3. Call KMS `Encrypt(plaintext=DEK, keyId=CMK)` → `wrapped_dek`.
4. Persist `wrapped_dek`, `ciphertext`, `nonce`, `kms_key_id`.
5. On read: KMS `Decrypt(wrapped_dek)` → DEK, then AES-GCM decrypt.
6. DEK lives only in memory for the duration of the request, zeroized via `zeroize` crate.

Rationale: one KMS round-trip per secret write, cheap decrypt on read, CMK rotation is a no-op for existing rows (decrypt with old key, re-encrypt the DEK with new key out-of-band if ever needed).

### D3 — KmsProvider trait, AWS and Local impls only

```rust
#[async_trait]
pub trait KmsProvider: Send + Sync {
    fn key_id(&self) -> &str;
    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes>;
}
```

`SecretBytes` wraps `Vec<u8>` with `zeroize::Zeroizing` and a `Debug` impl that prints `"<redacted>"`. Never logged. Never serialized.

**Implementations in B1:**
- `AwsKmsProvider` — `aws-sdk-kms` crate, uses default credential provider chain (AK/SK env, IRSA web identity, etc. — zero code-level choice).
- `LocalKmsProvider` — AES-256-GCM with key from env `AETERNA_LOCAL_KMS_KEY` (base64). Emits a `WARN` on startup. For dev only.

GCP, Azure, OpenBao: not present in code. New backend = ~150-line PR implementing the trait.

### D4 — Unified `SecretBackend` (kills the parallel secret systems)

Replace both:
- `storage/src/secret_provider.rs` (git tokens, AWS/Vault stubs)
- `storage/src/tenant_config_provider.rs::KubernetesTenantConfigProvider` (in-memory HashMap)

with one:

```rust
#[async_trait]
pub trait SecretBackend: Send + Sync {
    async fn put(&self, tenant_id: TenantId, logical_name: &str, value: SecretBytes) -> Result<SecretReference>;
    async fn get(&self, reference: &SecretReference) -> Result<SecretBytes>;
    async fn delete(&self, reference: &SecretReference) -> Result<()>;
    async fn list(&self, tenant_id: TenantId) -> Result<Vec<(String, SecretReference)>>;
}
```

Concrete impl in B1: `PostgresSecretBackend { pool: PgPool, kms: Arc<dyn KmsProvider> }`. The git-provider-connection store migrates to this backend with `logical_name = format!("git_token:{}", connection_id)`.

### D5 — Eager provider wiring, multi-pod, hot-swap on provision (resolves 0.2)

**Resolves decision 0.2** (`tasks.md`): lazy vs eager. The answer is **eager by default with three triggers**, accounting for multi-replica deployments and the "new tenant must be usable immediately after UI/CLI provisioning, with no pod restart" requirement.

#### Trigger matrix

| Trigger | When it fires | Which pods |
|---|---|---|
| **Boot loop** | `main()` after Postgres + vector + `SecretBackend` are up | The pod that just started |
| **Pub/sub** `tenant:changed` | In-process immediately after `provision_tenant` / `update_tenant` / `delete_tenant` commits to Postgres | Every *other* replica, via `SUBSCRIBE` |
| **Lazy fallback on registry miss** | Tenant-scoped request arrives on a pod whose registry does not know the tenant, but the tenant exists in Postgres | The pod handling that one request |

Default path is **eager**: boot-loop on pod start, pub/sub on provision/update. The lazy fallback exists only to close the race window between `COMMIT` on Pod A and the `SUBSCRIBE` callback firing on Pod B (milliseconds), and to self-heal a pod whose subscriber connection was flapping when a notification was emitted.

#### Pub/sub transport: Dragonfly (Redis-compatible), not Postgres LISTEN

The chart already ships Dragonfly (`charts/aeterna-prereqs/templates/dragonfly.yaml`) and the codebase already uses it for distributed locks, embedding/reasoning caches, and governance event publishing (`tools/src/redis_publisher.rs`). Reusing the existing Redis pub/sub channel for tenant invalidation is strictly better than introducing a parallel coordination plane via Postgres `LISTEN/NOTIFY`:

- one coordination plane for all cross-pod signalling,
- no long-lived `LISTEN` pgbouncer-slot cost,
- an existing publisher pattern (`tools/src/redis_publisher.rs`) and subscriber HA story to reuse,
- decouples invalidation from the write transaction in the failure direction we want: a flapping Dragonfly delays invalidation but does not block writes; lazy fallback catches stragglers.

Channel: `tenant:changed`. Message: `{ "slug": "<slug>", "rev": <generation>, "op": "upsert" | "delete" }`.

#### Wiring pipeline (shared by all three triggers)

Given a tenant slug, the wiring function:

1. Set `TenantRuntimeState::Loading { since }` in the pod-local registry.
2. Read the tenant row from Postgres (source of truth — do **not** trust a cache for this).
3. Resolve every `SecretReference` via `SecretBackend::get()` → KMS unwrap → decrypted `SecretBytes` held in pod memory, zeroize-on-drop.
4. Instantiate LLM + embedding HTTP clients (with connection pools, rate-limit tokens).
5. Register providers into `MemoryManager::register_provider`.
6. On success: `TenantRuntimeState::Available { rev, wired_at }`. On failure: `TenantRuntimeState::LoadingFailed { reason, last_attempt_at, retry_count }`.
7. On rewire for an existing tenant: swap providers atomically (new instance built first, then swapped into the registry under a write lock, then old instance's clients dropped). Never a window where the tenant has no providers registered.

All wiring runs on a `tokio::task::JoinSet` bounded by `Semaphore(AETERNA_TENANT_WIRE_CONCURRENCY)` (default 16). Each tenant has a per-wiring deadline of `AETERNA_TENANT_WIRE_TIMEOUT_SECONDS` (default 30). Timeout → `LoadingFailed { reason: "wire_timeout" }`.

#### Failure policy: per-tenant, not per-pod

A `LoadingFailed` tenant **does not** prevent the pod from serving traffic for other tenants. It does:

- make `/ready` return `503` only while **any** tenant is in `Loading` (not `LoadingFailed`),
- make tenant-scoped routes (`/api/v1/memory/*`, `/api/v1/tenants/{slug}/*`) for that specific tenant return `503 {error: "tenant_unavailable", slug, reason}`,
- fire `aeterna_tenant_state{state="loading_failed"}` for alerting,
- trigger a background re-wire loop with exponential backoff (1m, 5m, 15m, 1h, then hourly) until `Available` or the tenant is deleted.

Strict mode — "any `LoadingFailed` tenant fails the pod" — is available via `AETERNA_TENANT_WIRE_STRICT=true` for environments that want one-broken-tenant-blocks-the-pod semantics. Default is off.

#### Provision flow (resolves "new tenant usable immediately")

```
UI/CLI → Pod A: POST /api/v1/admin/tenants/provision
Pod A:
  1. validate manifest, BEGIN tx
  2. INSERT INTO tenants (+ tenant_secrets rows via SecretBackend.put)
  3. COMMIT
  4. redis_publisher.publish("tenant:changed", {slug, rev, op: "upsert"})
  5. wire(slug) synchronously            ← caller waits for this
  6. 200 OK with tenant summary
Pod B, Pod C (subscribed to tenant:changed):
  on recv → wire(slug) in background
```

Acceptance criterion: the moment Pod A returns `200`, the tenant is usable on Pod A. Within the pub/sub fan-out window (sub-second on a healthy Dragonfly), it is usable on every pod. The lazy fallback handles the race window on a per-request basis so **no user-visible 500 can occur** for a freshly-provisioned tenant, on any pod, ever.

No pod restart. No operator action. No warm-up delay the user can perceive.

#### Update / delete flows

`PUT /tenants/{slug}` publishes `tenant:changed {op: "upsert"}` → every pod re-runs wire(slug), atomically swapping providers.

`DELETE /tenants/{slug}` publishes `tenant:changed {op: "delete"}` → every pod drops the tenant from its registry, zeroizes in-memory secrets, drops HTTP client pools. Subsequent requests for the deleted tenant hit "tenant not found" in Postgres and return `404`.

#### Configuration surface

```
AETERNA_TENANT_WIRE_CONCURRENCY=16         # bounded parallelism on boot
AETERNA_TENANT_WIRE_TIMEOUT_SECONDS=30     # per-tenant deadline
AETERNA_TENANT_WIRE_STRICT=false           # strict mode: LoadingFailed fails the pod
AETERNA_TENANT_REWIRE_BACKOFF_MIN=60       # retry floor, seconds
AETERNA_TENANT_REWIRE_BACKOFF_MAX=3600     # retry ceiling, seconds
AETERNA_REDIS_CHANNEL_TENANT_CHANGED=tenant:changed  # overridable for tests
```

#### Observability

- `aeterna_tenant_state{slug, state}` — gauge, state ∈ {loading, available, loading_failed}
- `aeterna_tenant_wiring_duration_seconds{slug, trigger, outcome}` — histogram, trigger ∈ {boot, pubsub, lazy, rewire}
- `aeterna_tenant_wiring_failures_total{slug, reason}` — counter
- `aeterna_tenant_rewire_attempts_total{slug}` — counter
- `aeterna_tenant_pubsub_lag_seconds` — histogram, time from `PUBLISH` to `SUBSCRIBE` handler entry

Default alert: any tenant in `loading_failed` for > 5 minutes.

#### Why not pure lazy

Pure lazy technically meets "new tenant usable immediately" because the first request wires on demand. It fails on two non-negotiables:

1. A misconfigured manifest (bad secret reference, missing provider, unreachable LLM) is detected only when a user hits the tenant. The `POST /provision` call returns `200` on an unwireable tenant. Eager wiring inside the handler surfaces this synchronously and returns `422`.
2. `/ready` cannot honestly report cluster health for tenants. Every rolling restart lets the LB route traffic to a pod that has never touched any tenant, producing cold-start 500s across the fleet.

#### Why not eager without pub/sub

"Just wire at boot" solves the restart case but not the "new tenant without restart" case. Without cross-pod invalidation, Pod A knows about new tenant `acme` but Pod B does not, and the UI redirect lands on Pod B and 500s. Pub/sub closes this.

#### Why not pure pub/sub without lazy fallback

Pub/sub has an unavoidable race window between `COMMIT` on Pod A and the subscriber callback firing on Pod B (typically milliseconds). A UI redirect firing inside that window would hit Pod B before its wire completes. Lazy fallback on-miss closes this at the cost of one ~100ms request the first time. Without the fallback, the "zero user-visible 500s" invariant cannot be guaranteed.

### D6 — Manifest hash-based idempotent re-apply

Canonical form: JSON with sorted keys, inline `secretValue` stripped, references preserved. SHA-256, hex-encoded, prefixed `sha256:`.

Tenant row gains `last_applied_manifest_hash TEXT` and `generation BIGINT NOT NULL DEFAULT 0`.

`provision_tenant` flow:
1. Validate schema + references.
2. Compute `new_hash`.
3. If `new_hash == last_applied_manifest_hash` → no-op, return `{status: "unchanged", generation: current}`.
4. Enforce `manifest.metadata.generation > current` (strict monotonic, unless absent — then treat as `current + 1`).
5. Apply. On success, persist `new_hash` and `generation`.

### D7 — Helm chart config for KMS

Adds a `kms` section to `deploy/helm/aeterna/values.yaml` and templates that:
- Support `provider: aws | local`
- Support AWS credential modes `static` (K8s Secret → env) and `irsa` (SA annotation `eks.amazonaws.com/role-arn`)
- Never echo secret values into ConfigMaps or annotations

Migration AK/SK → IRSA is chart-values-only, no code change (AWS SDK default chain handles both).

### D8 — Top-level `tenant validate`, subsumes nested validates

Delete `tenant repo-binding validate` and `tenant config validate`. `tenant validate -f manifest.yaml` becomes the single validation path.

## Out of scope for B1

- `dryRun`, `diff`, `render` endpoints — B2
- CLI `apply/render/diff/watch` commands — B3
- Scoped tokens — B4
- Admin UI wizard, acceptance matrix, docs — B5+
- Any `SecretReference` variant other than `Postgres`
- GCP/Azure/OpenBao KMS
- External secret managers (`AwsSecretsManager`, etc.)
- Inline-secret gating (until CLI refactor in B3, inline is permitted at `provision` with a server-log WARN)

## Non-goals

- Rotating the CMK automatically. Operator responsibility. Our schema stores `kms_key_id` per row so a future rotation job can re-wrap DEKs.
- Revoking already-issued secret values from memory of long-lived processes. `SecretBytes` zeroizes on drop, but we do not track references.
- Protecting against a compromised operator with DB + KMS access. If they have both, they have everything.

## Risk register

| Risk | Mitigation |
|------|------------|
| AWS SDK cold-start latency on first decrypt | KMS client built once at startup, connection pooled by SDK |
| DB encrypted blobs + lost CMK = data loss | Operator runbook: AWS CMK deletion requires 7-30 day scheduled deletion; treat as unrecoverable by design |
| Git-token migration breaks existing connections on deploy | Migration runs in same transaction as schema change; rollback script in migration `down` |
| In-memory `TenantRuntimeState` desyncs across replicas | Each replica computes independently from the same DB; no shared state needed |
| `/ready` blocks forever if one tenant has unreachable providers | `LoadingFailed` is terminal, not retried — readiness completes, the tenant just returns 503. Operator fixes manifest + retries via `provision`. |
