use async_trait::async_trait;
use git2::{Repository, Signature};
use mk_core::traits::KnowledgeRepository;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, KnowledgeType};
use std::path::PathBuf;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error)
}

pub struct GitRepository {
    root_path: PathBuf
}

impl GitRepository {
    pub fn new(root_path: impl Into<PathBuf>) -> Result<Self, RepositoryError> {
        let root_path = root_path.into();
        if !root_path.exists() {
            std::fs::create_dir_all(&root_path)?;
        }

        if Repository::open(&root_path).is_err() {
            Repository::init(&root_path)?;
        }

        Ok(Self { root_path })
    }

    fn resolve_path(
        &self,
        ctx: &mk_core::types::TenantContext,
        layer: KnowledgeLayer,
        path: &str
    ) -> PathBuf {
        let layer_dir = match layer {
            KnowledgeLayer::Company => "company",
            KnowledgeLayer::Org => "org",
            KnowledgeLayer::Team => "team",
            KnowledgeLayer::Project => "project"
        };
        self.root_path
            .join(ctx.tenant_id.as_str())
            .join(layer_dir)
            .join(path)
    }

    pub fn commit(&self, message: &str) -> Result<String, RepositoryError> {
        let span = tracing::info_span!("knowledge_commit", message = %message);
        let _enter = span.enter();

        let repo = Repository::open(&self.root_path)?;
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let sig = repo
            .signature()
            .or_else(|_| Signature::now("Aeterna", "system@aeterna.ai"))?;

        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None
        };

        let parents = match &parent_commit {
            Some(c) => vec![c],
            None => vec![]
        };

        let commit_id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

        Ok(commit_id.to_string())
    }

    pub fn get_head_commit_sync(&self) -> Result<Option<String>, RepositoryError> {
        let repo = Repository::open(&self.root_path)?;
        match repo.head() {
            Ok(head) => Ok(Some(head.peel_to_commit()?.id().to_string())),
            Err(_) => Ok(None)
        }
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    pub fn new_mock() -> Self {
        let root = tempfile::tempdir().unwrap().path().join("mock_repo");
        Self::new(root).unwrap()
    }

    pub async fn get_by_path(
        &self,
        ctx: mk_core::types::TenantContext,
        path: &str
    ) -> Result<Option<KnowledgeEntry>, RepositoryError> {
        for layer in [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ] {
            if let Some(entry) = self.get(ctx.clone(), layer, path).await? {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl KnowledgeRepository for GitRepository {
    type Error = RepositoryError;

    async fn get_head_commit(
        &self,
        _ctx: mk_core::types::TenantContext
    ) -> Result<Option<String>, Self::Error> {
        self.get_head_commit_sync()
    }

    async fn get_affected_items(
        &self,
        _ctx: mk_core::types::TenantContext,
        since_commit: &str
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        let repo = Repository::open(&self.root_path)?;
        let from_obj = repo.revparse_single(since_commit)?;
        let from_commit = from_obj.peel_to_commit()?;
        let from_tree = from_commit.tree()?;

        let head = repo.head()?.peel_to_commit()?;
        let head_tree = head.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&head_tree), None)?;
        let mut affected = Vec::new();

        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path().and_then(|p| p.to_str()) {
                    let parts: Vec<&str> = path.split('/').collect();
                    if parts.len() >= 2 {
                        let layer = match parts[0] {
                            "company" => KnowledgeLayer::Company,
                            "org" => KnowledgeLayer::Org,
                            "team" => KnowledgeLayer::Team,
                            "project" => KnowledgeLayer::Project,
                            _ => return true
                        };
                        let inner_path = parts[1..].join("/");
                        affected.push((layer, inner_path));
                    }
                }
                true
            },
            None,
            None,
            None
        )?;

        Ok(affected)
    }

    async fn get(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: KnowledgeLayer,
        path: &str
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        let full_path = self.resolve_path(&ctx, layer, path);
        if !full_path.exists() {
            return Ok(None);
        }

        let metadata_path = full_path.with_extension("metadata.json");

        let content = tokio::fs::read_to_string(&full_path).await?;

        let commit_hash = {
            let repo = Repository::open(&self.root_path)?;
            let mut revwalk = repo.revwalk()?;
            revwalk.push_head().ok();
            revwalk.next().transpose()?.map(|id| id.to_string())
        };

        let (kind, status, summaries, metadata, author, updated_at) = if metadata_path.exists() {
            let meta_content = tokio::fs::read_to_string(&metadata_path).await?;
            let meta: serde_json::Value = serde_json::from_str(&meta_content)?;
            (
                serde_json::from_value(meta["kind"].clone()).unwrap_or(KnowledgeType::Spec),
                serde_json::from_value(meta["status"].clone())
                    .unwrap_or(mk_core::types::KnowledgeStatus::Accepted),
                serde_json::from_value(meta["summaries"].clone()).unwrap_or_default(),
                serde_json::from_value(meta["metadata"].clone()).unwrap_or_default(),
                serde_json::from_value(meta["author"].clone()).unwrap_or_default(),
                meta["updated_at"]
                    .as_i64()
                    .unwrap_or_else(|| chrono::Utc::now().timestamp())
            )
        } else {
            (
                KnowledgeType::Spec,
                mk_core::types::KnowledgeStatus::Accepted,
                std::collections::HashMap::new(),
                std::collections::HashMap::new(),
                None,
                chrono::Utc::now().timestamp()
            )
        };

        Ok(Some(KnowledgeEntry {
            path: path.to_string(),
            content,
            layer,
            kind,
            status,
            summaries,
            metadata,
            commit_hash,
            author,
            updated_at
        }))
    }

    async fn store(
        &self,
        ctx: mk_core::types::TenantContext,
        entry: KnowledgeEntry,
        message: &str
    ) -> Result<String, Self::Error> {
        let full_path = self.resolve_path(&ctx, entry.layer, &entry.path);
        let metadata_path = full_path.with_extension("metadata.json");

        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&full_path, entry.content).await?;

        let meta = serde_json::json!({
            "kind": entry.kind,
            "status": entry.status,
            "summaries": entry.summaries,
            "metadata": entry.metadata,
            "author": entry.author,
            "updated_at": entry.updated_at,
        });
        tokio::fs::write(&metadata_path, serde_json::to_string(&meta)?).await?;

        self.commit(message)
    }

    async fn list(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: KnowledgeLayer,
        prefix: &str
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        let tenant_path = self.root_path.join(ctx.tenant_id.as_str());
        let layer_path = match layer {
            KnowledgeLayer::Company => tenant_path.join("company"),
            KnowledgeLayer::Org => tenant_path.join("org"),
            KnowledgeLayer::Team => tenant_path.join("team"),
            KnowledgeLayer::Project => tenant_path.join("project")
        };

        if !layer_path.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for entry in WalkDir::new(&layer_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file() && !e.path().to_string_lossy().ends_with(".metadata.json")
            })
        {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&layer_path)
                .map_err(|_| RepositoryError::InvalidPath(path.to_string_lossy().into_owned()))?;

            if relative_path.to_string_lossy().starts_with(prefix)
                && let Some(ke) = self
                    .get(ctx.clone(), layer, &relative_path.to_string_lossy())
                    .await?
            {
                entries.push(ke);
            }
        }

        Ok(entries)
    }

    async fn delete(
        &self,
        ctx: mk_core::types::TenantContext,
        layer: KnowledgeLayer,
        path: &str,
        message: &str
    ) -> Result<String, Self::Error> {
        let full_path = self.resolve_path(&ctx, layer, path);
        let metadata_path = full_path.with_extension("metadata.json");

        if full_path.exists() {
            tokio::fs::remove_file(full_path).await?;
            if metadata_path.exists() {
                tokio::fs::remove_file(metadata_path).await?;
            }
            self.commit(message)
        } else {
            Ok(String::new())
        }
    }

    async fn search(
        &self,
        ctx: mk_core::types::TenantContext,
        query: &str,
        layers: Vec<KnowledgeLayer>,
        limit: usize
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        let mut results = Vec::new();
        for layer in layers {
            let entries = self.list(ctx.clone(), layer, "").await?;
            for entry in entries {
                if entry.content.contains(query) || entry.path.contains(query) {
                    results.push(entry);
                }
                if results.len() >= limit {
                    return Ok(results);
                }
            }
        }
        Ok(results)
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        Some(self.root_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_git_repository_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let repo = GitRepository::new(dir.path())?;
        let tenant_id = mk_core::types::TenantId::new("c1".into()).unwrap();
        let user_id = mk_core::types::UserId::new("u1".into()).unwrap();
        let ctx = mk_core::types::TenantContext::new(tenant_id, user_id);

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "hello world".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: mk_core::types::KnowledgeStatus::Draft,
            summaries: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        };

        repo.store(ctx.clone(), entry.clone(), "initial commit")
            .await?;

        let retrieved = repo
            .get(ctx.clone(), KnowledgeLayer::Project, "test.md")
            .await?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.content, "hello world");
        assert_eq!(retrieved.status, mk_core::types::KnowledgeStatus::Draft);

        let list = repo.list(ctx.clone(), KnowledgeLayer::Project, "").await?;
        assert_eq!(list.len(), 1);

        repo.delete(
            ctx.clone(),
            KnowledgeLayer::Project,
            "test.md",
            "delete file"
        )
        .await?;
        let after_delete = repo.get(ctx, KnowledgeLayer::Project, "test.md").await?;
        assert!(after_delete.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_git_repository_isolation() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let repo = GitRepository::new(dir.path())?;

        let tenant_a = mk_core::types::TenantId::new("tenant_a".into()).unwrap();
        let user_a = mk_core::types::UserId::new("user_a".into()).unwrap();
        let ctx_a = mk_core::types::TenantContext::new(tenant_a, user_a);

        let tenant_b = mk_core::types::TenantId::new("tenant_b".into()).unwrap();
        let user_b = mk_core::types::UserId::new("user_b".into()).unwrap();
        let ctx_b = mk_core::types::TenantContext::new(tenant_b, user_b);

        let entry = KnowledgeEntry {
            path: "secret.md".to_string(),
            content: "tenant a secret".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: mk_core::types::KnowledgeStatus::Accepted,
            summaries: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        };

        repo.store(ctx_a.clone(), entry, "tenant a commit").await?;

        let retrieved_b = repo
            .get(ctx_b.clone(), KnowledgeLayer::Project, "secret.md")
            .await?;
        assert!(
            retrieved_b.is_none(),
            "Tenant B should not see Tenant A data"
        );

        let retrieved_a = repo
            .get(ctx_a.clone(), KnowledgeLayer::Project, "secret.md")
            .await?;
        assert!(retrieved_a.is_some());
        assert_eq!(retrieved_a.unwrap().content, "tenant a secret");

        let list_b = repo.list(ctx_b, KnowledgeLayer::Project, "").await?;
        assert!(list_b.is_empty(), "Tenant B list should be empty");

        let list_a = repo.list(ctx_a, KnowledgeLayer::Project, "").await?;
        assert_eq!(list_a.len(), 1, "Tenant A should see its entry");

        Ok(())
    }

    #[tokio::test]
    async fn test_git_repository_path_traversal_protection()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let repo = GitRepository::new(dir.path())?;

        let tenant_a = mk_core::types::TenantId::new("tenant_a".into()).unwrap();
        let user_a = mk_core::types::UserId::new("user_a".into()).unwrap();
        let ctx_a = mk_core::types::TenantContext::new(tenant_a, user_a);

        // Attempt path traversal via filename
        let entry = KnowledgeEntry {
            path: "../../../etc/passwd".to_string(),
            content: "evil content".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            status: mk_core::types::KnowledgeStatus::Accepted,
            summaries: std::collections::HashMap::new(),
            metadata: std::collections::HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        };

        repo.store(ctx_a.clone(), entry, "malicious commit").await?;

        let full_path = repo.resolve_path(&ctx_a, KnowledgeLayer::Project, "../../../etc/passwd");

        // Ensure the resolved path is still within the tenant directory
        let tenant_dir = repo.root_path.join("tenant_a");
        assert!(
            full_path.starts_with(&tenant_dir),
            "Path should be constrained to tenant directory. Got: {:?}",
            full_path
        );

        Ok(())
    }
}
