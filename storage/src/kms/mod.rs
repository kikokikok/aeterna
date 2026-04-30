//! KMS-backed key management for envelope encryption.
//!
//! This module defines [`KmsProvider`], the abstraction over a Key Management
//! Service used to wrap and unwrap **data encryption keys** (DEKs). The DEKs
//! in turn encrypt the actual secret bytes stored in the `tenant_secrets`
//! Postgres table — this is standard envelope encryption.
//!
//! # Providers
//!
//! B1 ships two implementations:
//!
//! - [`local::LocalKmsProvider`] — development / testing. AES-256-GCM with a
//!   key loaded from the `AETERNA_LOCAL_KMS_KEY` environment variable. Emits
//!   a `WARN` on construction so production misconfiguration is loud.
//! - [`aws::AwsKmsProvider`] — production. Uses the AWS KMS service via
//!   `aws-sdk-kms`; the default AWS credential provider chain handles both
//!   static access keys and IAM Roles for Service Accounts (IRSA) transparently.
//!
//! Additional providers (GCP KMS, Azure Key Vault, OpenBao Transit) can be
//! added as new modules implementing the [`KmsProvider`] trait — no changes
//! to the call sites are required.
//!
//! # Contract
//!
//! Implementations MUST:
//!
//! 1. Generate ciphertext that is **authenticated** (AEAD) so tampering
//!    produces a decryption error rather than silent corruption.
//! 2. Be safe to clone and share across tasks (`Send + Sync`).
//! 3. Never log or otherwise leak the plaintext DEK.
//! 4. Return a stable [`KmsProvider::key_id`] so persisted ciphertexts carry
//!    their key lineage for future rotation.

pub mod aws;
pub mod local;

pub use aws::AwsKmsProvider;
pub use local::LocalKmsProvider;

use async_trait::async_trait;
use mk_core::SecretBytes;
use thiserror::Error;

/// Errors raised by any [`KmsProvider`] implementation.
#[derive(Debug, Error)]
pub enum KmsError {
    /// Wrapping the plaintext DEK failed (e.g. AWS KMS network error).
    #[error("kms encrypt failed: {0}")]
    Encrypt(String),

    /// Unwrapping the ciphertext DEK failed (bad ciphertext, wrong key,
    /// tampered bytes, or authorization denied).
    #[error("kms decrypt failed: {0}")]
    Decrypt(String),

    /// The configured provider could not be constructed (missing env var,
    /// malformed ARN, unreachable endpoint at startup, etc.).
    #[error("kms configuration invalid: {0}")]
    Config(String),
}

/// Abstraction over a Key Management Service capable of wrapping and
/// unwrapping data encryption keys.
///
/// Implementations do **not** store the plaintext key — they hold only the
/// handle / ARN / path that KMS uses to locate the Customer Master Key (CMK).
///
/// # Example
///
/// ```no_run
/// use storage::kms::{KmsProvider, LocalKmsProvider};
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let kms = LocalKmsProvider::from_env()?;
/// let dek = b"32-byte-data-encryption-key-here!";
/// let wrapped = kms.encrypt(dek).await?;
/// let unwrapped = kms.decrypt(&wrapped).await?;
/// assert_eq!(dek, unwrapped.expose());
/// # Ok(()) }
/// ```
#[async_trait]
pub trait KmsProvider: Send + Sync {
    /// Stable identifier for the key this provider wraps with.
    ///
    /// Persisted alongside every ciphertext so multi-key rotation can route
    /// decrypts to the right provider instance.
    fn key_id(&self) -> &str;

    /// Whether this provider is safe to use in a production deployment.
    ///
    /// Default is `false` — providers must explicitly opt in. The
    /// [`storage::secret_backend::build_secret_backend_from_env`] factory
    /// rejects providers that return `false` when
    /// [`mk_core::Environment::from_env`] reports
    /// [`mk_core::Environment::Production`].
    ///
    /// Override to `true` only on providers that:
    ///
    /// - Persist their wrapping key in a managed KMS (AWS KMS, GCP KMS,
    ///   Vault Transit, etc.) outside the application process;
    /// - Authenticate via a production-grade credential chain (IRSA,
    ///   workload identity, SPIFFE, …) — never a static developer key;
    /// - Have an audit trail at the KMS layer.
    fn is_production_grade(&self) -> bool {
        false
    }

    /// Wrap `plaintext` bytes (typically a 32-byte DEK) with the CMK.
    /// Returns the opaque ciphertext to be stored.
    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, KmsError>;

    /// Unwrap a ciphertext previously produced by [`Self::encrypt`].
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes, KmsError>;

    /// Boot-time probe: encrypt a sentinel value, decrypt it, and verify
    /// the round-trip yields the original bytes. Called by
    /// [`crate::secret_backend::build_secret_backend_from_env`] right after
    /// construction so the process aborts fast on a mis-wired KMS — wrong
    /// ARN, expired AWS credentials, unreachable endpoint — instead of
    /// failing on the first real tenant secret.
    ///
    /// The default implementation is correct for every provider whose
    /// `encrypt`/`decrypt` round-trips deterministically; override only if
    /// the probe needs custom payload semantics.
    ///
    /// # Cost
    ///
    /// One `encrypt` + one `decrypt` call. For [`crate::kms::AwsKmsProvider`]
    /// that means two AWS KMS API requests at startup; runbooks should
    /// expect this signal in CloudTrail under the operator role.
    async fn self_test(&self) -> Result<(), KmsError> {
        // 32-byte sentinel — same length as a real DEK so the probe
        // exercises the same code path as production traffic.
        const PROBE: &[u8; 32] = b"aeterna-kms-self-test-probe-0042";
        let ct = self.encrypt(PROBE).await.map_err(|e| {
            KmsError::Config(format!(
                "self-test encrypt failed for key '{}': {e}",
                self.key_id()
            ))
        })?;
        let pt = self.decrypt(&ct).await.map_err(|e| {
            KmsError::Config(format!(
                "self-test decrypt failed for key '{}': {e}",
                self.key_id()
            ))
        })?;
        if pt.expose() != PROBE.as_slice() {
            return Err(KmsError::Config(format!(
                "self-test round-trip mismatch for key '{}'",
                self.key_id()
            )));
        }
        Ok(())
    }
}
