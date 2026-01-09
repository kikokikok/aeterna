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
