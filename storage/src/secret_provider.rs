/// Secret Provider abstraction for managing sensitive Git tokens and keys.
///
/// Supports external secret managers like AWS Secrets Manager, GCP Secret Manager,
/// Azure Key Vault, and HashiCorp Vault.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Secret operation failed: {0}")]
    OperationFailed(String),

    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Format error: {0}")]
    FormatError(String),

    #[error("Retrieval failed: {0}")]
    RetrievalFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum SecretProviderConfig {
    #[serde(rename = "aws-secrets-manager")]
    Aws {
        region: String,
        endpoint: Option<String>,
    },
    #[serde(rename = "vault")]
    Vault {
        address: String,
        token: String,
        mount_path: String,
    },
    #[serde(rename = "local")]
    Local {
        secrets: std::collections::HashMap<String, String>,
    },
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Retrieve a secret value by its identifier
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError>;

    /// Health check for the provider
    async fn is_available(&self) -> bool;
}

/// Mock implementation for AWS Secrets Manager
pub struct AwsSecretProvider {
    region: String,
    _endpoint: Option<String>,
}

impl AwsSecretProvider {
    pub fn new(region: String, endpoint: Option<String>) -> Self {
        Self {
            region,
            _endpoint: endpoint,
        }
    }
}

#[async_trait]
impl SecretProvider for AwsSecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError> {
        // In a real implementation, we would use aws-sdk-secretsmanager
        // For now, returning a mock or placeholder result
        Ok(format!(
            "mock-secret-from-aws-{}-{}",
            self.region, secret_id
        ))
    }

    async fn is_available(&self) -> bool {
        true
    }
}

/// Local development secret provider
pub struct LocalSecretProvider {
    secrets: std::collections::HashMap<String, String>,
}

impl LocalSecretProvider {
    pub fn new(secrets: std::collections::HashMap<String, String>) -> Self {
        Self { secrets }
    }
}

#[async_trait]
impl SecretProvider for LocalSecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError> {
        self.secrets
            .get(secret_id)
            .cloned()
            .ok_or_else(|| SecretError::NotFound(secret_id.to_string()))
    }

    async fn is_available(&self) -> bool {
        true
    }
}

/// HashiCorp Vault secret provider
pub struct VaultSecretProvider {
    client: reqwest::Client,
    address: String,
    token: String,
}

impl VaultSecretProvider {
    pub fn new(address: String, token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            address,
            token,
        }
    }
}

#[async_trait]
impl SecretProvider for VaultSecretProvider {
    async fn get_secret(&self, secret_id: &str) -> Result<String, SecretError> {
        let url = format!("{}/v1/secret/data/{}", self.address, secret_id);
        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await
            .map_err(|e| SecretError::ConnectionFailed(e.to_string()))?;

        if response.status().is_success() {
            let body: serde_json::Value = response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| SecretError::FormatError(e.to_string()))?;
            let token = body["data"]["data"]["token"].as_str().ok_or_else(|| {
                SecretError::FormatError("Token not found in Vault secret".to_string())
            })?;
            Ok(token.to_string())
        } else {
            Err(SecretError::RetrievalFailed(format!(
                "Vault returned error: {}",
                response.status()
            )))
        }
    }

    async fn is_available(&self) -> bool {
        // Simple health check could go here
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ---------------------------------------------------------------------------
    // SecretError — Display
    // ---------------------------------------------------------------------------

    #[test]
    fn test_secret_error_display_variants() {
        let cases: Vec<(&str, SecretError)> = vec![
            ("Secret operation failed: boom", SecretError::OperationFailed("boom".into())),
            ("Secret not found: my-key", SecretError::NotFound("my-key".into())),
            ("Authentication failed: bad token", SecretError::AuthFailed("bad token".into())),
            ("Invalid configuration: missing field", SecretError::ConfigError("missing field".into())),
            ("Connection failed: timeout", SecretError::ConnectionFailed("timeout".into())),
            ("Format error: bad json", SecretError::FormatError("bad json".into())),
            ("Retrieval failed: 403", SecretError::RetrievalFailed("403".into())),
        ];

        for (expected, err) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    // ---------------------------------------------------------------------------
    // SecretProviderConfig — serde round-trip
    // ---------------------------------------------------------------------------

    #[test]
    fn test_secret_provider_config_serde_aws() {
        let cfg = SecretProviderConfig::Aws {
            region: "us-east-1".to_string(),
            endpoint: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecretProviderConfig = serde_json::from_str(&json).unwrap();
        match back {
            SecretProviderConfig::Aws { region, endpoint } => {
                assert_eq!(region, "us-east-1");
                assert!(endpoint.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_secret_provider_config_serde_vault() {
        let cfg = SecretProviderConfig::Vault {
            address: "http://vault:8200".to_string(),
            token: "root".to_string(),
            mount_path: "secret".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecretProviderConfig = serde_json::from_str(&json).unwrap();
        match back {
            SecretProviderConfig::Vault { address, token, mount_path } => {
                assert_eq!(address, "http://vault:8200");
                assert_eq!(token, "root");
                assert_eq!(mount_path, "secret");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_secret_provider_config_serde_local() {
        let mut secrets = HashMap::new();
        secrets.insert("key-a".to_string(), "value-a".to_string());
        let cfg = SecretProviderConfig::Local { secrets: secrets.clone() };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SecretProviderConfig = serde_json::from_str(&json).unwrap();
        match back {
            SecretProviderConfig::Local { secrets: s } => assert_eq!(s, secrets),
            _ => panic!("wrong variant"),
        }
    }

    // ---------------------------------------------------------------------------
    // LocalSecretProvider
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_local_provider_get_existing_secret() {
        let mut secrets = HashMap::new();
        secrets.insert("db-password".to_string(), "super-secret".to_string());
        let provider = LocalSecretProvider::new(secrets);

        let value = provider.get_secret("db-password").await.unwrap();
        assert_eq!(value, "super-secret");
    }

    #[tokio::test]
    async fn test_local_provider_get_missing_secret_returns_not_found() {
        let provider = LocalSecretProvider::new(HashMap::new());
        let err = provider.get_secret("nonexistent-key").await.unwrap_err();
        assert!(matches!(err, SecretError::NotFound(_)));
        assert!(err.to_string().contains("nonexistent-key"));
    }

    #[tokio::test]
    async fn test_local_provider_is_available() {
        let provider = LocalSecretProvider::new(HashMap::new());
        assert!(provider.is_available().await);
    }

    #[tokio::test]
    async fn test_local_provider_multiple_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("k1".to_string(), "v1".to_string());
        secrets.insert("k2".to_string(), "v2".to_string());
        secrets.insert("k3".to_string(), "v3".to_string());
        let provider = LocalSecretProvider::new(secrets);

        assert_eq!(provider.get_secret("k1").await.unwrap(), "v1");
        assert_eq!(provider.get_secret("k2").await.unwrap(), "v2");
        assert_eq!(provider.get_secret("k3").await.unwrap(), "v3");
    }

    // ---------------------------------------------------------------------------
    // AwsSecretProvider — mock (returns deterministic string, no real AWS call)
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_aws_provider_returns_mock_secret_with_region_and_id() {
        let provider = AwsSecretProvider::new("eu-west-1".to_string(), None);
        let value = provider.get_secret("my-token").await.unwrap();
        assert!(value.contains("eu-west-1"), "Expected region in mock secret: {}", value);
        assert!(value.contains("my-token"), "Expected secret_id in mock secret: {}", value);
    }

    #[tokio::test]
    async fn test_aws_provider_is_available() {
        let provider = AwsSecretProvider::new("us-west-2".to_string(), Some("http://localhost:4566".to_string()));
        assert!(provider.is_available().await);
    }

    #[tokio::test]
    async fn test_aws_provider_with_endpoint() {
        let provider = AwsSecretProvider::new("us-east-1".to_string(), Some("http://localstack:4566".to_string()));
        // Endpoint is stored but the mock ignores it — just verify construction & call succeed.
        let result = provider.get_secret("secret-abc").await;
        assert!(result.is_ok());
    }
}
