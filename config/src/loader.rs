//! # Environment Variable Loader
//!
//! Loads configuration from environment variables following 12-factor app
//! principles.
//!
//! # Naming Convention
//! - `MK_*`: Memory-related settings
//! - `KK_*`: Knowledge-related settings
//! - `SY_*`: Sync-related settings
//! - `TL_*`: Tool-related settings
//! - `PG_*`: PostgreSQL settings
//! - `QD_*`: Qdrant settings
//! - `RD_*`: Redis settings
//! - `OB_*`: Observability settings

use crate::config::{
    Config, ContentionAlertConfig, GraphConfig, MemoryConfig, ObservabilityConfig, PostgresConfig,
    ProviderConfig, QdrantConfig, ReasoningConfig, RedisConfig, SyncConfig, ToolConfig,
};
use std::env;

/// Load configuration from environment variables.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Loads configuration from environment variables following 12-factor app
/// principles. Environment variables override default values but can be
/// overridden by CLI arguments.
///
/// ## Usage
/// ```rust,no_run
/// use config::load_from_env;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = load_from_env()?;
///     println!("PostgreSQL host: {}", config.providers.postgres.host);
///     Ok(())
/// }
/// ```
///
/// ## Environment Variables
/// ### General Settings
/// - `MK_LOG_LEVEL`: Logging level (trace/debug/info/warn/error)
///
/// ### PostgreSQL Settings (`PG_*`)
/// - `PG_HOST`: Database host (default: "localhost")
/// - `PG_PORT`: Database port (default: 5432)
/// - `PG_DATABASE`: Database name (default: "memory_knowledge")
/// - `PG_USERNAME`: Database user (default: "postgres")
/// - `PG_PASSWORD`: Database password (default: "")
/// - `PG_POOL_SIZE`: Connection pool size (default: 10)
/// - `PG_TIMEOUT_SECONDS`: Connection timeout in seconds (default: 30)
///
/// ### Qdrant Settings (`QD_*`)
/// - `QD_HOST`: Qdrant host (default: "localhost")
/// - `QD_PORT`: Qdrant port (default: 6333)
/// - `QD_COLLECTION`: Collection name (default: "memory_embeddings")
/// - `QD_TIMEOUT_SECONDS`: Request timeout in seconds (default: 30)
///
/// ### Redis Settings (`RD_*`)
/// - `RD_HOST`: Redis host (default: "localhost")
/// - `RD_PORT`: Redis port (default: 6379)
/// - `RD_DB`: Redis database number (default: 0)
/// - `RD_POOL_SIZE`: Connection pool size (default: 10)
/// - `RD_TIMEOUT_SECONDS`: Connection timeout in seconds (default: 30)
///
/// ### Sync Settings (`SY_*`)
/// - `SY_ENABLED`: Enable sync (true/false, default: true)
/// - `SY_SYNC_INTERVAL_SECONDS`: Sync interval (default: 60)
/// - `SY_BATCH_SIZE`: Batch size (default: 100)
/// - `SY_CHECKPOINT_ENABLED`: Enable checkpointing (true/false, default: true)
/// - `SY_CONFLICT_RESOLUTION`: Conflict resolution
///   (prefer_knowledge/prefer_memory/manual, default: prefer_knowledge)
///
/// ### Tools Settings (`TL_*`)
/// - `TL_ENABLED`: Enable MCP server (true/false, default: true)
/// - `TL_HOST`: Server host (default: "localhost")
/// - `TL_PORT`: Server port (default: 8080)
/// - `TL_API_KEY`: API key for authentication (optional)
/// - `TL_RATE_LIMIT_REQUESTS_PER_MINUTE`: Rate limit (default: 60)
///
/// ### Observability Settings (`OB_*`)
/// - `OB_METRICS_ENABLED`: Enable metrics (true/false, default: true)
/// - `OB_TRACING_ENABLED`: Enable tracing (true/false, default: true)
/// - `OB_LOGGING_LEVEL`: Logging level (trace/debug/info/warn/error, default:
///   "info")
/// - `OB_METRICS_PORT`: Metrics server port (default: 9090)
pub fn load_from_env() -> Result<Config, Box<dyn std::error::Error>> {
    let config = Config {
        providers: load_provider_from_env()?,
        sync: load_sync_from_env()?,
        memory: load_memory_from_env()?,
        tools: load_tools_from_env()?,
        observability: load_observability_from_env()?,
        deployment: load_deployment_from_env()?,
        job: load_job_from_env()?,
        cca: crate::cca::CcaConfig::default(),
    };

    Ok(config)
}

fn load_deployment_from_env() -> Result<crate::config::DeploymentConfig, Box<dyn std::error::Error>>
{
    Ok(crate::config::DeploymentConfig {
        mode: env::var("AETERNA_DEPLOYMENT_MODE").unwrap_or_else(|_| "local".to_string()),
        remote_url: env::var("AETERNA_REMOTE_GOVERNANCE_URL").ok(),
        sync_enabled: parse_env("AETERNA_SYNC_ENABLED").unwrap_or(true),
    })
}

fn load_job_from_env() -> Result<crate::config::JobConfig, Box<dyn std::error::Error>> {
    Ok(crate::config::JobConfig {
        lock_ttl_seconds: parse_env("AETERNA_JOB_LOCK_TTL_SECONDS").unwrap_or(2100),
        job_timeout_seconds: parse_env("AETERNA_JOB_TIMEOUT_SECONDS").unwrap_or(1800),
        deduplication_window_seconds: parse_env("AETERNA_JOB_DEDUP_WINDOW_SECONDS").unwrap_or(300),
        checkpoint_interval: parse_env("AETERNA_JOB_CHECKPOINT_INTERVAL").unwrap_or(100),
        graceful_shutdown_timeout_seconds: parse_env("AETERNA_JOB_SHUTDOWN_TIMEOUT_SECONDS")
            .unwrap_or(30),
    })
}

fn load_provider_from_env() -> Result<ProviderConfig, Box<dyn std::error::Error>> {
    Ok(ProviderConfig {
        postgres: load_postgres_from_env()?,
        qdrant: load_qdrant_from_env()?,
        redis: load_redis_from_env()?,
        graph: load_graph_from_env()?,
    })
}

fn load_postgres_from_env() -> Result<PostgresConfig, Box<dyn std::error::Error>> {
    Ok(PostgresConfig {
        host: env::var("PG_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: parse_env("PG_PORT").unwrap_or(5432),
        database: env::var("PG_DATABASE").unwrap_or_else(|_| "memory_knowledge".to_string()),
        username: env::var("PG_USERNAME").unwrap_or_else(|_| "postgres".to_string()),
        password: env::var("PG_PASSWORD").unwrap_or_default(),
        pool_size: parse_env("PG_POOL_SIZE").unwrap_or(10),
        timeout_seconds: parse_env("PG_TIMEOUT_SECONDS").unwrap_or(30),
    })
}

fn load_qdrant_from_env() -> Result<QdrantConfig, Box<dyn std::error::Error>> {
    Ok(QdrantConfig {
        host: env::var("QD_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: parse_env("QD_PORT").unwrap_or(6333),
        collection: env::var("QD_COLLECTION").unwrap_or_else(|_| "memory_embeddings".to_string()),
        timeout_seconds: parse_env("QD_TIMEOUT_SECONDS").unwrap_or(30),
    })
}

fn load_redis_from_env() -> Result<RedisConfig, Box<dyn std::error::Error>> {
    Ok(RedisConfig {
        host: env::var("RD_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: parse_env("RD_PORT").unwrap_or(6379),
        db: parse_env("RD_DB").unwrap_or(0),
        pool_size: parse_env("RD_POOL_SIZE").unwrap_or(10),
        timeout_seconds: parse_env("RD_TIMEOUT_SECONDS").unwrap_or(30),
    })
}

fn load_graph_from_env() -> Result<GraphConfig, Box<dyn std::error::Error>> {
    Ok(GraphConfig {
        enabled: parse_env("GR_ENABLED").unwrap_or(true),
        database_path: env::var("GR_DATABASE_PATH").unwrap_or_else(|_| ":memory:".to_string()),
        s3_bucket: env::var("GR_S3_BUCKET").ok(),
        s3_prefix: env::var("GR_S3_PREFIX").ok(),
        s3_endpoint: env::var("GR_S3_ENDPOINT").ok(),
        s3_region: env::var("GR_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        contention_alerts: ContentionAlertConfig::default(),
    })
}

fn load_sync_from_env() -> Result<SyncConfig, Box<dyn std::error::Error>> {
    Ok(SyncConfig {
        enabled: parse_env("SY_ENABLED").unwrap_or(true),
        sync_interval_seconds: parse_env("SY_SYNC_INTERVAL_SECONDS").unwrap_or(60),
        batch_size: parse_env("SY_BATCH_SIZE").unwrap_or(100),
        checkpoint_enabled: parse_env("SY_CHECKPOINT_ENABLED").unwrap_or(true),
        conflict_resolution: env::var("SY_CONFLICT_RESOLUTION")
            .unwrap_or_else(|_| "prefer_knowledge".to_string()),
    })
}

fn load_memory_from_env() -> Result<MemoryConfig, Box<dyn std::error::Error>> {
    Ok(MemoryConfig {
        promotion_threshold: parse_env("MK_PROMOTION_THRESHOLD").unwrap_or(0.8),
        decay_interval_secs: parse_env("MK_DECAY_INTERVAL_SECS").unwrap_or(86400),
        decay_rate: parse_env("MK_DECAY_RATE").unwrap_or(0.05),
        optimization_trigger_count: parse_env("MK_OPTIMIZATION_TRIGGER_COUNT").unwrap_or(100),
        layer_summary_configs: std::collections::HashMap::new(),
        reasoning: ReasoningConfig::default(),
    })
}

fn load_tools_from_env() -> Result<ToolConfig, Box<dyn std::error::Error>> {
    Ok(ToolConfig {
        enabled: parse_env("TL_ENABLED").unwrap_or(true),
        host: env::var("TL_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: parse_env("TL_PORT").unwrap_or(8080),
        api_key: env::var("TL_API_KEY").ok(),
        rate_limit_requests_per_minute: parse_env("TL_RATE_LIMIT_REQUESTS_PER_MINUTE")
            .unwrap_or(60),
    })
}

fn load_observability_from_env() -> Result<ObservabilityConfig, Box<dyn std::error::Error>> {
    Ok(ObservabilityConfig {
        metrics_enabled: parse_env("OB_METRICS_ENABLED").unwrap_or(true),
        tracing_enabled: parse_env("OB_TRACING_ENABLED").unwrap_or(true),
        logging_level: env::var("OB_LOGGING_LEVEL").unwrap_or_else(|_| "info".to_string()),
        metrics_port: parse_env("OB_METRICS_PORT").unwrap_or(9090),
    })
}

fn parse_env<T>(key: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(key) {
        Ok(s) => s
            .parse::<T>()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>),
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_load_from_env_defaults() {
        unsafe {
            env::remove_var("PG_HOST");
            env::remove_var("QD_HOST");
            env::remove_var("RD_HOST");
            env::remove_var("SY_ENABLED");
            env::remove_var("TL_PORT");
            env::remove_var("OB_LOGGING_LEVEL");
        }
        let config = load_from_env().unwrap();
        assert_eq!(config.providers.postgres.host, "localhost");
        assert_eq!(config.providers.qdrant.host, "localhost");
        assert_eq!(config.providers.redis.host, "localhost");
        assert_eq!(config.sync.enabled, true);
        assert_eq!(config.tools.port, 8080);
        assert_eq!(config.observability.logging_level, "info");
    }

    #[test]
    #[serial]
    fn test_load_from_env_overrides() {
        unsafe {
            env::set_var("PG_HOST", "testhost");
            env::set_var("PG_PORT", "9999");
            env::set_var("SY_ENABLED", "false");
        }

        let config = load_from_env().unwrap();
        assert_eq!(config.providers.postgres.host, "testhost");
        assert_eq!(config.providers.postgres.port, 9999);
        assert_eq!(config.sync.enabled, false);

        unsafe {
            env::remove_var("PG_HOST");
            env::remove_var("PG_PORT");
            env::remove_var("SY_ENABLED");
        }
    }

    #[test]
    fn test_parse_env_missing() {
        let result: Result<u32, _> = parse_env("NONEXISTENT_VAR");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_env_valid_string() {
        unsafe {
            env::set_var("TEST_VAR", "test_value");
        }
        let result: Result<String, _> = parse_env("TEST_VAR");
        assert_eq!(result.unwrap(), "test_value");
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_parse_env_valid_number() {
        unsafe {
            env::set_var("TEST_VAR", "123");
        }
        let result: Result<u32, _> = parse_env("TEST_VAR");
        assert_eq!(result.unwrap(), 123);
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_parse_env_valid_number_with_parse_env() {
        unsafe {
            env::set_var("TEST_VAR", "123");
        }
        let result: Result<u32, _> = parse_env("TEST_VAR");
        assert_eq!(result.unwrap(), 123);
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_parse_env_invalid_number() {
        unsafe {
            env::set_var("TEST_VAR", "not_a_number");
        }
        let result: Result<u32, _> = parse_env("TEST_VAR");
        assert!(result.is_err());
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    #[serial]
    fn test_load_postgres_from_env() {
        unsafe {
            env::set_var("PG_HOST", "customhost");
            env::set_var("PG_PORT", "5433");
            env::set_var("PG_DATABASE", "testdb");
            env::set_var("PG_USERNAME", "testuser");
            env::set_var("PG_PASSWORD", "testpass");
            env::set_var("PG_POOL_SIZE", "20");
            env::set_var("PG_TIMEOUT_SECONDS", "60");
        }

        let postgres = load_postgres_from_env().unwrap();

        unsafe {
            env::remove_var("PG_HOST");
            env::remove_var("PG_PORT");
            env::remove_var("PG_DATABASE");
            env::remove_var("PG_USERNAME");
            env::remove_var("PG_PASSWORD");
            env::remove_var("PG_POOL_SIZE");
            env::remove_var("PG_TIMEOUT_SECONDS");
        }

        assert_eq!(postgres.host, "customhost");
        assert_eq!(postgres.port, 5433);
        assert_eq!(postgres.database, "testdb");
        assert_eq!(postgres.username, "testuser");
        assert_eq!(postgres.password, "testpass");
        assert_eq!(postgres.pool_size, 20);
        assert_eq!(postgres.timeout_seconds, 60);
    }

    #[test]
    #[serial]
    fn test_load_qdrant_from_env() {
        unsafe {
            env::set_var("QD_HOST", "qdranthost");
            env::set_var("QD_PORT", "7333");
            env::set_var("QD_COLLECTION", "test_collection");
            env::set_var("QD_TIMEOUT_SECONDS", "45");
        }

        let qdrant = load_qdrant_from_env().unwrap();
        assert_eq!(qdrant.host, "qdranthost");
        assert_eq!(qdrant.port, 7333);
        assert_eq!(qdrant.collection, "test_collection");
        assert_eq!(qdrant.timeout_seconds, 45);

        unsafe {
            env::remove_var("QD_HOST");
            env::remove_var("QD_PORT");
            env::remove_var("QD_COLLECTION");
            env::remove_var("QD_TIMEOUT_SECONDS");
        }
    }

    #[test]
    #[serial]
    fn test_load_redis_from_env() {
        unsafe {
            env::set_var("RD_HOST", "redishost");
            env::set_var("RD_PORT", "6380");
            env::set_var("RD_DB", "1");
            env::set_var("RD_POOL_SIZE", "15");
            env::set_var("RD_TIMEOUT_SECONDS", "45");
        }

        let redis = load_redis_from_env().unwrap();
        assert_eq!(redis.host, "redishost");
        assert_eq!(redis.port, 6380);
        assert_eq!(redis.db, 1);
        assert_eq!(redis.pool_size, 15);
        assert_eq!(redis.timeout_seconds, 45);

        unsafe {
            env::remove_var("RD_HOST");
            env::remove_var("RD_PORT");
            env::remove_var("RD_DB");
            env::remove_var("RD_POOL_SIZE");
            env::remove_var("RD_TIMEOUT_SECONDS");
        }
    }
}
