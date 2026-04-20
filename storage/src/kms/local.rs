//! Local (dev-only) KMS provider.
//!
//! Wraps DEKs with AES-256-GCM using a key loaded from the
//! `AETERNA_LOCAL_KMS_KEY` environment variable (base64-encoded 32 bytes).
//!
//! **Not for production.** Construction emits a `WARN` log; if you are
//! seeing that log in a production deployment, your Helm values are
//! misconfigured — set `kms.provider: aws` and supply `kms.aws.keyArn`.
//!
//! # Ciphertext format
//!
//! The returned ciphertext is the concatenation `nonce (12 B) || ct || tag`
//! where `ct || tag` is what `aes-gcm::Aes256Gcm::encrypt` produces. This
//! matches the on-disk format used by `AwsKmsProvider` for symmetry.

use super::{KmsError, KmsProvider};
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use mk_core::SecretBytes;

/// AES-256-GCM-backed local KMS for development and unit tests.
pub struct LocalKmsProvider {
    cipher: Aes256Gcm,
    key_id: String,
}

impl std::fmt::Debug for LocalKmsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalKmsProvider")
            .field("key_id", &self.key_id)
            .field("cipher", &"<redacted>")
            .finish()
    }
}

impl LocalKmsProvider {
    /// Construct directly from a 32-byte raw key. Prefer [`Self::from_env`]
    /// in application code.
    pub fn from_bytes(key_bytes: &[u8], key_id: impl Into<String>) -> Result<Self, KmsError> {
        if key_bytes.len() != 32 {
            return Err(KmsError::Config(format!(
                "local kms key must be exactly 32 bytes, got {}",
                key_bytes.len()
            )));
        }
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);
        let key_id = key_id.into();
        tracing::warn!(
            key_id = %key_id,
            "LocalKmsProvider in use — NOT FOR PRODUCTION. Set kms.provider=aws in Helm values."
        );
        Ok(Self { cipher, key_id })
    }

    /// Construct from `AETERNA_LOCAL_KMS_KEY` (base64-encoded 32 bytes).
    pub fn from_env() -> Result<Self, KmsError> {
        let b64 = std::env::var("AETERNA_LOCAL_KMS_KEY").map_err(|_| {
            KmsError::Config(
                "AETERNA_LOCAL_KMS_KEY not set; required when kms.provider=local".into(),
            )
        })?;
        let bytes = general_purpose::STANDARD.decode(b64.trim()).map_err(|e| {
            KmsError::Config(format!("AETERNA_LOCAL_KMS_KEY is not valid base64: {e}"))
        })?;
        Self::from_bytes(&bytes, "local:env")
    }

    /// Generate a fresh random 32-byte key, encoded as base64. Intended for
    /// bootstrapping a dev environment:
    ///
    /// ```ignore
    /// echo "AETERNA_LOCAL_KMS_KEY=$(aeterna kms gen-local-key)" >> .env
    /// ```
    #[must_use]
    pub fn generate_key_b64() -> String {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        general_purpose::STANDARD.encode(key)
    }
}

#[async_trait]
impl KmsProvider for LocalKmsProvider {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| KmsError::Encrypt(format!("aes-gcm seal failed: {e}")))?;
        let mut out = Vec::with_capacity(12 + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(out)
    }

    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes, KmsError> {
        if ciphertext.len() < 12 + 16 {
            return Err(KmsError::Decrypt(format!(
                "ciphertext too short: {} bytes",
                ciphertext.len()
            )));
        }
        let (nonce_bytes, ct) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let pt = self
            .cipher
            .decrypt(nonce, ct)
            .map_err(|e| KmsError::Decrypt(format!("aes-gcm open failed: {e}")))?;
        Ok(SecretBytes::from(pt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_key() -> LocalKmsProvider {
        let key = [0x42u8; 32];
        LocalKmsProvider::from_bytes(&key, "local:test").unwrap()
    }

    #[tokio::test]
    async fn roundtrip() {
        let kms = fixed_key();
        let pt = b"this-is-a-data-encryption-key!!!"; // 32B DEK
        let ct = kms.encrypt(pt).await.unwrap();
        assert_ne!(ct.as_slice(), pt.as_slice());
        let decrypted = kms.decrypt(&ct).await.unwrap();
        assert_eq!(decrypted.expose(), pt);
    }

    #[tokio::test]
    async fn distinct_ciphertexts_for_same_plaintext() {
        let kms = fixed_key();
        let pt = b"same-plaintext";
        let ct1 = kms.encrypt(pt).await.unwrap();
        let ct2 = kms.encrypt(pt).await.unwrap();
        assert_ne!(ct1, ct2, "random nonces must make ciphertexts differ");
    }

    #[tokio::test]
    async fn tampered_ciphertext_fails_decrypt() {
        let kms = fixed_key();
        let pt = b"authentic";
        let mut ct = kms.encrypt(pt).await.unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0xff;
        let err = kms.decrypt(&ct).await.unwrap_err();
        assert!(matches!(err, KmsError::Decrypt(_)));
    }

    #[tokio::test]
    async fn short_ciphertext_fails_decrypt() {
        let kms = fixed_key();
        let err = kms.decrypt(&[0u8; 4]).await.unwrap_err();
        assert!(matches!(err, KmsError::Decrypt(_)));
    }

    #[test]
    fn rejects_wrong_key_length() {
        let err = LocalKmsProvider::from_bytes(&[0u8; 16], "k").unwrap_err();
        assert!(matches!(err, KmsError::Config(_)));
    }

    #[test]
    fn generate_key_b64_length() {
        let k = LocalKmsProvider::generate_key_b64();
        let decoded = general_purpose::STANDARD.decode(k).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn key_id_reported() {
        let kms = fixed_key();
        assert_eq!(kms.key_id(), "local:test");
    }
}
