//! Application state for the OPAL Data Fetcher.

use sqlx::PgPool;
use std::sync::Arc;

use crate::error::{FetcherError, Result};

/// Configuration for the OPAL Data Fetcher server.
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    /// `PostgreSQL` connection URL.
    pub database_url: String,
    /// Host to bind the server to.
    pub host: String,
    /// Port to bind the server to.
    pub port: u16,
    /// Enable `PostgreSQL` LISTEN/NOTIFY for real-time updates.
    pub enable_listener: bool,
    /// OPAL server URL for publishing updates (optional).
    pub opal_server_url: Option<String>,
    /// Maximum database pool connections.
    pub max_connections: u32
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_listener: true,
            opal_server_url: None,
            max_connections: 10
        }
    }
}

impl FetcherConfig {
    /// Creates a new configuration from environment variables.
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| FetcherError::Configuration("DATABASE_URL not set".to_string()))?;

        Ok(Self {
            database_url,
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            enable_listener: std::env::var("ENABLE_LISTENER")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            opal_server_url: std::env::var("OPAL_SERVER_URL").ok(),
            max_connections: std::env::var("MAX_CONNECTIONS")
                .ok()
                .and_then(|c| c.parse().ok())
                .unwrap_or(10)
        })
    }

    /// Creates a builder for configuration.
    #[must_use]
    pub fn builder() -> FetcherConfigBuilder {
        FetcherConfigBuilder::default()
    }
}

/// Builder for `FetcherConfig`.
#[derive(Default)]
pub struct FetcherConfigBuilder {
    database_url: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    enable_listener: Option<bool>,
    opal_server_url: Option<String>,
    max_connections: Option<u32>
}

impl FetcherConfigBuilder {
    /// Sets the database URL.
    #[must_use]
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Sets the host to bind to.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Sets the port to bind to.
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Enables or disables the `PostgreSQL` listener.
    #[must_use]
    pub fn enable_listener(mut self, enable: bool) -> Self {
        self.enable_listener = Some(enable);
        self
    }

    /// Sets the OPAL server URL.
    #[must_use]
    pub fn opal_server_url(mut self, url: impl Into<String>) -> Self {
        self.opal_server_url = Some(url.into());
        self
    }

    /// Sets the maximum number of database connections.
    #[must_use]
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = Some(max);
        self
    }

    /// Builds the configuration.
    pub fn build(self) -> Result<FetcherConfig> {
        let database_url = self
            .database_url
            .ok_or_else(|| FetcherError::Configuration("database_url is required".to_string()))?;

        Ok(FetcherConfig {
            database_url,
            host: self.host.unwrap_or_else(|| "0.0.0.0".to_string()),
            port: self.port.unwrap_or(8080),
            enable_listener: self.enable_listener.unwrap_or(true),
            opal_server_url: self.opal_server_url,
            max_connections: self.max_connections.unwrap_or(10)
        })
    }
}

/// Shared application state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// `PostgreSQL` connection pool.
    pub pool: PgPool,
    /// Server configuration.
    pub config: Arc<FetcherConfig>
}

impl AppState {
    /// Creates a new application state.
    pub async fn new(config: FetcherConfig) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&config.database_url)
            .await?;

        Ok(Self {
            pool,
            config: Arc::new(config)
        })
    }

    /// Creates application state from an existing pool (useful for testing).
    #[must_use]
    pub fn with_pool(pool: PgPool, config: FetcherConfig) -> Self {
        Self {
            pool,
            config: Arc::new(config)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = FetcherConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.enable_listener);
        assert_eq!(config.max_connections, 10);
        assert!(config.opal_server_url.is_none());
    }

    #[test]
    fn test_config_builder_success() {
        let config = FetcherConfig::builder()
            .database_url("postgres://localhost/test")
            .host("127.0.0.1")
            .port(3000)
            .enable_listener(false)
            .opal_server_url("http://opal:8181")
            .max_connections(20)
            .build()
            .unwrap();

        assert_eq!(config.database_url, "postgres://localhost/test");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(!config.enable_listener);
        assert_eq!(config.opal_server_url, Some("http://opal:8181".to_string()));
        assert_eq!(config.max_connections, 20);
    }

    #[test]
    fn test_config_builder_missing_database_url() {
        let result = FetcherConfig::builder().host("127.0.0.1").build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, FetcherError::Configuration(_)));
    }

    #[test]
    fn test_config_builder_defaults() {
        let config = FetcherConfig::builder()
            .database_url("postgres://localhost/test")
            .build()
            .unwrap();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.enable_listener);
        assert_eq!(config.max_connections, 10);
    }
}
