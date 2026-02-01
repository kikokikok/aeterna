/// Field-level encryption utilities using AES-256-GCM
/// 
/// This module provides encryption/decryption capabilities for sensitive data fields
/// with support for key rotation and multiple encryption keys.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce, Key
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    
    #[error("Invalid encrypted data format")]
    InvalidFormat,
}

/// Encrypted data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Base64-encoded ciphertext
    pub ciphertext: String,
    
    /// Base64-encoded nonce
    pub nonce: String,
    
    /// Key ID used for encryption
    pub key_id: String,
    
    /// Encryption algorithm
    pub algorithm: String,
    
    /// Timestamp when encrypted
    pub encrypted_at: chrono::DateTime<chrono::Utc>,
}

/// Configuration for field-level encryption
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// List of fields to encrypt (e.g., "email", "api_key", "ssn")
    pub encrypted_fields: Vec<String>,
    
    /// Whether encryption is enabled
    pub enabled: bool,
    
    /// Key rotation period in days
    pub key_rotation_days: u32,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            encrypted_fields: vec![
                "email".to_string(),
                "api_key".to_string(),
                "access_token".to_string(),
            ],
            enabled: false, // Disabled by default for safety
            key_rotation_days: 90,
        }
    }
}

/// Encryption manager for handling field-level encryption
pub struct EncryptionManager {
    /// Map of key_id to encryption key
    keys: Arc<RwLock<HashMap<String, Arc<Aes256Gcm>>>>,
    
    /// Current active key ID
    active_key_id: Arc<RwLock<String>>,
    
    /// Configuration
    config: EncryptionConfig,
}

impl EncryptionManager {
    /// Create a new encryption manager with the given configuration
    pub fn new(config: EncryptionConfig) -> Result<Self, EncryptionError> {
        let keys = Arc::new(RwLock::new(HashMap::new()));
        let active_key_id = Arc::new(RwLock::new("key-1".to_string()));
        
        Ok(Self {
            keys,
            active_key_id,
            config,
        })
    }
    
    /// Add a new encryption key
    pub fn add_key(&self, key_id: String, key_bytes: &[u8]) -> Result<(), EncryptionError> {
        if key_bytes.len() != 32 {
            return Err(EncryptionError::InvalidKey(
                "Key must be 32 bytes (256 bits)".to_string()
            ));
        }
        
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);
        
        let mut keys = self.keys.write();
        keys.insert(key_id, Arc::new(cipher));
        
        Ok(())
    }
    
    /// Generate and add a new random encryption key
    pub fn generate_key(&self, key_id: String) -> Result<Vec<u8>, EncryptionError> {
        let key_bytes = Aes256Gcm::generate_key(&mut OsRng);
        let key_vec = key_bytes.to_vec();
        
        self.add_key(key_id, &key_vec)?;
        
        Ok(key_vec)
    }
    
    /// Set the active key ID
    pub fn set_active_key(&self, key_id: String) -> Result<(), EncryptionError> {
        let keys = self.keys.read();
        if !keys.contains_key(&key_id) {
            return Err(EncryptionError::KeyNotFound(key_id));
        }
        
        let mut active = self.active_key_id.write();
        *active = key_id;
        
        Ok(())
    }
    
    /// Get the active key ID
    pub fn get_active_key_id(&self) -> String {
        self.active_key_id.read().clone()
    }
    
    /// Encrypt a plaintext string
    pub fn encrypt(&self, plaintext: &str) -> Result<EncryptedData, EncryptionError> {
        let active_key_id = self.get_active_key_id();
        
        let keys = self.keys.read();
        let cipher = keys.get(&active_key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound(active_key_id.clone()))?;
        
        // Generate a random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;
        
        Ok(EncryptedData {
            ciphertext: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(nonce.as_slice()),
            key_id: active_key_id,
            algorithm: "AES-256-GCM".to_string(),
            encrypted_at: chrono::Utc::now(),
        })
    }
    
    /// Decrypt encrypted data
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<String, EncryptionError> {
        let keys = self.keys.read();
        let cipher = keys.get(&encrypted.key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound(encrypted.key_id.clone()))?;
        
        // Decode base64 data
        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted.ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;
        
        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted.nonce)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;
        
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }
    
    /// Encrypt a value if encryption is enabled and the field is in the encrypted list
    pub fn encrypt_field(&self, field_name: &str, value: &str) -> Result<String, EncryptionError> {
        if !self.config.enabled || !self.config.encrypted_fields.contains(&field_name.to_string()) {
            // Encryption disabled or field not in encrypted list, return as-is
            return Ok(value.to_string());
        }
        
        let encrypted = self.encrypt(value)?;
        serde_json::to_string(&encrypted)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))
    }
    
    /// Decrypt a value if it's encrypted
    pub fn decrypt_field(&self, value: &str) -> Result<String, EncryptionError> {
        if !self.config.enabled {
            // Encryption disabled, return as-is
            return Ok(value.to_string());
        }
        
        // Try to parse as encrypted data
        match serde_json::from_str::<EncryptedData>(value) {
            Ok(encrypted) => self.decrypt(&encrypted),
            Err(_) => {
                // Not encrypted format, return as-is
                Ok(value.to_string())
            }
        }
    }
    
    /// Check if a key should be rotated based on age
    pub fn should_rotate_key(&self, encrypted: &EncryptedData) -> bool {
        let age = chrono::Utc::now() - encrypted.encrypted_at;
        age.num_days() > self.config.key_rotation_days as i64
    }
    
    /// Re-encrypt data with the active key (for key rotation)
    pub fn re_encrypt(&self, encrypted: &EncryptedData) -> Result<EncryptedData, EncryptionError> {
        let plaintext = self.decrypt(encrypted)?;
        self.encrypt(&plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encryption_roundtrip() {
        let config = EncryptionConfig {
            enabled: true,
            encrypted_fields: vec!["email".to_string()],
            key_rotation_days: 90,
        };
        
        let manager = EncryptionManager::new(config).unwrap();
        let _key = manager.generate_key("test-key".to_string()).unwrap();
        manager.set_active_key("test-key".to_string()).unwrap();
        
        let plaintext = "sensitive@example.com";
        let encrypted = manager.encrypt(plaintext).unwrap();
        let decrypted = manager.decrypt(&encrypted).unwrap();
        
        assert_eq!(plaintext, decrypted);
        assert_eq!(encrypted.algorithm, "AES-256-GCM");
        assert_eq!(encrypted.key_id, "test-key");
    }
    
    #[test]
    fn test_field_encryption() {
        let config = EncryptionConfig {
            enabled: true,
            encrypted_fields: vec!["email".to_string()],
            key_rotation_days: 90,
        };
        
        let manager = EncryptionManager::new(config).unwrap();
        let _key = manager.generate_key("test-key".to_string()).unwrap();
        manager.set_active_key("test-key".to_string()).unwrap();
        
        // Encrypt field
        let value = "test@example.com";
        let encrypted = manager.encrypt_field("email", value).unwrap();
        assert_ne!(encrypted, value);
        
        // Decrypt field
        let decrypted = manager.decrypt_field(&encrypted).unwrap();
        assert_eq!(decrypted, value);
        
        // Non-encrypted field should pass through
        let non_encrypted = manager.encrypt_field("username", "john").unwrap();
        assert_eq!(non_encrypted, "john");
    }
    
    #[test]
    fn test_key_rotation() {
        let config = EncryptionConfig {
            enabled: true,
            encrypted_fields: vec!["email".to_string()],
            key_rotation_days: 0, // Immediate rotation
        };
        
        let manager = EncryptionManager::new(config).unwrap();
        let _key1 = manager.generate_key("key-1".to_string()).unwrap();
        let _key2 = manager.generate_key("key-2".to_string()).unwrap();
        
        // Encrypt with key-1
        manager.set_active_key("key-1".to_string()).unwrap();
        let encrypted1 = manager.encrypt("sensitive data").unwrap();
        assert_eq!(encrypted1.key_id, "key-1");
        
        // Should indicate rotation needed
        assert!(manager.should_rotate_key(&encrypted1));
        
        // Re-encrypt with key-2
        manager.set_active_key("key-2".to_string()).unwrap();
        let encrypted2 = manager.re_encrypt(&encrypted1).unwrap();
        assert_eq!(encrypted2.key_id, "key-2");
        
        // Both should decrypt to same value
        assert_eq!(
            manager.decrypt(&encrypted1).unwrap(),
            manager.decrypt(&encrypted2).unwrap()
        );
    }
}
