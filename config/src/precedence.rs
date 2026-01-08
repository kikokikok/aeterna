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
/// use config::{Config, merge_configs, load_from_file, load_from_env};
/// use std::path::Path;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let defaults = Config::default();
///     let from_file = load_from_file(Path::new("config.toml"))?;
///     let from_env = load_from_env()?;
///
///     let _config = merge_configs(
///         defaults,
///         from_file,
///         "file",
///         from_env,
///         "env",
///         None,
///         "cli",
///     );
///     Ok(())
/// }
/// ```
///
/// ## Deep Merge
/// Performs deep merge on nested structures (providers, sync, tools, observability).
/// String fields are overridden, not concatenated.
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
    if override_config.enabled != base.enabled {
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
    if override_config.checkpoint_enabled != base.checkpoint_enabled {
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
    if override_config.enabled != base.enabled {
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
            (None, Some(_)) => changes.push("tools.api_key = (none)".to_string()),
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
    if override_config.metrics_enabled != base.metrics_enabled {
        changes.push(format!(
            "observability.metrics_enabled = {}",
            override_config.metrics_enabled
        ));
        base.metrics_enabled = override_config.metrics_enabled;
    }
    if override_config.tracing_enabled != base.tracing_enabled {
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
}
