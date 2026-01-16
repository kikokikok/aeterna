//! DuckDB-based Graph Store Implementation
//!
//! This module provides a DuckDB-backed implementation of the `GraphStore` trait,
//! enabling relationship-based memory traversal and graph queries.
//!
//! ## Features
//! - In-memory or file-based storage
//! - Tenant isolation via parameterized queries
//! - Cascading deletion with soft-delete support
//! - Application-level referential integrity
//! - Path finding and neighbor traversal
//!
//! ## Production Gaps Addressed
//! - R1-C1: Cascading deletion for data integrity
//! - R1-C2: Application-level FK enforcement
//! - R1-H3: Parameterized tenant filtering
//! - R1-H9: Schema versioning with migrations

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use duckdb::{Connection, params};
use mk_core::types::TenantContext;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::graph::{GraphEdge, GraphNode, GraphStore};

/// Current schema version for migrations
const SCHEMA_VERSION: i32 = 1;

/// Maximum depth for path finding to prevent runaway queries (R1-M4)
const MAX_PATH_DEPTH: usize = 5;

/// Default query timeout in seconds
const DEFAULT_QUERY_TIMEOUT_SECS: i32 = 30;

/// Errors that can occur during graph operations
#[derive(Error, Debug)]
pub enum GraphError {
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Referential integrity violation: {0}")]
    ReferentialIntegrity(String),

    #[error("Tenant isolation violation: {0}")]
    TenantViolation(String),

    #[error("Schema migration failed: {0}")]
    Migration(String),

    #[error("Query timeout after {0} seconds")]
    Timeout(i32),

    #[error("Path depth exceeded maximum of {0}")]
    MaxDepthExceeded(usize),

    #[error("Invalid tenant context")]
    InvalidTenantContext,

    #[error("Invalid tenant ID format: {0}")]
    InvalidTenantIdFormat(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for Lambda cold start optimization (R1-H1, R1-H7)
#[derive(Debug, Clone)]
pub struct ColdStartConfig {
    /// Enable lazy partition loading (defer data load until first query)
    pub lazy_loading_enabled: bool,
    /// Cold start time budget in milliseconds (default: 3000ms)
    pub budget_ms: u64,
    /// Enable partition access tracking for pre-warming
    pub access_tracking_enabled: bool,
    /// Number of most recently accessed partitions to pre-warm
    pub prewarm_partition_count: usize,
    /// Enable warm pool strategy (provisioned concurrency)
    pub warm_pool_enabled: bool,
    /// Minimum warm instances to maintain
    pub warm_pool_min_instances: u32,
}

impl Default for ColdStartConfig {
    fn default() -> Self {
        Self {
            lazy_loading_enabled: true,
            budget_ms: 3000, // 3 second budget
            access_tracking_enabled: true,
            prewarm_partition_count: 5,
            warm_pool_enabled: false,
            warm_pool_min_instances: 1,
        }
    }
}

/// Tracks partition access patterns for pre-warming optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionAccessRecord {
    pub partition_key: String,
    pub tenant_id: String,
    pub access_count: u64,
    pub last_access: DateTime<Utc>,
    pub avg_load_time_ms: f64,
}

/// Result of lazy loading operation
#[derive(Debug, Clone)]
pub struct LazyLoadResult {
    pub partitions_loaded: usize,
    pub total_load_time_ms: u64,
    pub budget_remaining_ms: u64,
    pub deferred_partitions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WarmPoolRecommendation {
    pub recommended: bool,
    pub min_instances: u32,
    pub reason: String,
}

/// Configuration for the DuckDB graph store
#[derive(Debug, Clone)]
pub struct DuckDbGraphConfig {
    /// Path to the database file. Use ":memory:" for in-memory database.
    pub path: String,
    /// Enable query timeout (default: 30 seconds)
    pub query_timeout_secs: i32,
    /// Enable soft-delete for cascade operations
    pub soft_delete_enabled: bool,
    /// Maximum path depth for traversal queries
    pub max_path_depth: usize,
    /// S3 bucket for persistence (optional)
    pub s3_bucket: Option<String>,
    /// S3 key prefix for this store's data
    pub s3_prefix: Option<String>,
    /// S3 endpoint override (for MinIO/LocalStack)
    pub s3_endpoint: Option<String>,
    /// S3 region
    pub s3_region: Option<String>,
    /// Cold start optimization configuration
    pub cold_start: ColdStartConfig,
}

impl Default for DuckDbGraphConfig {
    fn default() -> Self {
        Self {
            path: ":memory:".to_string(),
            query_timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
            soft_delete_enabled: true,
            max_path_depth: MAX_PATH_DEPTH,
            s3_bucket: None,
            s3_prefix: None,
            s3_endpoint: None,
            s3_region: None,
            cold_start: ColdStartConfig::default(),
        }
    }
}

/// Entity extracted from memory content (for knowledge graph)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEdge {
    pub id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Extended GraphNode with soft-delete support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeExtended {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
    pub memory_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Extended GraphEdge with soft-delete support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdgeExtended {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub properties: serde_json::Value,
    pub tenant_id: String,
    pub weight: f64,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait EntityExtractor: Send + Sync {
    async fn extract_entities(&self, text: &str) -> Result<Vec<Entity>, GraphError>;
    async fn extract_relationships(
        &self,
        text: &str,
        entities: &[Entity],
    ) -> Result<Vec<EntityEdge>, GraphError>;
}

#[derive(Debug, Clone)]
pub struct Community {
    pub id: String,
    pub member_node_ids: Vec<String>,
    pub density: f64,
}

#[derive(Debug, Clone)]
pub struct ContentionAlertConfig {
    pub queue_depth_warn: u32,
    pub queue_depth_critical: u32,
    pub wait_time_warn_ms: u64,
    pub wait_time_critical_ms: u64,
    pub timeout_rate_warn_percent: f64,
    pub timeout_rate_critical_percent: f64,
}

impl Default for ContentionAlertConfig {
    fn default() -> Self {
        Self {
            queue_depth_warn: 5,
            queue_depth_critical: 10,
            wait_time_warn_ms: 1000,
            wait_time_critical_ms: 3000,
            timeout_rate_warn_percent: 5.0,
            timeout_rate_critical_percent: 15.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WriteCoordinatorConfig {
    pub lock_ttl_ms: u64,
    pub max_retries: u32,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub alert_config: ContentionAlertConfig,
}

impl Default for WriteCoordinatorConfig {
    fn default() -> Self {
        Self {
            lock_ttl_ms: 5000,
            max_retries: 5,
            base_backoff_ms: 50,
            max_backoff_ms: 2000,
            alert_config: ContentionAlertConfig::default(),
        }
    }
}

/// Configuration for backup and recovery (R1-H4)
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Interval between automatic snapshots (in seconds)
    pub snapshot_interval_secs: u64,
    /// Maximum number of snapshots to retain per tenant
    pub retention_count: usize,
    /// Maximum age of snapshots to retain (in seconds)
    pub retention_max_age_secs: u64,
    /// Enable automatic scheduled backups
    pub auto_backup_enabled: bool,
    /// S3 prefix for backup snapshots
    pub backup_prefix: String,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_secs: 3600,      // 1 hour
            retention_count: 24,               // Keep last 24 snapshots
            retention_max_age_secs: 86400 * 7, // 7 days
            auto_backup_enabled: false,
            backup_prefix: "backups".to_string(),
        }
    }
}

/// Snapshot metadata for versioning and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub snapshot_id: String,
    pub tenant_id: String,
    pub s3_key: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
    pub checksum: String,
    pub node_count: u64,
    pub edge_count: u64,
    pub schema_version: i32,
}

/// Result of a backup operation
#[derive(Debug, Clone)]
pub struct BackupResult {
    pub snapshot_id: String,
    pub s3_key: String,
    pub size_bytes: u64,
    pub duration_ms: u64,
    pub checksum: String,
}

/// Result of a recovery operation
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    pub snapshot_id: String,
    pub nodes_restored: u64,
    pub edges_restored: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub is_healthy: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub duckdb: ComponentHealth,
    pub s3: ComponentHealth,
    pub schema_version: i32,
    pub total_latency_ms: u64,
    pub duckdb_latency_ms: u64,
    pub s3_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResult {
    pub ready: bool,
    pub duckdb_ready: bool,
    pub schema_ready: bool,
    pub latency_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i32,
    pub description: String,
    pub up_sql: Vec<&'static str>,
    pub down_sql: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub version: i32,
    pub applied_at: String,
    pub description: String,
}

pub struct WriteCoordinator {
    redis_url: String,
    config: WriteCoordinatorConfig,
    metrics: GraphMetrics,
}

impl WriteCoordinator {
    pub fn new(redis_url: String, config: WriteCoordinatorConfig) -> Self {
        let metrics = GraphMetrics::with_alert_config(config.alert_config.clone());
        Self {
            redis_url,
            config,
            metrics,
        }
    }

    pub async fn acquire_lock(&self, tenant_id: &str) -> Result<String, GraphError> {
        let start_time = std::time::Instant::now();
        self.metrics.record_lock_attempt(tenant_id);

        let client = redis::Client::open(self.redis_url.as_str())
            .map_err(|e| GraphError::S3(format!("Redis connection failed: {}", e)))?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| GraphError::S3(format!("Redis connection failed: {}", e)))?;

        let lock_key = format!("aeterna:graph:lock:{}", tenant_id);
        let lock_value = Uuid::new_v4().to_string();
        let mut backoff = self.config.base_backoff_ms;

        for attempt in 0..self.config.max_retries {
            let result: Result<bool, _> = redis::cmd("SET")
                .arg(&lock_key)
                .arg(&lock_value)
                .arg("NX")
                .arg("PX")
                .arg(self.config.lock_ttl_ms)
                .query_async(&mut conn)
                .await;

            match result {
                Ok(true) => {
                    let wait_time_ms = start_time.elapsed().as_millis() as u64;
                    self.metrics
                        .record_lock_acquired(tenant_id, wait_time_ms, attempt);
                    debug!("Acquired lock {} on attempt {}", lock_key, attempt + 1);
                    return Ok(lock_value);
                }
                Ok(false) => {
                    if attempt < self.config.max_retries - 1 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                        backoff = (backoff * 2).min(self.config.max_backoff_ms);
                    }
                }
                Err(e) => {
                    let wait_time_ms = start_time.elapsed().as_millis() as u64;
                    self.metrics
                        .record_lock_timeout(tenant_id, wait_time_ms, attempt);
                    return Err(GraphError::S3(format!("Redis SET failed: {}", e)));
                }
            }
        }

        let wait_time_ms = start_time.elapsed().as_millis() as u64;
        self.metrics
            .record_lock_timeout(tenant_id, wait_time_ms, self.config.max_retries);
        Err(GraphError::Timeout(self.config.max_retries as i32))
    }

    pub async fn release_lock(
        &self,
        tenant_id: &str,
        lock_value: &str,
        acquired_at: std::time::Instant,
    ) -> Result<(), GraphError> {
        let client = redis::Client::open(self.redis_url.as_str())
            .map_err(|e| GraphError::S3(format!("Redis connection failed: {}", e)))?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| GraphError::S3(format!("Redis connection failed: {}", e)))?;

        let lock_key = format!("aeterna:graph:lock:{}", tenant_id);

        let script = redis::Script::new(
            r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
            "#,
        );

        let _: i32 = script
            .key(&lock_key)
            .arg(lock_value)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| GraphError::S3(format!("Redis EVAL failed: {}", e)))?;

        let hold_time_ms = acquired_at.elapsed().as_millis() as u64;
        self.metrics.record_lock_released(tenant_id, hold_time_ms);
        debug!("Released lock {}", lock_key);
        Ok(())
    }
}

/// Observability metrics for graph operations.
/// Uses the `metrics` crate for lightweight instrumentation.
#[derive(Clone, Debug)]
pub struct GraphMetrics {
    alert_config: Option<ContentionAlertConfig>,
}

impl Default for GraphMetrics {
    fn default() -> Self {
        Self { alert_config: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Warn,
    Critical,
}

impl GraphMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_alert_config(alert_config: ContentionAlertConfig) -> Self {
        Self {
            alert_config: Some(alert_config),
        }
    }

    fn emit_alert(&self, severity: AlertSeverity, metric_name: &str, value: f64, threshold: f64) {
        let severity_str = match severity {
            AlertSeverity::Warn => "warn",
            AlertSeverity::Critical => "critical",
        };
        metrics::counter!(
            "graph_contention_alerts_total",
            1,
            "severity" => severity_str,
            "metric" => metric_name.to_string()
        );
        warn!(
            severity = severity_str,
            metric = metric_name,
            value = value,
            threshold = threshold,
            "Contention alert triggered"
        );
    }

    fn check_wait_time_alert(&self, wait_time_ms: u64) {
        if let Some(ref config) = self.alert_config {
            if wait_time_ms >= config.wait_time_critical_ms {
                self.emit_alert(
                    AlertSeverity::Critical,
                    "wait_time_ms",
                    wait_time_ms as f64,
                    config.wait_time_critical_ms as f64,
                );
            } else if wait_time_ms >= config.wait_time_warn_ms {
                self.emit_alert(
                    AlertSeverity::Warn,
                    "wait_time_ms",
                    wait_time_ms as f64,
                    config.wait_time_warn_ms as f64,
                );
            }
        }
    }

    pub fn record_query(&self, duration_secs: f64, result_count: usize) {
        metrics::histogram!("graph_query_duration_seconds", duration_secs);
        metrics::histogram!("graph_query_result_count", result_count as f64);
    }

    pub fn record_cache_hit(&self) {
        metrics::counter!("graph_cache_hits_total", 1);
    }

    pub fn record_cache_miss(&self) {
        metrics::counter!("graph_cache_misses_total", 1);
    }

    pub fn record_lock_attempt(&self, _tenant_id: &str) {
        metrics::counter!("graph_write_lock_attempts_total", 1);
        metrics::gauge!("graph_write_queue_depth", 1.0);
    }

    pub fn record_lock_acquired(&self, _tenant_id: &str, wait_time_ms: u64, retry_count: u32) {
        metrics::counter!("graph_write_lock_acquired_total", 1);
        metrics::histogram!(
            "graph_write_lock_wait_seconds",
            wait_time_ms as f64 / 1000.0
        );
        metrics::histogram!("graph_write_lock_retries", retry_count as f64);
        metrics::gauge!("graph_write_queue_depth", -1.0);
        self.check_wait_time_alert(wait_time_ms);
    }

    pub fn record_lock_timeout(&self, _tenant_id: &str, wait_time_ms: u64, retry_count: u32) {
        metrics::counter!("graph_write_lock_timeouts_total", 1);
        metrics::histogram!(
            "graph_write_lock_wait_seconds",
            wait_time_ms as f64 / 1000.0
        );
        metrics::histogram!("graph_write_lock_retries", retry_count as f64);
        metrics::gauge!("graph_write_queue_depth", -1.0);
        self.check_wait_time_alert(wait_time_ms);
    }

    pub fn record_lock_released(&self, _tenant_id: &str, hold_time_ms: u64) {
        metrics::counter!("graph_write_lock_released_total", 1);
        metrics::histogram!(
            "graph_write_lock_hold_seconds",
            hold_time_ms as f64 / 1000.0
        );
    }
}

/// DuckDB-backed implementation of GraphStore
///
/// ## Thread Safety
/// Uses `parking_lot::Mutex` for thread-safe access to the DuckDB connection.
/// DuckDB supports single-writer semantics, so all write operations are serialized.
pub struct DuckDbGraphStore {
    conn: Arc<Mutex<Connection>>,
    config: DuckDbGraphConfig,
}

impl DuckDbGraphStore {
    /// Create a new DuckDB graph store with the given configuration
    #[instrument(skip(config), fields(path = %config.path))]
    pub fn new(config: DuckDbGraphConfig) -> Result<Self, GraphError> {
        info!("Initializing DuckDB graph store");

        let conn = if config.path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(Path::new(&config.path))?
        };

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        };

        store.initialize_schema()?;
        store.run_migrations()?;

        info!("DuckDB graph store initialized successfully");
        Ok(store)
    }

    /// Initialize the database schema
    #[instrument(skip(self))]
    fn initialize_schema(&self) -> Result<(), GraphError> {
        let conn = self.conn.lock();

        // Schema version tracking (R1-H9)
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TIMESTAMP DEFAULT (now()),
                description VARCHAR
            );
            "#,
        )?;

        // Memory nodes table with soft-delete
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory_nodes (
                id VARCHAR PRIMARY KEY,
                label VARCHAR NOT NULL,
                properties JSON,
                tenant_id VARCHAR NOT NULL,
                memory_id VARCHAR,
                created_at TIMESTAMP DEFAULT (now()),
                updated_at TIMESTAMP DEFAULT (now()),
                deleted_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_nodes_tenant ON memory_nodes(tenant_id);
            CREATE INDEX IF NOT EXISTS idx_nodes_tenant_deleted ON memory_nodes(tenant_id, deleted_at);
            CREATE INDEX IF NOT EXISTS idx_nodes_memory ON memory_nodes(memory_id);
            "#,
        )?;

        // Memory edges table with soft-delete (R1-H1)
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory_edges (
                id VARCHAR PRIMARY KEY,
                source_id VARCHAR NOT NULL,
                target_id VARCHAR NOT NULL,
                relation VARCHAR NOT NULL,
                properties JSON,
                tenant_id VARCHAR NOT NULL,
                weight DOUBLE DEFAULT 1.0,
                created_at TIMESTAMP DEFAULT (now()),
                deleted_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_edges_tenant_source ON memory_edges(tenant_id, source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_tenant_target ON memory_edges(tenant_id, target_id);
            CREATE INDEX IF NOT EXISTS idx_edges_tenant_deleted ON memory_edges(tenant_id, deleted_at);
            "#,
        )?;

        // Entity table (for knowledge graph entities)
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entities (
                id VARCHAR PRIMARY KEY,
                name VARCHAR NOT NULL,
                entity_type VARCHAR NOT NULL,
                properties JSON,
                tenant_id VARCHAR NOT NULL,
                created_at TIMESTAMP DEFAULT (now()),
                deleted_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_entities_tenant ON entities(tenant_id);
            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(tenant_id, entity_type);
            CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(tenant_id, name);
            "#,
        )?;

        // Entity edges table
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entity_edges (
                id VARCHAR PRIMARY KEY,
                source_entity_id VARCHAR NOT NULL,
                target_entity_id VARCHAR NOT NULL,
                relation VARCHAR NOT NULL,
                properties JSON,
                tenant_id VARCHAR NOT NULL,
                created_at TIMESTAMP DEFAULT (now()),
                deleted_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_entity_edges_tenant_source ON entity_edges(tenant_id, source_entity_id);
            CREATE INDEX IF NOT EXISTS idx_entity_edges_tenant_target ON entity_edges(tenant_id, target_entity_id);
            "#,
        )?;

        // Partition access tracking for cold start optimization (R1-H7)
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS partition_access (
                partition_key VARCHAR NOT NULL,
                tenant_id VARCHAR NOT NULL,
                access_count BIGINT DEFAULT 1,
                last_access TIMESTAMP DEFAULT (now()),
                total_load_time_ms DOUBLE DEFAULT 0,
                PRIMARY KEY (partition_key, tenant_id)
            );

            CREATE INDEX IF NOT EXISTS idx_partition_access_tenant ON partition_access(tenant_id, last_access DESC);
            "#,
        )?;

        debug!("Schema initialized successfully");
        Ok(())
    }

    // ==================== Schema Migrations (R1-H9) ====================

    #[instrument(skip(self))]
    fn run_migrations(&self) -> Result<(), GraphError> {
        let conn = self.conn.lock();

        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version >= SCHEMA_VERSION {
            debug!("Schema is up to date (version {})", current_version);
            return Ok(());
        }

        info!(
            "Running migrations from version {} to {}",
            current_version, SCHEMA_VERSION
        );

        let migrations = Self::get_migrations();

        for migration in migrations {
            if current_version < migration.version {
                info!(
                    "Applying migration v{}: {}",
                    migration.version, migration.description
                );

                conn.execute_batch("BEGIN TRANSACTION")?;

                let result = (|| -> Result<(), GraphError> {
                    for sql in &migration.up_sql {
                        conn.execute_batch(sql)?;
                    }

                    conn.execute(
                        "INSERT INTO schema_version (version, description) VALUES (?, ?)",
                        params![migration.version, migration.description],
                    )?;

                    Ok(())
                })();

                match result {
                    Ok(()) => {
                        conn.execute_batch("COMMIT")?;
                        info!("Migration v{} applied successfully", migration.version);
                    }
                    Err(e) => {
                        conn.execute_batch("ROLLBACK")?;
                        error!(
                            error = %e,
                            version = migration.version,
                            "Migration v{} failed, rolled back",
                            migration.version
                        );
                        return Err(GraphError::Migration(format!(
                            "Migration v{} failed: {}",
                            migration.version, e
                        )));
                    }
                }
            }
        }

        info!("All migrations completed successfully");
        Ok(())
    }

    fn get_migrations() -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "Initial schema with soft-delete support".to_string(),
            up_sql: vec![],
            down_sql: vec![],
        }]
    }

    pub fn get_current_schema_version(&self) -> Result<i32, GraphError> {
        self.get_schema_version()
    }

    pub fn get_migration_history(&self) -> Result<Vec<MigrationRecord>, GraphError> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT version, CAST(applied_at AS VARCHAR) as applied_at, description FROM schema_version ORDER BY version ASC",
        )?;

        let records = stmt
            .query_map([], |row| {
                Ok(MigrationRecord {
                    version: row.get(0)?,
                    applied_at: row.get(1)?,
                    description: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Validate tenant context and tenant ID format (R1-H3, Task 9.1-9.3)
    fn validate_tenant(&self, ctx: &TenantContext) -> Result<String, GraphError> {
        let tenant_id = ctx.tenant_id.as_str();
        if tenant_id.is_empty() {
            Self::log_security_audit("REJECTED", "empty_tenant_id", "", "Empty tenant ID");
            return Err(GraphError::InvalidTenantContext);
        }

        Self::validate_tenant_id_format(tenant_id)?;
        Ok(tenant_id.to_string())
    }

    /// Validate tenant ID format to prevent SQL injection (Task 9.1, 9.2)
    fn validate_tenant_id_format(tenant_id: &str) -> Result<(), GraphError> {
        if tenant_id.is_empty() {
            Self::log_security_audit("REJECTED", "empty_tenant_id", tenant_id, "Empty tenant ID");
            return Err(GraphError::InvalidTenantIdFormat(
                "Tenant ID cannot be empty".to_string(),
            ));
        }

        if tenant_id.len() > 128 {
            Self::log_security_audit(
                "REJECTED",
                "tenant_id_too_long",
                tenant_id,
                "Tenant ID exceeds 128 chars",
            );
            return Err(GraphError::InvalidTenantIdFormat(
                "Tenant ID exceeds maximum length of 128 characters".to_string(),
            ));
        }

        if !tenant_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            Self::log_security_audit(
                "REJECTED",
                "invalid_tenant_id_chars",
                tenant_id,
                "Invalid characters in tenant ID",
            );
            return Err(GraphError::InvalidTenantIdFormat(
                "Tenant ID contains invalid characters (allowed: alphanumeric, -, _)".to_string(),
            ));
        }

        let sql_injection_patterns = [
            "--", ";", "'", "\"", "/*", "*/", "UNION", "SELECT", "INSERT", "UPDATE", "DELETE",
            "DROP", "EXEC", "EXECUTE", "xp_",
        ];

        let upper_tenant_id = tenant_id.to_uppercase();
        for pattern in &sql_injection_patterns {
            if upper_tenant_id.contains(pattern) {
                Self::log_security_audit(
                    "BLOCKED",
                    "sql_injection_attempt",
                    tenant_id,
                    &format!("SQL injection pattern detected: {}", pattern),
                );
                return Err(GraphError::InvalidTenantIdFormat(
                    "Tenant ID contains disallowed pattern".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Log security audit events (Task 9.3)
    fn log_security_audit(action: &str, event_type: &str, tenant_id: &str, details: &str) {
        error!(
            target: "security_audit",
            action = action,
            event_type = event_type,
            tenant_id = tenant_id,
            details = details,
            "Security audit: {} - {} for tenant '{}': {}",
            action, event_type, tenant_id, details
        );
    }

    /// Check if a node exists and belongs to the tenant (R1-C2)
    #[instrument(skip(self, conn))]
    fn node_exists(
        &self,
        conn: &Connection,
        node_id: &str,
        tenant_id: &str,
    ) -> Result<bool, GraphError> {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM memory_nodes WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
            params![node_id, tenant_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Soft-delete a node and cascade to edges (R1-C1)
    #[instrument(skip(self), fields(node_id = %node_id))]
    pub fn soft_delete_node(&self, ctx: TenantContext, node_id: &str) -> Result<(), GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();

        // Soft-delete the node
        let updated = conn.execute(
            "UPDATE memory_nodes SET deleted_at = ? WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
            params![now, node_id, tenant_id],
        )?;

        if updated == 0 {
            return Err(GraphError::NodeNotFound(node_id.to_string()));
        }

        // Cascade soft-delete to edges (R1-C1)
        conn.execute(
            "UPDATE memory_edges SET deleted_at = ? WHERE (source_id = ? OR target_id = ?) AND tenant_id = ? AND deleted_at IS NULL",
            params![now, node_id, node_id, tenant_id],
        )?;

        info!("Soft-deleted node {} and cascaded to edges", node_id);
        Ok(())
    }

    /// Soft-delete all nodes originating from a specific memory entry
    /// Looks for nodes with `source_memory_id` in their properties JSON
    #[instrument(skip(self), fields(source_memory_id = %source_memory_id))]
    pub fn soft_delete_nodes_by_source_memory_id(
        &self,
        ctx: TenantContext,
        source_memory_id: &str,
    ) -> Result<usize, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id FROM memory_nodes 
             WHERE tenant_id = ? 
             AND deleted_at IS NULL 
             AND json_extract_string(properties, '$.source_memory_id') = ?",
        )?;

        let node_ids: Vec<String> = stmt
            .query_map(params![tenant_id, source_memory_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        if node_ids.is_empty() {
            debug!("No nodes found with source_memory_id: {}", source_memory_id);
            return Ok(0);
        }

        let nodes_deleted = conn.execute(
            "UPDATE memory_nodes SET deleted_at = ? 
             WHERE tenant_id = ? 
             AND deleted_at IS NULL 
             AND json_extract_string(properties, '$.source_memory_id') = ?",
            params![now, tenant_id, source_memory_id],
        )?;

        for node_id in &node_ids {
            conn.execute(
                "UPDATE memory_edges SET deleted_at = ? 
                 WHERE (source_id = ? OR target_id = ?) 
                 AND tenant_id = ? 
                 AND deleted_at IS NULL",
                params![now, node_id, node_id, tenant_id],
            )?;
        }

        info!(
            "Soft-deleted {} nodes with source_memory_id {} and cascaded to edges",
            nodes_deleted, source_memory_id
        );
        Ok(nodes_deleted)
    }

    /// Hard delete nodes and edges marked as deleted (cleanup job)
    #[instrument(skip(self))]
    pub fn cleanup_deleted(&self, older_than: DateTime<Utc>) -> Result<usize, GraphError> {
        let conn = self.conn.lock();
        let cutoff = older_than.to_rfc3339();

        // Delete edges first (referential integrity)
        let edges_deleted = conn.execute(
            "DELETE FROM memory_edges WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            params![cutoff],
        )?;

        // Then delete nodes
        let nodes_deleted = conn.execute(
            "DELETE FROM memory_nodes WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            params![cutoff],
        )?;

        // Delete entity edges
        let entity_edges_deleted = conn.execute(
            "DELETE FROM entity_edges WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            params![cutoff],
        )?;

        // Delete entities
        let entities_deleted = conn.execute(
            "DELETE FROM entities WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            params![cutoff],
        )?;

        let total = edges_deleted + nodes_deleted + entity_edges_deleted + entities_deleted;
        info!("Cleanup completed: {} records permanently deleted", total);
        Ok(total)
    }

    /// Find related nodes within N hops (Task 2.5)
    #[instrument(skip(self), fields(node_id = %node_id, max_hops = %max_hops))]
    pub fn find_related(
        &self,
        ctx: TenantContext,
        node_id: &str,
        max_hops: usize,
    ) -> Result<Vec<(GraphEdgeExtended, GraphNodeExtended)>, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let effective_max_hops = max_hops.min(self.config.max_path_depth);

        if max_hops > self.config.max_path_depth {
            warn!(
                "Requested hop depth {} exceeds maximum {}, limiting",
                max_hops, self.config.max_path_depth
            );
        }

        let conn = self.conn.lock();

        // Use recursive CTE for multi-hop traversal
        let query = format!(
            r#"
            WITH RECURSIVE related AS (
                -- Base case: direct neighbors
                SELECT 
                    e.id as edge_id,
                    e.source_id,
                    e.target_id,
                    e.relation,
                    e.properties as edge_properties,
                    e.weight,
                    CAST(e.created_at AS VARCHAR) as edge_created_at,
                    n.id as node_id,
                    n.label,
                    n.properties as node_properties,
                    n.memory_id,
                    CAST(n.created_at AS VARCHAR) as node_created_at,
                    CAST(n.updated_at AS VARCHAR) as node_updated_at,
                    1 as depth
                FROM memory_edges e
                JOIN memory_nodes n ON (
                    CASE WHEN e.source_id = ? THEN e.target_id ELSE e.source_id END = n.id
                )
                WHERE (e.source_id = ? OR e.target_id = ?)
                    AND e.tenant_id = ?
                    AND e.deleted_at IS NULL
                    AND n.tenant_id = ?
                    AND n.deleted_at IS NULL
                
                UNION ALL
                
                -- Recursive case: neighbors of neighbors
                SELECT 
                    e.id as edge_id,
                    e.source_id,
                    e.target_id,
                    e.relation,
                    e.properties as edge_properties,
                    e.weight,
                    CAST(e.created_at AS VARCHAR) as edge_created_at,
                    n.id as node_id,
                    n.label,
                    n.properties as node_properties,
                    n.memory_id,
                    CAST(n.created_at AS VARCHAR) as node_created_at,
                    CAST(n.updated_at AS VARCHAR) as node_updated_at,
                    r.depth + 1
                FROM related r
                JOIN memory_edges e ON (e.source_id = r.node_id OR e.target_id = r.node_id)
                JOIN memory_nodes n ON (
                    CASE WHEN e.source_id = r.node_id THEN e.target_id ELSE e.source_id END = n.id
                )
                WHERE e.tenant_id = ?
                    AND e.deleted_at IS NULL
                    AND n.tenant_id = ?
                    AND n.deleted_at IS NULL
                    AND n.id != ?  -- Don't revisit start node
                    AND r.depth < {}
            )
            SELECT DISTINCT * FROM related
            ORDER BY depth, edge_created_at
            "#,
            effective_max_hops
        );

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(
            params![
                node_id, node_id, node_id, tenant_id, tenant_id, tenant_id, tenant_id, node_id
            ],
            |row| {
                Ok((
                    GraphEdgeExtended {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: row
                            .get::<_, Option<String>>(4)?
                            .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                            .unwrap_or(serde_json::Value::Null),
                        tenant_id: tenant_id.clone(),
                        weight: row.get(5)?,
                        created_at: row
                            .get::<_, Option<String>>(6)?
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(Utc::now),
                        deleted_at: None,
                    },
                    GraphNodeExtended {
                        id: row.get(7)?,
                        label: row.get(8)?,
                        properties: row
                            .get::<_, Option<String>>(9)?
                            .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                            .unwrap_or(serde_json::Value::Null),
                        tenant_id: tenant_id.clone(),
                        memory_id: row.get(10)?,
                        created_at: row
                            .get::<_, Option<String>>(11)?
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(Utc::now),
                        updated_at: row
                            .get::<_, Option<String>>(12)?
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(Utc::now),
                        deleted_at: None,
                    },
                ))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        debug!(
            "Found {} related nodes within {} hops",
            results.len(),
            effective_max_hops
        );
        Ok(results)
    }

    /// Find shortest path between two nodes (Task 2.6)
    ///
    /// Uses BFS-style traversal with recursive CTE. Returns edge path as comma-separated
    /// string due to DuckDB array type limitations.
    #[instrument(skip(self), fields(start_id = %start_id, end_id = %end_id))]
    pub fn shortest_path(
        &self,
        ctx: TenantContext,
        start_id: &str,
        end_id: &str,
        max_depth: Option<usize>,
    ) -> Result<Vec<GraphEdgeExtended>, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let effective_max_depth = max_depth
            .unwrap_or(self.config.max_path_depth)
            .min(self.config.max_path_depth);

        if effective_max_depth > self.config.max_path_depth {
            return Err(GraphError::MaxDepthExceeded(self.config.max_path_depth));
        }

        let conn = self.conn.lock();

        // BFS-style path finding using recursive CTE
        // Use string concatenation instead of ARRAY to avoid FromSql limitation
        let query = format!(
            r#"
            WITH RECURSIVE paths AS (
                -- Base case: edges from start node
                SELECT 
                    e.id,
                    e.source_id,
                    e.target_id,
                    e.relation,
                    e.properties,
                    e.weight,
                    e.created_at,
                    e.id as path_str,
                    1 as depth,
                    CASE WHEN e.target_id = ? THEN true ELSE false END as found
                FROM memory_edges e
                WHERE e.source_id = ?
                    AND e.tenant_id = ?
                    AND e.deleted_at IS NULL
                
                UNION ALL
                
                SELECT 
                    e.id,
                    e.source_id,
                    e.target_id,
                    e.relation,
                    e.properties,
                    e.weight,
                    e.created_at,
                    p.path_str || ',' || e.id,
                    p.depth + 1,
                    CASE WHEN e.target_id = ? THEN true ELSE false END
                FROM memory_edges e
                JOIN paths p ON e.source_id = p.target_id
                WHERE e.tenant_id = ?
                    AND e.deleted_at IS NULL
                    AND p.path_str NOT LIKE '%' || e.id || '%'
                    AND NOT p.found
                    AND p.depth < {}
            )
            SELECT path_str, depth
            FROM paths
            WHERE found = true
            ORDER BY depth ASC
            LIMIT 1
            "#,
            effective_max_depth
        );

        let result = conn.query_row(
            &query,
            params![end_id, start_id, tenant_id, end_id, tenant_id],
            |row| {
                let path_str: String = row.get(0)?;
                Ok(path_str)
            },
        );

        match result {
            Ok(path_str) => {
                let path_ids: Vec<&str> = path_str.split(',').collect();
                let mut edges = Vec::new();
                for edge_id in path_ids {
                    let edge = self.get_edge_by_id(&conn, edge_id, &tenant_id)?;
                    edges.push(edge);
                }
                debug!("Found path with {} edges", edges.len());
                Ok(edges)
            }
            Err(duckdb::Error::QueryReturnedNoRows) => {
                debug!("No path found between {} and {}", start_id, end_id);
                Ok(vec![])
            }
            Err(e) => Err(GraphError::DuckDb(e)),
        }
    }

    /// Get edge by ID (helper)
    fn get_edge_by_id(
        &self,
        conn: &Connection,
        edge_id: &str,
        tenant_id: &str,
    ) -> Result<GraphEdgeExtended, GraphError> {
        conn.query_row(
            r#"
            SELECT id, source_id, target_id, relation, properties, weight, 
                   CAST(created_at AS VARCHAR) as created_at_str, 
                   CAST(deleted_at AS VARCHAR) as deleted_at_str
            FROM memory_edges
            WHERE id = ? AND tenant_id = ?
            "#,
            params![edge_id, tenant_id],
            |row| {
                Ok(GraphEdgeExtended {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    target_id: row.get(2)?,
                    relation: row.get(3)?,
                    properties: row
                        .get::<_, Option<String>>(4)?
                        .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    tenant_id: tenant_id.to_string(),
                    weight: row.get(5)?,
                    created_at: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(Utc::now),
                    deleted_at: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|s| s.parse().ok()),
                })
            },
        )
        .map_err(|e| match e {
            duckdb::Error::QueryReturnedNoRows => GraphError::EdgeNotFound(edge_id.to_string()),
            _ => GraphError::DuckDb(e),
        })
    }

    /// Add an entity to the knowledge graph
    #[instrument(skip(self, entity), fields(entity_id = %entity.id))]
    pub fn add_entity(&self, ctx: TenantContext, entity: Entity) -> Result<(), GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;

        if entity.tenant_id != tenant_id {
            return Err(GraphError::TenantViolation(
                "Entity tenant_id does not match context".to_string(),
            ));
        }

        let conn = self.conn.lock();
        let properties_json = serde_json::to_string(&entity.properties)
            .map_err(|e| GraphError::Serialization(e.to_string()))?;

        conn.execute(
            r#"
            INSERT INTO entities (id, name, entity_type, properties, tenant_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            params![
                entity.id,
                entity.name,
                entity.entity_type,
                properties_json,
                tenant_id,
                entity.created_at.to_rfc3339()
            ],
        )?;

        debug!("Added entity {} of type {}", entity.id, entity.entity_type);
        Ok(())
    }

    /// Link two entities with a relationship
    #[instrument(skip(self), fields(source = %source_id, target = %target_id, relation = %relation))]
    pub fn link_entities(
        &self,
        ctx: TenantContext,
        source_id: &str,
        target_id: &str,
        relation: &str,
        properties: Option<serde_json::Value>,
    ) -> Result<String, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();

        // Verify both entities exist (R1-C2)
        let source_exists: i32 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
            params![source_id, tenant_id],
            |row| row.get(0),
        )?;

        let target_exists: i32 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
            params![target_id, tenant_id],
            |row| row.get(0),
        )?;

        if source_exists == 0 {
            return Err(GraphError::ReferentialIntegrity(format!(
                "Source entity {} does not exist",
                source_id
            )));
        }

        if target_exists == 0 {
            return Err(GraphError::ReferentialIntegrity(format!(
                "Target entity {} does not exist",
                target_id
            )));
        }

        let edge_id = Uuid::new_v4().to_string();
        let properties_json = properties
            .map(|p| serde_json::to_string(&p).unwrap_or_default())
            .unwrap_or_default();

        conn.execute(
            r#"
            INSERT INTO entity_edges (id, source_entity_id, target_entity_id, relation, properties, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            params![edge_id, source_id, target_id, relation, properties_json, tenant_id],
        )?;

        debug!(
            "Linked entities {} -> {} via {}",
            source_id, target_id, relation
        );
        Ok(edge_id)
    }

    /// Get statistics about the graph
    pub fn get_stats(&self, ctx: TenantContext) -> Result<GraphStats, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();

        let node_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_nodes WHERE tenant_id = ? AND deleted_at IS NULL",
            params![tenant_id],
            |row| row.get(0),
        )?;

        let edge_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_edges WHERE tenant_id = ? AND deleted_at IS NULL",
            params![tenant_id],
            |row| row.get(0),
        )?;

        let entity_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE tenant_id = ? AND deleted_at IS NULL",
            params![tenant_id],
            |row| row.get(0),
        )?;

        let entity_edge_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entity_edges WHERE tenant_id = ? AND deleted_at IS NULL",
            params![tenant_id],
            |row| row.get(0),
        )?;

        Ok(GraphStats {
            node_count: node_count as usize,
            edge_count: edge_count as usize,
            entity_count: entity_count as usize,
            entity_edge_count: entity_edge_count as usize,
        })
    }

    /// Persist graph data to S3 as Parquet with two-phase commit (R1-C4)
    ///
    /// Returns the S3 key of the persisted snapshot on success.
    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn persist_to_s3(&self, tenant_id: &str) -> Result<String, GraphError> {
        use aws_config::BehaviorVersion;
        use aws_sdk_s3::primitives::ByteStream;
        use sha2::{Digest, Sha256};

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;
        let prefix = self.config.s3_prefix.as_deref().unwrap_or("graph");

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let snapshot_key = format!("{}/{}/snapshot_{}.parquet", prefix, tenant_id, timestamp);
        let staging_key = format!("{}/.staging/{}", snapshot_key, Uuid::new_v4());

        let parquet_data = self.export_to_parquet(tenant_id)?;

        let mut hasher = Sha256::new();
        hasher.update(&parquet_data);
        let checksum = hex::encode(hasher.finalize());

        s3_client
            .put_object()
            .bucket(bucket)
            .key(&staging_key)
            .body(ByteStream::from(parquet_data.clone()))
            .metadata("checksum", &checksum)
            .metadata("tenant_id", tenant_id)
            .metadata("timestamp", &timestamp)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to upload staging: {}", e)))?;

        s3_client
            .copy_object()
            .bucket(bucket)
            .copy_source(format!("{}/{}", bucket, staging_key))
            .key(&snapshot_key)
            .metadata_directive(aws_sdk_s3::types::MetadataDirective::Copy)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to commit snapshot: {}", e)))?;

        s3_client
            .delete_object()
            .bucket(bucket)
            .key(&staging_key)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to cleanup staging: {}", e)))?;

        info!("Persisted graph snapshot to S3: {}", snapshot_key);
        Ok(snapshot_key)
    }

    /// Load graph from S3 Parquet snapshot with checksum verification
    #[instrument(skip(self), fields(tenant_id = %tenant_id, snapshot_key = %snapshot_key))]
    pub async fn load_from_s3(
        &self,
        tenant_id: &str,
        snapshot_key: &str,
    ) -> Result<(), GraphError> {
        use aws_config::BehaviorVersion;
        use sha2::{Digest, Sha256};

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let response = s3_client
            .get_object()
            .bucket(bucket)
            .key(snapshot_key)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to fetch snapshot: {}", e)))?;

        let expected_checksum = response.metadata().and_then(|m| m.get("checksum")).cloned();

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to read body: {}", e)))?
            .into_bytes()
            .to_vec();

        if let Some(expected) = expected_checksum {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let actual = hex::encode(hasher.finalize());
            if actual != expected {
                return Err(GraphError::ChecksumMismatch { expected, actual });
            }
        }

        self.import_from_parquet(tenant_id, &data)?;

        info!("Loaded graph snapshot from S3: {}", snapshot_key);
        Ok(())
    }

    fn export_to_parquet(&self, tenant_id: &str) -> Result<Vec<u8>, GraphError> {
        Self::validate_tenant_id_format(tenant_id)?;

        let conn = self.conn.lock();

        // Use parameterized query to prevent SQL injection
        let export_sql = r#"
            COPY (
                SELECT 'node' as record_type, id, label, properties, memory_id, 
                       CAST(created_at AS VARCHAR) as created_at, 
                       CAST(updated_at AS VARCHAR) as updated_at,
                       NULL as source_id, NULL as target_id, NULL as relation, NULL as weight
                FROM memory_nodes WHERE tenant_id = ? AND deleted_at IS NULL
                UNION ALL
                SELECT 'edge' as record_type, id, NULL as label, properties, NULL as memory_id,
                       CAST(created_at AS VARCHAR) as created_at, NULL as updated_at,
                       source_id, target_id, relation, CAST(weight AS VARCHAR)
                FROM memory_edges WHERE tenant_id = ? AND deleted_at IS NULL
            ) TO '/dev/stdout' (FORMAT PARQUET)
            "#
        .to_string();

        let temp_path = format!("/tmp/graph_export_{}.parquet", Uuid::new_v4());
        let export_sql = export_sql.replace("/dev/stdout", &temp_path);
        conn.prepare(&export_sql)?.execute([tenant_id])?;

        let data = std::fs::read(&temp_path)?;
        std::fs::remove_file(&temp_path).ok();

        Ok(data)
    }

    fn import_from_parquet(&self, tenant_id: &str, data: &[u8]) -> Result<(), GraphError> {
        Self::validate_tenant_id_format(tenant_id)?;

        let conn = self.conn.lock();

        let temp_path = format!("/tmp/graph_import_{}.parquet", Uuid::new_v4());
        std::fs::write(&temp_path, data)?;

        conn.execute(
            "DELETE FROM memory_edges WHERE tenant_id = ?",
            params![tenant_id],
        )?;
        conn.execute(
            "DELETE FROM memory_nodes WHERE tenant_id = ?",
            params![tenant_id],
        )?;

        let import_nodes_sql = format!(
            r#"
            INSERT INTO memory_nodes (id, label, properties, memory_id, tenant_id, created_at, updated_at)
            SELECT id, label, properties, memory_id, '{tenant_id}', 
                   TRY_CAST(created_at AS TIMESTAMP), TRY_CAST(updated_at AS TIMESTAMP)
            FROM read_parquet('{path}')
            WHERE record_type = 'node'
            "#,
            tenant_id = tenant_id,
            path = temp_path
        );
        conn.execute_batch(&import_nodes_sql)?;

        let import_edges_sql = format!(
            r#"
            INSERT INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id, weight, created_at)
            SELECT id, source_id, target_id, relation, properties, '{tenant_id}', 
                   TRY_CAST(weight AS DOUBLE), TRY_CAST(created_at AS TIMESTAMP)
            FROM read_parquet('{path}')
            WHERE record_type = 'edge'
            "#,
            tenant_id = tenant_id,
            path = temp_path
        );
        conn.execute_batch(&import_edges_sql)?;

        std::fs::remove_file(&temp_path).ok();

        debug!("Imported graph data from parquet for tenant {}", tenant_id);
        Ok(())
    }

    // ==================== Backup & Recovery (R1-H4) ====================

    #[instrument(skip(self, backup_config), fields(tenant_id = %tenant_id))]
    pub async fn create_backup(
        &self,
        tenant_id: &str,
        backup_config: &BackupConfig,
    ) -> Result<BackupResult, GraphError> {
        use aws_config::BehaviorVersion;
        use aws_sdk_s3::primitives::ByteStream;
        use sha2::{Digest, Sha256};

        let start = std::time::Instant::now();
        Self::validate_tenant_id_format(tenant_id)?;

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let snapshot_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let s3_key = format!(
            "{}/{}/{}/snapshot_{}.parquet",
            backup_config.backup_prefix, tenant_id, timestamp, snapshot_id
        );

        let parquet_data = self.export_to_parquet(tenant_id)?;
        let size_bytes = parquet_data.len() as u64;

        let mut hasher = Sha256::new();
        hasher.update(&parquet_data);
        let checksum = hex::encode(hasher.finalize());

        let stats = self.get_stats_internal(tenant_id)?;
        let metadata = SnapshotMetadata {
            snapshot_id: snapshot_id.clone(),
            tenant_id: tenant_id.to_string(),
            s3_key: s3_key.clone(),
            created_at: Utc::now(),
            size_bytes,
            checksum: checksum.clone(),
            node_count: stats.node_count as u64,
            edge_count: stats.edge_count as u64,
            schema_version: SCHEMA_VERSION,
        };

        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| GraphError::Serialization(e.to_string()))?;

        s3_client
            .put_object()
            .bucket(bucket)
            .key(&s3_key)
            .body(ByteStream::from(parquet_data))
            .metadata("checksum", &checksum)
            .metadata("tenant_id", tenant_id)
            .metadata("snapshot_id", &snapshot_id)
            .metadata("snapshot_metadata", &metadata_json)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to upload backup: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            snapshot_id = %snapshot_id,
            size_bytes = size_bytes,
            duration_ms = duration_ms,
            "Created backup snapshot"
        );

        Ok(BackupResult {
            snapshot_id,
            s3_key,
            size_bytes,
            duration_ms,
            checksum,
        })
    }

    #[instrument(skip(self, backup_config), fields(tenant_id = %tenant_id))]
    pub async fn list_snapshots(
        &self,
        tenant_id: &str,
        backup_config: &BackupConfig,
    ) -> Result<Vec<SnapshotMetadata>, GraphError> {
        use aws_config::BehaviorVersion;

        Self::validate_tenant_id_format(tenant_id)?;

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let prefix = format!("{}/{}/", backup_config.backup_prefix, tenant_id);

        let response = s3_client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&prefix)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to list snapshots: {}", e)))?;

        let mut snapshots = Vec::new();

        for obj in response.contents() {
            if let Some(key) = obj.key() {
                if key.ends_with(".parquet") {
                    let head = s3_client
                        .head_object()
                        .bucket(bucket)
                        .key(key)
                        .send()
                        .await
                        .map_err(|e| {
                            GraphError::S3(format!("Failed to get object metadata: {}", e))
                        })?;

                    if let Some(metadata_json) =
                        head.metadata().and_then(|m| m.get("snapshot_metadata"))
                    {
                        if let Ok(metadata) =
                            serde_json::from_str::<SnapshotMetadata>(metadata_json)
                        {
                            snapshots.push(metadata);
                        }
                    }
                }
            }
        }

        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snapshots)
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, snapshot_id = %snapshot_id))]
    pub async fn restore_from_snapshot(
        &self,
        tenant_id: &str,
        snapshot_id: &str,
        backup_config: &BackupConfig,
    ) -> Result<RecoveryResult, GraphError> {
        use aws_config::BehaviorVersion;
        use sha2::{Digest, Sha256};

        let start = std::time::Instant::now();
        Self::validate_tenant_id_format(tenant_id)?;

        let snapshots = self.list_snapshots(tenant_id, backup_config).await?;
        let snapshot = snapshots
            .iter()
            .find(|s| s.snapshot_id == snapshot_id)
            .ok_or_else(|| GraphError::S3(format!("Snapshot not found: {}", snapshot_id)))?;

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let response = s3_client
            .get_object()
            .bucket(bucket)
            .key(&snapshot.s3_key)
            .send()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to fetch snapshot: {}", e)))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| GraphError::S3(format!("Failed to read body: {}", e)))?
            .into_bytes()
            .to_vec();

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let actual_checksum = hex::encode(hasher.finalize());
        if actual_checksum != snapshot.checksum {
            return Err(GraphError::ChecksumMismatch {
                expected: snapshot.checksum.clone(),
                actual: actual_checksum,
            });
        }

        self.import_from_parquet(tenant_id, &data)?;

        let stats = self.get_stats_internal(tenant_id)?;
        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            snapshot_id = %snapshot_id,
            nodes_restored = stats.node_count,
            edges_restored = stats.edge_count,
            duration_ms = duration_ms,
            "Restored from backup snapshot"
        );

        Ok(RecoveryResult {
            snapshot_id: snapshot_id.to_string(),
            nodes_restored: stats.node_count as u64,
            edges_restored: stats.edge_count as u64,
            duration_ms,
        })
    }

    #[instrument(skip(self, backup_config), fields(tenant_id = %tenant_id))]
    pub async fn apply_retention_policy(
        &self,
        tenant_id: &str,
        backup_config: &BackupConfig,
    ) -> Result<usize, GraphError> {
        use aws_config::BehaviorVersion;

        Self::validate_tenant_id_format(tenant_id)?;

        let bucket = self
            .config
            .s3_bucket
            .as_ref()
            .ok_or_else(|| GraphError::S3("S3 bucket not configured".to_string()))?;

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        let mut snapshots = self.list_snapshots(tenant_id, backup_config).await?;
        let mut deleted_count = 0;

        let cutoff_time =
            Utc::now() - chrono::Duration::seconds(backup_config.retention_max_age_secs as i64);

        let mut to_delete: Vec<String> = Vec::new();

        for snapshot in snapshots.iter() {
            if snapshot.created_at < cutoff_time {
                to_delete.push(snapshot.s3_key.clone());
            }
        }

        snapshots.retain(|s| s.created_at >= cutoff_time);

        if snapshots.len() > backup_config.retention_count {
            let excess = snapshots.len() - backup_config.retention_count;
            for snapshot in snapshots.iter().rev().take(excess) {
                if !to_delete.contains(&snapshot.s3_key) {
                    to_delete.push(snapshot.s3_key.clone());
                }
            }
        }

        for key in to_delete {
            s3_client
                .delete_object()
                .bucket(bucket)
                .key(&key)
                .send()
                .await
                .map_err(|e| GraphError::S3(format!("Failed to delete old snapshot: {}", e)))?;
            deleted_count += 1;
            debug!(s3_key = %key, "Deleted old snapshot per retention policy");
        }

        info!(
            tenant_id = %tenant_id,
            deleted_count = deleted_count,
            "Applied retention policy"
        );

        Ok(deleted_count)
    }

    fn get_stats_internal(&self, tenant_id: &str) -> Result<GraphStats, GraphError> {
        let conn = self.conn.lock();

        let node_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_nodes WHERE tenant_id = ? AND deleted_at IS NULL",
                params![tenant_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let edge_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_edges WHERE tenant_id = ? AND deleted_at IS NULL",
                params![tenant_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let entity_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE tenant_id = ? AND deleted_at IS NULL",
                params![tenant_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let entity_edge_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_edges WHERE tenant_id = ? AND deleted_at IS NULL",
                params![tenant_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(GraphStats {
            node_count: node_count as usize,
            edge_count: edge_count as usize,
            entity_count: entity_count as usize,
            entity_edge_count: entity_edge_count as usize,
        })
    }

    // ==================== Transaction Atomicity (R1-H5) ====================

    #[instrument(skip(self, nodes, edges), fields(tenant_id = %tenant_id, nodes = nodes.len(), edges = edges.len()))]
    pub fn add_nodes_and_edges_atomic(
        &self,
        ctx: &TenantContext,
        tenant_id: &str,
        nodes: Vec<GraphNode>,
        edges: Vec<GraphEdge>,
    ) -> Result<(), GraphError> {
        let _ = ctx;
        Self::validate_tenant_id_format(tenant_id)?;

        let conn = self.conn.lock();

        conn.execute_batch("BEGIN TRANSACTION")?;

        let result = (|| -> Result<(), GraphError> {
            for node in &nodes {
                if node.tenant_id != tenant_id {
                    return Err(GraphError::TenantViolation(
                        "Node tenant_id does not match context".to_string(),
                    ));
                }

                let properties_json = serde_json::to_string(&node.properties)
                    .map_err(|e| GraphError::Serialization(e.to_string()))?;

                conn.execute(
                    r#"
                    INSERT INTO memory_nodes (id, label, properties, tenant_id)
                    VALUES (?, ?, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        label = EXCLUDED.label,
                        properties = EXCLUDED.properties,
                        updated_at = now()
                    "#,
                    params![node.id, node.label, properties_json, tenant_id],
                )?;
            }

            for edge in &edges {
                if edge.tenant_id != tenant_id {
                    return Err(GraphError::TenantViolation(
                        "Edge tenant_id does not match context".to_string(),
                    ));
                }

                if !self.node_exists(&conn, &edge.source_id, tenant_id)? {
                    return Err(GraphError::ReferentialIntegrity(format!(
                        "Source node {} does not exist",
                        edge.source_id
                    )));
                }

                if !self.node_exists(&conn, &edge.target_id, tenant_id)? {
                    return Err(GraphError::ReferentialIntegrity(format!(
                        "Target node {} does not exist",
                        edge.target_id
                    )));
                }

                let properties_json = serde_json::to_string(&edge.properties)
                    .map_err(|e| GraphError::Serialization(e.to_string()))?;

                conn.execute(
                    r#"
                    INSERT INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        relation = EXCLUDED.relation,
                        properties = EXCLUDED.properties
                    "#,
                    params![
                        edge.id,
                        edge.source_id,
                        edge.target_id,
                        edge.relation,
                        properties_json,
                        tenant_id
                    ],
                )?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                info!(
                    nodes_added = nodes.len(),
                    edges_added = edges.len(),
                    "Atomic batch insert committed"
                );
                Ok(())
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK")?;
                warn!(error = %e, "Atomic batch insert rolled back");
                Err(e)
            }
        }
    }

    #[instrument(skip(self, entities, entity_edges), fields(tenant_id = %tenant_id))]
    pub fn add_entities_atomic(
        &self,
        ctx: &TenantContext,
        tenant_id: &str,
        entities: Vec<Entity>,
        entity_edges: Vec<EntityEdge>,
    ) -> Result<(), GraphError> {
        let _ = ctx;
        Self::validate_tenant_id_format(tenant_id)?;

        let conn = self.conn.lock();

        conn.execute_batch("BEGIN TRANSACTION")?;

        let result = (|| -> Result<(), GraphError> {
            for entity in &entities {
                if entity.tenant_id != tenant_id {
                    return Err(GraphError::TenantViolation(
                        "Entity tenant_id does not match context".to_string(),
                    ));
                }

                let properties_json = serde_json::to_string(&entity.properties)
                    .map_err(|e| GraphError::Serialization(e.to_string()))?;

                conn.execute(
                    r#"
                    INSERT INTO entities (id, name, entity_type, properties, tenant_id)
                    VALUES (?, ?, ?, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        name = EXCLUDED.name,
                        entity_type = EXCLUDED.entity_type,
                        properties = EXCLUDED.properties
                    "#,
                    params![
                        entity.id,
                        entity.name,
                        entity.entity_type,
                        properties_json,
                        tenant_id
                    ],
                )?;
            }

            for edge in &entity_edges {
                if edge.tenant_id != tenant_id {
                    return Err(GraphError::TenantViolation(
                        "EntityEdge tenant_id does not match context".to_string(),
                    ));
                }

                let properties_json = serde_json::to_string(&edge.properties)
                    .map_err(|e| GraphError::Serialization(e.to_string()))?;

                conn.execute(
                    r#"
                    INSERT INTO entity_edges (id, source_entity_id, target_entity_id, relation, properties, tenant_id)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT (id) DO UPDATE SET
                        relation = EXCLUDED.relation,
                        properties = EXCLUDED.properties
                    "#,
                    params![
                        edge.id,
                        edge.source_entity_id,
                        edge.target_entity_id,
                        edge.relation,
                        properties_json,
                        tenant_id
                    ],
                )?;
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                info!(
                    entities_added = entities.len(),
                    edges_added = entity_edges.len(),
                    "Atomic entity batch insert committed"
                );
                Ok(())
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK")?;
                warn!(error = %e, "Atomic entity batch insert rolled back");
                Err(e)
            }
        }
    }

    pub fn with_transaction<F, T>(&self, f: F) -> Result<T, GraphError>
    where
        F: FnOnce(&duckdb::Connection) -> Result<T, GraphError>,
    {
        let conn = self.conn.lock();

        conn.execute_batch("BEGIN TRANSACTION")?;

        match f(&conn) {
            Ok(result) => {
                conn.execute_batch("COMMIT")?;
                Ok(result)
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK")?;
                Err(e)
            }
        }
    }

    // ==================== Health Checks (R1-H8) ====================

    pub fn health_check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let duckdb_status = self.check_duckdb_health();
        let duckdb_latency_ms = start.elapsed().as_millis() as u64;

        let s3_start = std::time::Instant::now();
        let s3_status = self.check_s3_config();
        let s3_latency_ms = s3_start.elapsed().as_millis() as u64;

        let schema_version = self.get_schema_version().unwrap_or(-1);

        let overall_healthy = duckdb_status.is_healthy && s3_status.is_healthy;

        HealthCheckResult {
            healthy: overall_healthy,
            duckdb: duckdb_status,
            s3: s3_status,
            schema_version,
            total_latency_ms: start.elapsed().as_millis() as u64,
            duckdb_latency_ms,
            s3_latency_ms,
        }
    }

    pub fn readiness_check(&self) -> ReadinessResult {
        let start = std::time::Instant::now();

        let duckdb_ready = self.check_duckdb_ready();
        let schema_ready = self.check_schema_ready();

        let ready = duckdb_ready && schema_ready;

        ReadinessResult {
            ready,
            duckdb_ready,
            schema_ready,
            latency_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn check_duckdb_health(&self) -> ComponentHealth {
        let conn = self.conn.lock();

        match conn.query_row("SELECT 1", [], |row| row.get::<_, i32>(0)) {
            Ok(1) => ComponentHealth {
                is_healthy: true,
                message: "DuckDB connection OK".to_string(),
            },
            Ok(_) => ComponentHealth {
                is_healthy: false,
                message: "DuckDB returned unexpected value".to_string(),
            },
            Err(e) => ComponentHealth {
                is_healthy: false,
                message: format!("DuckDB query failed: {}", e),
            },
        }
    }

    fn check_s3_config(&self) -> ComponentHealth {
        if self.config.s3_bucket.is_none() {
            return ComponentHealth {
                is_healthy: true,
                message: "S3 not configured (optional)".to_string(),
            };
        }

        ComponentHealth {
            is_healthy: true,
            message: format!(
                "S3 configured: bucket={}",
                self.config.s3_bucket.as_ref().unwrap()
            ),
        }
    }

    fn check_duckdb_ready(&self) -> bool {
        let conn = self.conn.lock();
        conn.query_row("SELECT 1", [], |row| row.get::<_, i32>(0))
            .is_ok()
    }

    fn check_schema_ready(&self) -> bool {
        let conn = self.conn.lock();

        let tables_exist = conn
            .query_row(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('memory_nodes', 'memory_edges', 'entities', 'entity_edges', 'schema_version')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0);

        tables_exist >= 5
    }

    fn get_schema_version(&self) -> Result<i32, GraphError> {
        let conn = self.conn.lock();

        let version: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;

        Ok(version)
    }

    pub async fn check_s3_connectivity(&self) -> ComponentHealth {
        use aws_config::BehaviorVersion;

        let bucket = match &self.config.s3_bucket {
            Some(b) => b,
            None => {
                return ComponentHealth {
                    is_healthy: true,
                    message: "S3 not configured".to_string(),
                };
            }
        };

        let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
        if let Some(endpoint) = &self.config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }
        if let Some(region) = &self.config.s3_region {
            config_builder = config_builder.region(aws_config::Region::new(region.clone()));
        }
        let aws_config = config_builder.load().await;
        let s3_client = aws_sdk_s3::Client::new(&aws_config);

        match s3_client.head_bucket().bucket(bucket).send().await {
            Ok(_) => ComponentHealth {
                is_healthy: true,
                message: format!("S3 bucket '{}' accessible", bucket),
            },
            Err(e) => ComponentHealth {
                is_healthy: false,
                message: format!("S3 bucket '{}' not accessible: {}", bucket, e),
            },
        }
    }

    // ==================== Cold Start Optimization (R1-H1, R1-H7) ====================

    pub fn record_partition_access(
        &self,
        tenant_id: &str,
        partition_key: &str,
        load_time_ms: f64,
    ) -> Result<(), GraphError> {
        if !self.config.cold_start.access_tracking_enabled {
            return Ok(());
        }

        Self::validate_tenant_id_format(tenant_id)?;
        let conn = self.conn.lock();

        conn.execute(
            r#"
            INSERT INTO partition_access (partition_key, tenant_id, access_count, last_access, total_load_time_ms)
            VALUES (?, ?, 1, now(), ?)
            ON CONFLICT (partition_key, tenant_id) DO UPDATE SET
                access_count = partition_access.access_count + 1,
                last_access = now(),
                total_load_time_ms = partition_access.total_load_time_ms + EXCLUDED.total_load_time_ms
            "#,
            params![partition_key, tenant_id, load_time_ms],
        )?;

        Ok(())
    }

    pub fn get_partition_access_records(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<PartitionAccessRecord>, GraphError> {
        Self::validate_tenant_id_format(tenant_id)?;
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            r#"
            SELECT 
                partition_key,
                tenant_id,
                access_count,
                CAST(last_access AS VARCHAR) as last_access,
                CASE WHEN access_count > 0 THEN total_load_time_ms / access_count ELSE 0 END as avg_load_time_ms
            FROM partition_access
            WHERE tenant_id = ?
            ORDER BY last_access DESC
            LIMIT ?
            "#,
        )?;

        let records = stmt
            .query_map(
                params![
                    tenant_id,
                    self.config.cold_start.prewarm_partition_count as i64
                ],
                |row| {
                    let last_access_str: String = row.get(3)?;
                    let last_access =
                        DateTime::parse_from_str(&last_access_str, "%Y-%m-%d %H:%M:%S")
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now());

                    Ok(PartitionAccessRecord {
                        partition_key: row.get(0)?,
                        tenant_id: row.get(1)?,
                        access_count: row.get(2)?,
                        last_access,
                        avg_load_time_ms: row.get(4)?,
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    pub fn get_prewarm_partitions(&self, tenant_id: &str) -> Result<Vec<String>, GraphError> {
        let records = self.get_partition_access_records(tenant_id)?;
        Ok(records.into_iter().map(|r| r.partition_key).collect())
    }

    pub async fn lazy_load_partitions(
        &self,
        tenant_id: &str,
        partition_keys: &[String],
    ) -> Result<LazyLoadResult, GraphError> {
        if !self.config.cold_start.lazy_loading_enabled {
            return Ok(LazyLoadResult {
                partitions_loaded: 0,
                total_load_time_ms: 0,
                budget_remaining_ms: self.config.cold_start.budget_ms,
                deferred_partitions: vec![],
            });
        }

        Self::validate_tenant_id_format(tenant_id)?;

        let start = std::time::Instant::now();
        let budget_ms = self.config.cold_start.budget_ms;
        let mut loaded = 0;
        let mut deferred = vec![];

        for partition_key in partition_keys {
            let elapsed_ms = start.elapsed().as_millis() as u64;

            if elapsed_ms >= budget_ms {
                deferred.push(partition_key.clone());
                continue;
            }

            let partition_start = std::time::Instant::now();

            if let Err(e) = self.load_partition_data(tenant_id, partition_key).await {
                warn!(
                    partition = partition_key,
                    error = %e,
                    "Failed to load partition, deferring"
                );
                deferred.push(partition_key.clone());
                continue;
            }

            let load_time_ms = partition_start.elapsed().as_millis() as f64;
            self.record_partition_access(tenant_id, partition_key, load_time_ms)?;
            loaded += 1;

            metrics::histogram!("graph_partition_load_time_ms", load_time_ms);
        }

        let total_load_time_ms = start.elapsed().as_millis() as u64;
        let budget_remaining_ms = budget_ms.saturating_sub(total_load_time_ms);

        metrics::gauge!(
            "graph_cold_start_budget_remaining_ms",
            budget_remaining_ms as f64
        );
        metrics::counter!("graph_partitions_loaded_total", loaded as u64);
        metrics::counter!("graph_partitions_deferred_total", deferred.len() as u64);

        info!(
            loaded = loaded,
            deferred = deferred.len(),
            total_time_ms = total_load_time_ms,
            budget_remaining_ms = budget_remaining_ms,
            "Lazy partition loading completed"
        );

        Ok(LazyLoadResult {
            partitions_loaded: loaded,
            total_load_time_ms,
            budget_remaining_ms,
            deferred_partitions: deferred,
        })
    }

    async fn load_partition_data(
        &self,
        tenant_id: &str,
        partition_key: &str,
    ) -> Result<(), GraphError> {
        debug!(
            tenant_id = tenant_id,
            partition_key = partition_key,
            "Loading partition data"
        );

        match &self.config.s3_bucket {
            Some(bucket) => {
                let prefix = self.config.s3_prefix.as_deref().unwrap_or("partitions");
                let s3_key = format!("{}/{}/{}.parquet", prefix, tenant_id, partition_key);

                use aws_config::BehaviorVersion;
                let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
                if let Some(endpoint) = &self.config.s3_endpoint {
                    config_builder = config_builder.endpoint_url(endpoint);
                }
                if let Some(region) = &self.config.s3_region {
                    config_builder = config_builder.region(aws_config::Region::new(region.clone()));
                }
                let aws_config = config_builder.load().await;
                let s3_client = aws_sdk_s3::Client::new(&aws_config);

                match s3_client
                    .get_object()
                    .bucket(bucket)
                    .key(&s3_key)
                    .send()
                    .await
                {
                    Ok(_) => {
                        debug!(s3_key = s3_key, "Partition data loaded from S3");
                        Ok(())
                    }
                    Err(aws_sdk_s3::error::SdkError::ServiceError(e))
                        if e.err().is_no_such_key() =>
                    {
                        debug!(
                            s3_key = s3_key,
                            "Partition not found in S3, will be created on write"
                        );
                        Ok(())
                    }
                    Err(e) => Err(GraphError::S3(format!(
                        "Failed to load partition {}: {}",
                        partition_key, e
                    ))),
                }
            }
            None => {
                debug!("S3 not configured, partition loading skipped");
                Ok(())
            }
        }
    }

    pub fn enforce_cold_start_budget(
        &self,
        operation_start: std::time::Instant,
    ) -> Result<(), GraphError> {
        let elapsed_ms = operation_start.elapsed().as_millis() as u64;
        let budget_ms = self.config.cold_start.budget_ms;

        if elapsed_ms > budget_ms {
            metrics::counter!("graph_cold_start_budget_exceeded_total", 1);
            warn!(
                elapsed_ms = elapsed_ms,
                budget_ms = budget_ms,
                "Cold start budget exceeded"
            );
            return Err(GraphError::Timeout(budget_ms as i32));
        }

        Ok(())
    }

    pub fn get_cold_start_config(&self) -> &ColdStartConfig {
        &self.config.cold_start
    }

    pub fn get_warm_pool_recommendation(&self) -> WarmPoolRecommendation {
        let config = &self.config.cold_start;

        if !config.warm_pool_enabled {
            return WarmPoolRecommendation {
                recommended: false,
                min_instances: 0,
                reason: "Warm pool disabled in configuration".to_string(),
            };
        }

        WarmPoolRecommendation {
            recommended: true,
            min_instances: config.warm_pool_min_instances,
            reason: format!(
                "Maintain {} warm instances for cold start optimization",
                config.warm_pool_min_instances
            ),
        }
    }

    #[instrument(skip(self), fields(min_size = %min_community_size))]
    pub fn detect_communities(
        &self,
        ctx: TenantContext,
        min_community_size: usize,
    ) -> Result<Vec<Community>, GraphError> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            r#"
            SELECT id FROM memory_nodes
            WHERE tenant_id = ? AND deleted_at IS NULL
            "#,
        )?;

        let node_ids: Vec<String> = stmt
            .query_map(params![tenant_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        if node_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut adjacency: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for node_id in &node_ids {
            adjacency.insert(node_id.clone(), Vec::new());
        }

        let mut edge_stmt = conn.prepare(
            r#"
            SELECT source_id, target_id FROM memory_edges
            WHERE tenant_id = ? AND deleted_at IS NULL
            "#,
        )?;

        let edges: Vec<(String, String)> = edge_stmt
            .query_map(params![tenant_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        for (src, tgt) in &edges {
            if let Some(neighbors) = adjacency.get_mut(src) {
                neighbors.push(tgt.clone());
            }
            if let Some(neighbors) = adjacency.get_mut(tgt) {
                neighbors.push(src.clone());
            }
        }

        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut communities: Vec<Community> = Vec::new();

        for start_node in &node_ids {
            if visited.contains(start_node) {
                continue;
            }

            let mut component: Vec<String> = Vec::new();
            let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
            queue.push_back(start_node.clone());
            visited.insert(start_node.clone());

            while let Some(current) = queue.pop_front() {
                component.push(current.clone());
                if let Some(neighbors) = adjacency.get(&current) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }

            if component.len() >= min_community_size {
                let n = component.len();
                let internal_edges: usize = edges
                    .iter()
                    .filter(|(s, t)| component.contains(s) && component.contains(t))
                    .count();
                let max_edges = if n > 1 { n * (n - 1) / 2 } else { 1 };
                let density = internal_edges as f64 / max_edges as f64;

                communities.push(Community {
                    id: Uuid::new_v4().to_string(),
                    member_node_ids: component,
                    density,
                });
            }
        }

        debug!(
            "Detected {} communities with min size {}",
            communities.len(),
            min_community_size
        );
        Ok(communities)
    }
}

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub entity_count: usize,
    pub entity_edge_count: usize,
}

/// Implementation of the GraphStore trait for DuckDbGraphStore
#[async_trait]
impl GraphStore for DuckDbGraphStore {
    type Error = GraphError;

    #[instrument(skip(self, node), fields(node_id = %node.id))]
    async fn add_node(&self, ctx: TenantContext, node: GraphNode) -> Result<(), Self::Error> {
        let tenant_id = self.validate_tenant(&ctx)?;

        if node.tenant_id != tenant_id {
            return Err(GraphError::TenantViolation(
                "Node tenant_id does not match context".to_string(),
            ));
        }

        let conn = self.conn.lock();
        let properties_json = serde_json::to_string(&node.properties)
            .map_err(|e| GraphError::Serialization(e.to_string()))?;

        conn.execute(
            r#"
            INSERT INTO memory_nodes (id, label, properties, tenant_id)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = now()
            "#,
            params![node.id, node.label, properties_json, tenant_id],
        )?;

        debug!("Added/updated node {}", node.id);
        Ok(())
    }

    #[instrument(skip(self, edge), fields(edge_id = %edge.id))]
    async fn add_edge(&self, ctx: TenantContext, edge: GraphEdge) -> Result<(), Self::Error> {
        let tenant_id = self.validate_tenant(&ctx)?;

        if edge.tenant_id != tenant_id {
            return Err(GraphError::TenantViolation(
                "Edge tenant_id does not match context".to_string(),
            ));
        }

        let conn = self.conn.lock();

        // Verify both nodes exist (R1-C2: FK enforcement)
        if !self.node_exists(&conn, &edge.source_id, &tenant_id)? {
            return Err(GraphError::ReferentialIntegrity(format!(
                "Source node {} does not exist",
                edge.source_id
            )));
        }

        if !self.node_exists(&conn, &edge.target_id, &tenant_id)? {
            return Err(GraphError::ReferentialIntegrity(format!(
                "Target node {} does not exist",
                edge.target_id
            )));
        }

        let properties_json = serde_json::to_string(&edge.properties)
            .map_err(|e| GraphError::Serialization(e.to_string()))?;

        conn.execute(
            r#"
            INSERT INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                relation = EXCLUDED.relation,
                properties = EXCLUDED.properties
            "#,
            params![
                edge.id,
                edge.source_id,
                edge.target_id,
                edge.relation,
                properties_json,
                tenant_id
            ],
        )?;

        debug!(
            "Added/updated edge {} ({} -> {})",
            edge.id, edge.source_id, edge.target_id
        );
        Ok(())
    }

    #[instrument(skip(self), fields(node_id = %node_id))]
    async fn get_neighbors(
        &self,
        ctx: TenantContext,
        node_id: &str,
    ) -> Result<Vec<(GraphEdge, GraphNode)>, Self::Error> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            r#"
            SELECT 
                e.id as edge_id, e.source_id, e.target_id, e.relation, e.properties as edge_props,
                n.id as node_id, n.label, n.properties as node_props
            FROM memory_edges e
            JOIN memory_nodes n ON (
                CASE WHEN e.source_id = ? THEN e.target_id ELSE e.source_id END = n.id
            )
            WHERE (e.source_id = ? OR e.target_id = ?)
                AND e.tenant_id = ?
                AND e.deleted_at IS NULL
                AND n.tenant_id = ?
                AND n.deleted_at IS NULL
            "#,
        )?;

        let rows = stmt.query_map(
            params![node_id, node_id, node_id, tenant_id, tenant_id],
            |row| {
                let edge = GraphEdge {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    target_id: row.get(2)?,
                    relation: row.get(3)?,
                    properties: row
                        .get::<_, Option<String>>(4)?
                        .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    tenant_id: tenant_id.clone(),
                };
                let node = GraphNode {
                    id: row.get(5)?,
                    label: row.get(6)?,
                    properties: row
                        .get::<_, Option<String>>(7)?
                        .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    tenant_id: tenant_id.clone(),
                };
                Ok((edge, node))
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        debug!("Found {} neighbors for node {}", results.len(), node_id);
        Ok(results)
    }

    #[instrument(skip(self), fields(start = %start_id, end = %end_id))]
    async fn find_path(
        &self,
        ctx: TenantContext,
        start_id: &str,
        end_id: &str,
        max_depth: usize,
    ) -> Result<Vec<GraphEdge>, Self::Error> {
        let extended_edges = self.shortest_path(ctx, start_id, end_id, Some(max_depth))?;

        // Convert extended edges to basic edges
        Ok(extended_edges
            .into_iter()
            .map(|e| GraphEdge {
                id: e.id,
                source_id: e.source_id,
                target_id: e.target_id,
                relation: e.relation,
                properties: e.properties,
                tenant_id: e.tenant_id,
            })
            .collect())
    }

    #[instrument(skip(self), fields(query = %query, limit = %limit))]
    async fn search_nodes(
        &self,
        ctx: TenantContext,
        query: &str,
        limit: usize,
    ) -> Result<Vec<GraphNode>, Self::Error> {
        let tenant_id = self.validate_tenant(&ctx)?;
        let conn = self.conn.lock();

        // Simple text search on label and properties
        let search_pattern = format!("%{}%", query);

        let mut stmt = conn.prepare(
            r#"
            SELECT id, label, properties
            FROM memory_nodes
            WHERE tenant_id = ?
                AND deleted_at IS NULL
                AND (label ILIKE ? OR properties::TEXT ILIKE ?)
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(
            params![tenant_id, search_pattern, search_pattern, limit as i64],
            |row| {
                Ok(GraphNode {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    properties: row
                        .get::<_, Option<String>>(2)?
                        .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    tenant_id: tenant_id.clone(),
                })
            },
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        debug!("Found {} nodes matching query '{}'", results.len(), query);
        Ok(results)
    }

    #[instrument(skip(self), fields(source_memory_id = %source_memory_id))]
    async fn soft_delete_nodes_by_source_memory_id(
        &self,
        ctx: TenantContext,
        source_memory_id: &str,
    ) -> Result<usize, Self::Error> {
        DuckDbGraphStore::soft_delete_nodes_by_source_memory_id(self, ctx, source_memory_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{TenantId, UserId};

    fn test_tenant_context() -> TenantContext {
        let tenant_id = TenantId::new("test-company".to_string()).unwrap();
        let user_id = UserId::new("test-user".to_string()).unwrap();
        TenantContext::new(tenant_id, user_id)
    }

    fn create_test_store() -> DuckDbGraphStore {
        DuckDbGraphStore::new(DuckDbGraphConfig::default()).expect("Failed to create test store")
    }

    #[tokio::test]
    async fn test_add_and_get_node() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        let node = GraphNode {
            id: "node-1".to_string(),
            label: "TestNode".to_string(),
            properties: serde_json::json!({"key": "value"}),
            tenant_id: tenant_id.clone(),
        };

        store.add_node(ctx.clone(), node.clone()).await.unwrap();

        let neighbors = store.get_neighbors(ctx, "node-1").await.unwrap();
        assert!(neighbors.is_empty()); // No edges yet
    }

    #[tokio::test]
    async fn test_add_edge_validates_nodes() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Try to add edge without nodes
        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "RELATES_TO".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };

        let result = store.add_edge(ctx.clone(), edge).await;
        assert!(matches!(result, Err(GraphError::ReferentialIntegrity(_))));
    }

    #[tokio::test]
    async fn test_add_edge_with_valid_nodes() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Add nodes first
        let node1 = GraphNode {
            id: "node-1".to_string(),
            label: "Node1".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        let node2 = GraphNode {
            id: "node-2".to_string(),
            label: "Node2".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };

        store.add_node(ctx.clone(), node1).await.unwrap();
        store.add_node(ctx.clone(), node2).await.unwrap();

        // Now add edge
        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "RELATES_TO".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };

        store.add_edge(ctx.clone(), edge).await.unwrap();

        // Verify neighbors
        let neighbors = store.get_neighbors(ctx, "node-1").await.unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].1.id, "node-2");
    }

    #[tokio::test]
    async fn test_soft_delete_cascades() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Setup: nodes and edge
        let node1 = GraphNode {
            id: "node-1".to_string(),
            label: "Node1".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        let node2 = GraphNode {
            id: "node-2".to_string(),
            label: "Node2".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "RELATES_TO".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };

        store.add_node(ctx.clone(), node1).await.unwrap();
        store.add_node(ctx.clone(), node2).await.unwrap();
        store.add_edge(ctx.clone(), edge).await.unwrap();

        // Soft delete node-1
        store.soft_delete_node(ctx.clone(), "node-1").unwrap();

        // Verify edge is also soft-deleted (no neighbors visible)
        let neighbors = store.get_neighbors(ctx, "node-2").await.unwrap();
        assert!(neighbors.is_empty());
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let store = create_test_store();
        let ctx1 = TenantContext::new(
            TenantId::new("tenant-1".to_string()).unwrap(),
            UserId::new("user-1".to_string()).unwrap(),
        );
        let ctx2 = TenantContext::new(
            TenantId::new("tenant-2".to_string()).unwrap(),
            UserId::new("user-2".to_string()).unwrap(),
        );

        let node = GraphNode {
            id: "node-1".to_string(),
            label: "TenantNode".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: ctx1.tenant_id.as_str().to_string(),
        };

        store.add_node(ctx1.clone(), node).await.unwrap();

        // Tenant-2 should not see tenant-1's node
        let results = store.search_nodes(ctx2, "Tenant", 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_search_nodes() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Add multiple nodes
        for i in 1..=5 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("TestNode-{}", i),
                properties: serde_json::json!({"index": i}),
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        // Search
        let results = store.search_nodes(ctx, "TestNode", 10).await.unwrap();
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_find_path() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Create a chain: node-1 -> node-2 -> node-3
        for i in 1..=3 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("Node{}", i),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        // Add edges
        for i in 1..=2 {
            let edge = GraphEdge {
                id: format!("edge-{}", i),
                source_id: format!("node-{}", i),
                target_id: format!("node-{}", i + 1),
                relation: "NEXT".to_string(),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_edge(ctx.clone(), edge).await.unwrap();
        }

        // Find path from node-1 to node-3
        let path = store.find_path(ctx, "node-1", "node-3", 5).await.unwrap();
        assert_eq!(path.len(), 2);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        // Add nodes and edges
        for i in 1..=3 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("Node{}", i),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "RELATES".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        store.add_edge(ctx.clone(), edge).await.unwrap();

        let stats = store.get_stats(ctx).unwrap();
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 1);
    }

    #[tokio::test]
    async fn test_detect_communities_single_component() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        for i in 1..=4 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("Node{}", i),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        let edges = vec![
            ("node-1", "node-2"),
            ("node-2", "node-3"),
            ("node-3", "node-4"),
            ("node-4", "node-1"),
        ];
        for (i, (src, tgt)) in edges.iter().enumerate() {
            let edge = GraphEdge {
                id: format!("edge-{}", i),
                source_id: src.to_string(),
                target_id: tgt.to_string(),
                relation: "CONNECTS".to_string(),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_edge(ctx.clone(), edge).await.unwrap();
        }

        let communities = store.detect_communities(ctx, 2).unwrap();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].member_node_ids.len(), 4);
        assert!(communities[0].density > 0.0);
    }

    #[tokio::test]
    async fn test_detect_communities_multiple_components() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        for i in 1..=6 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("Node{}", i),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        let edge1 = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "CONNECTS".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        store.add_edge(ctx.clone(), edge1).await.unwrap();

        let edge2 = GraphEdge {
            id: "edge-2".to_string(),
            source_id: "node-4".to_string(),
            target_id: "node-5".to_string(),
            relation: "CONNECTS".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        store.add_edge(ctx.clone(), edge2).await.unwrap();

        let communities = store.detect_communities(ctx, 2).unwrap();
        assert_eq!(communities.len(), 2);
    }

    #[tokio::test]
    async fn test_detect_communities_min_size_filter() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        for i in 1..=3 {
            let node = GraphNode {
                id: format!("node-{}", i),
                label: format!("Node{}", i),
                properties: serde_json::Value::Null,
                tenant_id: tenant_id.clone(),
            };
            store.add_node(ctx.clone(), node).await.unwrap();
        }

        let edge = GraphEdge {
            id: "edge-1".to_string(),
            source_id: "node-1".to_string(),
            target_id: "node-2".to_string(),
            relation: "CONNECTS".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        store.add_edge(ctx.clone(), edge).await.unwrap();

        let communities = store.detect_communities(ctx, 3).unwrap();
        assert_eq!(communities.len(), 0);
    }

    #[tokio::test]
    async fn test_soft_delete_nodes_by_source_memory_id() {
        let store = create_test_store();
        let ctx = test_tenant_context();
        let tenant_id = ctx.tenant_id.as_str().to_string();

        let node1 = GraphNode {
            id: "entity-person".to_string(),
            label: "Person".to_string(),
            properties: serde_json::json!({"source_memory_id": "memory-123", "name": "Alice"}),
            tenant_id: tenant_id.clone(),
        };
        let node2 = GraphNode {
            id: "entity-place".to_string(),
            label: "Place".to_string(),
            properties: serde_json::json!({"source_memory_id": "memory-123", "name": "Office"}),
            tenant_id: tenant_id.clone(),
        };
        let node3 = GraphNode {
            id: "entity-other".to_string(),
            label: "Other".to_string(),
            properties: serde_json::json!({"source_memory_id": "memory-456", "name": "Unrelated"}),
            tenant_id: tenant_id.clone(),
        };

        store.add_node(ctx.clone(), node1).await.unwrap();
        store.add_node(ctx.clone(), node2).await.unwrap();
        store.add_node(ctx.clone(), node3).await.unwrap();

        let edge = GraphEdge {
            id: "edge-person-place".to_string(),
            source_id: "entity-person".to_string(),
            target_id: "entity-place".to_string(),
            relation: "WORKS_AT".to_string(),
            properties: serde_json::Value::Null,
            tenant_id: tenant_id.clone(),
        };
        store.add_edge(ctx.clone(), edge).await.unwrap();

        let deleted = store
            .soft_delete_nodes_by_source_memory_id(ctx.clone(), "memory-123")
            .unwrap();
        assert_eq!(deleted, 2);

        let results = store.search_nodes(ctx.clone(), "Other", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "entity-other");

        let neighbors = store
            .get_neighbors(ctx.clone(), "entity-other")
            .await
            .unwrap();
        assert!(neighbors.is_empty());
    }
}
