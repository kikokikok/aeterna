/// Key Management System (KMS) integration
/// 
/// This module provides integration with external KMS providers for secure key management,
/// including AWS KMS and HashiCorp Vault.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Error, Debug)]
pub enum KmsError {
    #[error("KMS operation failed: {0}")]
    OperationFailed(String),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

/// KMS provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum KmsConfig {
    /// AWS KMS configuration
    #[serde(rename = "aws-kms")]
    AwsKms {
        region: String,
        key_id: String,
        endpoint: Option<String>,
    },
    
    /// HashiCorp Vault configuration
    #[serde(rename = "vault")]
    Vault {
        address: String,
        token: String,
        mount_path: String,
        key_name: String,
    },
    
    /// Local development (insecure, for testing only)
    #[serde(rename = "local")]
    Local {
        keys: HashMap<String, String>,
    },
}

/// KMS key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmsKeyMetadata {
    pub key_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub enabled: bool,
    pub description: Option<String>,
    pub rotation_enabled: bool,
    pub last_rotated: Option<chrono::DateTime<chrono::Utc>>,
}

/// Trait for KMS providers
#[async_trait]
pub trait KmsProvider: Send + Sync {
    /// Generate a new data encryption key (DEK)
    /// Returns (plaintext_key, encrypted_key)
    async fn generate_data_key(&self, key_id: &str) -> Result<(Vec<u8>, Vec<u8>), KmsError>;
    
    /// Decrypt an encrypted data key
    async fn decrypt_data_key(&self, encrypted_key: &[u8]) -> Result<Vec<u8>, KmsError>;
    
    /// Encrypt data directly with KMS (for small payloads)
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, KmsError>;
    
    /// Decrypt data encrypted with KMS
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, KmsError>;
    
    /// Get key metadata
    async fn get_key_metadata(&self, key_id: &str) -> Result<KmsKeyMetadata, KmsError>;
    
    /// Rotate a key
    async fn rotate_key(&self, key_id: &str) -> Result<(), KmsError>;
    
    /// List all keys
    async fn list_keys(&self) -> Result<Vec<String>, KmsError>;
}

/// AWS KMS provider implementation
pub struct AwsKmsProvider {
    client: aws_sdk_kms::Client,
    key_id: String,
}

impl AwsKmsProvider {
    pub async fn new(region: String, key_id: String, endpoint: Option<String>) -> Result<Self, KmsError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_sdk_kms::config::Region::new(region))
            .load()
            .await;
        
        let mut client_config = aws_sdk_kms::config::Builder::from(&config);
        if let Some(ep) = endpoint {
            client_config = client_config.endpoint_url(ep);
        }
        
        let client = aws_sdk_kms::Client::from_conf(client_config.build());
        
        Ok(Self { client, key_id })
    }
}

#[async_trait]
impl KmsProvider for AwsKmsProvider {
    async fn generate_data_key(&self, _key_id: &str) -> Result<(Vec<u8>, Vec<u8>), KmsError> {
        let result = self.client
            .generate_data_key()
            .key_id(&self.key_id)
            .key_spec(aws_sdk_kms::types::DataKeySpec::Aes256)
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        let plaintext = result.plaintext()
            .ok_or_else(|| KmsError::OperationFailed("No plaintext returned".to_string()))?
            .as_ref()
            .to_vec();
        
        let ciphertext = result.ciphertext_blob()
            .ok_or_else(|| KmsError::OperationFailed("No ciphertext returned".to_string()))?
            .as_ref()
            .to_vec();
        
        Ok((plaintext, ciphertext))
    }
    
    async fn decrypt_data_key(&self, encrypted_key: &[u8]) -> Result<Vec<u8>, KmsError> {
        let result = self.client
            .decrypt()
            .ciphertext_blob(aws_sdk_kms::primitives::Blob::new(encrypted_key))
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        let plaintext = result.plaintext()
            .ok_or_else(|| KmsError::OperationFailed("No plaintext returned".to_string()))?
            .as_ref()
            .to_vec();
        
        Ok(plaintext)
    }
    
    async fn encrypt(&self, _key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let result = self.client
            .encrypt()
            .key_id(&self.key_id)
            .plaintext(aws_sdk_kms::primitives::Blob::new(plaintext))
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        let ciphertext = result.ciphertext_blob()
            .ok_or_else(|| KmsError::OperationFailed("No ciphertext returned".to_string()))?
            .as_ref()
            .to_vec();
        
        Ok(ciphertext)
    }
    
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, KmsError> {
        self.decrypt_data_key(ciphertext).await
    }
    
    async fn get_key_metadata(&self, key_id: &str) -> Result<KmsKeyMetadata, KmsError> {
        let result = self.client
            .describe_key()
            .key_id(key_id)
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        let metadata = result.key_metadata()
            .ok_or_else(|| KmsError::OperationFailed("No metadata returned".to_string()))?;
        
        Ok(KmsKeyMetadata {
            key_id: metadata.key_id().to_string(),
            created_at: chrono::DateTime::from_timestamp(
                metadata.creation_date()
                    .ok_or_else(|| KmsError::OperationFailed("No creation date".to_string()))?
                    .secs(),
                0
            ).ok_or_else(|| KmsError::OperationFailed("Invalid timestamp".to_string()))?,
            enabled: metadata.enabled(),
            description: metadata.description().map(String::from),
            rotation_enabled: false, // Would need separate API call
            last_rotated: None,
        })
    }
    
    async fn rotate_key(&self, key_id: &str) -> Result<(), KmsError> {
        self.client
            .enable_key_rotation()
            .key_id(key_id)
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        Ok(())
    }
    
    async fn list_keys(&self) -> Result<Vec<String>, KmsError> {
        let result = self.client
            .list_keys()
            .send()
            .await
            .map_err(|e| KmsError::OperationFailed(e.to_string()))?;
        
        Ok(result.keys()
            .iter()
            .filter_map(|k| k.key_id().map(String::from))
            .collect())
    }
}

/// Local KMS provider (for development/testing only - NOT SECURE)
pub struct LocalKmsProvider {
    keys: HashMap<String, Vec<u8>>,
}

impl LocalKmsProvider {
    pub fn new(keys: HashMap<String, String>) -> Result<Self, KmsError> {
        let mut parsed_keys = HashMap::new();
        
        for (key_id, key_hex) in keys {
            let key_bytes = hex::decode(&key_hex)
                .map_err(|e| KmsError::InvalidConfig(format!("Invalid key hex: {}", e)))?;
            
            if key_bytes.len() != 32 {
                return Err(KmsError::InvalidConfig(
                    "Key must be 32 bytes (256 bits)".to_string()
                ));
            }
            
            parsed_keys.insert(key_id, key_bytes);
        }
        
        Ok(Self { keys: parsed_keys })
    }
}

#[async_trait]
impl KmsProvider for LocalKmsProvider {
    async fn generate_data_key(&self, key_id: &str) -> Result<(Vec<u8>, Vec<u8>), KmsError> {
        let master_key = self.keys.get(key_id)
            .ok_or_else(|| KmsError::KeyNotFound(key_id.to_string()))?;
        
        // Generate a random DEK
        use aes_gcm::{aead::OsRng, Aes256Gcm, KeyInit};
        let dek = Aes256Gcm::generate_key(&mut OsRng);
        let dek_bytes = dek.to_vec();
        
        // "Encrypt" DEK with master key (simplified XOR for demo)
        let encrypted_dek: Vec<u8> = dek_bytes.iter()
            .zip(master_key.iter().cycle())
            .map(|(a, b)| a ^ b)
            .collect();
        
        Ok((dek_bytes, encrypted_dek))
    }
    
    async fn decrypt_data_key(&self, encrypted_key: &[u8]) -> Result<Vec<u8>, KmsError> {
        // Find which key can decrypt (try all keys)
        for master_key in self.keys.values() {
            let decrypted: Vec<u8> = encrypted_key.iter()
                .zip(master_key.iter().cycle())
                .map(|(a, b)| a ^ b)
                .collect();
            
            if decrypted.len() == 32 {
                return Ok(decrypted);
            }
        }
        
        Err(KmsError::OperationFailed("Failed to decrypt data key".to_string()))
    }
    
    async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        let key = self.keys.get(key_id)
            .ok_or_else(|| KmsError::KeyNotFound(key_id.to_string()))?;
        
        let ciphertext: Vec<u8> = plaintext.iter()
            .zip(key.iter().cycle())
            .map(|(a, b)| a ^ b)
            .collect();
        
        Ok(ciphertext)
    }
    
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, KmsError> {
        self.decrypt_data_key(ciphertext).await
    }
    
    async fn get_key_metadata(&self, key_id: &str) -> Result<KmsKeyMetadata, KmsError> {
        if !self.keys.contains_key(key_id) {
            return Err(KmsError::KeyNotFound(key_id.to_string()));
        }
        
        Ok(KmsKeyMetadata {
            key_id: key_id.to_string(),
            created_at: chrono::Utc::now(),
            enabled: true,
            description: Some("Local development key".to_string()),
            rotation_enabled: false,
            last_rotated: None,
        })
    }
    
    async fn rotate_key(&self, _key_id: &str) -> Result<(), KmsError> {
        // No-op for local provider
        Ok(())
    }
    
    async fn list_keys(&self) -> Result<Vec<String>, KmsError> {
        Ok(self.keys.keys().cloned().collect())
    }
}

/// KMS client that wraps different providers
pub struct KmsClient {
    provider: Arc<dyn KmsProvider>,
}

impl KmsClient {
    pub async fn new(config: KmsConfig) -> Result<Self, KmsError> {
        let provider: Arc<dyn KmsProvider> = match config {
            KmsConfig::AwsKms { region, key_id, endpoint } => {
                Arc::new(AwsKmsProvider::new(region, key_id, endpoint).await?)
            }
            KmsConfig::Vault { .. } => {
                return Err(KmsError::InvalidConfig(
                    "Vault provider not yet implemented".to_string()
                ));
            }
            KmsConfig::Local { keys } => {
                Arc::new(LocalKmsProvider::new(keys)?)
            }
        };
        
        Ok(Self { provider })
    }
    
    pub async fn generate_data_key(&self, key_id: &str) -> Result<(Vec<u8>, Vec<u8>), KmsError> {
        self.provider.generate_data_key(key_id).await
    }
    
    pub async fn decrypt_data_key(&self, encrypted_key: &[u8]) -> Result<Vec<u8>, KmsError> {
        self.provider.decrypt_data_key(encrypted_key).await
    }
    
    pub async fn encrypt(&self, key_id: &str, plaintext: &[u8]) -> Result<Vec<u8>, KmsError> {
        self.provider.encrypt(key_id, plaintext).await
    }
    
    pub async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, KmsError> {
        self.provider.decrypt(ciphertext).await
    }
    
    pub async fn get_key_metadata(&self, key_id: &str) -> Result<KmsKeyMetadata, KmsError> {
        self.provider.get_key_metadata(key_id).await
    }
    
    pub async fn rotate_key(&self, key_id: &str) -> Result<(), KmsError> {
        self.provider.rotate_key(key_id).await
    }
    
    pub async fn list_keys(&self) -> Result<Vec<String>, KmsError> {
        self.provider.list_keys().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_local_kms_provider() {
        let mut keys = HashMap::new();
        keys.insert(
            "test-key".to_string(),
            hex::encode(vec![0u8; 32])
        );
        
        let provider = LocalKmsProvider::new(keys).unwrap();
        
        // Test data key generation
        let (plaintext, encrypted) = provider.generate_data_key("test-key").await.unwrap();
        assert_eq!(plaintext.len(), 32);
        assert_eq!(encrypted.len(), 32);
        
        // Test decryption
        let decrypted = provider.decrypt_data_key(&encrypted).await.unwrap();
        assert_eq!(plaintext, decrypted);
        
        // Test encrypt/decrypt
        let message = b"Hello, KMS!";
        let ciphertext = provider.encrypt("test-key", message).await.unwrap();
        let plaintext = provider.decrypt(&ciphertext).await.unwrap();
        assert_eq!(message.to_vec(), plaintext);
    }
    
    #[tokio::test]
    async fn test_kms_client() {
        let mut keys = HashMap::new();
        keys.insert(
            "test-key".to_string(),
            hex::encode(vec![1u8; 32])
        );
        
        let config = KmsConfig::Local { keys };
        let client = KmsClient::new(config).await.unwrap();
        
        let (plaintext, encrypted) = client.generate_data_key("test-key").await.unwrap();
        let decrypted = client.decrypt_data_key(&encrypted).await.unwrap();
        
        assert_eq!(plaintext, decrypted);
    }
}
