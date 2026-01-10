use crate::repository::RepositoryError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub id: String,
    pub url: String,
    pub branch: String,
    pub auth_token: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationConfig {
    pub upstreams: Vec<UpstreamConfig>,
    pub sync_interval_secs: u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeManifest {
    pub version: String,
    pub items: HashMap<String, String>
}

#[async_trait::async_trait]
pub trait FederationProvider: Send + Sync {
    fn config(&self) -> &FederationConfig;
    async fn fetch_upstream_manifest(
        &self,
        upstream_id: &str
    ) -> Result<KnowledgeManifest, RepositoryError>;
    async fn sync_upstream(
        &self,
        upstream_id: &str,
        target_path: &std::path::Path
    ) -> Result<(), RepositoryError>;
}

pub struct FederationManager {
    config: FederationConfig
}

#[async_trait::async_trait]
impl FederationProvider for FederationManager {
    fn config(&self) -> &FederationConfig {
        &self.config
    }

    async fn fetch_upstream_manifest(
        &self,
        upstream_id: &str
    ) -> Result<KnowledgeManifest, RepositoryError> {
        let _upstream = self
            .config
            .upstreams
            .iter()
            .find(|u| u.id == upstream_id)
            .ok_or_else(|| {
                RepositoryError::InvalidPath(format!("Upstream not found: {}", upstream_id))
            })?;

        Ok(KnowledgeManifest {
            version: "1.0".to_string(),
            items: HashMap::new()
        })
    }

    async fn sync_upstream(
        &self,
        upstream_id: &str,
        target_path: &std::path::Path
    ) -> Result<(), RepositoryError> {
        let upstream = self
            .config
            .upstreams
            .iter()
            .find(|u| u.id == upstream_id)
            .ok_or_else(|| {
                RepositoryError::InvalidPath(format!("Upstream not found: {}", upstream_id))
            })?;

        if target_path.exists() {
            let repo = git2::Repository::open(target_path)?;
            let mut remote = repo.find_remote("origin")?;
            remote.fetch(&[&upstream.branch], None, None)?;

            let head = repo.head()?.peel_to_commit()?;
            let remote_ref =
                repo.find_reference(&format!("refs/remotes/origin/{}", upstream.branch))?;
            let remote_commit = remote_ref.peel_to_commit()?;

            if repo.merge_base(head.id(), remote_commit.id())? != remote_commit.id() {
                return Err(RepositoryError::InvalidPath(
                    "Local changes conflict with upstream".to_string()
                ));
            }
        } else {
            git2::Repository::clone(&upstream.url, target_path)?;
        }

        Ok(())
    }
}

impl FederationManager {
    pub fn new(config: FederationConfig) -> Self {
        Self { config }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_federation_config_serialization() {
        let config = FederationConfig {
            upstreams: vec![UpstreamConfig {
                id: "test".to_string(),
                url: "https://github.com/test/repo".to_string(),
                branch: "main".to_string(),
                auth_token: Some("secret".to_string())
            }],
            sync_interval_secs: 3600
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: FederationConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.upstreams.len(), 1);
        assert_eq!(decoded.upstreams[0].id, "test");
        assert_eq!(decoded.sync_interval_secs, 3600);
    }

    #[tokio::test]
    async fn test_fetch_upstream_manifest_not_found() {
        let manager = FederationManager::new(FederationConfig {
            upstreams: vec![],
            sync_interval_secs: 60
        });

        let result = manager.fetch_upstream_manifest("nonexistent").await;
        assert!(result.is_err());
        match result {
            Err(RepositoryError::InvalidPath(msg)) => assert!(msg.contains("Upstream not found")),
            _ => panic!("Expected InvalidPath error")
        }
    }

    #[tokio::test]
    async fn test_sync_upstream_not_found() {
        let manager = FederationManager::new(FederationConfig {
            upstreams: vec![],
            sync_interval_secs: 60
        });

        let result = manager
            .sync_upstream("nonexistent", std::path::Path::new("/tmp"))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_knowledge_manifest_serialization() {
        let mut items = HashMap::new();
        items.insert("key1".to_string(), "hash1".to_string());

        let manifest = KnowledgeManifest {
            version: "1.0".to_string(),
            items
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: KnowledgeManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.version, "1.0");
        assert_eq!(decoded.items.get("key1").unwrap(), "hash1");
    }
}
