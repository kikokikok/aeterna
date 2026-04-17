//! In-memory store (V1) for remediation requests.
//!
//! Provides CRUD operations and lifecycle management for remediation requests
//! created by the lifecycle manager. Can be upgraded to PostgreSQL in a future
//! iteration.

use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::Utc;
use mk_core::types::{RemediationRequest, RemediationStatus};
use tokio::sync::RwLock;
use uuid::Uuid;

static REMEDIATION_STORE: LazyLock<RemediationStore> = LazyLock::new(RemediationStore::new);

/// Errors from the remediation store.
#[derive(Debug, thiserror::Error)]
pub enum RemediationStoreError {
    /// The requested remediation ID was not found.
    #[error("remediation request not found: {0}")]
    NotFound(String),
    /// The request is not in the expected status for the operation.
    #[error("invalid status transition: {0}")]
    InvalidStatus(String),
}

/// In-memory remediation request store.
///
/// Thread-safe via `RwLock`. Designed for single-instance deployments; for HA
/// deployments, swap to a Postgres-backed implementation.
pub struct RemediationStore {
    requests: RwLock<HashMap<String, RemediationRequest>>,
}

impl RemediationStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
        }
    }

    /// Return the process-global singleton.
    pub fn global() -> &'static Self {
        &REMEDIATION_STORE
    }

    /// Insert a new remediation request. If the `id` field is empty, a UUID is
    /// generated. Returns the final ID.
    pub async fn create(&self, mut req: RemediationRequest) -> String {
        if req.id.is_empty() {
            req.id = Uuid::new_v4().to_string();
        }
        let id = req.id.clone();
        self.requests.write().await.insert(id.clone(), req);
        id
    }

    /// Retrieve a single request by ID.
    pub async fn get(&self, id: &str) -> Option<RemediationRequest> {
        self.requests.read().await.get(id).cloned()
    }

    /// List all requests with `Pending` status.
    pub async fn list_pending(&self) -> Vec<RemediationRequest> {
        self.requests
            .read()
            .await
            .values()
            .filter(|r| r.status == RemediationStatus::Pending)
            .cloned()
            .collect()
    }

    /// List every request regardless of status.
    pub async fn list_all(&self) -> Vec<RemediationRequest> {
        self.requests.read().await.values().cloned().collect()
    }

    /// Approve a pending request. Returns the updated request.
    pub async fn approve(
        &self,
        id: &str,
        reviewer: &str,
        notes: Option<String>,
    ) -> Result<RemediationRequest, RemediationStoreError> {
        let mut map = self.requests.write().await;
        let req = map
            .get_mut(id)
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;
        if req.status != RemediationStatus::Pending {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot approve request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Approved;
        req.reviewed_by = Some(reviewer.to_string());
        req.reviewed_at = Some(Utc::now().timestamp());
        req.resolution_notes = notes;
        Ok(req.clone())
    }

    /// Reject a pending request with a reason.
    pub async fn reject(
        &self,
        id: &str,
        reviewer: &str,
        reason: &str,
    ) -> Result<RemediationRequest, RemediationStoreError> {
        let mut map = self.requests.write().await;
        let req = map
            .get_mut(id)
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;
        if req.status != RemediationStatus::Pending {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot reject request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Rejected;
        req.reviewed_by = Some(reviewer.to_string());
        req.reviewed_at = Some(Utc::now().timestamp());
        req.resolution_notes = Some(reason.to_string());
        Ok(req.clone())
    }

    /// Mark an approved request as executed.
    pub async fn mark_executed(&self, id: &str) -> Result<(), RemediationStoreError> {
        let mut map = self.requests.write().await;
        let req = map
            .get_mut(id)
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;
        if req.status != RemediationStatus::Approved {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot execute request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Executed;
        req.executed_at = Some(Utc::now().timestamp());
        Ok(())
    }

    /// Mark a request as failed with an error message.
    pub async fn mark_failed(&self, id: &str, error: &str) -> Result<(), RemediationStoreError> {
        let mut map = self.requests.write().await;
        let req = map
            .get_mut(id)
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;
        req.status = RemediationStatus::Failed;
        req.resolution_notes = Some(error.to_string());
        Ok(())
    }

    /// Expire requests that have been pending longer than `max_age_secs`.
    /// Returns the number of requests expired.
    pub async fn expire_stale(&self, max_age_secs: i64) -> usize {
        let now = Utc::now().timestamp();
        let mut map = self.requests.write().await;
        let mut count = 0;
        for req in map.values_mut() {
            if req.status == RemediationStatus::Pending && (now - req.created_at) > max_age_secs {
                req.status = RemediationStatus::Expired;
                count += 1;
            }
        }
        count
    }

    /// Remove old completed/rejected/expired/failed requests older than
    /// `max_age_secs`. Returns the number of records removed.
    pub async fn cleanup_old(&self, max_age_secs: i64) -> usize {
        let now = Utc::now().timestamp();
        let mut map = self.requests.write().await;
        let before = map.len();
        map.retain(|_, req| {
            let terminal = matches!(
                req.status,
                RemediationStatus::Executed
                    | RemediationStatus::Rejected
                    | RemediationStatus::Expired
                    | RemediationStatus::Failed
            );
            if terminal {
                (now - req.created_at) <= max_age_secs
            } else {
                true
            }
        });
        before - map.len()
    }
}

impl Default for RemediationStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Redis-backed remediation store for multi-instance deployments
// ---------------------------------------------------------------------------

/// Redis-backed remediation store for HA / multi-instance deployments.
///
/// Uses [`crate::redis_store::RedisStore`] with prefix `aeterna:remediations`.
/// Falls back to in-memory [`RemediationStore`] if Redis is unavailable.
pub struct RedisRemediationStore {
    store: crate::redis_store::RedisStore,
}

impl RedisRemediationStore {
    /// Create a new Redis-backed remediation store.
    pub fn new(conn: std::sync::Arc<redis::aio::ConnectionManager>) -> Self {
        Self {
            store: crate::redis_store::RedisStore::new(conn, "aeterna:remediations"),
        }
    }

    /// Insert a new remediation request. If the `id` field is empty, a UUID is
    /// generated. Returns the final ID.
    pub async fn create(&self, mut req: RemediationRequest) -> anyhow::Result<String> {
        if req.id.is_empty() {
            req.id = Uuid::new_v4().to_string();
        }
        let id = req.id.clone();
        self.store.set(&id, &req, None).await?;
        Ok(id)
    }

    /// Retrieve a single request by ID.
    pub async fn get(&self, id: &str) -> anyhow::Result<Option<RemediationRequest>> {
        self.store.get(id).await
    }

    /// List all requests with `Pending` status.
    pub async fn list_pending(&self) -> anyhow::Result<Vec<RemediationRequest>> {
        let all: Vec<RemediationRequest> = self.store.list_all().await?;
        Ok(all
            .into_iter()
            .filter(|r| r.status == RemediationStatus::Pending)
            .collect())
    }

    /// List every request regardless of status.
    pub async fn list_all(&self) -> anyhow::Result<Vec<RemediationRequest>> {
        self.store.list_all().await
    }

    /// Approve a pending request. Returns the updated request.
    pub async fn approve(
        &self,
        id: &str,
        reviewer: &str,
        notes: Option<String>,
    ) -> Result<RemediationRequest, RemediationStoreError> {
        let mut req = self
            .store
            .get::<RemediationRequest>(id)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("{id}: {e}")))?
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;

        if req.status != RemediationStatus::Pending {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot approve request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Approved;
        req.reviewed_by = Some(reviewer.to_string());
        req.reviewed_at = Some(Utc::now().timestamp());
        req.resolution_notes = notes;
        self.store
            .set(id, &req, None)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("write failed: {e}")))?;
        Ok(req)
    }

    /// Reject a pending request with a reason.
    pub async fn reject(
        &self,
        id: &str,
        reviewer: &str,
        reason: &str,
    ) -> Result<RemediationRequest, RemediationStoreError> {
        let mut req = self
            .store
            .get::<RemediationRequest>(id)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("{id}: {e}")))?
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;

        if req.status != RemediationStatus::Pending {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot reject request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Rejected;
        req.reviewed_by = Some(reviewer.to_string());
        req.reviewed_at = Some(Utc::now().timestamp());
        req.resolution_notes = Some(reason.to_string());
        self.store
            .set(id, &req, None)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("write failed: {e}")))?;
        Ok(req)
    }

    /// Mark an approved request as executed.
    pub async fn mark_executed(&self, id: &str) -> Result<(), RemediationStoreError> {
        let mut req = self
            .store
            .get::<RemediationRequest>(id)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("{id}: {e}")))?
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;

        if req.status != RemediationStatus::Approved {
            return Err(RemediationStoreError::InvalidStatus(format!(
                "cannot execute request in {:?} status",
                req.status
            )));
        }
        req.status = RemediationStatus::Executed;
        req.executed_at = Some(Utc::now().timestamp());
        self.store
            .set(id, &req, None)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("write failed: {e}")))?;
        Ok(())
    }

    /// Mark a request as failed with an error message.
    pub async fn mark_failed(&self, id: &str, error: &str) -> Result<(), RemediationStoreError> {
        let mut req = self
            .store
            .get::<RemediationRequest>(id)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("{id}: {e}")))?
            .ok_or_else(|| RemediationStoreError::NotFound(id.to_string()))?;

        req.status = RemediationStatus::Failed;
        req.resolution_notes = Some(error.to_string());
        self.store
            .set(id, &req, None)
            .await
            .map_err(|e| RemediationStoreError::NotFound(format!("write failed: {e}")))?;
        Ok(())
    }

    /// Expire requests that have been pending longer than `max_age_secs`.
    /// Returns the number of requests expired.
    pub async fn expire_stale(&self, max_age_secs: i64) -> anyhow::Result<usize> {
        let now = Utc::now().timestamp();
        let all: Vec<RemediationRequest> = self.store.list_all().await?;
        let mut count = 0;
        for mut req in all {
            if req.status == RemediationStatus::Pending && (now - req.created_at) > max_age_secs {
                req.status = RemediationStatus::Expired;
                self.store.set(&req.id, &req, None).await?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove old completed/rejected/expired/failed requests older than
    /// `max_age_secs`. Returns the number of records removed.
    pub async fn cleanup_old(&self, max_age_secs: i64) -> anyhow::Result<usize> {
        let now = Utc::now().timestamp();
        let all: Vec<RemediationRequest> = self.store.list_all().await?;
        let mut count = 0;
        for req in all {
            let terminal = matches!(
                req.status,
                RemediationStatus::Executed
                    | RemediationStatus::Rejected
                    | RemediationStatus::Expired
                    | RemediationStatus::Failed
            );
            if terminal && (now - req.created_at) > max_age_secs {
                self.store.delete(&req.id).await?;
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::RemediationRiskTier;

    fn make_request(id: &str, status: RemediationStatus, created_at: i64) -> RemediationRequest {
        RemediationRequest {
            id: id.to_string(),
            request_type: "test".to_string(),
            risk_tier: RemediationRiskTier::RequireApproval,
            entity_type: "memory".to_string(),
            entity_ids: vec!["e1".to_string()],
            tenant_id: Some("tenant-1".to_string()),
            description: "Test remediation".to_string(),
            proposed_action: "delete".to_string(),
            detected_by: "lifecycle_manager".to_string(),
            status,
            created_at,
            reviewed_by: None,
            reviewed_at: None,
            resolution_notes: None,
            executed_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let store = RemediationStore::new();
        let req = make_request("", RemediationStatus::Pending, Utc::now().timestamp());
        let id = store.create(req).await;
        assert!(!id.is_empty());

        let fetched = store.get(&id).await.expect("should exist");
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.status, RemediationStatus::Pending);
    }

    #[tokio::test]
    async fn create_with_explicit_id() {
        let store = RemediationStore::new();
        let req = make_request("my-id", RemediationStatus::Pending, Utc::now().timestamp());
        let id = store.create(req).await;
        assert_eq!(id, "my-id");
    }

    #[tokio::test]
    async fn list_pending_filters_correctly() {
        let store = RemediationStore::new();
        let now = Utc::now().timestamp();
        store
            .create(make_request("a", RemediationStatus::Pending, now))
            .await;
        store
            .create(make_request("b", RemediationStatus::Executed, now))
            .await;
        store
            .create(make_request("c", RemediationStatus::Pending, now))
            .await;

        let pending = store.list_pending().await;
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn approve_and_execute() {
        let store = RemediationStore::new();
        let now = Utc::now().timestamp();
        store
            .create(make_request("r1", RemediationStatus::Pending, now))
            .await;

        let approved = store
            .approve("r1", "admin", Some("lgtm".to_string()))
            .await
            .unwrap();
        assert_eq!(approved.status, RemediationStatus::Approved);
        assert_eq!(approved.reviewed_by.as_deref(), Some("admin"));

        store.mark_executed("r1").await.unwrap();
        let executed = store.get("r1").await.unwrap();
        assert_eq!(executed.status, RemediationStatus::Executed);
        assert!(executed.executed_at.is_some());
    }

    #[tokio::test]
    async fn reject_request() {
        let store = RemediationStore::new();
        let now = Utc::now().timestamp();
        store
            .create(make_request("r2", RemediationStatus::Pending, now))
            .await;

        let rejected = store.reject("r2", "admin", "too risky").await.unwrap();
        assert_eq!(rejected.status, RemediationStatus::Rejected);
        assert_eq!(rejected.resolution_notes.as_deref(), Some("too risky"));
    }

    #[tokio::test]
    async fn cannot_approve_non_pending() {
        let store = RemediationStore::new();
        let now = Utc::now().timestamp();
        store
            .create(make_request("r3", RemediationStatus::Executed, now))
            .await;

        let err = store.approve("r3", "admin", None).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn mark_failed() {
        let store = RemediationStore::new();
        let now = Utc::now().timestamp();
        store
            .create(make_request("r4", RemediationStatus::Approved, now))
            .await;

        store.mark_failed("r4", "connection timeout").await.unwrap();
        let req = store.get("r4").await.unwrap();
        assert_eq!(req.status, RemediationStatus::Failed);
    }

    #[tokio::test]
    async fn expire_stale_requests() {
        let store = RemediationStore::new();
        let old = Utc::now().timestamp() - 1000;
        let recent = Utc::now().timestamp();

        store
            .create(make_request("old", RemediationStatus::Pending, old))
            .await;
        store
            .create(make_request("new", RemediationStatus::Pending, recent))
            .await;

        let expired = store.expire_stale(500).await;
        assert_eq!(expired, 1);

        let old_req = store.get("old").await.unwrap();
        assert_eq!(old_req.status, RemediationStatus::Expired);

        let new_req = store.get("new").await.unwrap();
        assert_eq!(new_req.status, RemediationStatus::Pending);
    }

    #[tokio::test]
    async fn cleanup_old_removes_terminal_records() {
        let store = RemediationStore::new();
        let old = Utc::now().timestamp() - 1000;
        let recent = Utc::now().timestamp();

        store
            .create(make_request("done-old", RemediationStatus::Executed, old))
            .await;
        store
            .create(make_request(
                "done-new",
                RemediationStatus::Executed,
                recent,
            ))
            .await;
        store
            .create(make_request("pending", RemediationStatus::Pending, old))
            .await;

        let cleaned = store.cleanup_old(500).await;
        assert_eq!(cleaned, 1);

        // Old executed should be gone
        assert!(store.get("done-old").await.is_none());
        // New executed still present
        assert!(store.get("done-new").await.is_some());
        // Pending kept regardless of age
        assert!(store.get("pending").await.is_some());
    }

    #[tokio::test]
    async fn not_found_error_for_missing_id() {
        let store = RemediationStore::new();
        let err = store.approve("nonexistent", "admin", None).await;
        assert!(matches!(err, Err(RemediationStoreError::NotFound(_))));
    }
}
