//! Retention policy enforcement — hard-delete expired records across storage backends.
//!
//! Provides configurable retention periods for audit logs, governance events,
//! soft-deleted graph nodes, drift results, and stale promotion requests.

use serde::Serialize;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during retention enforcement.
#[derive(Error, Debug)]
pub enum RetentionError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Graph store error: {0}")]
    GraphStore(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Retention policy configuration with per-entity-type expiry durations.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// Days to keep GDPR audit logs after S3 archival (default: 90).
    pub audit_log_days: u32,
    /// Days to keep governance events (default: 180).
    pub governance_event_days: u32,
    /// Days to keep soft-deleted graph nodes before hard-delete (default: 7).
    pub soft_delete_days: u32,
    /// Days to keep drift analysis results (default: 30).
    pub drift_result_days: u32,
    /// Days to keep rejected/abandoned promotion requests (default: 30).
    pub promotion_request_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            audit_log_days: 90,
            governance_event_days: 180,
            soft_delete_days: 7,
            drift_result_days: 30,
            promotion_request_days: 30,
        }
    }
}

impl RetentionConfig {
    /// Load configuration from environment variables, falling back to defaults.
    ///
    /// Recognised variables:
    /// - `AETERNA_RETENTION_AUDIT_DAYS`
    /// - `AETERNA_RETENTION_GOVERNANCE_DAYS`
    /// - `AETERNA_RETENTION_SOFT_DELETE_DAYS`
    /// - `AETERNA_RETENTION_DRIFT_DAYS`
    /// - `AETERNA_RETENTION_PROMOTION_DAYS`
    pub fn from_env() -> Self {
        Self {
            audit_log_days: env_or("AETERNA_RETENTION_AUDIT_DAYS", 90),
            governance_event_days: env_or("AETERNA_RETENTION_GOVERNANCE_DAYS", 180),
            soft_delete_days: env_or("AETERNA_RETENTION_SOFT_DELETE_DAYS", 7),
            drift_result_days: env_or("AETERNA_RETENTION_DRIFT_DAYS", 30),
            promotion_request_days: env_or("AETERNA_RETENTION_PROMOTION_DAYS", 30),
        }
    }
}

/// Parse an environment variable as `u32`, returning `default` on absence or
/// parse failure.
fn env_or(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Summary of a retention enforcement run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionReport {
    /// Number of GDPR audit log rows purged.
    pub audit_logs_purged: u64,
    /// Number of governance event rows purged.
    pub governance_events_purged: u64,
    /// Number of soft-deleted graph nodes hard-deleted.
    pub graph_nodes_purged: u64,
    /// Number of drift result rows purged.
    pub drift_results_purged: u64,
    /// Number of stale promotion records purged.
    pub promotions_purged: u64,
    /// Total records purged across all categories.
    pub total_purged: u64,
}

// ---------------------------------------------------------------------------
// Graph node purger trait
// ---------------------------------------------------------------------------

/// Abstraction for hard-deleting soft-deleted graph nodes.
///
/// Implementations should remove nodes (and their edges) from DuckDB where
/// `deleted_at` is older than the configured retention period.
#[async_trait::async_trait]
pub trait GraphNodePurger: Send + Sync {
    /// Hard-delete graph nodes soft-deleted more than `retention_days` ago.
    /// Returns the number of nodes removed.
    async fn purge_soft_deleted_nodes(&self, retention_days: u32) -> Result<u64, RetentionError>;
}

// ---------------------------------------------------------------------------
// Retention enforcer
// ---------------------------------------------------------------------------

/// Drives retention policy enforcement across all storage backends.
pub struct RetentionEnforcer {
    pool: PgPool,
    config: RetentionConfig,
    graph_purger: Option<Box<dyn GraphNodePurger>>,
}

impl RetentionEnforcer {
    /// Create a new enforcer.
    pub fn new(
        pool: PgPool,
        config: RetentionConfig,
        graph_purger: Option<Box<dyn GraphNodePurger>>,
    ) -> Self {
        Self {
            pool,
            config,
            graph_purger,
        }
    }

    /// Run all retention purge operations and return a combined report.
    pub async fn enforce_all(&self) -> Result<RetentionReport, RetentionError> {
        let audit = self.purge_audit_logs().await?;
        let governance = self.purge_governance_events().await?;
        let graph = self.purge_soft_deleted_graph_nodes().await?;
        let drift = self.purge_drift_results().await?;
        let promotions = self.purge_stale_promotions().await?;

        let total = audit + governance + graph + drift + promotions;

        let report = RetentionReport {
            audit_logs_purged: audit,
            governance_events_purged: governance,
            graph_nodes_purged: graph,
            drift_results_purged: drift,
            promotions_purged: promotions,
            total_purged: total,
        };

        info!(
            audit = audit,
            governance = governance,
            graph = graph,
            drift = drift,
            promotions = promotions,
            total = total,
            "Retention enforcement complete"
        );

        Ok(report)
    }

    /// Purge GDPR audit logs older than the configured retention period.
    ///
    /// Targets the `gdpr_audit_logs` table.
    pub async fn purge_audit_logs(&self) -> Result<u64, RetentionError> {
        let days = self.config.audit_log_days;
        let interval = format!("{days} days");

        let result =
            sqlx::query("DELETE FROM gdpr_audit_logs WHERE created_at < NOW() - $1::interval")
                .bind(&interval)
                .execute(&self.pool)
                .await?;

        let count = result.rows_affected();
        info!(purged = count, days = days, "Purged GDPR audit logs");
        Ok(count)
    }

    /// Purge governance events older than the configured retention period.
    ///
    /// Targets the `governance_events` table.
    pub async fn purge_governance_events(&self) -> Result<u64, RetentionError> {
        let days = self.config.governance_event_days;
        let interval = format!("{days} days");

        let result =
            sqlx::query("DELETE FROM governance_events WHERE timestamp < NOW() - $1::interval")
                .bind(&interval)
                .execute(&self.pool)
                .await?;

        let count = result.rows_affected();
        info!(purged = count, days = days, "Purged governance events");
        Ok(count)
    }

    /// Hard-delete soft-deleted graph nodes past the retention window.
    ///
    /// Delegates to the `GraphNodePurger` trait implementation.
    pub async fn purge_soft_deleted_graph_nodes(&self) -> Result<u64, RetentionError> {
        let purger = match &self.graph_purger {
            Some(p) => p,
            None => {
                warn!("No graph node purger configured; skipping graph retention");
                return Ok(0);
            }
        };

        let count = purger
            .purge_soft_deleted_nodes(self.config.soft_delete_days)
            .await?;
        info!(
            purged = count,
            days = self.config.soft_delete_days,
            "Purged soft-deleted graph nodes"
        );
        Ok(count)
    }

    /// Purge drift analysis results older than the configured retention period.
    ///
    /// Targets the `drift_results` table.
    pub async fn purge_drift_results(&self) -> Result<u64, RetentionError> {
        let days = self.config.drift_result_days;
        let interval = format!("{days} days");

        let result =
            sqlx::query("DELETE FROM drift_results WHERE created_at < NOW() - $1::interval")
                .bind(&interval)
                .execute(&self.pool)
                .await?;

        let count = result.rows_affected();
        info!(purged = count, days = days, "Purged drift results");
        Ok(count)
    }

    /// Purge rejected/abandoned promotion governance events older than the
    /// configured retention period.
    ///
    /// Targets promotion-related events in `governance_events` that have been
    /// rejected or remain unprocessed past the threshold.
    pub async fn purge_stale_promotions(&self) -> Result<u64, RetentionError> {
        let days = self.config.promotion_request_days;
        let interval = format!("{days} days");

        let result = sqlx::query(
            "DELETE FROM governance_events \
             WHERE event_type = 'knowledge_promotion_requested' \
               AND timestamp < NOW() - $1::interval \
               AND (status = 'rejected' OR status = 'abandoned' OR status IS NULL)",
        )
        .bind(&interval)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        info!(
            purged = count,
            days = days,
            "Purged stale promotion requests"
        );
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_retention_config_values() {
        let config = RetentionConfig::default();
        assert_eq!(config.audit_log_days, 90);
        assert_eq!(config.governance_event_days, 180);
        assert_eq!(config.soft_delete_days, 7);
        assert_eq!(config.drift_result_days, 30);
        assert_eq!(config.promotion_request_days, 30);
    }

    #[test]
    fn env_or_returns_default_on_missing_key() {
        // Use a key that is extremely unlikely to be set in any environment.
        assert_eq!(env_or("AETERNA_TEST_XYZZY_NONEXISTENT_KEY_29384", 42), 42);
    }

    #[test]
    fn env_or_parses_valid_value() {
        // SAFETY: test-only env manipulation with a unique key.
        unsafe {
            std::env::set_var("AETERNA_TEST_RET_VALID_8271", "99");
        }
        assert_eq!(env_or("AETERNA_TEST_RET_VALID_8271", 42), 99);
        unsafe {
            std::env::remove_var("AETERNA_TEST_RET_VALID_8271");
        }
    }

    #[test]
    fn env_or_falls_back_on_invalid_value() {
        // SAFETY: test-only env manipulation with a unique key.
        unsafe {
            std::env::set_var("AETERNA_TEST_RET_INVALID_8272", "not-a-number");
        }
        assert_eq!(env_or("AETERNA_TEST_RET_INVALID_8272", 77), 77);
        unsafe {
            std::env::remove_var("AETERNA_TEST_RET_INVALID_8272");
        }
    }

    #[test]
    fn env_or_falls_back_on_negative_value() {
        // SAFETY: test-only env manipulation with a unique key.
        unsafe {
            std::env::set_var("AETERNA_TEST_RET_NEG_8273", "-5");
        }
        // Negative values don't parse as u32, so default is used.
        assert_eq!(env_or("AETERNA_TEST_RET_NEG_8273", 30), 30);
        unsafe {
            std::env::remove_var("AETERNA_TEST_RET_NEG_8273");
        }
    }

    #[test]
    fn retention_report_serializes_to_camel_case() {
        let report = RetentionReport {
            audit_logs_purged: 10,
            governance_events_purged: 20,
            graph_nodes_purged: 5,
            drift_results_purged: 3,
            promotions_purged: 2,
            total_purged: 40,
        };

        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["auditLogsPurged"], 10);
        assert_eq!(json["governanceEventsPurged"], 20);
        assert_eq!(json["graphNodesPurged"], 5);
        assert_eq!(json["driftResultsPurged"], 3);
        assert_eq!(json["promotionsPurged"], 2);
        assert_eq!(json["totalPurged"], 40);
    }

    #[test]
    fn retention_report_zero_values() {
        let report = RetentionReport {
            audit_logs_purged: 0,
            governance_events_purged: 0,
            graph_nodes_purged: 0,
            drift_results_purged: 0,
            promotions_purged: 0,
            total_purged: 0,
        };

        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["totalPurged"], 0);
    }
}
