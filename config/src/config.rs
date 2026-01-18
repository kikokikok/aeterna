//! # Configuration Structures
//!
//! This module defines all configuration structures for the Memory-Knowledge
//! system.
//!
//! All configuration structures:
//! - Use `serde` for serialization/deserialization
//! - Use `validator` for input validation
//! - Follow Microsoft Pragmatic Rust Guidelines
//! - Include comprehensive M-CANONICAL-DOCS

use crate::cca::CcaConfig;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Main configuration structure for the Memory-Knowledge system.
///
/// This is the top-level configuration that aggregates all subsystem
/// configurations.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Provides centralized configuration for the entire Memory-Knowledge system,
/// including storage providers, sync behavior, MCP tools, observability, and
/// CCA.
///
/// ## Usage
/// ```rust,no_run
/// use config::Config;
///
/// let config = Config::default();
/// println!("PostgreSQL host: {}", config.providers.postgres.host);
/// ```
///
/// ## Fields
/// - `providers`: Configuration for storage backends (PostgreSQL, Qdrant,
///   Redis)
/// - `sync`: Configuration for memory-knowledge synchronization
/// - `tools`: Configuration for MCP server tools
/// - `observability`: Configuration for metrics and tracing
/// - `cca`: Configuration for CCA (Confucius Code Agent) capabilities
///
/// ## Validation
/// All nested configurations must pass their own validation rules.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default, PartialEq)]
pub struct Config {
    /// Storage provider configurations (PostgreSQL, Qdrant, Redis)
    #[serde(default)]
    pub providers: ProviderConfig,

    /// Memory-knowledge synchronization configuration
    #[serde(default)]
    pub sync: SyncConfig,

    /// Memory system configuration
    #[serde(default)]
    pub memory: MemoryConfig,

    /// MCP tool interface configuration
    #[serde(default)]
    pub tools: ToolConfig,

    /// Observability configuration (metrics, tracing, logging)
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Deployment mode configuration (Local, Hybrid, Remote)
    #[serde(default)]
    pub deployment: DeploymentConfig,

    /// Job coordination configuration (locks, timeouts, checkpoints)
    #[serde(default)]
    pub job: JobConfig,

    /// CCA (Confucius Code Agent) capabilities configuration
    #[serde(default)]
    pub cca: CcaConfig
}

impl Config {
    /// Detects environment settings for deployment mode.
    ///
    /// # M-CANONICAL-DOCS
    ///
    /// ## Purpose
    /// Initializes configuration based on AETERNA_ environment variables.
    pub fn detect_env() -> Self {
        let mut config = Self::default();

        if let Ok(url) = std::env::var("AETERNA_REMOTE_GOVERNANCE_URL") {
            config.deployment.remote_url = Some(url);
            config.deployment.mode =
                std::env::var("AETERNA_DEPLOYMENT_MODE").unwrap_or_else(|_| "hybrid".to_string());
        }

        if std::env::var("AETERNA_THIN_CLIENT").is_ok() {
            config.deployment.mode = "remote".to_string();
            config.sync.enabled = false;
        }

        config
    }
}

/// Deployment mode configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages the deployment mode of the system (Local, Hybrid, or Remote).
///
/// ## Fields
/// - `mode`: Deployment mode (default: "local")
/// - `remote_url`: URL of the remote governance server (required for
///   Hybrid/Remote)
/// - `sync_enabled`: Enable synchronization in Hybrid mode (default: true)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct DeploymentConfig {
    /// Deployment mode
    #[serde(default = "default_deployment_mode")]
    #[validate(custom(function = "validate_deployment_mode"))]
    pub mode: String,

    /// URL of the remote governance server
    #[serde(default)]
    pub remote_url: Option<String>,

    /// Enable synchronization in Hybrid mode
    #[serde(default = "default_deployment_sync_enabled")]
    pub sync_enabled: bool
}

fn default_deployment_mode() -> String {
    "local".to_string()
}

fn default_deployment_sync_enabled() -> bool {
    true
}

fn validate_deployment_mode(value: &str) -> Result<(), validator::ValidationError> {
    match value {
        "local" | "hybrid" | "remote" => Ok(()),
        _ => Err(validator::ValidationError::new("Invalid deployment mode"))
    }
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            mode: default_deployment_mode(),
            remote_url: None,
            sync_enabled: default_deployment_sync_enabled()
        }
    }
}

impl DeploymentConfig {
    pub fn auto_detect() -> Self {
        let mut config = Self::default();

        if let Ok(mode) = std::env::var("AETERNA_DEPLOYMENT_MODE") {
            config.mode = mode;
        }

        if let Ok(url) = std::env::var("AETERNA_REMOTE_GOVERNANCE_URL") {
            config.remote_url = Some(url);
            if config.mode == "local" {
                config.mode = "hybrid".to_string();
            }
        }

        if std::env::var("AETERNA_THIN_CLIENT").is_ok() {
            config.mode = "remote".to_string();
            config.sync_enabled = false;
        }

        if let Ok(sync) = std::env::var("AETERNA_SYNC_ENABLED") {
            config.sync_enabled = sync.to_lowercase() == "true" || sync == "1";
        }

        config
    }

    pub fn is_local(&self) -> bool {
        self.mode == "local"
    }

    pub fn is_hybrid(&self) -> bool {
        self.mode == "hybrid"
    }

    pub fn is_remote(&self) -> bool {
        self.mode == "remote"
    }

    pub fn requires_remote_url(&self) -> bool {
        self.is_hybrid() || self.is_remote()
    }

    pub fn requires_local_engine(&self) -> bool {
        self.is_local() || self.is_hybrid()
    }
}

/// Configuration for storage providers.
///
/// Manages connection settings for all storage backends.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Centralizes connection configuration for all storage backends:
/// - PostgreSQL: Primary data storage with pgvector extension
/// - Qdrant: Vector similarity search
/// - Redis: Caching layer
/// - Graph: DuckDB-based knowledge graph
///
/// ## Usage
/// ```rust,no_run
/// use config::ProviderConfig;
///
/// let providers = ProviderConfig::default();
/// assert_eq!(providers.postgres.host, "localhost");
/// ```
///
/// ## Fields
/// - `postgres`: PostgreSQL connection configuration
/// - `qdrant`: Qdrant vector database configuration
/// - `redis`: Redis caching configuration
/// - `graph`: DuckDB graph store configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default, PartialEq)]
pub struct ProviderConfig {
    /// PostgreSQL connection configuration
    #[serde(default)]
    pub postgres: PostgresConfig,

    /// Qdrant vector database configuration
    #[serde(default)]
    pub qdrant: QdrantConfig,

    /// Redis caching configuration
    #[serde(default)]
    pub redis: RedisConfig,

    /// DuckDB graph store configuration
    #[serde(default)]
    pub graph: GraphConfig
}

/// PostgreSQL configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages connection settings for PostgreSQL, the primary data storage
/// backend.
///
/// ## Fields
/// - `host`: Database server hostname (default: "localhost")
/// - `port`: Database server port (default: 5432)
/// - `database`: Database name (required)
/// - `username`: Database user (required)
/// - `password`: Database password (required, should use environment variable)
/// - `pool_size`: Maximum connections in pool (default: 10, range: 1-100)
/// - `timeout_seconds`: Connection timeout (default: 30, range: 1-300)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct PostgresConfig {
    /// Database server hostname
    #[serde(default = "default_postgres_host")]
    #[validate(length(min = 1, max = 255))]
    pub host: String,

    /// Database server port
    #[serde(default = "default_postgres_port")]
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Database name
    #[serde(default = "default_postgres_database")]
    #[validate(length(min = 1, max = 63))]
    pub database: String,

    /// Database username
    #[serde(default = "default_postgres_username")]
    #[validate(length(min = 1, max = 63))]
    pub username: String,

    /// Database password
    #[serde(default = "default_postgres_password")]
    #[validate(length(min = 1))]
    pub password: String,

    /// Maximum connections in pool
    #[serde(default = "default_postgres_pool_size")]
    #[validate(range(min = 1, max = 100))]
    pub pool_size: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_postgres_timeout")]
    #[validate(range(min = 1, max = 300))]
    pub timeout_seconds: u64
}

fn default_postgres_host() -> String {
    "localhost".to_string()
}

fn default_postgres_port() -> u16 {
    5432
}

fn default_postgres_database() -> String {
    "memory_knowledge".to_string()
}

fn default_postgres_username() -> String {
    "postgres".to_string()
}

fn default_postgres_password() -> String {
    "".to_string()
}

fn default_postgres_pool_size() -> u32 {
    10
}

fn default_postgres_timeout() -> u64 {
    30
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: default_postgres_host(),
            port: default_postgres_port(),
            database: default_postgres_database(),
            username: default_postgres_username(),
            password: default_postgres_password(),
            pool_size: default_postgres_pool_size(),
            timeout_seconds: default_postgres_timeout()
        }
    }
}

/// Qdrant vector database configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages connection settings for Qdrant, used for vector similarity search.
///
/// ## Fields
/// - `host`: Qdrant server hostname (default: "localhost")
/// - `port`: Qdrant server port (default: 6333)
/// - `collection`: Default collection name (required)
/// - `timeout_seconds`: Request timeout (default: 30, range: 1-300)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct QdrantConfig {
    /// Qdrant server hostname
    #[serde(default = "default_qdrant_host")]
    #[validate(length(min = 1, max = 255))]
    pub host: String,

    /// Qdrant server port
    #[serde(default = "default_qdrant_port")]
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Default collection name
    #[serde(default = "default_qdrant_collection")]
    #[validate(length(min = 1, max = 255))]
    pub collection: String,

    /// Request timeout in seconds
    #[serde(default = "default_qdrant_timeout")]
    #[validate(range(min = 1, max = 300))]
    pub timeout_seconds: u64
}

fn default_qdrant_host() -> String {
    "localhost".to_string()
}

fn default_qdrant_port() -> u16 {
    6333
}

fn default_qdrant_collection() -> String {
    "memory_embeddings".to_string()
}

fn default_qdrant_timeout() -> u64 {
    30
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            host: default_qdrant_host(),
            port: default_qdrant_port(),
            collection: default_qdrant_collection(),
            timeout_seconds: default_qdrant_timeout()
        }
    }
}

/// Redis configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages connection settings for Redis, used as a caching layer.
///
/// ## Fields
/// - `host`: Redis server hostname (default: "localhost")
/// - `port`: Redis server port (default: 6379)
/// - `db`: Redis database number (default: 0, range: 0-15)
/// - `pool_size`: Maximum connections in pool (default: 10, range: 1-100)
/// - `timeout_seconds`: Connection timeout (default: 30, range: 1-300)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct RedisConfig {
    /// Redis server hostname
    #[serde(default = "default_redis_host")]
    #[validate(length(min = 1, max = 255))]
    pub host: String,

    /// Redis server port
    #[serde(default = "default_redis_port")]
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// Redis database number
    #[serde(default = "default_redis_db")]
    #[validate(range(min = 0, max = 15))]
    pub db: u8,

    /// Maximum connections in pool
    #[serde(default = "default_redis_pool_size")]
    #[validate(range(min = 1, max = 100))]
    pub pool_size: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_redis_timeout")]
    #[validate(range(min = 1, max = 300))]
    pub timeout_seconds: u64
}

fn default_redis_host() -> String {
    "localhost".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

fn default_redis_db() -> u8 {
    0
}

fn default_redis_pool_size() -> u32 {
    10
}

fn default_redis_timeout() -> u64 {
    30
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: default_redis_host(),
            port: default_redis_port(),
            db: default_redis_db(),
            pool_size: default_redis_pool_size(),
            timeout_seconds: default_redis_timeout()
        }
    }
}

/// DuckDB Graph Store configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages configuration for the DuckDB-based knowledge graph storage layer.
///
/// ## Fields
/// - `enabled`: Enable/disable graph store (default: true)
/// - `database_path`: Path to DuckDB database file (default: ":memory:")
/// - `s3_bucket`: Optional S3 bucket for persistence
/// - `s3_prefix`: Optional S3 key prefix
/// - `s3_endpoint`: Optional S3 endpoint for MinIO/localstack
/// - `s3_region`: S3 region (default: "us-east-1")
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct GraphConfig {
    /// Enable/disable graph store
    #[serde(default = "default_graph_enabled")]
    pub enabled: bool,

    /// Path to DuckDB database file (use ":memory:" for in-memory)
    #[serde(default = "default_graph_path")]
    #[validate(length(min = 1, max = 255))]
    pub database_path: String,

    /// Optional S3 bucket for graph persistence
    #[serde(default)]
    pub s3_bucket: Option<String>,

    /// Optional S3 key prefix
    #[serde(default)]
    pub s3_prefix: Option<String>,

    /// Optional S3 endpoint (for MinIO or localstack)
    #[serde(default)]
    pub s3_endpoint: Option<String>,

    /// S3 region
    #[serde(default = "default_graph_s3_region")]
    pub s3_region: String,

    /// Alerting thresholds for write contention
    #[serde(default)]
    pub contention_alerts: ContentionAlertConfig
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct ContentionAlertConfig {
    #[serde(default = "default_queue_depth_warn")]
    #[validate(range(min = 1, max = 100))]
    pub queue_depth_warn: u32,

    #[serde(default = "default_queue_depth_critical")]
    #[validate(range(min = 1, max = 100))]
    pub queue_depth_critical: u32,

    #[serde(default = "default_wait_time_warn_ms")]
    #[validate(range(min = 100, max = 60000))]
    pub wait_time_warn_ms: u64,

    #[serde(default = "default_wait_time_critical_ms")]
    #[validate(range(min = 100, max = 60000))]
    pub wait_time_critical_ms: u64,

    #[serde(default = "default_timeout_rate_warn")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub timeout_rate_warn_percent: f64,

    #[serde(default = "default_timeout_rate_critical")]
    #[validate(range(min = 0.0, max = 100.0))]
    pub timeout_rate_critical_percent: f64
}

fn default_queue_depth_warn() -> u32 {
    5
}
fn default_queue_depth_critical() -> u32 {
    10
}
fn default_wait_time_warn_ms() -> u64 {
    1000
}
fn default_wait_time_critical_ms() -> u64 {
    3000
}
fn default_timeout_rate_warn() -> f64 {
    5.0
}
fn default_timeout_rate_critical() -> f64 {
    15.0
}

impl Default for ContentionAlertConfig {
    fn default() -> Self {
        Self {
            queue_depth_warn: default_queue_depth_warn(),
            queue_depth_critical: default_queue_depth_critical(),
            wait_time_warn_ms: default_wait_time_warn_ms(),
            wait_time_critical_ms: default_wait_time_critical_ms(),
            timeout_rate_warn_percent: default_timeout_rate_warn(),
            timeout_rate_critical_percent: default_timeout_rate_critical()
        }
    }
}

fn default_graph_enabled() -> bool {
    true
}

fn default_graph_path() -> String {
    ":memory:".to_string()
}

fn default_graph_s3_region() -> String {
    "us-east-1".to_string()
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: default_graph_enabled(),
            database_path: default_graph_path(),
            s3_bucket: None,
            s3_prefix: None,
            s3_endpoint: None,
            s3_region: default_graph_s3_region(),
            contention_alerts: ContentionAlertConfig::default()
        }
    }
}

/// Synchronization configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages synchronization behavior between memory and knowledge systems.
///
/// ## Fields
/// - `enabled`: Enable/disable automatic sync (default: true)
/// - `sync_interval_seconds`: Sync interval (default: 60, range: 10-3600)
/// - `batch_size`: Number of items per sync batch (default: 100, range: 1-1000)
/// - `checkpoint_enabled`: Enable checkpointing for rollback (default: true)
/// - `conflict_resolution`: Strategy for conflict resolution (default:
///   "prefer_knowledge")
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct SyncConfig {
    /// Enable/disable automatic synchronization
    #[serde(default = "default_sync_enabled")]
    pub enabled: bool,

    /// Sync interval in seconds
    #[serde(default = "default_sync_interval")]
    #[validate(range(min = 10, max = 3600))]
    pub sync_interval_seconds: u64,

    /// Number of items per sync batch
    #[serde(default = "default_sync_batch_size")]
    #[validate(range(min = 1, max = 1000))]
    pub batch_size: u32,

    /// Enable checkpointing for rollback
    #[serde(default = "default_sync_checkpoint")]
    pub checkpoint_enabled: bool,

    /// Conflict resolution strategy
    #[serde(default = "default_sync_conflict_resolution")]
    #[validate(custom(function = "validate_conflict_resolution"))]
    pub conflict_resolution: String
}

fn default_sync_enabled() -> bool {
    true
}

fn default_sync_interval() -> u64 {
    60
}

fn default_sync_batch_size() -> u32 {
    100
}

fn default_sync_checkpoint() -> bool {
    true
}

fn default_sync_conflict_resolution() -> String {
    "prefer_knowledge".to_string()
}

fn validate_conflict_resolution(value: &str) -> Result<(), validator::ValidationError> {
    match value {
        "prefer_knowledge" | "prefer_memory" | "manual" => Ok(()),
        _ => Err(validator::ValidationError::new(
            "Invalid conflict resolution strategy"
        ))
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: default_sync_enabled(),
            sync_interval_seconds: default_sync_interval(),
            batch_size: default_sync_batch_size(),
            checkpoint_enabled: default_sync_checkpoint(),
            conflict_resolution: default_sync_conflict_resolution()
        }
    }
}

/// MCP tool interface configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages configuration for the MCP (Model Context Protocol) server interface.
///
/// ## Fields
/// - `enabled`: Enable/disable MCP server (default: true)
/// - `host`: Server hostname (default: "localhost")
/// - `port`: Server port (default: 8080)
/// - `api_key`: API key for authentication (optional)
/// - `rate_limit_requests_per_minute`: Rate limit (default: 60, range: 1-1000)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct ToolConfig {
    /// Enable/disable MCP server
    #[serde(default = "default_tools_enabled")]
    pub enabled: bool,

    /// Server hostname
    #[serde(default = "default_tools_host")]
    #[validate(length(min = 1, max = 255))]
    pub host: String,

    /// Server port
    #[serde(default = "default_tools_port")]
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    /// API key for authentication
    #[serde(default)]
    pub api_key: Option<String>,

    /// Rate limit: requests per minute
    #[serde(default = "default_tools_rate_limit")]
    #[validate(range(min = 1, max = 1000))]
    pub rate_limit_requests_per_minute: u32
}

fn default_tools_enabled() -> bool {
    true
}

fn default_tools_host() -> String {
    "localhost".to_string()
}

fn default_tools_port() -> u16 {
    8080
}

fn default_tools_rate_limit() -> u32 {
    60
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            enabled: default_tools_enabled(),
            host: default_tools_host(),
            port: default_tools_port(),
            api_key: None,
            rate_limit_requests_per_minute: default_tools_rate_limit()
        }
    }
}

/// Observability configuration.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Manages configuration for metrics, tracing, and logging.
///
/// ## Fields
/// - `metrics_enabled`: Enable metrics collection (default: true)
/// - `tracing_enabled`: Enable distributed tracing (default: true)
/// - `logging_level`: Log level (default: "info")
/// - `metrics_port`: Metrics server port (default: 9090)
#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct ObservabilityConfig {
    /// Enable metrics collection
    #[serde(default = "default_observability_metrics_enabled")]
    pub metrics_enabled: bool,

    /// Enable distributed tracing
    #[serde(default = "default_observability_tracing_enabled")]
    pub tracing_enabled: bool,

    /// Logging level
    #[serde(default = "default_observability_logging_level")]
    #[validate(custom(function = "validate_logging_level"))]
    pub logging_level: String,

    /// Metrics server port
    #[serde(default = "default_observability_metrics_port")]
    #[validate(range(min = 1, max = 65535))]
    pub metrics_port: u16
}

fn default_observability_metrics_enabled() -> bool {
    true
}

fn default_observability_tracing_enabled() -> bool {
    true
}

fn default_observability_logging_level() -> String {
    "info".to_string()
}

fn default_observability_metrics_port() -> u16 {
    9090
}

fn validate_logging_level(value: &str) -> Result<(), validator::ValidationError> {
    match value {
        "trace" | "debug" | "info" | "warn" | "error" => Ok(()),
        _ => Err(validator::ValidationError::new("Invalid logging level"))
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: default_observability_metrics_enabled(),
            tracing_enabled: default_observability_tracing_enabled(),
            logging_level: default_observability_logging_level(),
            metrics_port: default_observability_metrics_port()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct MemoryConfig {
    #[serde(default = "default_promotion_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub promotion_threshold: f32,

    #[serde(default = "default_decay_interval")]
    #[validate(range(min = 3600, max = 86400))]
    pub decay_interval_secs: u64,

    #[serde(default = "default_decay_rate")]
    #[validate(range(min = 0.0, max = 0.5))]
    pub decay_rate: f32,

    #[serde(default = "default_optimization_trigger_count")]
    #[validate(range(min = 10, max = 1000))]
    pub optimization_trigger_count: usize,

    #[serde(default)]
    pub layer_summary_configs:
        std::collections::HashMap<mk_core::types::MemoryLayer, mk_core::types::SummaryConfig>
}

fn default_promotion_threshold() -> f32 {
    0.8
}

fn default_decay_interval() -> u64 {
    86400 // 24 hours
}

fn default_decay_rate() -> f32 {
    0.05 // 5% decay
}

fn default_optimization_trigger_count() -> usize {
    100
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            promotion_threshold: default_promotion_threshold(),
            decay_interval_secs: default_decay_interval(),
            decay_rate: default_decay_rate(),
            optimization_trigger_count: default_optimization_trigger_count(),
            layer_summary_configs: std::collections::HashMap::new()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, PartialEq)]
pub struct JobConfig {
    #[serde(default = "default_lock_ttl_seconds")]
    #[validate(range(min = 60, max = 7200))]
    pub lock_ttl_seconds: u64,

    #[serde(default = "default_job_timeout_seconds")]
    #[validate(range(min = 30, max = 3600))]
    pub job_timeout_seconds: u64,

    #[serde(default = "default_deduplication_window_seconds")]
    #[validate(range(min = 0, max = 3600))]
    pub deduplication_window_seconds: u64,

    #[serde(default = "default_checkpoint_interval")]
    #[validate(range(min = 10, max = 1000))]
    pub checkpoint_interval: usize,

    #[serde(default = "default_graceful_shutdown_timeout_seconds")]
    #[validate(range(min = 5, max = 300))]
    pub graceful_shutdown_timeout_seconds: u64
}

fn default_lock_ttl_seconds() -> u64 {
    2100 // 35 minutes
}

fn default_job_timeout_seconds() -> u64 {
    1800 // 30 minutes
}

fn default_deduplication_window_seconds() -> u64 {
    300 // 5 minutes
}

fn default_checkpoint_interval() -> usize {
    100
}

fn default_graceful_shutdown_timeout_seconds() -> u64 {
    30
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            lock_ttl_seconds: default_lock_ttl_seconds(),
            job_timeout_seconds: default_job_timeout_seconds(),
            deduplication_window_seconds: default_deduplication_window_seconds(),
            checkpoint_interval: default_checkpoint_interval(),
            graceful_shutdown_timeout_seconds: default_graceful_shutdown_timeout_seconds()
        }
    }
}

impl JobConfig {
    pub fn lock_key(&self, job_name: &str) -> String {
        format!("job_lock:{}", job_name)
    }

    pub fn should_checkpoint(&self, processed_count: usize) -> bool {
        processed_count > 0 && processed_count % self.checkpoint_interval == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.providers.postgres.host, "localhost");
        assert_eq!(config.sync.enabled, true);
        assert_eq!(config.tools.port, 8080);
        assert_eq!(config.observability.logging_level, "info");
    }

    #[test]
    fn test_provider_config_default() {
        let providers = ProviderConfig::default();
        assert_eq!(providers.postgres.port, 5432);
        assert_eq!(providers.qdrant.port, 6333);
        assert_eq!(providers.redis.port, 6379);
    }

    #[test]
    fn test_sync_config_default() {
        let sync = SyncConfig::default();
        assert_eq!(sync.enabled, true);
        assert_eq!(sync.sync_interval_seconds, 60);
        assert_eq!(sync.conflict_resolution, "prefer_knowledge");
    }

    #[test]
    fn test_postgres_config_validation() {
        let mut postgres = PostgresConfig::default();
        postgres.host = "".to_string();
        assert!(postgres.validate().is_err());
    }

    #[test]
    fn test_qdrant_config_validation() {
        let mut qdrant = QdrantConfig::default();
        qdrant.port = 0;
        assert!(qdrant.validate().is_err());
    }

    #[test]
    fn test_redis_config_validation() {
        let mut redis = RedisConfig::default();
        redis.db = 16;
        assert!(redis.validate().is_err());
    }

    #[test]
    fn test_sync_config_conflict_resolution_validation() {
        let mut sync = SyncConfig::default();
        sync.conflict_resolution = "invalid".to_string();
        assert!(sync.validate().is_err());

        sync.conflict_resolution = "prefer_memory".to_string();
        assert!(sync.validate().is_ok());
    }

    #[test]
    fn test_observability_config_logging_level_validation() {
        let mut obs = ObservabilityConfig::default();
        obs.logging_level = "invalid".to_string();
        assert!(obs.validate().is_err());

        obs.logging_level = "debug".to_string();
        assert!(obs.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(
            config.providers.postgres.host,
            deserialized.providers.postgres.host
        );
    }

    #[test]
    fn test_deployment_config_default() {
        let config = DeploymentConfig::default();
        assert_eq!(config.mode, "local");
        assert!(config.remote_url.is_none());
        assert!(config.sync_enabled);
    }

    #[test]
    fn test_deployment_config_is_local() {
        let config = DeploymentConfig {
            mode: "local".to_string(),
            remote_url: None,
            sync_enabled: true
        };
        assert!(config.is_local());
        assert!(!config.is_hybrid());
        assert!(!config.is_remote());
    }

    #[test]
    fn test_deployment_config_is_hybrid() {
        let config = DeploymentConfig {
            mode: "hybrid".to_string(),
            remote_url: Some("http://localhost:8080".to_string()),
            sync_enabled: true
        };
        assert!(!config.is_local());
        assert!(config.is_hybrid());
        assert!(!config.is_remote());
    }

    #[test]
    fn test_deployment_config_is_remote() {
        let config = DeploymentConfig {
            mode: "remote".to_string(),
            remote_url: Some("http://localhost:8080".to_string()),
            sync_enabled: false
        };
        assert!(!config.is_local());
        assert!(!config.is_hybrid());
        assert!(config.is_remote());
    }

    #[test]
    fn test_deployment_config_requires_remote_url() {
        let local = DeploymentConfig {
            mode: "local".to_string(),
            ..Default::default()
        };
        let hybrid = DeploymentConfig {
            mode: "hybrid".to_string(),
            ..Default::default()
        };
        let remote = DeploymentConfig {
            mode: "remote".to_string(),
            ..Default::default()
        };

        assert!(!local.requires_remote_url());
        assert!(hybrid.requires_remote_url());
        assert!(remote.requires_remote_url());
    }

    #[test]
    fn test_deployment_config_requires_local_engine() {
        let local = DeploymentConfig {
            mode: "local".to_string(),
            ..Default::default()
        };
        let hybrid = DeploymentConfig {
            mode: "hybrid".to_string(),
            ..Default::default()
        };
        let remote = DeploymentConfig {
            mode: "remote".to_string(),
            ..Default::default()
        };

        assert!(local.requires_local_engine());
        assert!(hybrid.requires_local_engine());
        assert!(!remote.requires_local_engine());
    }

    #[test]
    fn test_deployment_mode_validation() {
        assert!(validate_deployment_mode("local").is_ok());
        assert!(validate_deployment_mode("hybrid").is_ok());
        assert!(validate_deployment_mode("remote").is_ok());
        assert!(validate_deployment_mode("invalid").is_err());
    }

    #[test]
    fn test_graph_config_default() {
        let config = GraphConfig::default();
        assert!(config.enabled);
        assert_eq!(config.database_path, ":memory:");
        assert!(config.s3_bucket.is_none());
        assert!(config.s3_prefix.is_none());
        assert!(config.s3_endpoint.is_none());
        assert_eq!(config.s3_region, "us-east-1");
    }

    #[test]
    fn test_graph_config_validation() {
        let mut config = GraphConfig::default();
        config.database_path = "".to_string();
        assert!(config.validate().is_err());

        config.database_path = "/path/to/graph.duckdb".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_provider_config_includes_graph() {
        let providers = ProviderConfig::default();
        assert!(providers.graph.enabled);
        assert_eq!(providers.graph.database_path, ":memory:");
    }
}
