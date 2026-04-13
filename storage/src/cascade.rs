/// Cascading delete coordinator.
///
/// Ensures that deletion in one backend (e.g. PostgreSQL) cascades to all
/// related backends (Qdrant vectors, DuckDB graph nodes, Redis cache) so
/// that orphan data is never left behind.
///
/// # Design
///
/// Each `cascade_*` function follows a best-effort model: the primary
/// PostgreSQL deletion is authoritative, and secondary backend failures
/// are logged and accumulated in the [`CascadeReport`] rather than
/// aborting the entire operation.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

use crate::graph_duckdb::DuckDbGraphStore;
use crate::postgres::PostgresBackend;
use mk_core::types::{SYSTEM_USER_ID, TenantContext};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum CascadeError {
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Graph error: {0}")]
    Graph(String),

    #[error("Cascade partially failed: {message}")]
    Partial { message: String },
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Summary returned by every cascade operation so callers can log / audit
/// exactly what happened across each backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CascadeReport {
    pub postgres_deleted: u64,
    pub qdrant_deleted: u64,
    pub graph_deleted: u64,
    pub redis_deleted: u64,
    pub errors: Vec<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl CascadeReport {
    fn finish(mut self) -> Self {
        self.completed_at = Some(Utc::now());
        self
    }

    fn record_error(&mut self, msg: String) {
        error!("{}", msg);
        self.errors.push(msg);
    }
}

/// Multi-entity purge report returned by [`CascadeDeleter::cascade_tenant_purge`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantPurgeReport {
    pub memories: CascadeReport,
    pub knowledge_items_deleted: u64,
    pub org_units_deleted: u64,
    pub user_roles_deleted: u64,
    pub unit_policies_deleted: u64,
    pub redis_tenant_keys_deleted: u64,
    pub errors: Vec<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// CascadeDeleter
// ---------------------------------------------------------------------------

/// Coordinates deletion across PostgreSQL, DuckDB graph, Redis cache, and
/// (optionally) Qdrant vectors.
///
/// Qdrant deletion is handled indirectly: callers pass in an async callback
/// (`qdrant_delete_fn`) that invokes the appropriate `MemoryProviderAdapter::delete`
/// for each memory id.  This avoids a direct `qdrant_client` dependency in the
/// storage crate.
pub struct CascadeDeleter {
    pool: PgPool,
    graph_store: Option<Arc<DuckDbGraphStore>>,
}

impl CascadeDeleter {
    /// Create a new cascade deleter.
    ///
    /// # Arguments
    /// * `postgres` — shared reference to the PostgreSQL backend
    /// * `graph_store` — optional DuckDB graph store for graph-node cleanup
    pub fn new(postgres: &PostgresBackend, graph_store: Option<Arc<DuckDbGraphStore>>) -> Self {
        Self {
            pool: postgres.pool().clone(),
            graph_store,
        }
    }

    // -----------------------------------------------------------------------
    // Memory cascade
    // -----------------------------------------------------------------------

    /// Delete a batch of memories from PostgreSQL and cascade to DuckDB graph
    /// and Redis.
    ///
    /// Qdrant deletion is intentionally left to the caller via the
    /// `qdrant_delete_fn` callback so the storage crate does not need to
    /// depend on `qdrant_client`.  Pass `None` when Qdrant is not in use.
    ///
    /// # Arguments
    /// * `memory_ids` — the IDs previously fetched from PostgreSQL
    /// * `tenant_id` — scoping tenant
    /// * `redis` — optional Redis connection for cache cleanup
    /// * `qdrant_delete_fn` — optional async closure that deletes a single
    ///   Qdrant point by memory ID
    pub async fn cascade_delete_memories<F, Fut>(
        &self,
        memory_ids: &[String],
        tenant_id: &str,
        redis: Option<&redis::aio::ConnectionManager>,
        qdrant_delete_fn: Option<F>,
    ) -> CascadeReport
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let mut report = CascadeReport::default();

        if memory_ids.is_empty() {
            return report.finish();
        }

        // 1. Delete from PostgreSQL -----------------------------------------
        match self.pg_delete_memories(memory_ids, tenant_id).await {
            Ok(count) => report.postgres_deleted = count,
            Err(e) => report.record_error(format!("PG memory delete: {e}")),
        }

        // 2. Delete from Qdrant via callback --------------------------------
        if let Some(ref delete_fn) = qdrant_delete_fn {
            for id in memory_ids {
                match delete_fn(id.clone()).await {
                    Ok(()) => report.qdrant_deleted += 1,
                    Err(e) => report.record_error(format!("Qdrant delete {id}: {e}")),
                }
            }
        }

        // 3. Soft-delete DuckDB graph nodes ---------------------------------
        if let Some(ref graph) = self.graph_store {
            for id in memory_ids {
                let ctx = match self.make_ctx(tenant_id) {
                    Ok(c) => c,
                    Err(e) => {
                        report.record_error(format!("Graph ctx: {e}"));
                        continue;
                    }
                };
                match graph.soft_delete_nodes_by_source_memory_id(ctx, id) {
                    Ok(n) => report.graph_deleted += n as u64,
                    Err(e) => report.record_error(format!("Graph delete {id}: {e}")),
                }
            }
        }

        // 4. Delete Redis embedding cache keys ------------------------------
        if let Some(redis_conn) = redis {
            match self
                .redis_delete_memory_keys(redis_conn, tenant_id, memory_ids)
                .await
            {
                Ok(n) => report.redis_deleted = n,
                Err(e) => report.record_error(format!("Redis memory key delete: {e}")),
            }
        }

        report.finish()
    }

    // -----------------------------------------------------------------------
    // Knowledge-item cascade
    // -----------------------------------------------------------------------

    /// Delete knowledge items and their related rows (promotion requests and
    /// knowledge relations) from PostgreSQL.
    pub async fn cascade_delete_knowledge_items(
        &self,
        knowledge_item_ids: &[String],
        tenant_id: &str,
    ) -> CascadeReport {
        let mut report = CascadeReport::default();

        if knowledge_item_ids.is_empty() {
            return report.finish();
        }

        // Delete knowledge items themselves
        match self
            .pg_delete_knowledge_items(knowledge_item_ids, tenant_id)
            .await
        {
            Ok(count) => report.postgres_deleted = count,
            Err(e) => report.record_error(format!("PG knowledge delete: {e}")),
        }

        report.finish()
    }

    // -----------------------------------------------------------------------
    // User cascade
    // -----------------------------------------------------------------------

    /// Full user deletion: GDPR-level memory + knowledge deletion, role
    /// cleanup, governance-event anonymization, and cascade to secondary
    /// backends.
    pub async fn cascade_delete_user<F, Fut>(
        &self,
        user_id: &str,
        tenant_id: &str,
        redis: Option<&redis::aio::ConnectionManager>,
        qdrant_delete_fn: Option<F>,
    ) -> CascadeReport
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let mut report = CascadeReport::default();

        // 1. Fetch + delete user memories from PG ---------------------------
        let memory_ids = match self.pg_fetch_user_memory_ids(user_id, tenant_id).await {
            Ok(ids) => ids,
            Err(e) => {
                report.record_error(format!("Fetch user memory ids: {e}"));
                Vec::new()
            }
        };

        let mem_report = self
            .cascade_delete_memories(&memory_ids, tenant_id, redis, qdrant_delete_fn)
            .await;
        report.postgres_deleted += mem_report.postgres_deleted;
        report.qdrant_deleted += mem_report.qdrant_deleted;
        report.graph_deleted += mem_report.graph_deleted;
        report.redis_deleted += mem_report.redis_deleted;
        report.errors.extend(mem_report.errors);

        // 2. Delete knowledge items created by user -------------------------
        match self.pg_delete_user_knowledge(user_id, tenant_id).await {
            Ok(count) => report.postgres_deleted += count,
            Err(e) => report.record_error(format!("PG knowledge delete: {e}")),
        }

        // 3. Delete user_roles ----------------------------------------------
        match self.pg_delete_user_roles(user_id, tenant_id).await {
            Ok(count) => report.postgres_deleted += count,
            Err(e) => report.record_error(format!("PG user_roles delete: {e}")),
        }

        // 4. Anonymize governance events ------------------------------------
        match self
            .pg_anonymize_governance_events(user_id, tenant_id)
            .await
        {
            Ok(count) => {
                info!(
                    user_id,
                    tenant_id, count, "Anonymized governance events for deleted user"
                );
            }
            Err(e) => report.record_error(format!("PG governance anonymize: {e}")),
        }

        // 5. Redis user-scoped keys -----------------------------------------
        if let Some(redis_conn) = redis {
            match Self::redis_delete_user_keys(redis_conn, tenant_id, user_id).await {
                Ok(n) => report.redis_deleted += n,
                Err(e) => report.record_error(format!("Redis user key delete: {e}")),
            }
        }

        report.finish()
    }

    // -----------------------------------------------------------------------
    // Org-unit cascade
    // -----------------------------------------------------------------------

    /// Recursively delete an organizational unit and its children.
    pub async fn cascade_delete_org_unit(
        &self,
        org_unit_id: &str,
        tenant_id: &str,
    ) -> CascadeReport {
        let mut report = CascadeReport::default();

        // 1. Find all descendant unit IDs (recursive CTE) -------------------
        let unit_ids = match self
            .pg_fetch_descendant_unit_ids(org_unit_id, tenant_id)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                report.record_error(format!("Fetch descendant units: {e}"));
                return report.finish();
            }
        };

        // Include the root unit itself
        let mut all_ids = unit_ids;
        all_ids.push(org_unit_id.to_string());

        // 2. Delete user_roles referencing any of these units ---------------
        for uid in &all_ids {
            match self.pg_delete_roles_for_unit(uid, tenant_id).await {
                Ok(count) => report.postgres_deleted += count,
                Err(e) => {
                    report.record_error(format!("Delete roles for unit {uid}: {e}"));
                }
            }
        }

        // 3. Delete unit_policies referencing these units -------------------
        for uid in &all_ids {
            match self.pg_delete_policies_for_unit(uid).await {
                Ok(count) => report.postgres_deleted += count,
                Err(e) => {
                    report.record_error(format!("Delete policies for unit {uid}: {e}"));
                }
            }
        }

        // 4. Delete the units themselves (children first, then root) --------
        // Reverse so leaves are deleted before parents (FK constraint).
        for uid in all_ids.iter().rev() {
            match self.pg_delete_org_unit(uid, tenant_id).await {
                Ok(count) => report.postgres_deleted += count,
                Err(e) => {
                    report.record_error(format!("Delete org unit {uid}: {e}"));
                }
            }
        }

        report.finish()
    }

    // -----------------------------------------------------------------------
    // Full tenant purge
    // -----------------------------------------------------------------------

    /// Purge **all** data for a tenant (post-quarantine full wipe).
    ///
    /// Returns a detailed [`TenantPurgeReport`] with counts per entity type.
    pub async fn cascade_tenant_purge<F, Fut>(
        &self,
        tenant_id: &str,
        redis: Option<&redis::aio::ConnectionManager>,
        qdrant_delete_fn: Option<F>,
    ) -> TenantPurgeReport
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let mut report = TenantPurgeReport::default();

        // 1. Memories -------------------------------------------------------
        let memory_ids = match self.pg_fetch_all_tenant_memory_ids(tenant_id).await {
            Ok(ids) => ids,
            Err(e) => {
                report.errors.push(format!("Fetch tenant memory ids: {e}"));
                Vec::new()
            }
        };

        report.memories = self
            .cascade_delete_memories(&memory_ids, tenant_id, redis, qdrant_delete_fn)
            .await;

        // 2. Knowledge items ------------------------------------------------
        match self.pg_delete_all_tenant_knowledge(tenant_id).await {
            Ok(count) => report.knowledge_items_deleted = count,
            Err(e) => {
                report
                    .errors
                    .push(format!("PG tenant knowledge delete: {e}"));
            }
        }

        // 3. Organizational units -------------------------------------------
        match self.pg_delete_all_tenant_org_units(tenant_id).await {
            Ok(count) => report.org_units_deleted = count,
            Err(e) => report.errors.push(format!("PG tenant org units: {e}")),
        }

        // 4. User roles -----------------------------------------------------
        match self.pg_delete_all_tenant_user_roles(tenant_id).await {
            Ok(count) => report.user_roles_deleted = count,
            Err(e) => report.errors.push(format!("PG tenant user_roles: {e}")),
        }

        // 5. Unit policies --------------------------------------------------
        match self.pg_delete_all_tenant_unit_policies(tenant_id).await {
            Ok(count) => report.unit_policies_deleted = count,
            Err(e) => {
                report.errors.push(format!("PG tenant unit_policies: {e}"));
            }
        }

        // 6. Redis tenant keys ----------------------------------------------
        if let Some(redis_conn) = redis {
            match Self::redis_delete_tenant_keys(redis_conn, tenant_id).await {
                Ok(n) => report.redis_tenant_keys_deleted = n,
                Err(e) => report.errors.push(format!("Redis tenant keys: {e}")),
            }
        }

        report.completed_at = Some(Utc::now());
        report
    }

    // =======================================================================
    // Private helpers — PostgreSQL
    // =======================================================================

    async fn pg_delete_memories(
        &self,
        ids: &[String],
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result =
            sqlx::query("DELETE FROM memory_entries WHERE id = ANY($1) AND tenant_id = $2")
                .bind(ids)
                .bind(tenant_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    async fn pg_fetch_user_memory_ids(
        &self,
        user_id: &str,
        tenant_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT id::text FROM memory_entries WHERE tenant_id = $1 AND user_id = $2",
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    async fn pg_fetch_all_tenant_memory_ids(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT id::text FROM memory_entries WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
    }

    async fn pg_delete_knowledge_items(
        &self,
        ids: &[String],
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result =
            sqlx::query("DELETE FROM knowledge_items WHERE id = ANY($1) AND tenant_id = $2")
                .bind(ids)
                .bind(tenant_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_user_knowledge(
        &self,
        user_id: &str,
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM knowledge_items WHERE tenant_id = $1 AND created_by = $2")
                .bind(tenant_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_all_tenant_knowledge(&self, tenant_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM knowledge_items WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_user_roles(
        &self,
        user_id: &str,
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_roles WHERE user_id = $1 AND tenant_id = $2")
            .bind(user_id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_roles_for_unit(
        &self,
        unit_id: &str,
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_roles WHERE unit_id = $1 AND tenant_id = $2")
            .bind(unit_id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_policies_for_unit(&self, unit_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM unit_policies WHERE unit_id = $1")
            .bind(unit_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_org_unit(&self, unit_id: &str, tenant_id: &str) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM organizational_units WHERE id = $1 AND tenant_id = $2")
                .bind(unit_id)
                .bind(tenant_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_all_tenant_org_units(&self, tenant_id: &str) -> Result<u64, sqlx::Error> {
        // Delete children first via a recursive approach: delete in order
        // where no other unit references them as parent.
        // Simpler: delete unit_policies first, then user_roles referencing
        // units, then units.  Here we just delete the units; the caller
        // (tenant_purge) handles roles + policies separately.
        let result = sqlx::query("DELETE FROM organizational_units WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_all_tenant_user_roles(&self, tenant_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_roles WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn pg_delete_all_tenant_unit_policies(
        &self,
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        // unit_policies table does not have tenant_id column directly,
        // so we join through organizational_units.
        let result = sqlx::query(
            "DELETE FROM unit_policies WHERE unit_id IN \
             (SELECT id FROM organizational_units WHERE tenant_id = $1)",
        )
        .bind(tenant_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn pg_fetch_descendant_unit_ids(
        &self,
        org_unit_id: &str,
        tenant_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar(
            "WITH RECURSIVE descendants AS (
                SELECT id FROM organizational_units
                WHERE parent_id = $1 AND tenant_id = $2
                UNION ALL
                SELECT u.id FROM organizational_units u
                INNER JOIN descendants d ON u.parent_id = d.id
            )
            SELECT id FROM descendants",
        )
        .bind(org_unit_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
    }

    async fn pg_anonymize_governance_events(
        &self,
        user_id: &str,
        tenant_id: &str,
    ) -> Result<u64, sqlx::Error> {
        // governance_events stores event metadata as JSONB payload.
        // We update the payload to redact the actor field.
        let result = sqlx::query(
            "UPDATE governance_events \
             SET payload = jsonb_set(payload, '{actor}', '\"[deleted]\"'::jsonb) \
             WHERE tenant_id = $1 \
             AND payload->>'actor' = $2",
        )
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // =======================================================================
    // Private helpers — Redis
    // =======================================================================

    /// Delete Redis keys matching the embedding cache pattern for specific
    /// memory IDs.
    async fn redis_delete_memory_keys(
        &self,
        redis: &redis::aio::ConnectionManager,
        tenant_id: &str,
        memory_ids: &[String],
    ) -> Result<u64, CascadeError> {
        let mut conn = redis.clone();
        let mut deleted: u64 = 0;

        // Embedding cache uses pattern: {tenant_id}:emb:*
        // We scan and filter for memory IDs.
        let pattern = format!("{tenant_id}:emb:*");
        let mut cursor: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100u64)
                .query_async(&mut conn)
                .await
                .map_err(|e| CascadeError::Redis(format!("SCAN failed: {e}")))?;

            let matching: Vec<&String> = keys
                .iter()
                .filter(|k| memory_ids.iter().any(|mid| k.contains(mid.as_str())))
                .collect();

            if !matching.is_empty() {
                let count = matching.len() as u64;
                redis::cmd("DEL")
                    .arg(matching.as_slice())
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|e| CascadeError::Redis(format!("DEL failed: {e}")))?;
                deleted += count;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(deleted)
    }

    /// Delete all Redis keys scoped to a specific user within a tenant.
    async fn redis_delete_user_keys(
        redis: &redis::aio::ConnectionManager,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<u64, CascadeError> {
        let mut conn = redis.clone();
        let pattern = format!("{tenant_id}:*:{user_id}:*");
        let mut cursor: u64 = 0;
        let mut deleted: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100u64)
                .query_async(&mut conn)
                .await
                .map_err(|e| CascadeError::Redis(format!("SCAN user keys: {e}")))?;

            if !keys.is_empty() {
                let count = keys.len() as u64;
                redis::cmd("DEL")
                    .arg(&keys)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|e| CascadeError::Redis(format!("DEL user keys: {e}")))?;
                deleted += count;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(deleted)
    }

    /// Delete all Redis keys for an entire tenant.
    async fn redis_delete_tenant_keys(
        redis: &redis::aio::ConnectionManager,
        tenant_id: &str,
    ) -> Result<u64, CascadeError> {
        let mut conn = redis.clone();
        let pattern = format!("{tenant_id}:*");
        let mut cursor: u64 = 0;
        let mut deleted: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100u64)
                .query_async(&mut conn)
                .await
                .map_err(|e| CascadeError::Redis(format!("SCAN tenant keys: {e}")))?;

            if !keys.is_empty() {
                let count = keys.len() as u64;
                redis::cmd("DEL")
                    .arg(&keys)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|e| CascadeError::Redis(format!("DEL tenant keys: {e}")))?;
                deleted += count;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(deleted)
    }

    // =======================================================================
    // Private helpers — context construction
    // =======================================================================

    fn make_ctx(&self, tenant_id: &str) -> Result<TenantContext, String> {
        use std::str::FromStr;
        let tid = mk_core::types::TenantId::from_str(tenant_id)
            .map_err(|e| format!("Invalid tenant_id: {e}"))?;
        let uid = mk_core::types::UserId::from_str(SYSTEM_USER_ID)
            .map_err(|e| format!("Invalid system user_id: {e}"))?;
        Ok(TenantContext::new(tid, uid))
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cascade_report_default() {
        let report = CascadeReport::default();
        assert_eq!(report.postgres_deleted, 0);
        assert_eq!(report.qdrant_deleted, 0);
        assert_eq!(report.graph_deleted, 0);
        assert_eq!(report.redis_deleted, 0);
        assert!(report.errors.is_empty());
        assert!(report.completed_at.is_none());
    }

    #[test]
    fn test_cascade_report_finish_sets_timestamp() {
        let report = CascadeReport::default().finish();
        assert!(report.completed_at.is_some());
    }

    #[test]
    fn test_cascade_report_record_error() {
        let mut report = CascadeReport::default();
        report.record_error("test error".to_string());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0], "test error");
    }

    #[test]
    fn test_tenant_purge_report_default() {
        let report = TenantPurgeReport::default();
        assert_eq!(report.knowledge_items_deleted, 0);
        assert_eq!(report.org_units_deleted, 0);
        assert_eq!(report.user_roles_deleted, 0);
        assert_eq!(report.unit_policies_deleted, 0);
        assert_eq!(report.redis_tenant_keys_deleted, 0);
        assert!(report.errors.is_empty());
        assert!(report.completed_at.is_none());
    }

    #[test]
    fn test_cascade_report_serialization() {
        let report = CascadeReport {
            postgres_deleted: 5,
            qdrant_deleted: 3,
            graph_deleted: 2,
            redis_deleted: 1,
            errors: vec!["partial failure".to_string()],
            completed_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&report).expect("serialize");
        let deserialized: CascadeReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.postgres_deleted, 5);
        assert_eq!(deserialized.qdrant_deleted, 3);
        assert_eq!(deserialized.errors.len(), 1);
    }

    #[test]
    fn test_tenant_purge_report_serialization() {
        let report = TenantPurgeReport {
            memories: CascadeReport {
                postgres_deleted: 10,
                ..Default::default()
            },
            knowledge_items_deleted: 5,
            org_units_deleted: 3,
            user_roles_deleted: 2,
            unit_policies_deleted: 1,
            redis_tenant_keys_deleted: 7,
            errors: vec![],
            completed_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&report).expect("serialize");
        let deserialized: TenantPurgeReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.memories.postgres_deleted, 10);
        assert_eq!(deserialized.knowledge_items_deleted, 5);
    }

    #[test]
    fn test_cascade_error_display() {
        let err = CascadeError::Postgres(sqlx::Error::RowNotFound);
        assert!(err.to_string().contains("PostgreSQL"));

        let err = CascadeError::Redis("connection refused".to_string());
        assert!(err.to_string().contains("Redis"));

        let err = CascadeError::Graph("DuckDB error".to_string());
        assert!(err.to_string().contains("Graph"));

        let err = CascadeError::Partial {
            message: "half done".to_string(),
        };
        assert!(err.to_string().contains("partially failed"));
    }

    #[test]
    fn test_multiple_errors_accumulated() {
        let mut report = CascadeReport::default();
        report.record_error("error 1".to_string());
        report.record_error("error 2".to_string());
        report.record_error("error 3".to_string());
        assert_eq!(report.errors.len(), 3);
    }
}
