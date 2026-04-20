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

### D5 — Eager provider wiring at startup

On boot, after DB is up:
1. Query all persisted tenants.
2. For each tenant, set `TenantRuntimeState::Loading`.
3. Resolve the manifest's provider declarations (memory layers, LLM, embedding). Resolve secret references via `SecretBackend::get`.
4. Register providers into `MemoryManager` via `register_provider`.
5. On success: `TenantRuntimeState::Available`. On failure: `TenantRuntimeState::LoadingFailed { reason }`. Never `panic`.

`/ready` stays `503` until **every** tenant is `Available` or `LoadingFailed`. `Loading` → `503`. This ensures the pod does not accept traffic until tenant wiring is deterministic.

Tenant-scoped request paths check `TenantRuntimeState` for the target tenant. `LoadingFailed` or unknown → `503 tenant_unavailable`.

Late-created tenants (provisioned after boot) transition `Loading` → `Available` inside `provision_tenant` itself.

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
