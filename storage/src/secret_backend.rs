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
use mk_core::{SecretBytes, SecretReference};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::kms::{KmsError, KmsProvider};

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

    fn aead_seal(dek: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), SecretBackendError> {
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
        let SecretReference::Postgres { secret_id } = reference;
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
        let SecretReference::Postgres { secret_id } = reference;
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
