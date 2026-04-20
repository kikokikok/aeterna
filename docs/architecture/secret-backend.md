# Secret Backend & KMS Architecture

> Status: implemented in [PR #94](https://github.com/kikokikok/aeterna/pull/94) / [PR #95](https://github.com/kikokikok/aeterna/pull/95) / [PR #96](https://github.com/kikokikok/aeterna/pull/96) (harden-tenant-provisioning B1). Design doc: [`openspec/changes/harden-tenant-provisioning/design.md`](https://github.com/kikokikok/aeterna/blob/master/openspec/changes/harden-tenant-provisioning/design.md).

Aeterna stores tenant-scoped secrets — OpenAI keys, GitHub tokens, embedding-provider credentials — in Postgres with **envelope encryption**. Every row carries its own data encryption key (DEK), and the DEK is wrapped by a KMS-held customer master key (CMK).

This page is the concept reference for the three layers that make it work: `SecretBytes`, `KmsProvider`, `SecretBackend`. For operator-facing configuration see the [Helm KMS guide](../helm/kms.md); for runbooks see [Secret Rotation](../guides/secret-rotation.md).

## Why envelope encryption

The naive alternative — encrypting every secret directly with a KMS `Encrypt` call — couples every read and write to a round-trip to the KMS. Envelope encryption decouples that:

- The DEK is a one-time 32-byte symmetric key, generated per row.
- Plaintext is sealed with AES-256-GCM using the DEK.
- The DEK itself is wrapped by the CMK (one `Encrypt` call at write time, one `Decrypt` call at read time).
- Rotating the CMK rewraps only the DEKs, never the ciphertext rows. Rotating a DEK re-encrypts only its row.

Operational properties that fall out:

| Property | Why it matters |
|---|---|
| CMK rotation is O(#rows) of network calls, but zero row rewrites | KMS rotates without touching Postgres |
| A compromised DEK leaks exactly one secret | Blast radius bounded per-row |
| AWS KMS `decrypt()` routes from the ciphertext blob's embedded key hint | We can retire old CMK versions without orphaning rows |
| `PostgresSecretBackend` is the only code that sees plaintext DEKs; they are zeroized on both paths | Memory hygiene |

## Layer 1 — `SecretBytes` + `SecretReference` (`mk_core`)

```rust
pub struct SecretBytes(Vec<u8>);          // zeroize-on-drop, redacted Debug/Display/Serialize

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SecretReference {
    Postgres { secret_id: Uuid },
}
```

- `SecretBytes` is the in-memory carrier for *plaintext* secret material. Its `Debug`, `Display`, and `serde::Serialize` impls emit `<redacted>` so the material cannot leak through `tracing`, `format!`, or a JSON API response.
- `SecretReference` is a **tagged enum** so future backends (cloud secret managers, Vault, etc.) land as additive variants. B1 ships only `Postgres`.

Refer to `mk_core/src/secret.rs` for the full surface (constant-time `PartialEq`, `expose()`, `into_bytes()`).

## Layer 2 — `KmsProvider` (`storage::kms`)

```rust
#[async_trait]
pub trait KmsProvider: Send + Sync {
    fn key_id(&self) -> &str;
    async fn wrap(&self, dek: &[u8])       -> Result<Vec<u8>, KmsError>;
    async fn unwrap(&self, wrapped: &[u8]) -> Result<SecretBytes, KmsError>;
}
```

Two implementations:

| Impl | Backing | Use |
|---|---|---|
| `AwsKmsProvider` | `aws-sdk-kms` against a real CMK ARN | **Production.** Default credential chain handles static AK/SK, IRSA, and EC2 instance profile transparently. |
| `LocalKmsProvider` | AES-256-GCM with a key from `AETERNA_LOCAL_KMS_KEY` (base64 32 bytes) | **Dev / CI / DR drill only.** Emits `WARN: LocalKmsProvider in use …` on every encrypt and decrypt. |

Design note (D4 in the design doc): `AwsKmsProvider::unwrap` deliberately **omits** `key_id`. AWS KMS routes decryption from the ciphertext blob itself, so rotating the CMK does not orphan existing rows.

## Layer 3 — `SecretBackend` (`storage::secret_backend`)

```rust
#[async_trait]
pub trait SecretBackend: Send + Sync + 'static {
    async fn put   (&self, tenant_db_id: Uuid, logical_name: &str, value: SecretBytes)
                        -> Result<SecretReference, SecretBackendError>;
    async fn get   (&self, reference: &SecretReference)  -> Result<SecretBytes, SecretBackendError>;
    async fn delete(&self, reference: &SecretReference)  -> Result<(), SecretBackendError>;
    async fn list  (&self, tenant_db_id: Uuid)           -> Result<Vec<(String, SecretReference)>, SecretBackendError>;
}
```

Two implementations:

- `PostgresSecretBackend` — envelope-encrypted storage in the `tenant_secrets` table. On `put`, it generates a fresh 32-byte DEK, seals the value with AES-256-GCM (12-byte nonce), wraps the DEK with the configured `KmsProvider`, and upserts. On upsert conflict `(tenant_id, logical_name)`, the row is re-encrypted with a fresh DEK and `generation` is bumped — the `SecretReference` stays stable.
- `InMemorySecretBackend` — `HashMap`-backed, for unit tests only.

### `tenant_secrets` table (migration 026)

```
tenant_secrets(
  id             UUID PK,
  tenant_id      UUID FK → tenants ON DELETE CASCADE,
  logical_name   TEXT,
  kms_key_id     TEXT,          -- which CMK wrapped wrapped_dek
  wrapped_dek    BYTEA,
  ciphertext     BYTEA,         -- AES-256-GCM sealed plaintext
  nonce          BYTEA,         -- 12-byte per-row GCM nonce
  generation     INTEGER,       -- bumped on envelope rotation
  created_at     TIMESTAMPTZ,
  updated_at     TIMESTAMPTZ    -- BEFORE UPDATE trigger
)
UNIQUE (tenant_id, logical_name);
```

## Bootstrap — `build_secret_backend_from_env`

```rust
let backend: Arc<dyn SecretBackend> =
    storage::secret_backend::build_secret_backend_from_env(pool).await?;
```

Reads `AETERNA_KMS_PROVIDER` and returns a ready-to-use `PostgresSecretBackend`:

| `AETERNA_KMS_PROVIDER` | Additional env | Behaviour |
|---|---|---|
| `aws` | `AETERNA_KMS_AWS_KEY_ARN` (required) + `AWS_*` or IRSA role | `AwsKmsProvider` against the CMK |
| `local` (default) | `AETERNA_LOCAL_KMS_KEY` (base64 32 bytes) | `LocalKmsProvider`, emits `WARN` on every use |
| *anything else* | — | Falls through to `local` (will tighten in B2) |

In tests, inject an `InMemorySecretBackend` directly instead of calling the helper.

## Where this sits in the stack

```
  aeterna-cli (bootstrap)                ─┐
  admin_ui routes (PUT /tenants/:id/…)   ─┼──▶ TenantConfigProvider
  knowledge / memory consumers           ─┘          │
                                                     ▼
                                       ┌─────────────────────────────┐
                                       │  SecretBackend              │  (put / get / delete / list)
                                       │   └── PostgresSecretBackend │
                                       └──────────────┬──────────────┘
                                                      │
                                           ┌──────────┴──────────┐
                                           ▼                     ▼
                                   AES-256-GCM row       KmsProvider
                                   in tenant_secrets    (AwsKms | LocalKms)
```

## See also

- [Helm KMS configuration](../helm/kms.md) — operator values reference, IRSA walkthrough
- [Secret Rotation runbook](../guides/secret-rotation.md) — CMK + local-key rotation, DR drill
- `openspec/changes/harden-tenant-provisioning/design.md` — design decisions D1–D8
- Source: [`mk_core/src/secret.rs`](https://github.com/kikokikok/aeterna/blob/master/mk_core/src/secret.rs), [`storage/src/kms/`](https://github.com/kikokikok/aeterna/tree/master/storage/src/kms), [`storage/src/secret_backend.rs`](https://github.com/kikokikok/aeterna/blob/master/storage/src/secret_backend.rs)
