//! Unified secret backend: storage-agnostic API for tenant secret material.
//!
//! [`SecretBackend`] is the single trait every call site now uses to read,
//! write, list, and delete tenant secrets. Before this module existed, the
//! codebase had two parallel systems:
//!
//! - `storage::secret_provider` — git-token-focused, with stub AWS / Vault
//!   backends that were never wired.
//! - `storage::tenant_config_provider::KubernetesTenantConfigProvider` —
//!   an in-memory `HashMap` misleadingly named "Kubernetes", which forgot
//!   every secret on pod restart.
//!
//! The concrete implementation shipped here is [`PostgresSecretBackend`]:
//! envelope-encrypted rows in the `tenant_secrets` table, with the DEK
//! wrapped by a [`KmsProvider`] (AWS KMS in production, local AES-256-GCM
//! for dev). See `openspec/changes/harden-tenant-provisioning/design.md`
//! decisions D2 and D4.
//!
//! # Why a single trait
//!
//! Future alternate backends (on-prem Vault, GCP Secret Manager, etc.) land
//! as additive `impl SecretBackend` without touching any call site. The one
//! reference type the public API exposes — [`mk_core::SecretReference`] — is
//! a tagged enum with the same extensibility property for stored references.

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use async_trait::async_trait;
use mk_core::{Environment, SecretBytes, SecretReference};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::kms::{KmsError, KmsProvider};

/// Build a production-ready [`SecretBackend`] from env configuration.
///
/// The selector lives in `AETERNA_KMS_PROVIDER`:
///
/// - `local` (default) — [`crate::kms::LocalKmsProvider`] seeded from
///   `AETERNA_LOCAL_KMS_KEY` (base64-encoded 32 bytes). Logs a WARN on every
///   encrypt/decrypt and is intended for dev / CI only.
/// - `aws` — [`crate::kms::AwsKmsProvider`] targeting the CMK ARN in
///   `AETERNA_KMS_AWS_KEY_ARN`. Uses the default AWS credential chain
///   (static AK/SK, IRSA, or instance profile).
///
/// # Production safety gate (A4)
///
/// When [`Environment::from_env`] reports
/// [`Environment::Production`] (i.e. `AETERNA_ENV=production`), the constructed
/// KMS provider must report [`KmsProvider::is_production_grade`] = `true`. If
/// not — for example, a stray `AETERNA_KMS_PROVIDER=local` slipped through into
/// the prod Helm values — startup fails with
/// [`SecretBackendError::ProductionSafety`] *before* any tenant-bearing
/// request can be served. This eliminates the "dev fallback reaching prod"
/// failure mode flagged in rc.9 §6.
pub async fn build_secret_backend_from_env(
    pool: PgPool,
) -> Result<Arc<dyn SecretBackend>, SecretBackendError> {
    let env = Environment::from_env();
    let selector = std::env::var("AETERNA_KMS_PROVIDER").unwrap_or_else(|_| "local".to_string());

    let kms: Arc<dyn KmsProvider> = match selector.to_ascii_lowercase().as_str() {
        "aws" => {
            let arn = std::env::var("AETERNA_KMS_AWS_KEY_ARN")
                .map_err(|_| SecretBackendError::UnsupportedReference("AETERNA_KMS_AWS_KEY_ARN"))?;
            Arc::new(crate::kms::AwsKmsProvider::new(arn).await?)
        }
        // Anything else falls through to Local — including the empty string,
        // which is how local-dev unit tests instantiate this.
        _ => Arc::new(crate::kms::LocalKmsProvider::from_env()?),
    };

    // Production safety gate. Refuse to boot a non-production-grade KMS
    // provider in a production environment. This is a startup-time check
    // (loud, fail-fast) rather than a per-request guard so the failure
    // signal surfaces in deployment health checks immediately.
    let kms = enforce_production_safety_gate(env, &selector, kms)?;

    // B2 startup self-test: probe encrypt+decrypt before the first tenant
    // secret request lands. Catches wrong ARN / expired creds / unreachable
    // endpoint at boot rather than at first-use. Cheap (1 encrypt + 1 decrypt)
    // and surfaces in deployment health checks the same way the prod gate does.
    kms.self_test().await?;

    Ok(Arc::new(PostgresSecretBackend::new(pool, kms)))
}

/// Enforce the production safety gate on a freshly-constructed KMS provider.
///
/// Pure function — no env lookups, no I/O — so it can be unit-tested without
/// a Postgres pool. [`build_secret_backend_from_env`] is the only intended
/// caller.
///
/// Returns the input handle unchanged on success, or
/// [`SecretBackendError::ProductionSafety`] when:
///
/// - `env` is [`Environment::Production`], **and**
/// - the provider reports [`KmsProvider::is_production_grade`] = `false`.
fn enforce_production_safety_gate(
    env: Environment,
    selector: &str,
    kms: Arc<dyn KmsProvider>,
) -> Result<Arc<dyn KmsProvider>, SecretBackendError> {
    if env.is_production() && !kms.is_production_grade() {
        return Err(SecretBackendError::ProductionSafety {
            selector: selector.to_ascii_lowercase(),
            env: env.to_string(),
        });
    }
    Ok(kms)
}

/// Errors raised by any [`SecretBackend`] implementation.
#[derive(Debug, Error)]
pub enum SecretBackendError {
    /// The underlying datastore (Postgres, etc.) returned an error.
    #[error("storage error: {0}")]
    Storage(#[from] sqlx::Error),

    /// The configured KMS provider failed to wrap or unwrap a DEK.
    #[error("kms error: {0}")]
    Kms(#[from] KmsError),

    /// AES-GCM sealing or opening failed. For `decrypt` this usually means
    /// the row was tampered with; for `encrypt` it is an unrecoverable
    /// cipher init failure.
    #[error("aead error: {0}")]
    Aead(String),

    /// The referenced secret does not exist.
    #[error("secret not found: {0}")]
    NotFound(String),

    /// A [`SecretReference`] variant was passed that this backend does not
    /// serve. Added when new variants land ahead of their backends.
    #[error("unsupported reference kind: {0}")]
    UnsupportedReference(&'static str),

    /// `AETERNA_ENV=production` was set, but the configured KMS provider
    /// reports `is_production_grade() = false`. The startup is aborted to
    /// prevent a developer key from protecting production tenant secrets.
    /// Set `AETERNA_KMS_PROVIDER=aws` (or another production-grade backend)
    /// or change the deployment environment.
    #[error(
        "production safety gate tripped: KMS provider '{selector}' is not production-grade but AETERNA_ENV={env}"
    )]
    ProductionSafety { selector: String, env: String },
}

/// Storage-agnostic API for tenant secret material.
///
/// Implementations MUST:
///
/// - Be `Send + Sync + 'static` so handles can be cloned across tasks.
/// - Never log or serialise secret material.
/// - Treat [`SecretBytes`] as opaque — rely only on its length, not its
///   contents, for control flow.
#[async_trait]
pub trait SecretBackend: Send + Sync + 'static {
    /// Upsert a secret for a tenant by logical name. On conflict the existing
    /// row is re-encrypted with a fresh DEK (envelope rotation) and its
    /// `generation` is bumped.
    async fn put(
        &self,
        tenant_db_id: Uuid,
        logical_name: &str,
        value: SecretBytes,
    ) -> Result<SecretReference, SecretBackendError>;

    /// Resolve a reference to the plaintext bytes. Callers MUST drop the
    /// returned [`SecretBytes`] as soon as the value is consumed.
    async fn get(&self, reference: &SecretReference) -> Result<SecretBytes, SecretBackendError>;

    /// Delete a secret by reference. Idempotent: deleting a non-existent
    /// reference is not an error.
    async fn delete(&self, reference: &SecretReference) -> Result<(), SecretBackendError>;

    /// List `(logical_name, reference)` pairs for the given tenant. Intended
    /// for admin UIs and manifest-render endpoints; callers MUST NOT dereference
    /// every entry just to render metadata.
    async fn list(
        &self,
        tenant_db_id: Uuid,
    ) -> Result<Vec<(String, SecretReference)>, SecretBackendError>;
}

/// Postgres-backed secret store using envelope encryption.
///
/// # Write path
///
/// 1. Generate 32 random DEK bytes from the OS CSPRNG.
/// 2. Encrypt the caller's `value` with AES-256-GCM using the DEK and a
///    fresh 12-byte nonce.
/// 3. Call `kms.encrypt(DEK)` to obtain `wrapped_dek`.
/// 4. Upsert the row `(tenant_id, logical_name, kms_key_id, wrapped_dek,
///    ciphertext, nonce, generation)`. On conflict, bump `generation` and
///    replace all cipher fields atomically.
///
/// # Read path
///
/// 1. Fetch the row by `id`.
/// 2. Call `kms.decrypt(wrapped_dek)` to recover the DEK.
/// 3. AES-GCM-decrypt `(nonce, ciphertext)` with the DEK.
/// 4. Zeroize the DEK on drop; hand [`SecretBytes`] back to the caller.
#[derive(Clone)]
pub struct PostgresSecretBackend {
    pool: PgPool,
    kms: Arc<dyn KmsProvider>,
}

impl PostgresSecretBackend {
    #[must_use]
    pub fn new(pool: PgPool, kms: Arc<dyn KmsProvider>) -> Self {
        Self { pool, kms }
    }

    fn generate_dek() -> [u8; 32] {
        use rand::RngCore;
        let mut dek = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut dek);
        dek
    }

    fn aead_seal(
        dek: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), SecretBackendError> {
        let key = Key::<Aes256Gcm>::from_slice(dek);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| SecretBackendError::Aead(format!("seal failed: {e}")))?;
        Ok((nonce.to_vec(), ct))
    }

    fn aead_open(
        dek: &[u8; 32],
        nonce_bytes: &[u8],
        ciphertext: &[u8],
    ) -> Result<SecretBytes, SecretBackendError> {
        let key = Key::<Aes256Gcm>::from_slice(dek);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(nonce_bytes);
        let pt = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| SecretBackendError::Aead(format!("open failed: {e}")))?;
        Ok(SecretBytes::from(pt))
    }
}

#[async_trait]
impl SecretBackend for PostgresSecretBackend {
    async fn put(
        &self,
        tenant_db_id: Uuid,
        logical_name: &str,
        value: SecretBytes,
    ) -> Result<SecretReference, SecretBackendError> {
        let dek = Self::generate_dek();
        let (nonce, ciphertext) = Self::aead_seal(&dek, value.expose())?;
        let wrapped_dek = self.kms.encrypt(&dek).await?;
        let kms_key_id = self.kms.key_id().to_string();

        // Zeroize the DEK now — we won't need it again.
        let mut dek_zero = dek;
        use zeroize::Zeroize;
        dek_zero.zeroize();

        let row = sqlx::query(
            r#"
            INSERT INTO tenant_secrets
                (tenant_id, logical_name, kms_key_id, wrapped_dek, ciphertext, nonce, generation)
            VALUES ($1, $2, $3, $4, $5, $6, 1)
            ON CONFLICT (tenant_id, logical_name) DO UPDATE
                SET kms_key_id  = EXCLUDED.kms_key_id,
                    wrapped_dek = EXCLUDED.wrapped_dek,
                    ciphertext  = EXCLUDED.ciphertext,
                    nonce       = EXCLUDED.nonce,
                    generation  = tenant_secrets.generation + 1
            RETURNING id
            "#,
        )
        .bind(tenant_db_id)
        .bind(logical_name)
        .bind(&kms_key_id)
        .bind(&wrapped_dek)
        .bind(&ciphertext)
        .bind(&nonce)
        .fetch_one(&self.pool)
        .await?;

        let id: Uuid = row.try_get("id")?;
        Ok(SecretReference::Postgres { secret_id: id })
    }

    async fn get(&self, reference: &SecretReference) -> Result<SecretBytes, SecretBackendError> {
        let secret_id = match reference {
            SecretReference::Postgres { secret_id } => secret_id,
            other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
        };
        let row = sqlx::query(
            r#"SELECT wrapped_dek, ciphertext, nonce FROM tenant_secrets WHERE id = $1"#,
        )
        .bind(secret_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| SecretBackendError::NotFound(secret_id.to_string()))?;

        let wrapped_dek: Vec<u8> = row.try_get("wrapped_dek")?;
        let ciphertext: Vec<u8> = row.try_get("ciphertext")?;
        let nonce: Vec<u8> = row.try_get("nonce")?;

        let dek_bytes = self.kms.decrypt(&wrapped_dek).await?;
        let dek_slice = dek_bytes.expose();
        if dek_slice.len() != 32 {
            return Err(SecretBackendError::Aead(format!(
                "unexpected DEK length after unwrap: {}",
                dek_slice.len()
            )));
        }
        let mut dek = [0u8; 32];
        dek.copy_from_slice(dek_slice);
        let out = Self::aead_open(&dek, &nonce, &ciphertext);
        // Zeroize the local DEK copy regardless of success/failure.
        use zeroize::Zeroize;
        dek.zeroize();
        out
    }

    async fn delete(&self, reference: &SecretReference) -> Result<(), SecretBackendError> {
        let secret_id = match reference {
            SecretReference::Postgres { secret_id } => secret_id,
            other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
        };
        sqlx::query("DELETE FROM tenant_secrets WHERE id = $1")
            .bind(secret_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list(
        &self,
        tenant_db_id: Uuid,
    ) -> Result<Vec<(String, SecretReference)>, SecretBackendError> {
        let rows = sqlx::query(
            r#"SELECT id, logical_name FROM tenant_secrets
               WHERE tenant_id = $1
               ORDER BY logical_name ASC"#,
        )
        .bind(tenant_db_id)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let logical_name: String = row.try_get("logical_name")?;
            out.push((logical_name, SecretReference::Postgres { secret_id: id }));
        }
        Ok(out)
    }
}

/// In-memory [`SecretBackend`] for tests and CLI fixtures.
///
/// Stores plaintext in a `Mutex<HashMap>` keyed by `(tenant_db_id,
/// logical_name) -> (secret_id, bytes)`. Issues `SecretReference::Postgres`
/// references so it is swap-in compatible with the production backend.
///
/// NOT for production use: values are not encrypted, nothing is persisted,
/// and the backend leaks memory for the lifetime of the process.
#[derive(Default)]
pub struct InMemorySecretBackend {
    inner: std::sync::Mutex<
        std::collections::HashMap<(Uuid, String), (Uuid, Vec<u8>)>, /* (tenant, logical) -> (secret_id, bytes) */
    >,
    by_id: std::sync::Mutex<std::collections::HashMap<Uuid, (Uuid, String)>>, /* secret_id -> (tenant, logical) */
}

impl InMemorySecretBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SecretBackend for InMemorySecretBackend {
    async fn put(
        &self,
        tenant_db_id: Uuid,
        logical_name: &str,
        value: SecretBytes,
    ) -> Result<SecretReference, SecretBackendError> {
        let mut map = self.inner.lock().expect("poisoned");
        let mut by_id = self.by_id.lock().expect("poisoned");
        let key = (tenant_db_id, logical_name.to_string());
        let secret_id = map
            .get(&key)
            .map(|(id, _)| *id)
            .unwrap_or_else(Uuid::new_v4);
        map.insert(key.clone(), (secret_id, value.expose().to_vec()));
        by_id.insert(secret_id, key);
        Ok(SecretReference::Postgres { secret_id })
    }

    async fn get(&self, reference: &SecretReference) -> Result<SecretBytes, SecretBackendError> {
        let secret_id = match reference {
            SecretReference::Postgres { secret_id } => secret_id,
            other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
        };
        let by_id = self.by_id.lock().expect("poisoned");
        let key = by_id
            .get(secret_id)
            .ok_or_else(|| SecretBackendError::NotFound(secret_id.to_string()))?
            .clone();
        drop(by_id);
        let map = self.inner.lock().expect("poisoned");
        let (_, bytes) = map
            .get(&key)
            .ok_or_else(|| SecretBackendError::NotFound(secret_id.to_string()))?;
        Ok(SecretBytes::new(bytes.clone()))
    }

    async fn delete(&self, reference: &SecretReference) -> Result<(), SecretBackendError> {
        let secret_id = match reference {
            SecretReference::Postgres { secret_id } => secret_id,
            other => return Err(SecretBackendError::UnsupportedReference(other.kind())),
        };
        let mut by_id = self.by_id.lock().expect("poisoned");
        if let Some(key) = by_id.remove(secret_id) {
            self.inner.lock().expect("poisoned").remove(&key);
        }
        Ok(())
    }

    async fn list(
        &self,
        tenant_db_id: Uuid,
    ) -> Result<Vec<(String, SecretReference)>, SecretBackendError> {
        let map = self.inner.lock().expect("poisoned");
        let mut out: Vec<(String, SecretReference)> = map
            .iter()
            .filter_map(|((tid, name), (sid, _))| {
                if *tid == tenant_db_id {
                    Some((name.clone(), SecretReference::Postgres { secret_id: *sid }))
                } else {
                    None
                }
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

#[cfg(test)]
mod gate_tests {
    //! Unit tests for the A4 production safety gate.
    //!
    //! These tests exercise [`enforce_production_safety_gate`] directly and
    //! therefore need no Postgres pool or live KMS — they use the two
    //! [`FakeKms`] variants below to model a production-grade and a
    //! non-production-grade provider.

    use super::*;
    use async_trait::async_trait;
    use mk_core::SecretBytes;

    /// Fake KMS that lets the test choose its `is_production_grade()` answer.
    /// `encrypt`/`decrypt` are unreachable so the gate tests assert the
    /// gate trips *before* any cipher work starts.
    struct FakeKms {
        production_grade: bool,
    }

    #[async_trait]
    impl KmsProvider for FakeKms {
        fn key_id(&self) -> &str {
            "fake-kms-key"
        }
        fn is_production_grade(&self) -> bool {
            self.production_grade
        }
        async fn encrypt(&self, _plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
            unreachable!("gate tests never call encrypt/decrypt")
        }
        async fn decrypt(&self, _ciphertext: &[u8]) -> Result<SecretBytes, KmsError> {
            unreachable!("gate tests never call encrypt/decrypt")
        }
    }

    fn fake(production_grade: bool) -> Arc<dyn KmsProvider> {
        Arc::new(FakeKms { production_grade })
    }

    /// B2 self-test fakes: deliberately broken KMS variants so we can
    /// assert the default `KmsProvider::self_test` impl catches each
    /// failure mode at boot.
    enum BrokenMode {
        EncryptFails,
        DecryptFails,
        Mismatches,
    }

    struct BrokenKms {
        mode: BrokenMode,
    }

    #[async_trait]
    impl KmsProvider for BrokenKms {
        fn key_id(&self) -> &str {
            "broken-kms"
        }
        async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
            match self.mode {
                BrokenMode::EncryptFails => Err(KmsError::Encrypt("simulated".into())),
                BrokenMode::DecryptFails => Ok(plaintext.to_vec()),
                // Returns a deterministic but wrong payload so decrypt
                // succeeds yet the round-trip mismatches.
                BrokenMode::Mismatches => Ok(b"wrong-bytes-from-encrypt".to_vec()),
            }
        }
        async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes, KmsError> {
            match self.mode {
                BrokenMode::DecryptFails => Err(KmsError::Decrypt("simulated".into())),
                _ => Ok(SecretBytes::from(ciphertext.to_vec())),
            }
        }
    }

    #[tokio::test]
    async fn self_test_passes_for_a_correct_kms() {
        // LocalKmsProvider is the canonical correct provider.
        let kms = crate::kms::LocalKmsProvider::from_bytes(&[0x55u8; 32], "self-test").unwrap();
        kms.self_test().await.expect("a correct KMS must self-test");
    }

    #[tokio::test]
    async fn self_test_surfaces_encrypt_failure_as_config_error() {
        let kms = BrokenKms {
            mode: BrokenMode::EncryptFails,
        };
        match kms.self_test().await {
            Err(KmsError::Config(msg)) => {
                assert!(msg.contains("self-test encrypt failed"), "msg={msg}");
                assert!(msg.contains("broken-kms"), "key id missing: {msg}");
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn self_test_surfaces_decrypt_failure_as_config_error() {
        let kms = BrokenKms {
            mode: BrokenMode::DecryptFails,
        };
        match kms.self_test().await {
            Err(KmsError::Config(msg)) => {
                assert!(msg.contains("self-test decrypt failed"), "msg={msg}");
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn self_test_catches_silent_corruption() {
        // The most insidious failure: encrypt and decrypt both succeed but
        // produce different bytes. The probe must catch this — otherwise
        // a tampered or aliased KMS would only blow up at first real read.
        let kms = BrokenKms {
            mode: BrokenMode::Mismatches,
        };
        match kms.self_test().await {
            Err(KmsError::Config(msg)) => {
                assert!(msg.contains("round-trip mismatch"), "msg={msg}");
            }
            other => panic!("expected mismatch error, got {other:?}"),
        }
    }

    #[test]
    fn gate_blocks_non_prod_kms_in_production() {
        // Note: cannot use `expect_err` because Arc<dyn KmsProvider> is !Debug.
        match enforce_production_safety_gate(Environment::Production, "local", fake(false)) {
            Err(SecretBackendError::ProductionSafety { selector, env }) => {
                assert_eq!(selector, "local");
                assert_eq!(env, "production");
            }
            Err(other) => panic!("wrong error variant: {other:?}"),
            Ok(_) => panic!("gate must reject non-production-grade KMS in production"),
        }
    }

    #[test]
    fn gate_allows_prod_kms_in_production() {
        let kms = enforce_production_safety_gate(Environment::Production, "aws", fake(true))
            .expect("production-grade KMS must pass the gate in production");
        assert!(kms.is_production_grade());
    }

    #[test]
    fn gate_allows_non_prod_kms_in_development() {
        // Default behaviour for local dev workflows: nothing changes.
        let kms = enforce_production_safety_gate(Environment::Development, "local", fake(false))
            .expect("non-production-grade KMS is fine in development");
        assert!(!kms.is_production_grade());
    }

    #[test]
    fn gate_allows_non_prod_kms_in_ci_and_staging() {
        // CI and staging are intentionally permissive — only Production trips the gate.
        for env in [Environment::Ci, Environment::Staging] {
            let kms = enforce_production_safety_gate(env, "local", fake(false))
                .unwrap_or_else(|e| panic!("env={env} should not trip gate: {e}"));
            assert!(!kms.is_production_grade());
        }
    }

    #[test]
    fn gate_lowercases_selector_in_error_message() {
        // Operators sometimes set AETERNA_KMS_PROVIDER=LOCAL or =Local; the error
        // diagnostic should normalise to lowercase so dashboards/log scrapes match.
        let err = match enforce_production_safety_gate(
            Environment::Production,
            "LoCaL",
            fake(false),
        ) {
            Err(e) => e,
            Ok(_) => panic!("gate must trip on non-production-grade KMS in production"),
        };
        let msg = err.to_string();
        assert!(msg.contains("'local'"), "expected lowercased selector in: {msg}");
        assert!(msg.contains("production"), "expected env in: {msg}");
    }
}
