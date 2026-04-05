//! In-process registry of platform-owned Git provider connections.
//!
//! [`InMemoryGitProviderConnectionStore`] stores [`GitProviderConnection`]
//! records in a `DashMap` so that the resolver and server handlers can
//! look up connection metadata without hitting a database on every request.
//!
//! # Design notes
//! - The store is the authoritative source for connection visibility checks.
//! - PEM material is never stored inline; only the `pem_secret_ref` handle is kept.
//! - Thread-safe via `DashMap`; no external locking required.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use mk_core::traits::GitProviderConnectionRegistry;
use mk_core::types::{GitProviderConnection, TenantId};
use thiserror::Error;

/// Errors returned by [`InMemoryGitProviderConnectionStore`].
#[derive(Debug, Error, Clone)]
pub enum GitProviderConnectionError {
    #[error("git provider connection not found: {0}")]
    NotFound(String),

    #[error("git provider connection validation failed: {0}")]
    Validation(String),
}

/// In-process, `Arc`-sharable registry of platform-owned Git provider
/// connections.  Each entry is keyed by `connection.id`.
#[derive(Clone)]
pub struct InMemoryGitProviderConnectionStore {
    connections: Arc<DashMap<String, GitProviderConnection>>,
}

impl std::fmt::Debug for InMemoryGitProviderConnectionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryGitProviderConnectionStore")
            .field("connection_count", &self.connections.len())
            .finish()
    }
}

impl Default for InMemoryGitProviderConnectionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryGitProviderConnectionStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl GitProviderConnectionRegistry for InMemoryGitProviderConnectionStore {
    type Error = GitProviderConnectionError;

    async fn create_connection(
        &self,
        connection: GitProviderConnection,
    ) -> Result<GitProviderConnection, Self::Error> {
        if connection.id.trim().is_empty() {
            return Err(GitProviderConnectionError::Validation(
                "connection id must not be empty".to_string(),
            ));
        }
        if !connection.has_valid_pem_ref() {
            return Err(GitProviderConnectionError::Validation(format!(
                "pem_secret_ref '{}' must use a supported secret-provider prefix \
                 (local/, secret/, arn:aws:)",
                connection.pem_secret_ref
            )));
        }
        self.connections
            .insert(connection.id.clone(), connection.clone());
        Ok(connection)
    }

    async fn get_connection(
        &self,
        id: &str,
    ) -> Result<Option<GitProviderConnection>, Self::Error> {
        Ok(self.connections.get(id).map(|r| r.value().clone()))
    }

    async fn list_connections(&self) -> Result<Vec<GitProviderConnection>, Self::Error> {
        let mut connections: Vec<GitProviderConnection> = self
            .connections
            .iter()
            .map(|r| r.value().clone())
            .collect();
        connections.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(connections)
    }

    async fn list_connections_for_tenant(
        &self,
        tenant_id: &TenantId,
    ) -> Result<Vec<GitProviderConnection>, Self::Error> {
        let mut visible: Vec<GitProviderConnection> = self
            .connections
            .iter()
            .filter(|r| r.value().is_visible_to(tenant_id))
            .map(|r| r.value().clone())
            .collect();
        visible.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(visible)
    }

    async fn grant_tenant_visibility(
        &self,
        connection_id: &str,
        tenant_id: &TenantId,
    ) -> Result<(), Self::Error> {
        let mut entry = self
            .connections
            .get_mut(connection_id)
            .ok_or_else(|| GitProviderConnectionError::NotFound(connection_id.to_string()))?;
        if !entry.allowed_tenant_ids.contains(tenant_id) {
            entry.allowed_tenant_ids.push(tenant_id.clone());
        }
        Ok(())
    }

    async fn revoke_tenant_visibility(
        &self,
        connection_id: &str,
        tenant_id: &TenantId,
    ) -> Result<(), Self::Error> {
        let mut entry = self
            .connections
            .get_mut(connection_id)
            .ok_or_else(|| GitProviderConnectionError::NotFound(connection_id.to_string()))?;
        entry
            .allowed_tenant_ids
            .retain(|t| t != tenant_id);
        Ok(())
    }

    async fn tenant_can_use(
        &self,
        connection_id: &str,
        tenant_id: &TenantId,
    ) -> Result<bool, Self::Error> {
        Ok(self
            .connections
            .get(connection_id)
            .map(|r: dashmap::mapref::one::Ref<'_, String, GitProviderConnection>| r.is_visible_to(tenant_id))
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{GitProviderKind, TenantId};

    fn store() -> InMemoryGitProviderConnectionStore {
        InMemoryGitProviderConnectionStore::new()
    }

    fn tenant(id: &str) -> TenantId {
        TenantId::new(id.to_string()).unwrap()
    }

    fn make_connection(id: &str, allowed: Vec<TenantId>) -> GitProviderConnection {
        GitProviderConnection {
            id: id.to_string(),
            name: "Test App".to_string(),
            provider_kind: GitProviderKind::GitHubApp,
            app_id: 12345,
            installation_id: 67890,
            pem_secret_ref: "local/test-pem".to_string(),
            webhook_secret_ref: None,
            allowed_tenant_ids: allowed,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn create_and_get_connection() {
        let store = store();
        let conn = make_connection("conn-1", vec![]);
        let created = store.create_connection(conn.clone()).await.unwrap();
        assert_eq!(created.id, "conn-1");

        let found = store.get_connection("conn-1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().app_id, 12345);
    }

    #[tokio::test]
    async fn get_unknown_connection_returns_none() {
        let store = store();
        let result = store.get_connection("no-such-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_connections_sorted_by_id() {
        let store = store();
        store.create_connection(make_connection("b-conn", vec![])).await.unwrap();
        store.create_connection(make_connection("a-conn", vec![])).await.unwrap();
        let list = store.list_connections().await.unwrap();
        assert_eq!(list[0].id, "a-conn");
        assert_eq!(list[1].id, "b-conn");
    }

    #[tokio::test]
    async fn list_connections_for_tenant_filters_correctly() {
        let store = store();
        let t1 = tenant("tenant-1");
        let t2 = tenant("tenant-2");
        store.create_connection(make_connection("shared", vec![t1.clone(), t2.clone()])).await.unwrap();
        store.create_connection(make_connection("only-t1", vec![t1.clone()])).await.unwrap();
        store.create_connection(make_connection("none", vec![])).await.unwrap();

        let for_t2 = store.list_connections_for_tenant(&t2).await.unwrap();
        assert_eq!(for_t2.len(), 1);
        assert_eq!(for_t2[0].id, "shared");
    }

    #[tokio::test]
    async fn grant_and_revoke_tenant_visibility() {
        let store = store();
        let t = tenant("acme");
        store.create_connection(make_connection("conn-x", vec![])).await.unwrap();

        assert!(!store.tenant_can_use("conn-x", &t).await.unwrap());

        store.grant_tenant_visibility("conn-x", &t).await.unwrap();
        assert!(store.tenant_can_use("conn-x", &t).await.unwrap());

        // Granting again is idempotent.
        store.grant_tenant_visibility("conn-x", &t).await.unwrap();
        let list = store.list_connections_for_tenant(&t).await.unwrap();
        assert_eq!(list.len(), 1);

        store.revoke_tenant_visibility("conn-x", &t).await.unwrap();
        assert!(!store.tenant_can_use("conn-x", &t).await.unwrap());
    }

    #[tokio::test]
    async fn grant_on_missing_connection_returns_error() {
        let store = store();
        let t = tenant("acme");
        let err = store.grant_tenant_visibility("ghost", &t).await.unwrap_err();
        assert!(matches!(err, GitProviderConnectionError::NotFound(_)));
    }

    #[tokio::test]
    async fn revoke_on_missing_connection_returns_error() {
        let store = store();
        let t = tenant("acme");
        let err = store.revoke_tenant_visibility("ghost", &t).await.unwrap_err();
        assert!(matches!(err, GitProviderConnectionError::NotFound(_)));
    }

    #[tokio::test]
    async fn create_rejects_empty_id() {
        let store = store();
        let conn = make_connection("", vec![]);
        let err = store.create_connection(conn).await.unwrap_err();
        assert!(matches!(err, GitProviderConnectionError::Validation(_)));
    }

    #[tokio::test]
    async fn create_rejects_invalid_pem_ref() {
        let store = store();
        let mut conn = make_connection("c1", vec![]);
        conn.pem_secret_ref = "raw-pem-material".to_string();
        let err = store.create_connection(conn).await.unwrap_err();
        assert!(matches!(err, GitProviderConnectionError::Validation(_)));
        assert!(err.to_string().contains("pem_secret_ref"));
    }

    #[tokio::test]
    async fn redacted_view_masks_pem_ref() {
        let conn = make_connection("c1", vec![]);
        let redacted = conn.redacted();
        assert_eq!(redacted["pemSecretRef"], "[redacted]");
        assert_ne!(redacted["appId"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn tenant_can_use_unknown_connection_returns_false() {
        let store = store();
        let t = tenant("acme");
        let can_use = store.tenant_can_use("nonexistent", &t).await.unwrap();
        assert!(!can_use);
    }
}
