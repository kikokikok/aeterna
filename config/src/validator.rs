//! # Configuration Validation
//!
//! Provides validation for all configuration structures using the `validator` crate.

use crate::config::{
    Config, ObservabilityConfig, PostgresConfig, ProviderConfig, QdrantConfig, RedisConfig,
    SyncConfig, ToolConfig,
};
use validator::Validate;

/// Validate configuration structure.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Validates all configuration fields using the `validator` crate.
/// Ensures all required fields are present and within valid ranges.
///
/// ## Usage
/// ```rust,no_run
/// use memory_knowledge_config::{Config, validate};
///
/// let config = Config::default();
/// match validate(&config) {
///     Ok(()) => println!("Configuration is valid"),
///     Err(errors) => println!("Validation errors: {:?}", errors),
/// }
/// ```
///
/// ## Validation Rules
/// ### General
/// - All string fields: minimum length 1, maximum length varies
///
/// ### PostgreSQL
/// - `host`: 1-255 characters
/// - `port`: 1-65535
/// - `database`: 1-63 characters
/// - `username`: 1-63 characters
/// - `password`: 1+ characters
/// - `pool_size`: 1-100
/// - `timeout_seconds`: 1-300
///
/// ### Qdrant
/// - `host`: 1-255 characters
/// - `port`: 1-65535
/// - `collection`: 1-255 characters
/// - `timeout_seconds`: 1-300
///
/// ### Redis
/// - `host`: 1-255 characters
/// - `port`: 1-65535
/// - `db`: 0-15
/// - `pool_size`: 1-100
/// - `timeout_seconds`: 1-300
///
/// ### Sync
/// - `sync_interval_seconds`: 10-3600
/// - `batch_size`: 1-1000
/// - `conflict_resolution`: must be "prefer_knowledge", "prefer_memory", or "manual"
///
/// ### Tools
/// - `host`: 1-255 characters
/// - `port`: 1-65535
/// - `rate_limit_requests_per_minute`: 1-1000
///
/// ### Observability
/// - `logging_level`: must be "trace", "debug", "info", "warn", or "error"
/// - `metrics_port`: 1-65535
pub fn validate(config: &Config) -> Result<(), validator::ValidationErrors> {
    config.validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_postgres_host() {
        let mut config = Config::default();
        config.providers.postgres.host = "".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_postgres_port() {
        let mut config = Config::default();
        config.providers.postgres.port = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_qdrant_port() {
        let mut config = Config::default();
        config.providers.qdrant.port = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_redis_db() {
        let mut config = Config::default();
        config.providers.redis.db = 16;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_sync_interval() {
        let mut config = Config::default();
        config.sync.sync_interval_seconds = 5;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_sync_interval_high() {
        let mut config = Config::default();
        config.sync.sync_interval_seconds = 4000;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_batch_size() {
        let mut config = Config::default();
        config.sync.batch_size = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_conflict_resolution() {
        let mut config = Config::default();
        config.sync.conflict_resolution = "invalid".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_valid_conflict_resolution() {
        let mut config = Config::default();
        config.sync.conflict_resolution = "prefer_memory".to_string();
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_logging_level() {
        let mut config = Config::default();
        config.observability.logging_level = "invalid".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_valid_logging_levels() {
        for level in ["trace", "debug", "info", "warn", "error"] {
            let mut config = Config::default();
            config.observability.logging_level = level.to_string();
            assert!(validate(&config).is_ok());
        }
    }

    #[test]
    fn test_validate_invalid_tools_rate_limit() {
        let mut config = Config::default();
        config.tools.rate_limit_requests_per_minute = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_postgres_pool_size_out_of_range() {
        let mut config = Config::default();
        config.providers.postgres.pool_size = 101;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_postgres_timeout_out_of_range() {
        let mut config = Config::default();
        config.providers.postgres.timeout_seconds = 301;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_qdrant_collection_empty() {
        let mut config = Config::default();
        config.providers.qdrant.collection = "".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_redis_host_empty() {
        let mut config = Config::default();
        config.providers.redis.host = "".to_string();
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_tools_port_zero() {
        let mut config = Config::default();
        config.tools.port = 0;
        assert!(validate(&config).is_err());
    }

    #[test]
    fn test_validate_observability_metrics_port_zero() {
        let mut config = Config::default();
        config.observability.metrics_port = 0;
        assert!(validate(&config).is_err());
    }
}
