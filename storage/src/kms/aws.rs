//! AWS KMS provider for production envelope encryption.
//!
//! Uses `aws-sdk-kms` with the default AWS credential provider chain. The
//! chain tries, in order:
//!
//! 1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
//!    optionally `AWS_SESSION_TOKEN`).
//! 2. Web Identity Token from file — this is **IRSA** (IAM Roles for Service
//!    Accounts) on EKS. The pod identity webhook injects
//!    `AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN` automatically.
//! 3. ECS task role, EC2 instance profile, shared credentials file.
//!
//! This means **no code change** is needed to migrate from AK/SK to IRSA:
//! flip the Helm values and the SDK picks up the new credentials on pod
//! restart.
//!
//! # Key ARN format
//!
//! Accepts any form the AWS KMS `Encrypt` API accepts:
//! - Full key ARN: `arn:aws:kms:<region>:<account-id>:key/<key-uuid>`
//! - Alias ARN:    `arn:aws:kms:<region>:<account-id>:alias/<alias-name>`
//! - Alias name:   `alias/my-alias`
//! - Key ID:       `1234abcd-12ab-34cd-56ef-1234567890ab`
//!
//! # Ciphertext
//!
//! The ciphertext returned by AWS KMS is an opaque blob that is self-describing;
//! at decrypt time the SDK does **not** require the key ARN (the blob carries
//! the key handle). We still track `key_arn` on the provider for observability
//! and for the `key_id()` accessor used by persistence.

use super::{KmsError, KmsProvider};
use async_trait::async_trait;
use aws_sdk_kms::primitives::Blob;
use mk_core::SecretBytes;

/// AWS KMS-backed provider.
#[derive(Debug, Clone)]
pub struct AwsKmsProvider {
    client: aws_sdk_kms::Client,
    key_arn: String,
}

impl AwsKmsProvider {
    /// Build an AWS KMS provider using the default credential provider chain.
    ///
    /// Performs no network call at construction time — the first real call
    /// happens on `encrypt` / `decrypt`. This keeps startup fast and makes
    /// constructor failures rare (basically only argument-validation errors).
    pub async fn new(key_arn: impl Into<String>) -> Result<Self, KmsError> {
        let key_arn = key_arn.into();
        if key_arn.trim().is_empty() {
            return Err(KmsError::Config(
                "kms.aws.keyArn is empty; set it to a KMS key id, alias, or ARN".into(),
            ));
        }
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .load()
            .await;
        let client = aws_sdk_kms::Client::new(&config);
        tracing::info!(
            key_arn = %key_arn,
            "AwsKmsProvider initialised (credentials resolved lazily by aws-sdk default chain)"
        );
        Ok(Self { client, key_arn })
    }

    /// Build directly from a pre-constructed SDK client. Used by tests that
    /// want to inject a stubbed client, and by callers that share one AWS
    /// config across multiple services.
    #[must_use]
    pub fn from_client(client: aws_sdk_kms::Client, key_arn: impl Into<String>) -> Self {
        Self {
            client,
            key_arn: key_arn.into(),
        }
    }
}

#[async_trait]
impl KmsProvider for AwsKmsProvider {
    fn key_id(&self) -> &str {
        &self.key_arn
    }

    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let resp = self
            .client
            .encrypt()
            .key_id(&self.key_arn)
            .plaintext(Blob::new(plaintext.to_vec()))
            .send()
            .await
            .map_err(|e| KmsError::Encrypt(format!("aws kms encrypt failed: {e}")))?;
        let blob = resp
            .ciphertext_blob
            .ok_or_else(|| KmsError::Encrypt("aws kms returned no ciphertext blob".into()))?;
        Ok(blob.into_inner())
    }

    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes, KmsError> {
        // We intentionally do NOT pass `.key_id()` to `decrypt` — the
        // ciphertext blob is self-describing and KMS will route to the right
        // key. Passing our tracked ARN would *restrict* decrypt to that key,
        // which breaks CMK rotation (old rows encrypted with the previous
        // CMK would fail). See AWS KMS rotation docs.
        let resp = self
            .client
            .decrypt()
            .ciphertext_blob(Blob::new(ciphertext.to_vec()))
            .send()
            .await
            .map_err(|e| KmsError::Decrypt(format!("aws kms decrypt failed: {e}")))?;
        let blob = resp
            .plaintext
            .ok_or_else(|| KmsError::Decrypt("aws kms returned no plaintext".into()))?;
        Ok(SecretBytes::from(blob.into_inner()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_empty_key_arn() {
        let err = AwsKmsProvider::new("").await.unwrap_err();
        assert!(matches!(err, KmsError::Config(_)));
    }

    #[tokio::test]
    async fn rejects_whitespace_key_arn() {
        let err = AwsKmsProvider::new("   ").await.unwrap_err();
        assert!(matches!(err, KmsError::Config(_)));
    }

    // Live AWS integration tests. Run with:
    //   AETERNA_KMS_LIVE_TEST_ARN=arn:... cargo test -p storage aws_kms_live -- --ignored
    #[tokio::test]
    #[ignore = "requires live AWS credentials + AETERNA_KMS_LIVE_TEST_ARN"]
    async fn live_roundtrip() {
        let arn = std::env::var("AETERNA_KMS_LIVE_TEST_ARN")
            .expect("AETERNA_KMS_LIVE_TEST_ARN must be set for live test");
        let kms = AwsKmsProvider::new(&arn).await.unwrap();
        let pt = b"this-is-a-32-byte-data-enc-key!!";
        let ct = kms.encrypt(pt).await.expect("encrypt");
        assert_ne!(ct.as_slice(), pt.as_slice());
        let decrypted = kms.decrypt(&ct).await.expect("decrypt");
        assert_eq!(decrypted.expose(), pt);
    }
}
