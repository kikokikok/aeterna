//! Multi-tenant workspace management for the Central Index Service.
//!
//! Each tenant gets an isolated workspace with its own collection prefix and
//! project registry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Derive a workspace name from a tenant identifier.
///
/// Convention: `org-<lowercase-tenant>` with spaces replaced by hyphens.
pub fn workspace_name(tenant_id: &str) -> String {
    format!("org-{}", tenant_id.to_lowercase().replace(' ', "-"))
}

/// Derive a Qdrant/storage collection prefix from a tenant identifier.
///
/// Convention: `codesearch_<normalised>_` with hyphens and spaces collapsed to
/// underscores.
pub fn collection_prefix(tenant_id: &str) -> String {
    let normalised = tenant_id.to_lowercase().replace(['-', ' '], "_");
    format!("codesearch_{normalised}_")
}

/// Metadata about an active workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Tenant that owns this workspace.
    pub tenant_id: String,
    /// Derived workspace name (see [`workspace_name`]).
    pub workspace_name: String,
    /// Projects registered in this workspace.
    pub projects: Vec<String>,
    /// Vector store type (e.g. `qdrant`, `postgres`).
    pub store_type: String,
    /// Embedder type (e.g. `openai`, `ollama`).
    pub embedder_type: String,
    /// When the workspace was first created.
    pub created_at: DateTime<Utc>,
    /// When the workspace was last accessed.
    pub last_active: DateTime<Utc>,
}

/// In-memory workspace registry.
///
/// Thread-safe via `parking_lot::Mutex`. In production this would be backed by
/// a persistent store; the in-memory implementation is intentional for the
/// initial iteration.
pub struct WorkspaceManager {
    workspaces: Arc<parking_lot::Mutex<HashMap<String, WorkspaceInfo>>>,
}

impl WorkspaceManager {
    /// Create a new, empty workspace manager.
    pub fn new() -> Self {
        Self {
            workspaces: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    /// Return the workspace for `tenant_id`, creating it if it doesn't exist.
    pub fn get_or_create(
        &self,
        tenant_id: &str,
        store_type: &str,
        embedder_type: &str,
    ) -> WorkspaceInfo {
        let mut map = self.workspaces.lock();
        map.entry(tenant_id.to_string())
            .or_insert_with(|| {
                let now = Utc::now();
                WorkspaceInfo {
                    tenant_id: tenant_id.to_string(),
                    workspace_name: workspace_name(tenant_id),
                    projects: vec![],
                    store_type: store_type.to_string(),
                    embedder_type: embedder_type.to_string(),
                    created_at: now,
                    last_active: now,
                }
            })
            .clone()
    }

    /// Register a project within a tenant's workspace. Returns `true` if the
    /// project was newly added, `false` if it was already registered.
    pub fn register_project(&self, tenant_id: &str, project: &str) -> bool {
        let mut map = self.workspaces.lock();
        if let Some(ws) = map.get_mut(tenant_id) {
            ws.last_active = Utc::now();
            if ws.projects.contains(&project.to_string()) {
                return false;
            }
            ws.projects.push(project.to_string());
            true
        } else {
            false
        }
    }

    /// Get status information for a tenant's workspace.
    pub fn get_status(&self, tenant_id: &str) -> Option<WorkspaceInfo> {
        let map = self.workspaces.lock();
        map.get(tenant_id).cloned()
    }

    /// List all projects registered under a tenant.
    pub fn list_projects(&self, tenant_id: &str) -> Vec<String> {
        let map = self.workspaces.lock();
        map.get(tenant_id)
            .map(|ws| ws.projects.clone())
            .unwrap_or_default()
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_name_lowercases_and_replaces_spaces() {
        assert_eq!(workspace_name("Acme Corp"), "org-acme-corp");
        assert_eq!(workspace_name("simple"), "org-simple");
        assert_eq!(workspace_name("UPPER"), "org-upper");
    }

    #[test]
    fn collection_prefix_normalises() {
        assert_eq!(collection_prefix("acme-corp"), "codesearch_acme_corp_");
        assert_eq!(collection_prefix("Acme Corp"), "codesearch_acme_corp_");
        assert_eq!(collection_prefix("simple"), "codesearch_simple_");
    }

    #[test]
    fn workspace_manager_get_or_create() {
        let mgr = WorkspaceManager::new();
        let ws = mgr.get_or_create("tenant1", "qdrant", "openai");
        assert_eq!(ws.tenant_id, "tenant1");
        assert_eq!(ws.workspace_name, "org-tenant1");
        assert_eq!(ws.store_type, "qdrant");
        assert_eq!(ws.embedder_type, "openai");
        assert!(ws.projects.is_empty());
    }

    #[test]
    fn workspace_manager_idempotent_create() {
        let mgr = WorkspaceManager::new();
        let ws1 = mgr.get_or_create("t", "qdrant", "openai");
        let ws2 = mgr.get_or_create("t", "postgres", "ollama");
        // First creation wins – store/embedder types are NOT overwritten.
        assert_eq!(ws1.store_type, ws2.store_type);
    }

    #[test]
    fn register_project_and_list() {
        let mgr = WorkspaceManager::new();
        mgr.get_or_create("t1", "qdrant", "openai");
        assert!(mgr.register_project("t1", "project-a"));
        assert!(mgr.register_project("t1", "project-b"));
        // Duplicate registration returns false.
        assert!(!mgr.register_project("t1", "project-a"));
        let projects = mgr.list_projects("t1");
        assert_eq!(projects, vec!["project-a", "project-b"]);
    }

    #[test]
    fn register_project_unknown_tenant() {
        let mgr = WorkspaceManager::new();
        assert!(!mgr.register_project("unknown", "proj"));
    }

    #[test]
    fn get_status_returns_none_for_missing_tenant() {
        let mgr = WorkspaceManager::new();
        assert!(mgr.get_status("nonexistent").is_none());
    }

    #[test]
    fn get_status_returns_info() {
        let mgr = WorkspaceManager::new();
        mgr.get_or_create("t1", "qdrant", "openai");
        let status = mgr.get_status("t1").unwrap();
        assert_eq!(status.tenant_id, "t1");
    }

    #[test]
    fn default_workspace_manager() {
        let mgr = WorkspaceManager::default();
        assert!(mgr.list_projects("any").is_empty());
    }
}
