use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpSyncConfig {
    pub provider: IdpProvider,
    pub sync_interval_seconds: u64,
    pub batch_size: usize,
    pub database_url: String,
    pub webhook_port: u16,
    pub webhook_secret: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_retry_config")]
    pub retry: RetryConfig
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum IdpProvider {
    Okta(OktaConfig),
    AzureAd(AzureAdConfig)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OktaConfig {
    pub domain: String,
    pub api_token: String,
    #[serde(default)]
    pub scim_enabled: bool,
    pub group_filter: Option<String>,
    pub user_filter: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureAdConfig {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub group_filter: Option<String>,
    #[serde(default)]
    pub include_nested_groups: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64
}

fn default_retry_config() -> RetryConfig {
    RetryConfig {
        max_retries: 3,
        initial_backoff_ms: 1000,
        max_backoff_ms: 30000
    }
}

impl Default for IdpSyncConfig {
    fn default() -> Self {
        Self {
            provider: IdpProvider::Okta(OktaConfig {
                domain: String::new(),
                api_token: String::new(),
                scim_enabled: false,
                group_filter: None,
                user_filter: None
            }),
            sync_interval_seconds: 300,
            batch_size: 100,
            database_url: String::new(),
            webhook_port: 8090,
            webhook_secret: None,
            dry_run: false,
            retry: default_retry_config()
        }
    }
}

impl IdpSyncConfig {
    pub fn sync_interval(&self) -> Duration {
        Duration::from_secs(self.sync_interval_seconds)
    }
}
