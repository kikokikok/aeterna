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

    fn resolve_path(&self, layer: KnowledgeLayer, path: &str) -> PathBuf {
        let layer_dir = match layer {
            KnowledgeLayer::Company => "company",
            KnowledgeLayer::Org => "org",
            KnowledgeLayer::Team => "team",
            KnowledgeLayer::Project => "project"
        };
        self.root_path.join(layer_dir).join(path)
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

    pub async fn get_by_path(&self, path: &str) -> Result<Option<KnowledgeEntry>, RepositoryError> {
        for layer in [
            KnowledgeLayer::Company,
            KnowledgeLayer::Org,
            KnowledgeLayer::Team,
            KnowledgeLayer::Project
        ] {
            if let Some(entry) = self.get(layer, path).await? {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl KnowledgeRepository for GitRepository {
    type Error = RepositoryError;

    async fn get_head_commit(&self) -> Result<Option<String>, Self::Error> {
        self.get_head_commit_sync()
    }

    async fn get_affected_items(
        &self,
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

    #[tracing::instrument(skip(self), fields(path = %path, layer = ?layer))]
    async fn get(
        &self,
        layer: KnowledgeLayer,
        path: &str
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        let full_path = self.resolve_path(layer, path);
        if !full_path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        let repo = Repository::open(&self.root_path)?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push_head().ok();

        let commit_hash = revwalk.next().transpose()?.map(|id| id.to_string());

        Ok(Some(KnowledgeEntry {
            path: path.to_string(),
            content,
            layer,
            kind: KnowledgeType::Spec,
            metadata: std::collections::HashMap::new(),
            commit_hash,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        }))
    }

    #[tracing::instrument(skip(self, entry), fields(path = %entry.path, layer = ?entry.layer))]
    async fn store(&self, entry: KnowledgeEntry, message: &str) -> Result<String, Self::Error> {
        let full_path = self.resolve_path(entry.layer, &entry.path);
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&full_path, entry.content).await?;
        self.commit(message)
    }

    async fn list(
        &self,
        layer: KnowledgeLayer,
        prefix: &str
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        let layer_path = match layer {
            KnowledgeLayer::Company => self.root_path.join("company"),
            KnowledgeLayer::Org => self.root_path.join("org"),
            KnowledgeLayer::Team => self.root_path.join("team"),
            KnowledgeLayer::Project => self.root_path.join("project")
        };

        if !layer_path.exists() {
            return Ok(vec![]);
        }

        let mut entries = Vec::new();
        for entry in WalkDir::new(&layer_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&layer_path)
                .map_err(|_| RepositoryError::InvalidPath(path.to_string_lossy().into_owned()))?;

            if relative_path.to_string_lossy().starts_with(prefix)
                && let Some(ke) = self.get(layer, &relative_path.to_string_lossy()).await?
            {
                entries.push(ke);
            }
        }

        Ok(entries)
    }

    async fn delete(
        &self,
        layer: KnowledgeLayer,
        path: &str,
        message: &str
    ) -> Result<String, Self::Error> {
        let full_path = self.resolve_path(layer, path);
        if full_path.exists() {
            tokio::fs::remove_file(full_path).await?;
            self.commit(message)
        } else {
            Ok(String::new())
        }
    }

    async fn search(
        &self,
        query: &str,
        layers: Vec<KnowledgeLayer>,
        limit: usize
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        let mut results = Vec::new();
        for layer in layers {
            let entries = self.list(layer, "").await?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_git_repository_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let repo = GitRepository::new(dir.path())?;

        let entry = KnowledgeEntry {
            path: "test.md".to_string(),
            content: "hello world".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Spec,
            metadata: std::collections::HashMap::new(),
            commit_hash: None,
            author: None,
            updated_at: chrono::Utc::now().timestamp()
        };

        repo.store(entry.clone(), "initial commit").await?;

        let retrieved = repo.get(KnowledgeLayer::Project, "test.md").await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "hello world");

        let list = repo.list(KnowledgeLayer::Project, "").await?;
        assert_eq!(list.len(), 1);

        repo.delete(KnowledgeLayer::Project, "test.md", "delete file")
            .await?;
        let after_delete = repo.get(KnowledgeLayer::Project, "test.md").await?;
        assert!(after_delete.is_none());

        Ok(())
    }
}
