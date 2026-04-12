//! Per-tenant storage quota enforcement with in-memory caching.
//!
//! Provides a `QuotaEnforcer` that checks whether a tenant can write new
//! entities based on configurable hard/soft limits, with a `DashMap`-backed
//! cache to avoid hitting the database on every write.

use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Serialize;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during quota enforcement.
#[derive(Error, Debug)]
pub enum QuotaError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Unknown entity type: {0}")]
    UnknownEntityType(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Cached storage usage counts for a single tenant.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantStorageUsage {
    /// The tenant these counts belong to.
    pub tenant_id: String,
    /// Number of memory entries.
    pub memory_count: u64,
    /// Number of knowledge entries.
    pub knowledge_count: u64,
    /// Number of graph nodes.
    pub graph_node_count: u64,
    /// Sum of all entity counts.
    pub total_count: u64,
}

/// Quota limits for a tenant. `None` means unlimited.
#[derive(Debug, Clone)]
pub struct TenantQuota {
    /// Maximum memory entries allowed.
    pub memory_max: Option<u64>,
    /// Maximum knowledge entries allowed.
    pub knowledge_max: Option<u64>,
    /// Maximum total entities allowed (across all types).
    pub total_max: Option<u64>,
}

/// Result of a quota check before a write operation.
#[derive(Debug)]
pub enum QuotaCheckResult {
    /// Write is allowed.
    Ok,
    /// Soft limit warning — usage is above 80% but below hard cap.
    SoftLimitWarning {
        /// Current usage as a percentage of the limit.
        usage_pct: f64,
        /// Which entity type triggered the warning.
        entity_type: String,
    },
    /// Hard limit exceeded — the write must be rejected.
    HardLimitExceeded {
        /// Which entity type is over the limit.
        entity_type: String,
        /// Current count.
        current: u64,
        /// Configured maximum.
        max: u64,
    },
}

// ---------------------------------------------------------------------------
// QuotaEnforcer
// ---------------------------------------------------------------------------

/// Soft-limit threshold expressed as a fraction of the hard limit (80%).
const SOFT_LIMIT_THRESHOLD: f64 = 0.8;

/// Enforces per-tenant storage quotas with a time-based cache.
pub struct QuotaEnforcer {
    cache: DashMap<String, (TenantStorageUsage, Instant)>,
    cache_ttl: Duration,
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaEnforcer {
    /// Create a new enforcer with a 5-minute cache TTL.
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Create a new enforcer with a custom cache TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            cache_ttl: ttl,
        }
    }

    /// Check if a tenant can write a new entity of `entity_type`.
    ///
    /// Returns `QuotaCheckResult::Ok` when usage is well under limits,
    /// `SoftLimitWarning` when above 80%, or `HardLimitExceeded` when at cap.
    pub async fn check_write_allowed(
        &self,
        pool: &PgPool,
        tenant_id: &str,
        entity_type: &str,
        quota: &TenantQuota,
    ) -> Result<QuotaCheckResult, QuotaError> {
        let usage = self.get_usage(pool, tenant_id).await?;

        // Determine the relevant count and limit for this entity type.
        let (current, limit) = match entity_type {
            "memory" => (usage.memory_count, quota.memory_max),
            "knowledge" => (usage.knowledge_count, quota.knowledge_max),
            _ => {
                return Err(QuotaError::UnknownEntityType(entity_type.to_string()));
            }
        };

        // Check per-type limit.
        if let Some(max) = limit {
            if current >= max {
                return Ok(QuotaCheckResult::HardLimitExceeded {
                    entity_type: entity_type.to_string(),
                    current,
                    max,
                });
            }
            let pct = current as f64 / max as f64;
            if pct >= SOFT_LIMIT_THRESHOLD {
                return Ok(QuotaCheckResult::SoftLimitWarning {
                    usage_pct: pct * 100.0,
                    entity_type: entity_type.to_string(),
                });
            }
        }

        // Check total limit.
        if let Some(total_max) = quota.total_max {
            if usage.total_count >= total_max {
                return Ok(QuotaCheckResult::HardLimitExceeded {
                    entity_type: "total".to_string(),
                    current: usage.total_count,
                    max: total_max,
                });
            }
            let pct = usage.total_count as f64 / total_max as f64;
            if pct >= SOFT_LIMIT_THRESHOLD {
                return Ok(QuotaCheckResult::SoftLimitWarning {
                    usage_pct: pct * 100.0,
                    entity_type: "total".to_string(),
                });
            }
        }

        Ok(QuotaCheckResult::Ok)
    }

    /// Get current storage usage for a tenant (from cache or fresh query).
    pub async fn get_usage(
        &self,
        pool: &PgPool,
        tenant_id: &str,
    ) -> Result<TenantStorageUsage, QuotaError> {
        // Check cache first.
        if let Some(entry) = self.cache.get(tenant_id) {
            let (usage, cached_at) = entry.value();
            if cached_at.elapsed() < self.cache_ttl {
                debug!(tenant_id = %tenant_id, "Returning cached usage");
                return Ok(usage.clone());
            }
        }

        // Cache miss or expired — query the database.
        let usage = self.query_usage(pool, tenant_id).await?;

        self.cache
            .insert(tenant_id.to_string(), (usage.clone(), Instant::now()));

        info!(
            tenant_id = %tenant_id,
            memory = usage.memory_count,
            knowledge = usage.knowledge_count,
            graph = usage.graph_node_count,
            total = usage.total_count,
            "Refreshed tenant storage usage"
        );

        Ok(usage)
    }

    /// Invalidate the cached usage for a tenant (e.g. after a write or delete).
    pub fn invalidate(&self, tenant_id: &str) {
        self.cache.remove(tenant_id);
        debug!(tenant_id = %tenant_id, "Invalidated quota cache");
    }

    /// Query actual usage counts from PostgreSQL.
    async fn query_usage(
        &self,
        pool: &PgPool,
        tenant_id: &str,
    ) -> Result<TenantStorageUsage, QuotaError> {
        let memory_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM memory_entries WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;

        // Knowledge entries may not exist yet; default to 0 on table-missing errors.
        let knowledge_count: i64 = match sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM knowledge_entries WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        {
            Ok((count,)) => count,
            Err(e) => {
                warn!(
                    tenant_id = %tenant_id,
                    error = %e,
                    "Failed to query knowledge_entries (table may not exist); defaulting to 0"
                );
                0
            }
        };

        // Graph nodes live in DuckDB; we approximate via PG-side metadata if available.
        let graph_node_count: i64 = match sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM graph_node_metadata WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        {
            Ok((count,)) => count,
            Err(e) => {
                warn!(
                    tenant_id = %tenant_id,
                    error = %e,
                    "Failed to query graph_node_metadata (table may not exist); defaulting to 0"
                );
                0
            }
        };

        let mem = memory_count.0 as u64;
        let know = knowledge_count as u64;
        let graph = graph_node_count as u64;

        Ok(TenantStorageUsage {
            tenant_id: tenant_id.to_string(),
            memory_count: mem,
            knowledge_count: know,
            graph_node_count: graph,
            total_count: mem + know + graph,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_storage_usage_serializes_to_camel_case() {
        let usage = TenantStorageUsage {
            tenant_id: "t-1".to_string(),
            memory_count: 100,
            knowledge_count: 50,
            graph_node_count: 25,
            total_count: 175,
        };

        let json = serde_json::to_value(&usage).expect("serialize");
        assert_eq!(json["tenantId"], "t-1");
        assert_eq!(json["memoryCount"], 100);
        assert_eq!(json["knowledgeCount"], 50);
        assert_eq!(json["graphNodeCount"], 25);
        assert_eq!(json["totalCount"], 175);
    }

    #[test]
    fn tenant_storage_usage_zero_values() {
        let usage = TenantStorageUsage {
            tenant_id: "t-empty".to_string(),
            memory_count: 0,
            knowledge_count: 0,
            graph_node_count: 0,
            total_count: 0,
        };

        let json = serde_json::to_value(&usage).expect("serialize");
        assert_eq!(json["totalCount"], 0);
    }

    #[test]
    fn quota_check_result_ok_variant() {
        let result = QuotaCheckResult::Ok;
        assert!(matches!(result, QuotaCheckResult::Ok));
    }

    #[test]
    fn quota_check_result_soft_warning_variant() {
        let result = QuotaCheckResult::SoftLimitWarning {
            usage_pct: 85.0,
            entity_type: "memory".to_string(),
        };
        match result {
            QuotaCheckResult::SoftLimitWarning {
                usage_pct,
                entity_type,
            } => {
                assert!((usage_pct - 85.0).abs() < f64::EPSILON);
                assert_eq!(entity_type, "memory");
            }
            _ => panic!("Expected SoftLimitWarning"),
        }
    }

    #[test]
    fn quota_check_result_hard_exceeded_variant() {
        let result = QuotaCheckResult::HardLimitExceeded {
            entity_type: "knowledge".to_string(),
            current: 1000,
            max: 1000,
        };
        match result {
            QuotaCheckResult::HardLimitExceeded {
                entity_type,
                current,
                max,
            } => {
                assert_eq!(entity_type, "knowledge");
                assert_eq!(current, 1000);
                assert_eq!(max, 1000);
            }
            _ => panic!("Expected HardLimitExceeded"),
        }
    }

    #[test]
    fn cache_expiry_logic() {
        let enforcer = QuotaEnforcer::with_ttl(Duration::from_millis(50));

        let usage = TenantStorageUsage {
            tenant_id: "t-cache".to_string(),
            memory_count: 10,
            knowledge_count: 5,
            graph_node_count: 2,
            total_count: 17,
        };

        // Insert into cache.
        enforcer
            .cache
            .insert("t-cache".to_string(), (usage.clone(), Instant::now()));

        // Should be present.
        assert!(enforcer.cache.contains_key("t-cache"));

        // After TTL, the entry should be considered stale.
        std::thread::sleep(Duration::from_millis(60));

        let entry = enforcer.cache.get("t-cache").unwrap();
        let (_, cached_at) = entry.value();
        assert!(cached_at.elapsed() >= enforcer.cache_ttl);
    }

    #[test]
    fn invalidate_removes_entry() {
        let enforcer = QuotaEnforcer::new();

        let usage = TenantStorageUsage {
            tenant_id: "t-inv".to_string(),
            memory_count: 1,
            knowledge_count: 1,
            graph_node_count: 1,
            total_count: 3,
        };

        enforcer
            .cache
            .insert("t-inv".to_string(), (usage, Instant::now()));
        assert!(enforcer.cache.contains_key("t-inv"));

        enforcer.invalidate("t-inv");
        assert!(!enforcer.cache.contains_key("t-inv"));
    }

    #[test]
    fn default_enforcer_has_five_minute_ttl() {
        let enforcer = QuotaEnforcer::default();
        assert_eq!(enforcer.cache_ttl, Duration::from_secs(300));
    }

    #[test]
    fn custom_ttl_enforcer() {
        let enforcer = QuotaEnforcer::with_ttl(Duration::from_secs(60));
        assert_eq!(enforcer.cache_ttl, Duration::from_secs(60));
    }
}
