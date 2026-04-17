//! Cross-layer reconciliation for detecting orphaned records between storage backends.
//!
//! Provides sampling-based reconciliation between PostgreSQL, Qdrant, and DuckDB
//! to detect data inconsistencies (orphaned records, missing vectors, stale soft-deletes).

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during reconciliation.
#[derive(Error, Debug)]
pub enum ReconciliationError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("Graph store error: {0}")]
    GraphStore(String),

    #[error("Invalid sample rate: {0} (must be 1..=100)")]
    InvalidSampleRate(u8),
}

// ---------------------------------------------------------------------------
// Trait for vector store existence checks
// ---------------------------------------------------------------------------

/// Abstraction over the vector store so reconciliation does not depend on a
/// concrete Qdrant client. Implementors check whether a given point exists
/// in a named collection.
#[async_trait]
pub trait VectorStoreChecker: Send + Sync {
    /// Returns `true` if a point with the given ID exists in `collection`.
    async fn point_exists(
        &self,
        collection: &str,
        point_id: &str,
    ) -> Result<bool, ReconciliationError>;

    /// List point IDs in a collection, optionally filtered by tenant.
    async fn list_point_ids(
        &self,
        collection: &str,
        tenant_filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<String>, ReconciliationError>;
}

// ---------------------------------------------------------------------------
// Trait for graph store existence checks
// ---------------------------------------------------------------------------

/// Abstraction over the graph store (DuckDB) for reconciliation.
#[async_trait]
pub trait GraphStoreChecker: Send + Sync {
    /// Returns `true` if a node with the given ID exists (non-deleted).
    async fn node_exists(
        &self,
        node_id: &str,
        tenant_id: &str,
    ) -> Result<bool, ReconciliationError>;

    /// Find graph nodes whose `deleted_at` is older than `retention_days` days ago.
    async fn find_stale_soft_deleted_nodes(
        &self,
        retention_days: u32,
    ) -> Result<Vec<OrphanRecord>, ReconciliationError>;
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of a reconciliation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReport {
    /// Unique identifier for this reconciliation run.
    pub run_id: String,
    /// ISO-8601 timestamp when reconciliation started.
    pub started_at: String,
    /// ISO-8601 timestamp when reconciliation completed.
    pub completed_at: String,
    /// The sample rate used (1..=100).
    pub sample_rate_pct: u8,
    /// Total records sampled from the source backend.
    pub records_sampled: u64,
    /// Orphaned records detected.
    pub orphans_found: Vec<OrphanRecord>,
    /// Human-readable status: "completed", "partial", "error".
    pub status: String,
}

/// A single orphaned record detected during reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanRecord {
    /// The entity's primary ID.
    pub entity_id: String,
    /// The type of entity (e.g. "memory", "graph_node").
    pub entity_type: String,
    /// Backend where the record *does* exist.
    pub present_in: String,
    /// Backend where the record *should* exist but does not.
    pub missing_from: String,
    /// Tenant that owns the record.
    pub tenant_id: String,
    /// ISO-8601 timestamp when the orphan was detected.
    pub detected_at: String,
}

// ---------------------------------------------------------------------------
// Reconciliation engine
// ---------------------------------------------------------------------------

/// Drives cross-layer reconciliation between PostgreSQL and other backends.
pub struct Reconciler {
    pool: PgPool,
    vector_checker: Option<Box<dyn VectorStoreChecker>>,
    graph_checker: Option<Box<dyn GraphStoreChecker>>,
}

impl Reconciler {
    /// Create a new reconciler.
    ///
    /// Pass `None` for checkers that are unavailable; those reconciliation
    /// steps will be skipped with a warning.
    pub fn new(
        pool: PgPool,
        vector_checker: Option<Box<dyn VectorStoreChecker>>,
        graph_checker: Option<Box<dyn GraphStoreChecker>>,
    ) -> Self {
        Self {
            pool,
            vector_checker,
            graph_checker,
        }
    }

    /// Reconcile PostgreSQL memory entries against Qdrant vectors.
    ///
    /// Samples `sample_pct`% of memory entries for the given tenant and checks
    /// whether each has a corresponding vector in Qdrant.
    ///
    /// # Errors
    ///
    /// Returns `ReconciliationError::InvalidSampleRate` if `sample_pct` is 0 or > 100.
    pub async fn reconcile_pg_qdrant_sample(
        &self,
        tenant_id: &str,
        sample_pct: u8,
        collection: &str,
    ) -> Result<ReconciliationReport, ReconciliationError> {
        if sample_pct == 0 || sample_pct > 100 {
            return Err(ReconciliationError::InvalidSampleRate(sample_pct));
        }

        let run_id = Uuid::new_v4().to_string();
        let started_at = Utc::now().to_rfc3339();

        let checker = match &self.vector_checker {
            Some(c) => c,
            None => {
                warn!("No vector store checker configured; skipping PG-Qdrant reconciliation");
                return Ok(ReconciliationReport {
                    run_id,
                    started_at: started_at.clone(),
                    completed_at: Utc::now().to_rfc3339(),
                    sample_rate_pct: sample_pct,
                    records_sampled: 0,
                    orphans_found: vec![],
                    status: "skipped".to_string(),
                });
            }
        };

        // Sample memory IDs from PostgreSQL using Bernoulli sampling.
        let sample_f64 = f64::from(sample_pct);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM memory_entries TABLESAMPLE BERNOULLI($1) WHERE tenant_id = $2",
        )
        .bind(sample_f64)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        let records_sampled = rows.len() as u64;
        info!(
            run_id = %run_id,
            tenant_id = %tenant_id,
            sampled = records_sampled,
            "PG-Qdrant reconciliation: sampled memory entries"
        );

        let mut orphans = Vec::new();
        let now = Utc::now().to_rfc3339();

        for (memory_id,) in &rows {
            match checker.point_exists(collection, memory_id).await {
                Ok(true) => {
                    debug!(memory_id = %memory_id, "Vector exists");
                }
                Ok(false) => {
                    orphans.push(OrphanRecord {
                        entity_id: memory_id.clone(),
                        entity_type: "memory".to_string(),
                        present_in: "postgres".to_string(),
                        missing_from: "qdrant".to_string(),
                        tenant_id: tenant_id.to_string(),
                        detected_at: now.clone(),
                    });
                }
                Err(e) => {
                    warn!(memory_id = %memory_id, error = %e, "Failed to check vector existence");
                }
            }
        }

        let completed_at = Utc::now().to_rfc3339();
        info!(
            run_id = %run_id,
            orphans = orphans.len(),
            "PG-Qdrant reconciliation complete"
        );

        Ok(ReconciliationReport {
            run_id,
            started_at,
            completed_at,
            sample_rate_pct: sample_pct,
            records_sampled,
            orphans_found: orphans,
            status: "completed".to_string(),
        })
    }

    /// Reconcile PostgreSQL memory entries against DuckDB graph nodes.
    ///
    /// Samples `sample_pct`% of memory entries for the given tenant and checks
    /// whether each has a corresponding graph node in DuckDB.
    pub async fn reconcile_pg_graph_sample(
        &self,
        tenant_id: &str,
        sample_pct: u8,
    ) -> Result<ReconciliationReport, ReconciliationError> {
        if sample_pct == 0 || sample_pct > 100 {
            return Err(ReconciliationError::InvalidSampleRate(sample_pct));
        }

        let run_id = Uuid::new_v4().to_string();
        let started_at = Utc::now().to_rfc3339();

        let checker = match &self.graph_checker {
            Some(c) => c,
            None => {
                warn!("No graph store checker configured; skipping PG-Graph reconciliation");
                return Ok(ReconciliationReport {
                    run_id,
                    started_at: started_at.clone(),
                    completed_at: Utc::now().to_rfc3339(),
                    sample_rate_pct: sample_pct,
                    records_sampled: 0,
                    orphans_found: vec![],
                    status: "skipped".to_string(),
                });
            }
        };

        let sample_f64 = f64::from(sample_pct);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM memory_entries TABLESAMPLE BERNOULLI($1) WHERE tenant_id = $2",
        )
        .bind(sample_f64)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        let records_sampled = rows.len() as u64;
        info!(
            run_id = %run_id,
            tenant_id = %tenant_id,
            sampled = records_sampled,
            "PG-Graph reconciliation: sampled memory entries"
        );

        let mut orphans = Vec::new();
        let now = Utc::now().to_rfc3339();

        for (memory_id,) in &rows {
            match checker.node_exists(memory_id, tenant_id).await {
                Ok(true) => {
                    debug!(memory_id = %memory_id, "Graph node exists");
                }
                Ok(false) => {
                    orphans.push(OrphanRecord {
                        entity_id: memory_id.clone(),
                        entity_type: "memory".to_string(),
                        present_in: "postgres".to_string(),
                        missing_from: "duckdb".to_string(),
                        tenant_id: tenant_id.to_string(),
                        detected_at: now.clone(),
                    });
                }
                Err(e) => {
                    warn!(memory_id = %memory_id, error = %e, "Failed to check graph node existence");
                }
            }
        }

        let completed_at = Utc::now().to_rfc3339();
        info!(
            run_id = %run_id,
            orphans = orphans.len(),
            "PG-Graph reconciliation complete"
        );

        Ok(ReconciliationReport {
            run_id,
            started_at,
            completed_at,
            sample_rate_pct: sample_pct,
            records_sampled,
            orphans_found: orphans,
            status: "completed".to_string(),
        })
    }

    /// Find graph nodes that were soft-deleted longer than `retention_days` ago.
    ///
    /// Delegates to the `GraphStoreChecker` to query DuckDB.
    pub async fn find_stale_soft_deletes(
        &self,
        retention_days: u32,
    ) -> Result<Vec<OrphanRecord>, ReconciliationError> {
        let checker = match &self.graph_checker {
            Some(c) => c,
            None => {
                warn!("No graph store checker configured; skipping stale soft-delete scan");
                return Ok(vec![]);
            }
        };

        let stale = checker
            .find_stale_soft_deleted_nodes(retention_days)
            .await?;
        info!(
            stale_count = stale.len(),
            retention_days, "Found stale soft-deleted nodes"
        );
        Ok(stale)
    }

    /// Find promotion records that reference non-existent source memory entries.
    ///
    /// Queries `governance_events` for promotion-related events whose referenced
    /// source memory IDs no longer exist in `memory_entries`.
    pub async fn find_orphaned_promotions(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<OrphanRecord>, ReconciliationError> {
        // Query governance events of type 'knowledge_promotion_requested' whose
        // payload references a source memory that no longer exists.
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT ge.id::text, ge.payload->>'source_memory_id' AS source_id \
             FROM governance_events ge \
             WHERE ge.tenant_id = $1 \
               AND ge.event_type = 'knowledge_promotion_requested' \
               AND NOT EXISTS ( \
                   SELECT 1 FROM memory_entries me WHERE me.id = ge.payload->>'source_memory_id' \
               )",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now().to_rfc3339();
        let orphans: Vec<OrphanRecord> = rows
            .into_iter()
            .map(|(event_id, source_id)| OrphanRecord {
                entity_id: event_id,
                entity_type: "promotion_event".to_string(),
                present_in: "postgres".to_string(),
                missing_from: format!("memory_entries(source={})", source_id),
                tenant_id: tenant_id.to_string(),
                detected_at: now.clone(),
            })
            .collect();

        info!(
            tenant_id = %tenant_id,
            orphaned = orphans.len(),
            "Orphaned promotion scan complete"
        );

        Ok(orphans)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_record_serializes_to_camel_case() {
        let orphan = OrphanRecord {
            entity_id: "mem-001".to_string(),
            entity_type: "memory".to_string(),
            present_in: "postgres".to_string(),
            missing_from: "qdrant".to_string(),
            tenant_id: "tenant-1".to_string(),
            detected_at: "2026-04-12T00:00:00Z".to_string(),
        };

        let json = serde_json::to_value(&orphan).expect("serialize");
        assert_eq!(json["entityId"], "mem-001");
        assert_eq!(json["entityType"], "memory");
        assert_eq!(json["presentIn"], "postgres");
        assert_eq!(json["missingFrom"], "qdrant");
        assert_eq!(json["tenantId"], "tenant-1");
        assert_eq!(json["detectedAt"], "2026-04-12T00:00:00Z");
    }

    #[test]
    fn orphan_record_deserializes_from_camel_case() {
        let json = r#"{
            "entityId": "mem-002",
            "entityType": "graph_node",
            "presentIn": "duckdb",
            "missingFrom": "postgres",
            "tenantId": "tenant-2",
            "detectedAt": "2026-04-12T01:00:00Z"
        }"#;

        let orphan: OrphanRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(orphan.entity_id, "mem-002");
        assert_eq!(orphan.entity_type, "graph_node");
        assert_eq!(orphan.present_in, "duckdb");
        assert_eq!(orphan.missing_from, "postgres");
    }

    #[test]
    fn reconciliation_report_structure() {
        let report = ReconciliationReport {
            run_id: "run-123".to_string(),
            started_at: "2026-04-12T00:00:00Z".to_string(),
            completed_at: "2026-04-12T00:01:00Z".to_string(),
            sample_rate_pct: 10,
            records_sampled: 42,
            orphans_found: vec![OrphanRecord {
                entity_id: "e1".to_string(),
                entity_type: "memory".to_string(),
                present_in: "postgres".to_string(),
                missing_from: "qdrant".to_string(),
                tenant_id: "t1".to_string(),
                detected_at: "2026-04-12T00:00:30Z".to_string(),
            }],
            status: "completed".to_string(),
        };

        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["runId"], "run-123");
        assert_eq!(json["sampleRatePct"], 10);
        assert_eq!(json["recordsSampled"], 42);
        assert_eq!(json["orphansFound"].as_array().unwrap().len(), 1);
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn reconciliation_report_empty_orphans() {
        let report = ReconciliationReport {
            run_id: "run-456".to_string(),
            started_at: "2026-04-12T00:00:00Z".to_string(),
            completed_at: "2026-04-12T00:00:05Z".to_string(),
            sample_rate_pct: 5,
            records_sampled: 100,
            orphans_found: vec![],
            status: "completed".to_string(),
        };

        let json = serde_json::to_value(&report).expect("serialize");
        assert!(json["orphansFound"].as_array().unwrap().is_empty());
    }

    #[test]
    fn reconciliation_report_roundtrip() {
        let report = ReconciliationReport {
            run_id: "rt-1".to_string(),
            started_at: "2026-04-12T00:00:00Z".to_string(),
            completed_at: "2026-04-12T00:02:00Z".to_string(),
            sample_rate_pct: 50,
            records_sampled: 500,
            orphans_found: vec![],
            status: "completed".to_string(),
        };

        let serialized = serde_json::to_string(&report).expect("to_string");
        let deserialized: ReconciliationReport =
            serde_json::from_str(&serialized).expect("from_str");
        assert_eq!(deserialized.run_id, "rt-1");
        assert_eq!(deserialized.sample_rate_pct, 50);
        assert_eq!(deserialized.records_sampled, 500);
    }
}
