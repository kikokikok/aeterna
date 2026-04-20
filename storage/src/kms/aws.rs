//! AWS KMS provider (stub — real implementation lands in the next commit).
//!
//! This file exists so the module compiles. Attempting to construct or call
//! this provider panics with `unimplemented!()`. See the follow-up commit
//! that wires `aws-sdk-kms` and the default credential provider chain.

use super::{KmsError, KmsProvider};
use async_trait::async_trait;
use mk_core::SecretBytes;

/// AWS KMS-backed provider. **Not yet implemented.**
#[derive(Debug)]
pub struct AwsKmsProvider {
    _key_arn: String,
}

impl AwsKmsProvider {
    pub async fn new(_key_arn: impl Into<String>) -> Result<Self, KmsError> {
        Err(KmsError::Config(
            "AwsKmsProvider not yet implemented; use LocalKmsProvider or wait for the next commit".into(),
        ))
    }
}

#[async_trait]
impl KmsProvider for AwsKmsProvider {
    fn key_id(&self) -> &str {
        &self._key_arn
    }

    async fn encrypt(&self, _plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        unimplemented!("AwsKmsProvider::encrypt lands in the next commit")
    }

    async fn decrypt(&self, _ciphertext: &[u8]) -> Result<SecretBytes, KmsError> {
        unimplemented!("AwsKmsProvider::decrypt lands in the next commit")
    }
}
