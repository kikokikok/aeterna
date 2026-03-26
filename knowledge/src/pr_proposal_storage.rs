//! PR-backed proposal storage.
//!
//! Replaces `InMemoryKnowledgeProposalStorage` with a storage backend
//! that creates governance branches and PRs via the `GitProvider` trait.

use std::sync::Arc;

use crate::git_provider::{GitProvider, GovernanceBranch, PullRequestInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalInfo {
    pub branch_name: String,
    pub file_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProposalError {
    #[error("Git provider error: {0}")]
    Provider(String),
}

pub struct PrProposalStorage {
    git_provider: Arc<dyn GitProvider>,
    default_branch: String,
}

impl PrProposalStorage {
    pub fn new(git_provider: Arc<dyn GitProvider>, default_branch: String) -> Self {
        Self {
            git_provider,
            default_branch,
        }
    }

    pub async fn propose(
        &self,
        tenant_id: &str,
        layer: &str,
        path: &str,
        content: &str,
        message: &str,
    ) -> Result<ProposalInfo, ProposalError> {
        let slug = format!("{}-{}-{}", tenant_id, layer, path.replace('/', "-"));
        let branch = GovernanceBranch::new("propose", &slug);

        let base_sha = self
            .git_provider
            .get_default_branch_sha()
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;

        self.git_provider
            .create_branch(&branch.name, &base_sha)
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;

        let file_path = format!("{}/{}/{}", tenant_id, layer, path);
        self.git_provider
            .commit_to_branch(&branch.name, &file_path, content.as_bytes(), message)
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;

        Ok(ProposalInfo {
            branch_name: branch.name,
            file_path,
        })
    }

    pub async fn submit(
        &self,
        proposal: &ProposalInfo,
        title: &str,
        description: Option<&str>,
    ) -> Result<PullRequestInfo, ProposalError> {
        let pr = self
            .git_provider
            .create_pull_request(
                title,
                description.unwrap_or(""),
                &proposal.branch_name,
                &self.default_branch,
            )
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;

        Ok(pr)
    }

    pub async fn list_pending(&self) -> Result<Vec<PullRequestInfo>, ProposalError> {
        let prs = self
            .git_provider
            .list_open_prs(Some("governance/"))
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;
        Ok(prs)
    }

    pub async fn get(&self, pr_number: u64) -> Result<Option<PullRequestInfo>, ProposalError> {
        let prs = self
            .git_provider
            .list_open_prs(None)
            .await
            .map_err(|e| ProposalError::Provider(e.to_string()))?;
        Ok(prs.into_iter().find(|pr| pr.number == pr_number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    use crate::git_provider::{GitProviderError, MergeMethod, PullRequestState, WebhookEvent};

    #[derive(Default)]
    struct MockState {
        branches: Vec<(String, String)>,
        commits: Vec<(String, String, Vec<u8>, String)>,
        prs: Vec<PullRequestInfo>,
    }

    struct MockGitProvider {
        default_sha: String,
        state: Mutex<MockState>,
    }

    impl MockGitProvider {
        fn new(default_sha: &str, prs: Vec<PullRequestInfo>) -> Self {
            Self {
                default_sha: default_sha.to_string(),
                state: Mutex::new(MockState {
                    prs,
                    ..MockState::default()
                }),
            }
        }
    }

    #[async_trait]
    impl GitProvider for MockGitProvider {
        async fn create_branch(&self, name: &str, from_sha: &str) -> Result<(), GitProviderError> {
            self.state
                .lock()
                .expect("lock")
                .branches
                .push((name.to_string(), from_sha.to_string()));
            Ok(())
        }

        async fn commit_to_branch(
            &self,
            branch: &str,
            path: &str,
            content: &[u8],
            message: &str,
        ) -> Result<String, GitProviderError> {
            self.state.lock().expect("lock").commits.push((
                branch.to_string(),
                path.to_string(),
                content.to_vec(),
                message.to_string(),
            ));
            Ok("mock-commit-sha".to_string())
        }

        async fn create_pull_request(
            &self,
            title: &str,
            body: &str,
            head: &str,
            base: &str,
        ) -> Result<PullRequestInfo, GitProviderError> {
            let mut state = self.state.lock().expect("lock");
            let number = (state.prs.len() as u64) + 1;
            let pr = PullRequestInfo {
                number,
                title: title.to_string(),
                body: if body.is_empty() {
                    None
                } else {
                    Some(body.to_string())
                },
                head_branch: head.to_string(),
                base_branch: base.to_string(),
                state: PullRequestState::Open,
                html_url: format!("https://example.test/pr/{number}"),
                merged: false,
                merge_commit_sha: None,
            };
            state.prs.push(pr.clone());
            Ok(pr)
        }

        async fn merge_pull_request(
            &self,
            _pr_number: u64,
            _merge_method: MergeMethod,
        ) -> Result<String, GitProviderError> {
            Ok("merged-sha".to_string())
        }

        async fn list_open_prs(
            &self,
            head_prefix: Option<&str>,
        ) -> Result<Vec<PullRequestInfo>, GitProviderError> {
            let state = self.state.lock().expect("lock");
            let prs = state
                .prs
                .iter()
                .filter(|pr| {
                    pr.state == PullRequestState::Open
                        && head_prefix.is_none_or(|prefix| pr.head_branch.starts_with(prefix))
                })
                .cloned()
                .collect();
            Ok(prs)
        }

        async fn parse_webhook(
            &self,
            _event_type: &str,
            _signature: Option<&str>,
            _body: &[u8],
        ) -> Result<WebhookEvent, GitProviderError> {
            Ok(WebhookEvent::Unknown {
                event_type: "mock".to_string(),
            })
        }

        async fn get_default_branch_sha(&self) -> Result<String, GitProviderError> {
            Ok(self.default_sha.clone())
        }
    }

    fn pr(number: u64, head: &str) -> PullRequestInfo {
        PullRequestInfo {
            number,
            title: format!("PR {number}"),
            body: None,
            head_branch: head.to_string(),
            base_branch: "main".to_string(),
            state: PullRequestState::Open,
            html_url: format!("https://example.test/pr/{number}"),
            merged: false,
            merge_commit_sha: None,
        }
    }

    #[tokio::test]
    async fn propose_creates_branch_and_commit() {
        let provider = Arc::new(MockGitProvider::new("base-sha", vec![]));
        let storage = PrProposalStorage::new(provider.clone(), "main".to_string());

        let info = storage
            .propose(
                "tenant-a",
                "team",
                "patterns/auth.md",
                "# content",
                "propose auth pattern",
            )
            .await
            .expect("propose should succeed");

        assert!(
            info.branch_name
                .starts_with("governance/propose-tenant-a-team-patterns-auth-md-")
        );
        assert_eq!(info.file_path, "tenant-a/team/patterns/auth.md");

        let state = provider.state.lock().expect("lock");
        assert_eq!(state.branches.len(), 1);
        assert_eq!(state.branches[0].1, "base-sha");
        assert_eq!(state.commits.len(), 1);
        assert_eq!(state.commits[0].1, "tenant-a/team/patterns/auth.md");
        assert_eq!(state.commits[0].3, "propose auth pattern");
    }

    #[tokio::test]
    async fn submit_opens_pr_against_default_branch() {
        let provider = Arc::new(MockGitProvider::new("base-sha", vec![]));
        let storage = PrProposalStorage::new(provider, "main".to_string());
        let proposal = ProposalInfo {
            branch_name: "governance/propose-x".to_string(),
            file_path: "tenant/layer/path.md".to_string(),
        };

        let pr = storage
            .submit(&proposal, "My PR", Some("details"))
            .await
            .expect("submit should succeed");

        assert_eq!(pr.title, "My PR");
        assert_eq!(pr.head_branch, "governance/propose-x");
        assert_eq!(pr.base_branch, "main");
        assert_eq!(pr.body.as_deref(), Some("details"));
    }

    #[tokio::test]
    async fn list_pending_filters_governance_prefix() {
        let provider = Arc::new(MockGitProvider::new(
            "base-sha",
            vec![pr(1, "governance/a"), pr(2, "feature/x")],
        ));
        let storage = PrProposalStorage::new(provider, "main".to_string());

        let pending = storage.list_pending().await.expect("list should succeed");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].number, 1);
    }

    #[tokio::test]
    async fn get_returns_matching_open_pr() {
        let provider = Arc::new(MockGitProvider::new(
            "base-sha",
            vec![pr(11, "governance/one"), pr(12, "governance/two")],
        ));
        let storage = PrProposalStorage::new(provider, "main".to_string());

        let found = storage.get(12).await.expect("get should succeed");
        assert!(found.is_some());
        assert_eq!(found.expect("exists").number, 12);

        let missing = storage.get(99).await.expect("get should succeed");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn submit_uses_empty_body_when_description_missing() {
        let provider = Arc::new(MockGitProvider::new("base-sha", vec![]));
        let storage = PrProposalStorage::new(provider, "main".to_string());
        let proposal = ProposalInfo {
            branch_name: "governance/propose-x".to_string(),
            file_path: "tenant/layer/path.md".to_string(),
        };

        let pr = storage
            .submit(&proposal, "My PR", None)
            .await
            .expect("submit should succeed");
        assert!(pr.body.is_none());
    }
}
