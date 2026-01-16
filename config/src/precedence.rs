//! # Configuration Precedence
//!
//! Merges configuration from multiple sources with precedence rules.
//!
//! # Precedence Order
//! 1. CLI arguments (highest priority)
//! 2. Environment variables
//! 3. Configuration file
//! 4. Default values (lowest priority)

use crate::config::{
    Config, ObservabilityConfig, PostgresConfig, QdrantConfig, RedisConfig, SyncConfig, ToolConfig,
};

/// Merge multiple configuration sources with precedence.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Merges configuration from multiple sources following precedence rules:
/// CLI arguments > environment variables > config file > defaults.
///
/// ## Usage
/// ```rust,no_run
/// use config::{Config, load_from_env, load_from_file, merge_configs};
/// use std::path::Path;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let defaults = Config::default();
///     let from_file = load_from_file(Path::new("config.toml"))?;
///     let from_env = load_from_env()?;
///
///     let _config = merge_configs(defaults, from_file, "file", from_env, "env", None, "cli");
///     Ok(())
/// }
/// ```
///
/// ## Deep Merge
/// Performs deep merge on nested structures (providers, sync, tools,
/// observability). String fields are overridden, not concatenated.
pub fn merge_configs(
    defaults: Config,
    file_config: Config,
    file_source_name: &str,
    env_config: Config,
    env_source_name: &str,
    cli_config: Option<Config>,
    cli_source_name: &str,
) -> Config {
    let mut config = defaults;

    config = merge_with_logging(config, file_config, file_source_name);
    config = merge_with_logging(config, env_config, env_source_name);

    if let Some(cli) = cli_config {
        config = merge_with_logging(config, cli, cli_source_name);
    }

    config
}

fn merge_with_logging(mut base: Config, override_config: Config, source_name: &str) -> Config {
    let mut changes = Vec::new();

    let mut temp_postgres = base.providers.postgres.clone();
    merge_postgres(
        &mut temp_postgres,
        &override_config.providers.postgres,
        source_name,
        &mut changes,
    );
    if !changes.is_empty() {
        base.providers.postgres = temp_postgres;
    }

    let mut qdrant_changes = Vec::new();
    let mut temp_qdrant = base.providers.qdrant.clone();
    merge_qdrant(
        &mut temp_qdrant,
        &override_config.providers.qdrant,
        source_name,
        &mut qdrant_changes,
    );
    if !qdrant_changes.is_empty() {
        base.providers.qdrant = temp_qdrant;
        changes.extend(qdrant_changes);
    }

    let mut redis_changes = Vec::new();
    let mut temp_redis = base.providers.redis.clone();
    merge_redis(
        &mut temp_redis,
        &override_config.providers.redis,
        source_name,
        &mut redis_changes,
    );
    if !redis_changes.is_empty() {
        base.providers.redis = temp_redis;
        changes.extend(redis_changes);
    }

    let mut sync_changes = Vec::new();
    let mut temp_sync = base.sync.clone();
    merge_sync(
        &mut temp_sync,
        &override_config.sync,
        source_name,
        &mut sync_changes,
    );
    if !sync_changes.is_empty() {
        base.sync = temp_sync;
        changes.extend(sync_changes);
    }

    let mut tool_changes = Vec::new();
    let mut temp_tools = base.tools.clone();
    merge_tools(
        &mut temp_tools,
        &override_config.tools,
        source_name,
        &mut tool_changes,
    );
    if !tool_changes.is_empty() {
        base.tools = temp_tools;
        changes.extend(tool_changes);
    }

    let mut obs_changes = Vec::new();
    let mut temp_obs = base.observability.clone();
    merge_observability(
        &mut temp_obs,
        &override_config.observability,
        source_name,
        &mut obs_changes,
    );
    if !obs_changes.is_empty() {
        base.observability = temp_obs;
        changes.extend(obs_changes);
    }

    if !changes.is_empty() {
        tracing::info!("Configuration from {}: {:?}", source_name, changes);
    }

    base
}

fn merge_postgres(
    base: &mut PostgresConfig,
    override_config: &PostgresConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if override_config.host != "localhost" && override_config.host != base.host {
        changes.push(format!(
            "providers.postgres.host = {}",
            override_config.host
        ));
        base.host.clone_from(&override_config.host);
    }
    if override_config.port != 5432 && override_config.port != base.port {
        changes.push(format!(
            "providers.postgres.port = {}",
            override_config.port
        ));
        base.port = override_config.port;
    }
    if override_config.database != "memory_knowledge" && override_config.database != base.database {
        changes.push(format!(
            "providers.postgres.database = {}",
            override_config.database
        ));
        base.database.clone_from(&override_config.database);
    }
    if override_config.username != "postgres" && override_config.username != base.username {
        changes.push(format!(
            "providers.postgres.username = {}",
            override_config.username
        ));
        base.username.clone_from(&override_config.username);
    }
    if !override_config.password.is_empty() && override_config.password != base.password {
        changes.push("providers.postgres.password = ***".to_string());
        base.password.clone_from(&override_config.password);
    }
    if override_config.pool_size != 10 && override_config.pool_size != base.pool_size {
        changes.push(format!(
            "providers.postgres.pool_size = {}",
            override_config.pool_size
        ));
        base.pool_size = override_config.pool_size;
    }
    if override_config.timeout_seconds != 30
        && override_config.timeout_seconds != base.timeout_seconds
    {
        changes.push(format!(
            "providers.postgres.timeout_seconds = {}",
            override_config.timeout_seconds
        ));
        base.timeout_seconds = override_config.timeout_seconds;
    }
}

fn merge_qdrant(
    base: &mut QdrantConfig,
    override_config: &QdrantConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if override_config.host != "localhost" && override_config.host != base.host {
        changes.push(format!("providers.qdrant.host = {}", override_config.host));
        base.host.clone_from(&override_config.host);
    }
    if override_config.port != 6333 && override_config.port != base.port {
        changes.push(format!("providers.qdrant.port = {}", override_config.port));
        base.port = override_config.port;
    }
    if override_config.collection != "memory_embeddings"
        && override_config.collection != base.collection
    {
        changes.push(format!(
            "providers.qdrant.collection = {}",
            override_config.collection
        ));
        base.collection.clone_from(&override_config.collection);
    }
    if override_config.timeout_seconds != 30
        && override_config.timeout_seconds != base.timeout_seconds
    {
        changes.push(format!(
            "providers.qdrant.timeout_seconds = {}",
            override_config.timeout_seconds
        ));
        base.timeout_seconds = override_config.timeout_seconds;
    }
}

fn merge_redis(
    base: &mut RedisConfig,
    override_config: &RedisConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if override_config.host != "localhost" && override_config.host != base.host {
        changes.push(format!("providers.redis.host = {}", override_config.host));
        base.host.clone_from(&override_config.host);
    }
    if override_config.port != 6379 && override_config.port != base.port {
        changes.push(format!("providers.redis.port = {}", override_config.port));
        base.port = override_config.port;
    }
    if override_config.db != 0 && override_config.db != base.db {
        changes.push(format!("providers.redis.db = {}", override_config.db));
        base.db = override_config.db;
    }
    if override_config.pool_size != 10 && override_config.pool_size != base.pool_size {
        changes.push(format!(
            "providers.redis.pool_size = {}",
            override_config.pool_size
        ));
        base.pool_size = override_config.pool_size;
    }
    if override_config.timeout_seconds != 30
        && override_config.timeout_seconds != base.timeout_seconds
    {
        changes.push(format!(
            "providers.redis.timeout_seconds = {}",
            override_config.timeout_seconds
        ));
        base.timeout_seconds = override_config.timeout_seconds;
    }
}

fn merge_sync(
    base: &mut SyncConfig,
    override_config: &SyncConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if !override_config.enabled && override_config.enabled != base.enabled {
        changes.push(format!("sync.enabled = {}", override_config.enabled));
        base.enabled = override_config.enabled;
    }
    if override_config.sync_interval_seconds != 60
        && override_config.sync_interval_seconds != base.sync_interval_seconds
    {
        changes.push(format!(
            "sync.sync_interval_seconds = {}",
            override_config.sync_interval_seconds
        ));
        base.sync_interval_seconds = override_config.sync_interval_seconds;
    }
    if override_config.batch_size != 100 && override_config.batch_size != base.batch_size {
        changes.push(format!("sync.batch_size = {}", override_config.batch_size));
        base.batch_size = override_config.batch_size;
    }
    if !override_config.checkpoint_enabled
        && override_config.checkpoint_enabled != base.checkpoint_enabled
    {
        changes.push(format!(
            "sync.checkpoint_enabled = {}",
            override_config.checkpoint_enabled
        ));
        base.checkpoint_enabled = override_config.checkpoint_enabled;
    }
    if override_config.conflict_resolution != "prefer_knowledge"
        && override_config.conflict_resolution != base.conflict_resolution
    {
        changes.push(format!(
            "sync.conflict_resolution = {}",
            override_config.conflict_resolution
        ));
        base.conflict_resolution
            .clone_from(&override_config.conflict_resolution);
    }
}

fn merge_tools(
    base: &mut ToolConfig,
    override_config: &ToolConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if !override_config.enabled && override_config.enabled != base.enabled {
        changes.push(format!("tools.enabled = {}", override_config.enabled));
        base.enabled = override_config.enabled;
    }
    if override_config.host != "localhost" && override_config.host != base.host {
        changes.push(format!("tools.host = {}", override_config.host));
        base.host.clone_from(&override_config.host);
    }
    if override_config.port != 8080 && override_config.port != base.port {
        changes.push(format!("tools.port = {}", override_config.port));
        base.port = override_config.port;
    }
    if override_config.api_key.is_some() && override_config.api_key != base.api_key {
        match (&override_config.api_key, &base.api_key) {
            (Some(_), None) => changes.push("tools.api_key = ***".to_string()),
            (Some(new_key), Some(old_key)) if new_key != old_key => {
                changes.push("tools.api_key = ***".to_string())
            }
            _ => {}
        }
        base.api_key.clone_from(&override_config.api_key);
    }
    if override_config.rate_limit_requests_per_minute != 60
        && override_config.rate_limit_requests_per_minute != base.rate_limit_requests_per_minute
    {
        changes.push(format!(
            "tools.rate_limit_requests_per_minute = {}",
            override_config.rate_limit_requests_per_minute
        ));
        base.rate_limit_requests_per_minute = override_config.rate_limit_requests_per_minute;
    }
}

fn merge_observability(
    base: &mut ObservabilityConfig,
    override_config: &ObservabilityConfig,
    _source: &str,
    changes: &mut Vec<String>,
) {
    if !override_config.metrics_enabled && override_config.metrics_enabled != base.metrics_enabled {
        changes.push(format!(
            "observability.metrics_enabled = {}",
            override_config.metrics_enabled
        ));
        base.metrics_enabled = override_config.metrics_enabled;
    }
    if !override_config.tracing_enabled && override_config.tracing_enabled != base.tracing_enabled {
        changes.push(format!(
            "observability.tracing_enabled = {}",
            override_config.tracing_enabled
        ));
        base.tracing_enabled = override_config.tracing_enabled;
    }
    if override_config.logging_level != "info"
        && override_config.logging_level != base.logging_level
    {
        changes.push(format!(
            "observability.logging_level = {}",
            override_config.logging_level
        ));
        base.logging_level
            .clone_from(&override_config.logging_level);
    }
    if override_config.metrics_port != 9090 && override_config.metrics_port != base.metrics_port {
        changes.push(format!(
            "observability.metrics_port = {}",
            override_config.metrics_port
        ));
        base.metrics_port = override_config.metrics_port;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderConfig;

    #[test]
    fn test_merge_configs_precedence() {
        let defaults = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    host: "default_host".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let file_config = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    host: "file_host".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let env_config = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    port: 9999,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = merge_configs(
            defaults,
            file_config,
            "file",
            env_config,
            "env",
            None,
            "cli",
        );

        assert_eq!(merged.providers.postgres.host, "file_host");
        assert_eq!(merged.providers.postgres.port, 9999);
    }

    #[test]
    fn test_merge_postgres() {
        let mut base = PostgresConfig {
            host: "base_host".to_string(),
            port: 5432,
            database: "base_db".to_string(),
            ..Default::default()
        };

        let override_config = PostgresConfig {
            host: "override_host".to_string(),
            port: 9999,
            database: "override_db".to_string(),
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_postgres(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.host, "override_host");
        assert_eq!(base.port, 9999);
        assert_eq!(base.database, "override_db");
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn test_merge_sync() {
        let mut base = SyncConfig {
            enabled: true,
            sync_interval_seconds: 60,
            ..Default::default()
        };

        let override_config = SyncConfig {
            enabled: false,
            sync_interval_seconds: 120,
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_sync(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.enabled, false);
        assert_eq!(base.sync_interval_seconds, 120);
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_merge_tools_with_api_key() {
        let mut base = ToolConfig {
            api_key: Some("old_key".to_string()),
            ..Default::default()
        };

        let override_config = ToolConfig {
            api_key: Some("new_key".to_string()),
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.api_key, Some("new_key".to_string()));
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("api_key = ***"));
    }

    #[test]
    fn test_merge_cli_overrides_all() {
        let defaults = Config::default();
        let file_config = defaults.clone();
        let env_config = defaults.clone();
        let cli_config = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    host: "cli_host".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = merge_configs(
            defaults,
            file_config,
            "file",
            env_config,
            "env",
            Some(cli_config),
            "cli",
        );

        assert_eq!(merged.providers.postgres.host, "cli_host");
    }

    #[test]
    fn test_merge_qdrant() {
        let mut base = QdrantConfig {
            host: "base_host".to_string(),
            port: 6333,
            collection: "base_collection".to_string(),
            ..Default::default()
        };

        let override_config = QdrantConfig {
            host: "override_host".to_string(),
            port: 9999,
            collection: "override_collection".to_string(),
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_qdrant(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.host, "override_host");
        assert_eq!(base.port, 9999);
        assert_eq!(base.collection, "override_collection");
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn test_merge_redis() {
        let mut base = RedisConfig {
            host: "base_host".to_string(),
            port: 6379,
            db: 0,
            ..Default::default()
        };

        let override_config = RedisConfig {
            host: "override_host".to_string(),
            port: 9999,
            db: 1,
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_redis(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.host, "override_host");
        assert_eq!(base.port, 9999);
        assert_eq!(base.db, 1);
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn test_merge_observability() {
        let mut base = ObservabilityConfig {
            metrics_enabled: true,
            tracing_enabled: true,
            logging_level: "info".to_string(),
            metrics_port: 9090,
        };

        let override_config = ObservabilityConfig {
            metrics_enabled: false,
            tracing_enabled: false,
            logging_level: "debug".to_string(),
            metrics_port: 9999,
        };

        let mut changes = Vec::new();
        merge_observability(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.metrics_enabled, false);
        assert_eq!(base.tracing_enabled, false);
        assert_eq!(base.logging_level, "debug");
        assert_eq!(base.metrics_port, 9999);
        assert_eq!(changes.len(), 4);
    }

    #[test]
    fn test_merge_with_default_values() {
        let mut base = PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "memory_knowledge".to_string(),
            username: "postgres".to_string(),
            password: "".to_string(),
            pool_size: 10,
            timeout_seconds: 30,
        };

        let override_config = PostgresConfig {
            host: "localhost".to_string(),
            port: 5432,
            database: "memory_knowledge".to_string(),
            username: "postgres".to_string(),
            password: "".to_string(),
            pool_size: 10,
            timeout_seconds: 30,
        };

        let mut changes = Vec::new();
        merge_postgres(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_merge_tools_without_api_key() {
        let mut base = ToolConfig {
            api_key: None,
            ..Default::default()
        };

        let override_config = ToolConfig {
            api_key: None,
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.api_key, None);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_merge_tools_remove_api_key() {
        let mut base = ToolConfig {
            api_key: Some("old_key".to_string()),
            ..Default::default()
        };

        let override_config = ToolConfig {
            api_key: None,
            ..Default::default()
        };

        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.api_key, Some("old_key".to_string()));
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_merge_configs_no_changes() {
        let defaults = Config::default();
        let file_config = Config::default();
        let env_config = Config::default();

        let merged = merge_configs(
            defaults,
            file_config,
            "file",
            env_config,
            "env",
            None,
            "cli",
        );

        assert_eq!(merged, Config::default());
    }

    #[test]
    fn test_merge_configs_partial_changes() {
        let defaults = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    host: "default_host".to_string(),
                    port: 5432,
                    ..Default::default()
                },
                qdrant: QdrantConfig {
                    host: "default_qdrant".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let file_config = Config {
            providers: ProviderConfig {
                postgres: PostgresConfig {
                    host: "file_host".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let env_config = Config {
            providers: ProviderConfig {
                qdrant: QdrantConfig {
                    host: "env_qdrant".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = merge_configs(
            defaults,
            file_config,
            "file",
            env_config,
            "env",
            None,
            "cli",
        );

        assert_eq!(merged.providers.postgres.host, "file_host");
        assert_eq!(merged.providers.postgres.port, 5432);
        assert_eq!(merged.providers.qdrant.host, "env_qdrant");
    }

    #[test]
    fn test_merge_postgres_no_changes() {
        let mut base = PostgresConfig::default();
        let override_config = PostgresConfig::default();
        let mut changes = Vec::new();

        merge_postgres(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, PostgresConfig::default());
    }

    #[test]
    fn test_merge_qdrant_no_changes() {
        let mut base = QdrantConfig::default();
        let override_config = QdrantConfig::default();
        let mut changes = Vec::new();

        merge_qdrant(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, QdrantConfig::default());
    }

    #[test]
    fn test_merge_redis_no_changes() {
        let mut base = RedisConfig::default();
        let override_config = RedisConfig::default();
        let mut changes = Vec::new();

        merge_redis(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, RedisConfig::default());
    }

    #[test]
    fn test_merge_sync_no_changes() {
        let mut base = SyncConfig::default();
        let override_config = SyncConfig::default();
        let mut changes = Vec::new();

        merge_sync(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, SyncConfig::default());
    }

    #[test]
    fn test_merge_tools_no_changes() {
        let mut base = ToolConfig::default();
        let override_config = ToolConfig::default();
        let mut changes = Vec::new();

        merge_tools(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, ToolConfig::default());
    }

    #[test]
    fn test_merge_observability_no_changes() {
        let mut base = ObservabilityConfig::default();
        let override_config = ObservabilityConfig::default();
        let mut changes = Vec::new();

        merge_observability(&mut base, &override_config, "test", &mut changes);

        assert_eq!(changes.len(), 0);
        assert_eq!(base, ObservabilityConfig::default());
    }

    #[test]
    fn test_merge_all_fields() {
        let mut base = Config::default();
        let mut override_config = Config::default();

        override_config.providers.postgres.host = "new_pg_host".to_string();
        override_config.providers.postgres.port = 5433;
        override_config.providers.postgres.database = "new_pg_db".to_string();
        override_config.providers.postgres.username = "new_pg_user".to_string();
        override_config.providers.postgres.password = "new_pg_pass".to_string();
        override_config.providers.postgres.pool_size = 20;
        override_config.providers.postgres.timeout_seconds = 60;

        override_config.providers.qdrant.host = "new_qdrant_host".to_string();
        override_config.providers.qdrant.port = 6334;
        override_config.providers.qdrant.collection = "new_collection".to_string();
        override_config.providers.qdrant.timeout_seconds = 60;

        override_config.providers.redis.host = "new_redis_host".to_string();
        override_config.providers.redis.port = 6380;
        override_config.providers.redis.db = 1;
        override_config.providers.redis.pool_size = 20;
        override_config.providers.redis.timeout_seconds = 60;

        override_config.sync.enabled = false;
        override_config.sync.sync_interval_seconds = 120;
        override_config.sync.batch_size = 200;
        override_config.sync.checkpoint_enabled = false;
        override_config.sync.conflict_resolution = "prefer_memory".to_string();

        override_config.tools.enabled = false;
        override_config.tools.host = "new_tool_host".to_string();
        override_config.tools.port = 8081;
        override_config.tools.api_key = Some("new_api_key".to_string());
        override_config.tools.rate_limit_requests_per_minute = 120;

        override_config.observability.metrics_enabled = false;
        override_config.observability.tracing_enabled = false;
        override_config.observability.logging_level = "debug".to_string();
        override_config.observability.metrics_port = 9091;

        let merged = merge_configs(
            base,
            override_config.clone(),
            "file",
            Config::default(),
            "env",
            None,
            "cli",
        );

        assert_eq!(merged.providers.postgres.host, "new_pg_host");
        assert_eq!(merged.providers.postgres.port, 5433);
        assert_eq!(merged.providers.postgres.database, "new_pg_db");
        assert_eq!(merged.providers.postgres.username, "new_pg_user");
        assert_eq!(merged.providers.postgres.password, "new_pg_pass");
        assert_eq!(merged.providers.postgres.pool_size, 20);
        assert_eq!(merged.providers.postgres.timeout_seconds, 60);

        assert_eq!(merged.providers.qdrant.host, "new_qdrant_host");
        assert_eq!(merged.providers.qdrant.port, 6334);
        assert_eq!(merged.providers.qdrant.collection, "new_collection");
        assert_eq!(merged.providers.qdrant.timeout_seconds, 60);

        assert_eq!(merged.providers.redis.host, "new_redis_host");
        assert_eq!(merged.providers.redis.port, 6380);
        assert_eq!(merged.providers.redis.db, 1);
        assert_eq!(merged.providers.redis.pool_size, 20);
        assert_eq!(merged.providers.redis.timeout_seconds, 60);

        assert_eq!(merged.sync.enabled, false);
        assert_eq!(merged.sync.sync_interval_seconds, 120);
        assert_eq!(merged.sync.batch_size, 200);
        assert_eq!(merged.sync.checkpoint_enabled, false);
        assert_eq!(merged.sync.conflict_resolution, "prefer_memory");

        assert_eq!(merged.tools.enabled, false);
        assert_eq!(merged.tools.host, "new_tool_host");
        assert_eq!(merged.tools.port, 8081);
        assert_eq!(merged.tools.api_key, Some("new_api_key".to_string()));
        assert_eq!(merged.tools.rate_limit_requests_per_minute, 120);

        assert_eq!(merged.observability.metrics_enabled, false);
        assert_eq!(merged.observability.tracing_enabled, false);
        assert_eq!(merged.observability.logging_level, "debug");
        assert_eq!(merged.observability.metrics_port, 9091);
    }

    #[test]
    fn test_merge_tools_api_key_scenarios() {
        let mut base = ToolConfig {
            api_key: None,
            ..Default::default()
        };
        let mut override_config = ToolConfig {
            api_key: Some("new_key".to_string()),
            ..Default::default()
        };
        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);
        assert_eq!(base.api_key, Some("new_key".to_string()));
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("api_key = ***"));

        let mut base = ToolConfig {
            api_key: Some("old_key".to_string()),
            ..Default::default()
        };
        let mut override_config = ToolConfig {
            api_key: None,
            ..Default::default()
        };
        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);
        assert_eq!(base.api_key, Some("old_key".to_string()));
        assert_eq!(changes.len(), 0);

        let mut base = ToolConfig {
            api_key: Some("same_key".to_string()),
            ..Default::default()
        };
        let mut override_config = ToolConfig {
            api_key: Some("same_key".to_string()),
            ..Default::default()
        };
        let mut changes = Vec::new();
        merge_tools(&mut base, &override_config, "test", &mut changes);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_merge_postgres_non_default_base() {
        let mut base = PostgresConfig {
            host: "not_localhost".to_string(),
            port: 5433,
            database: "not_memory_knowledge".to_string(),
            username: "not_postgres".to_string(),
            password: "old_password".to_string(),
            pool_size: 20,
            timeout_seconds: 60,
        };

        let override_config = PostgresConfig {
            host: "even_newer_host".to_string(),
            port: 5434,
            database: "even_newer_db".to_string(),
            username: "even_newer_user".to_string(),
            password: "even_newer_password".to_string(),
            pool_size: 30,
            timeout_seconds: 90,
        };

        let mut changes = Vec::new();
        merge_postgres(&mut base, &override_config, "test", &mut changes);

        assert_eq!(base.host, "even_newer_host");
        assert_eq!(base.port, 5434);
        assert_eq!(base.database, "even_newer_db");
        assert_eq!(base.username, "even_newer_user");
        assert_eq!(base.password, "even_newer_password");
        assert_eq!(base.pool_size, 30);
        assert_eq!(base.timeout_seconds, 90);
        assert_eq!(changes.len(), 7);
    }
}
