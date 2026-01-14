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
use tracing::{debug, info, instrument, warn};
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

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
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
pub struct WriteCoordinatorConfig {
    pub lock_ttl_ms: u64,
    pub max_retries: u32,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Default for WriteCoordinatorConfig {
    fn default() -> Self {
        Self {
            lock_ttl_ms: 5000,
            max_retries: 5,
            base_backoff_ms: 50,
            max_backoff_ms: 2000,
        }
    }
}

pub struct WriteCoordinator {
    redis_url: String,
    config: WriteCoordinatorConfig,
}

impl WriteCoordinator {
    pub fn new(redis_url: String, config: WriteCoordinatorConfig) -> Self {
        Self { redis_url, config }
    }

    pub async fn acquire_lock(&self, tenant_id: &str) -> Result<String, GraphError> {
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
                    return Err(GraphError::S3(format!("Redis SET failed: {}", e)));
                }
            }
        }

        Err(GraphError::Timeout(self.config.max_retries as i32))
    }

    pub async fn release_lock(&self, tenant_id: &str, lock_value: &str) -> Result<(), GraphError> {
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

        debug!("Released lock {}", lock_key);
        Ok(())
    }
}

/// Observability metrics for graph operations.
/// Uses the `metrics` crate for lightweight instrumentation.
#[derive(Clone, Debug, Default)]
pub struct GraphMetrics {
    _private: (),
}

impl GraphMetrics {
    pub fn new() -> Self {
        Self { _private: () }
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

        debug!("Schema initialized successfully");
        Ok(())
    }

    /// Run database migrations
    #[instrument(skip(self))]
    fn run_migrations(&self) -> Result<(), GraphError> {
        let conn = self.conn.lock();

        // Check current schema version
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current_version < SCHEMA_VERSION {
            info!(
                "Running migrations from version {} to {}",
                current_version, SCHEMA_VERSION
            );

            // Migration v1: Initial schema (already applied in initialize_schema)
            if current_version < 1 {
                conn.execute(
                    "INSERT INTO schema_version (version, description) VALUES (1, 'Initial schema with soft-delete support')",
                    [],
                )?;
            }

            info!("Migrations completed successfully");
        } else {
            debug!("Schema is up to date (version {})", current_version);
        }

        Ok(())
    }

    /// Validate tenant context (R1-H3)
    fn validate_tenant(&self, ctx: &TenantContext) -> Result<String, GraphError> {
        let tenant_id = ctx.tenant_id.as_str();
        if tenant_id.is_empty() {
            return Err(GraphError::InvalidTenantContext);
        }
        Ok(tenant_id.to_string())
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
        let conn = self.conn.lock();

        let export_sql = format!(
            r#"
            COPY (
                SELECT 'node' as record_type, id, label, properties, memory_id, 
                       CAST(created_at AS VARCHAR) as created_at, 
                       CAST(updated_at AS VARCHAR) as updated_at,
                       NULL as source_id, NULL as target_id, NULL as relation, NULL as weight
                FROM memory_nodes WHERE tenant_id = '{tenant_id}' AND deleted_at IS NULL
                UNION ALL
                SELECT 'edge' as record_type, id, NULL as label, properties, NULL as memory_id,
                       CAST(created_at AS VARCHAR) as created_at, NULL as updated_at,
                       source_id, target_id, relation, CAST(weight AS VARCHAR)
                FROM memory_edges WHERE tenant_id = '{tenant_id}' AND deleted_at IS NULL
            ) TO '/dev/stdout' (FORMAT PARQUET)
            "#,
            tenant_id = tenant_id
        );

        let temp_path = format!("/tmp/graph_export_{}.parquet", Uuid::new_v4());
        let export_sql = export_sql.replace("/dev/stdout", &temp_path);
        conn.execute_batch(&export_sql)?;

        let data = std::fs::read(&temp_path)?;
        std::fs::remove_file(&temp_path).ok();

        Ok(data)
    }

    fn import_from_parquet(&self, tenant_id: &str, data: &[u8]) -> Result<(), GraphError> {
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
