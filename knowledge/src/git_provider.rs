use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::Mutex;

type HmacSha256 = Hmac<Sha256>;

/// Cached installation access token with expiry.
struct CachedToken {
    token: String,
    /// Tokens expire after 1 hour; we refresh 5 minutes early.
    expires_at: std::time::Instant,
}

/// Credentials needed to mint installation tokens via GitHub App JWT.
struct AppCredentials {
    app_id: u64,
    installation_id: u64,
    pem_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeMethod {
    Squash,
    Merge,
    Rebase,
}

#[derive(Debug, Clone)]
pub struct PullRequestInfo {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub head_branch: String,
    pub base_branch: String,
    pub state: PullRequestState,
    pub html_url: String,
    pub merged: bool,
    pub merge_commit_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
}

#[derive(Debug, Clone)]
pub enum WebhookEvent {
    PullRequestOpened {
        pr: PullRequestInfo,
    },
    PullRequestMerged {
        pr: PullRequestInfo,
        merge_commit_sha: String,
    },
    PullRequestClosed {
        pr: PullRequestInfo,
    },
    Unknown {
        event_type: String,
    },
}

#[derive(Debug, Clone)]
pub struct GovernanceBranch {
    pub name: String,
    pub verb: String,
    pub slug: String,
    pub date: String,
}

impl GovernanceBranch {
    pub fn new(verb: &str, slug: &str) -> Self {
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let sanitized_slug = slug
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let trimmed = sanitized_slug.trim_matches('-');
        let name = format!("governance/{verb}-{trimmed}-{date}");
        Self {
            name,
            verb: verb.to_string(),
            slug: trimmed.to_string(),
            date,
        }
    }

    pub fn with_suffix(verb: &str, slug: &str, suffix: u32) -> Self {
        let mut branch = Self::new(verb, slug);
        if suffix > 0 {
            branch.name = format!("{}-{suffix}", branch.name);
        }
        branch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOperation {
    Create,
    Update,
    StatusChange {
        to: mk_core::types::KnowledgeStatus,
    },
    Promote {
        target_layer: mk_core::types::KnowledgeLayer,
    },
    Delete,
}

pub fn requires_governance(
    layer: mk_core::types::KnowledgeLayer,
    status: mk_core::types::KnowledgeStatus,
    operation: &WriteOperation,
) -> bool {
    match operation {
        WriteOperation::Create | WriteOperation::Update => {
            layer != mk_core::types::KnowledgeLayer::Project
                || status != mk_core::types::KnowledgeStatus::Draft
        }
        WriteOperation::StatusChange { to } => *to != mk_core::types::KnowledgeStatus::Draft,
        WriteOperation::Promote { .. } => true,
        WriteOperation::Delete => {
            layer != mk_core::types::KnowledgeLayer::Project
                || status != mk_core::types::KnowledgeStatus::Draft
        }
    }
}

#[async_trait]
pub trait GitProvider: Send + Sync {
    async fn create_branch(&self, name: &str, from_sha: &str) -> Result<(), GitProviderError>;

    async fn commit_to_branch(
        &self,
        branch: &str,
        path: &str,
        content: &[u8],
        message: &str,
    ) -> Result<String, GitProviderError>;

    async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequestInfo, GitProviderError>;

    async fn merge_pull_request(
        &self,
        pr_number: u64,
        merge_method: MergeMethod,
    ) -> Result<String, GitProviderError>;

    async fn list_open_prs(
        &self,
        head_prefix: Option<&str>,
    ) -> Result<Vec<PullRequestInfo>, GitProviderError>;

    async fn parse_webhook(
        &self,
        event_type: &str,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<WebhookEvent, GitProviderError>;

    async fn get_default_branch_sha(&self) -> Result<String, GitProviderError>;

    async fn get_installation_token(&self) -> Result<String, GitProviderError>;
}

#[derive(Debug, thiserror::Error)]
pub enum GitProviderError {
    #[error("GitHub API error: {0}")]
    Api(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Webhook signature validation failed")]
    InvalidSignature,
    #[error("Unsupported webhook event: {0}")]
    UnsupportedEvent(String),
    #[error("Branch already exists: {0}")]
    BranchExists(String),
    #[error("PR not found: {0}")]
    PrNotFound(u64),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub struct GitHubProvider {
    client: Arc<octocrab::Octocrab>,
    owner: String,
    repo: String,
    webhook_secret: Option<String>,
    app_credentials: Option<AppCredentials>,
    token_cache: Arc<Mutex<Option<CachedToken>>>,
}

impl GitHubProvider {
    pub fn new(
        token: &str,
        owner: String,
        repo: String,
        webhook_secret: Option<String>,
    ) -> Result<Self, GitProviderError> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let client = octocrab::Octocrab::builder()
            .personal_token(token.to_string())
            .build()
            .map_err(|e| GitProviderError::Auth(e.to_string()))?;
        let token_cache = Arc::new(Mutex::new(Some(CachedToken {
            token: token.to_string(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(86400 * 365),
        })));
        Ok(Self {
            client: Arc::new(client),
            owner,
            repo,
            webhook_secret,
            app_credentials: None,
            token_cache,
        })
    }

    pub fn new_with_app(
        app_id: u64,
        installation_id: u64,
        pem_key: &str,
        owner: String,
        repo: String,
        webhook_secret: Option<String>,
    ) -> Result<Self, GitProviderError> {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(pem_key.as_bytes())
            .map_err(|e| GitProviderError::Auth(format!("Invalid PEM key: {e}")))?;

        let app_client = octocrab::Octocrab::builder()
            .app(app_id.into(), key)
            .build()
            .map_err(|e| GitProviderError::Auth(e.to_string()))?;

        let installation_client = app_client
            .installation(octocrab::models::InstallationId(installation_id))
            .map_err(|e| GitProviderError::Auth(e.to_string()))?;

        Ok(Self {
            client: Arc::new(installation_client),
            owner,
            repo,
            webhook_secret,
            app_credentials: Some(AppCredentials {
                app_id,
                installation_id,
                pem_key: pem_key.to_string(),
            }),
            token_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn find_existing_pr(
        &self,
        head_branch: &str,
    ) -> Result<Option<PullRequestInfo>, GitProviderError> {
        let prs = self.list_open_prs(Some(head_branch)).await?;
        Ok(prs.into_iter().find(|pr| pr.head_branch == head_branch))
    }

    fn validate_hmac(&self, signature: &str, body: &[u8]) -> Result<(), GitProviderError> {
        let secret = self
            .webhook_secret
            .as_ref()
            .ok_or_else(|| GitProviderError::Auth("No webhook secret configured".to_string()))?;

        let hex_sig = signature
            .strip_prefix("sha256=")
            .ok_or(GitProviderError::InvalidSignature)?;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| GitProviderError::Auth(e.to_string()))?;
        mac.update(body);

        let expected = hex::decode(hex_sig).map_err(|_| GitProviderError::InvalidSignature)?;
        mac.verify_slice(&expected)
            .map_err(|_| GitProviderError::InvalidSignature)
    }

    fn pr_from_octocrab(pr: &octocrab::models::pulls::PullRequest) -> PullRequestInfo {
        PullRequestInfo {
            number: pr.number,
            title: pr.title.clone().unwrap_or_default(),
            body: pr.body.clone(),
            head_branch: pr.head.ref_field.clone(),
            base_branch: pr.base.ref_field.clone(),
            state: if pr.state == Some(octocrab::models::IssueState::Open) {
                PullRequestState::Open
            } else {
                PullRequestState::Closed
            },
            html_url: pr
                .html_url
                .as_ref()
                .map_or_else(String::new, |u| u.to_string()),
            merged: pr.merged.unwrap_or(false),
            merge_commit_sha: pr.merge_commit_sha.clone(),
        }
    }
}

#[async_trait]
impl GitProvider for GitHubProvider {
    async fn create_branch(&self, name: &str, from_sha: &str) -> Result<(), GitProviderError> {
        use octocrab::params::repos::Reference;
        self.client
            .repos(&self.owner, &self.repo)
            .create_ref(&Reference::Branch(name.to_string()), from_sha)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("Reference already exists") {
                    GitProviderError::BranchExists(name.to_string())
                } else {
                    GitProviderError::Api(msg)
                }
            })?;
        Ok(())
    }

    async fn commit_to_branch(
        &self,
        branch: &str,
        path: &str,
        content: &[u8],
        message: &str,
    ) -> Result<String, GitProviderError> {
        let existing_sha = self.get_file_sha(branch, path).await?;
        let repos = self.client.repos(&self.owner, &self.repo);

        let file_update = if let Some(sha) = existing_sha {
            repos
                .update_file(path, message, content, sha)
                .branch(branch)
                .send()
                .await
        } else {
            repos
                .create_file(path, message, content)
                .branch(branch)
                .send()
                .await
        }
        .map_err(|e| GitProviderError::Api(e.to_string()))?;

        let commit_sha = file_update.commit.sha.unwrap_or_default();
        Ok(commit_sha)
    }

    async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequestInfo, GitProviderError> {
        let pr = self
            .client
            .pulls(&self.owner, &self.repo)
            .create(title, head, base)
            .body(body)
            .send()
            .await
            .map_err(|e| GitProviderError::Api(e.to_string()))?;
        Ok(Self::pr_from_octocrab(&pr))
    }

    async fn merge_pull_request(
        &self,
        pr_number: u64,
        merge_method: MergeMethod,
    ) -> Result<String, GitProviderError> {
        let method = match merge_method {
            MergeMethod::Squash => octocrab::params::pulls::MergeMethod::Squash,
            MergeMethod::Merge => octocrab::params::pulls::MergeMethod::Merge,
            MergeMethod::Rebase => octocrab::params::pulls::MergeMethod::Rebase,
        };
        let result = self
            .client
            .pulls(&self.owner, &self.repo)
            .merge(pr_number)
            .method(method)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("404") {
                    GitProviderError::PrNotFound(pr_number)
                } else {
                    GitProviderError::Api(msg)
                }
            })?;
        Ok(result.sha.unwrap_or_default())
    }

    async fn list_open_prs(
        &self,
        head_prefix: Option<&str>,
    ) -> Result<Vec<PullRequestInfo>, GitProviderError> {
        let mut page = self
            .client
            .pulls(&self.owner, &self.repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .map_err(|e| GitProviderError::Api(e.to_string()))?;

        let mut results: Vec<PullRequestInfo> = Vec::new();
        loop {
            for pr in &page.items {
                let info = Self::pr_from_octocrab(pr);
                if let Some(prefix) = head_prefix {
                    if info.head_branch.starts_with(prefix) {
                        results.push(info);
                    }
                } else {
                    results.push(info);
                }
            }
            match self
                .client
                .get_page::<octocrab::models::pulls::PullRequest>(&page.next)
                .await
                .map_err(|e| GitProviderError::Api(e.to_string()))?
            {
                Some(next_page) => page = next_page,
                None => break,
            }
        }
        Ok(results)
    }

    async fn parse_webhook(
        &self,
        event_type: &str,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<WebhookEvent, GitProviderError> {
        if let Some(sig) = signature {
            self.validate_hmac(sig, body)?;
        } else if self.webhook_secret.is_some() {
            return Err(GitProviderError::InvalidSignature);
        }

        if event_type != "pull_request" {
            return Ok(WebhookEvent::Unknown {
                event_type: event_type.to_string(),
            });
        }

        let payload: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| GitProviderError::Serialization(e.to_string()))?;

        let action = payload["action"].as_str().unwrap_or_default();
        let pr_obj = &payload["pull_request"];

        let pr_info = PullRequestInfo {
            number: pr_obj["number"].as_u64().unwrap_or_default(),
            title: pr_obj["title"].as_str().unwrap_or_default().to_string(),
            body: pr_obj["body"].as_str().map(String::from),
            head_branch: pr_obj["head"]["ref"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            base_branch: pr_obj["base"]["ref"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            state: if pr_obj["state"].as_str() == Some("open") {
                PullRequestState::Open
            } else {
                PullRequestState::Closed
            },
            html_url: pr_obj["html_url"].as_str().unwrap_or_default().to_string(),
            merged: pr_obj["merged"].as_bool().unwrap_or(false),
            merge_commit_sha: pr_obj["merge_commit_sha"].as_str().map(String::from),
        };

        match action {
            "opened" | "reopened" => Ok(WebhookEvent::PullRequestOpened { pr: pr_info }),
            "closed" => {
                if pr_info.merged {
                    let merge_sha = pr_info.merge_commit_sha.clone().unwrap_or_default();
                    Ok(WebhookEvent::PullRequestMerged {
                        pr: pr_info,
                        merge_commit_sha: merge_sha,
                    })
                } else {
                    Ok(WebhookEvent::PullRequestClosed { pr: pr_info })
                }
            }
            _ => Ok(WebhookEvent::Unknown {
                event_type: format!("pull_request.{action}"),
            }),
        }
    }

    async fn get_default_branch_sha(&self) -> Result<String, GitProviderError> {
        use octocrab::params::repos::Reference;

        let repo = self
            .client
            .repos(&self.owner, &self.repo)
            .get()
            .await
            .map_err(|e| GitProviderError::Api(e.to_string()))?;

        let default_branch = repo.default_branch.as_deref().unwrap_or("main");

        let git_ref = self
            .client
            .repos(&self.owner, &self.repo)
            .get_ref(&Reference::Branch(default_branch.to_string()))
            .await
            .map_err(|e| GitProviderError::Api(e.to_string()))?;

        let sha = match git_ref.object {
            octocrab::models::repos::Object::Commit { sha, .. }
            | octocrab::models::repos::Object::Tag { sha, .. } => sha,
            other => {
                return Err(GitProviderError::Api(format!(
                    "Unexpected ref object type: {other:?}"
                )));
            }
        };
        Ok(sha)
    }

    async fn get_installation_token(&self) -> Result<String, GitProviderError> {
        {
            let cache = self.token_cache.lock().await;
            if let Some(ref cached) = *cache {
                if cached.expires_at > std::time::Instant::now() {
                    return Ok(cached.token.clone());
                }
            }
        }

        let creds = self
            .app_credentials
            .as_ref()
            .ok_or_else(|| GitProviderError::Auth(
                "No App credentials configured; cannot mint installation token (PAT auth does not support token extraction)".to_string(),
            ))?;

        let now = chrono::Utc::now();
        let claims = serde_json::json!({
            "iat": (now - chrono::Duration::seconds(60)).timestamp(),
            "exp": (now + chrono::Duration::seconds(600)).timestamp(),
            "iss": creds.app_id.to_string(),
        });

        let jwt_key = jsonwebtoken::EncodingKey::from_rsa_pem(creds.pem_key.as_bytes())
            .map_err(|e| GitProviderError::Auth(format!("PEM encode error: {e}")))?;
        let jwt = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &jwt_key,
        )
        .map_err(|e| GitProviderError::Auth(format!("JWT sign error: {e}")))?;

        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let http_client = octocrab::Octocrab::builder()
            .personal_token(jwt)
            .build()
            .map_err(|e| GitProviderError::Auth(e.to_string()))?;

        let url = format!("/app/installations/{}/access_tokens", creds.installation_id);
        let resp: serde_json::Value = http_client
            .post(url, None::<&()>)
            .await
            .map_err(|e| GitProviderError::Auth(format!("Token exchange failed: {e}")))?;

        let token = resp["token"]
            .as_str()
            .ok_or_else(|| GitProviderError::Auth("No token in response".to_string()))?
            .to_string();

        let mut cache = self.token_cache.lock().await;
        *cache = Some(CachedToken {
            token: token.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(55 * 60),
        });

        Ok(token)
    }
}

impl GitHubProvider {
    async fn get_file_sha(
        &self,
        branch: &str,
        path: &str,
    ) -> Result<Option<String>, GitProviderError> {
        let url = format!(
            "/repos/{}/{}/contents/{path}?ref={branch}",
            self.owner, self.repo
        );
        match self
            .client
            .get::<serde_json::Value, _, _>(url, None::<&()>)
            .await
        {
            Ok(resp) => Ok(resp["sha"].as_str().map(String::from)),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("Not Found") {
                    Ok(None)
                } else {
                    Err(GitProviderError::Api(msg))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initialize rustls CryptoProvider for tests that create Octocrab::default().
    /// Octocrab builds a reqwest Client that initializes rustls, which panics
    /// without a CryptoProvider installed. Safe to call multiple times.
    fn init_crypto() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    fn test_provider(webhook_secret: Option<String>) -> GitHubProvider {
        init_crypto();
        GitHubProvider {
            client: Arc::new(octocrab::Octocrab::default()),
            owner: "test".to_string(),
            repo: "test".to_string(),
            webhook_secret,
            app_credentials: None,
            token_cache: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn test_governance_branch_naming() {
        let branch = GovernanceBranch::new("promote", "api-auth-pattern");
        assert!(
            branch
                .name
                .starts_with("governance/promote-api-auth-pattern-")
        );
        assert_eq!(branch.verb, "promote");
        assert_eq!(branch.slug, "api-auth-pattern");
        assert_eq!(branch.date.len(), 8);
    }

    #[test]
    fn test_governance_branch_sanitization() {
        let branch = GovernanceBranch::new("create", "Team Coding Standards!");
        assert!(
            branch
                .name
                .starts_with("governance/create-team-coding-standards-")
        );
        assert!(!branch.slug.contains(' '));
        assert!(!branch.slug.contains('!'));
    }

    #[test]
    fn test_governance_branch_with_suffix() {
        let branch = GovernanceBranch::with_suffix("accept", "baseline", 0);
        assert!(!branch.name.ends_with("-0"));
        let branch = GovernanceBranch::with_suffix("accept", "baseline", 2);
        assert!(branch.name.ends_with("-2"));
    }

    #[tokio::test]
    async fn test_hmac_validation() {
        let provider = test_provider(Some("mysecret".to_string()));

        let body = b"hello world";
        let mut mac = HmacSha256::new_from_slice(b"mysecret").unwrap();
        mac.update(body);
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());
        let signature = format!("sha256={hex_sig}");

        assert!(provider.validate_hmac(&signature, body).is_ok());
    }

    #[tokio::test]
    async fn test_hmac_validation_invalid() {
        let provider = test_provider(Some("mysecret".to_string()));

        assert!(
            provider
                .validate_hmac("sha256=deadbeef00112233", b"hello world")
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_hmac_validation_missing_prefix() {
        let provider = test_provider(Some("mysecret".to_string()));

        assert!(provider.validate_hmac("badprefix=abc", b"hello").is_err());
    }

    #[test]
    fn test_requires_governance_project_draft_create() {
        assert!(!requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::Create,
        ));
    }

    #[test]
    fn test_requires_governance_project_accepted_create() {
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Accepted,
            &WriteOperation::Create,
        ));
    }

    #[test]
    fn test_requires_governance_team_draft_create() {
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Team,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::Create,
        ));
    }

    #[test]
    fn test_requires_governance_status_change_to_accepted() {
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::StatusChange {
                to: mk_core::types::KnowledgeStatus::Accepted,
            },
        ));
    }

    #[test]
    fn test_requires_governance_status_change_to_draft() {
        assert!(!requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::StatusChange {
                to: mk_core::types::KnowledgeStatus::Draft,
            },
        ));
    }

    #[test]
    fn test_requires_governance_promote_always() {
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::Promote {
                target_layer: mk_core::types::KnowledgeLayer::Team,
            },
        ));
    }

    #[test]
    fn test_requires_governance_delete_project_draft_fast_track() {
        // Project/Draft deletes use fast-track (same as create/update)
        assert!(!requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::Delete,
        ));
    }

    #[test]
    fn test_requires_governance_delete_team_governance_track() {
        // Non-Project deletes always go through governance
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Team,
            mk_core::types::KnowledgeStatus::Draft,
            &WriteOperation::Delete,
        ));
    }

    #[test]
    fn test_requires_governance_delete_project_accepted_governance_track() {
        // Non-Draft deletes always go through governance
        assert!(requires_governance(
            mk_core::types::KnowledgeLayer::Project,
            mk_core::types::KnowledgeStatus::Accepted,
            &WriteOperation::Delete,
        ));
    }

    #[tokio::test]
    async fn test_parse_webhook_pr_opened() {
        let provider = test_provider(None);

        let payload = serde_json::json!({
            "action": "opened",
            "pull_request": {
                "number": 42,
                "title": "governance/promote-api-auth",
                "body": "Promoting API auth pattern to Team layer",
                "head": { "ref": "governance/promote-api-auth-20260326" },
                "base": { "ref": "main" },
                "state": "open",
                "html_url": "https://github.com/test/test/pull/42",
                "merged": false,
                "merge_commit_sha": null
            }
        });

        let body = serde_json::to_vec(&payload).unwrap();
        let event = provider
            .parse_webhook("pull_request", None, &body)
            .await
            .unwrap();

        match event {
            WebhookEvent::PullRequestOpened { pr } => {
                assert_eq!(pr.number, 42);
                assert_eq!(pr.head_branch, "governance/promote-api-auth-20260326");
            }
            _ => panic!("Expected PullRequestOpened"),
        }
    }

    #[tokio::test]
    async fn test_parse_webhook_pr_merged() {
        let provider = test_provider(None);

        let payload = serde_json::json!({
            "action": "closed",
            "pull_request": {
                "number": 42,
                "title": "governance/accept-security-baseline",
                "body": null,
                "head": { "ref": "governance/accept-security-baseline-20260326" },
                "base": { "ref": "main" },
                "state": "closed",
                "html_url": "https://github.com/test/test/pull/42",
                "merged": true,
                "merge_commit_sha": "abc123def456"
            }
        });

        let body = serde_json::to_vec(&payload).unwrap();
        let event = provider
            .parse_webhook("pull_request", None, &body)
            .await
            .unwrap();

        match event {
            WebhookEvent::PullRequestMerged {
                pr,
                merge_commit_sha,
            } => {
                assert_eq!(pr.number, 42);
                assert!(pr.merged);
                assert_eq!(merge_commit_sha, "abc123def456");
            }
            _ => panic!("Expected PullRequestMerged"),
        }
    }

    #[tokio::test]
    async fn test_parse_webhook_pr_closed_not_merged() {
        let provider = test_provider(None);

        let payload = serde_json::json!({
            "action": "closed",
            "pull_request": {
                "number": 43,
                "title": "governance/create-team-standards",
                "body": null,
                "head": { "ref": "governance/create-team-standards-20260326" },
                "base": { "ref": "main" },
                "state": "closed",
                "html_url": "https://github.com/test/test/pull/43",
                "merged": false,
                "merge_commit_sha": null
            }
        });

        let body = serde_json::to_vec(&payload).unwrap();
        let event = provider
            .parse_webhook("pull_request", None, &body)
            .await
            .unwrap();

        match event {
            WebhookEvent::PullRequestClosed { pr } => {
                assert_eq!(pr.number, 43);
                assert!(!pr.merged);
            }
            _ => panic!("Expected PullRequestClosed"),
        }
    }

    #[tokio::test]
    async fn test_parse_webhook_unknown_event() {
        let provider = test_provider(None);

        let event = provider.parse_webhook("push", None, b"{}").await.unwrap();

        match event {
            WebhookEvent::Unknown { event_type } => {
                assert_eq!(event_type, "push");
            }
            _ => panic!("Expected Unknown"),
        }
    }
}
