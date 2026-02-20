use chrono::{DateTime, Utc};
use errors::CodeSearchError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum RepositoryType {
    Local,
    Remote,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum SyncStrategy {
    Hook,
    Job,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum RepositoryStatus {
    Requested,
    Pending,
    Approved,
    Cloning,
    Indexing,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum RepoRequestStatus {
    Requested,
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Identity {
    pub id: Uuid,
    pub tenant_id: String,
    pub name: String,
    pub provider: String,
    pub auth_type: String,
    pub secret_id: String,
    pub secret_provider: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub branch: String,
    pub search_count: i32,
    pub trace_count: i32,
    pub last_active_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CleanupLog {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub repository_name: String,
    pub branch: Option<String>,
    pub reason: String,
    pub action_taken: String,
    pub performed_by: Option<String>,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Repository {
    pub id: Uuid,
    pub tenant_id: String,
    pub identity_id: Option<Uuid>,
    pub name: String,
    pub r#type: RepositoryType,
    pub remote_url: Option<String>,
    pub local_path: Option<String>,
    pub current_branch: String,
    pub tracked_branches: Vec<String>,
    pub sync_strategy: SyncStrategy,
    pub sync_interval_mins: Option<i32>,
    pub status: RepositoryStatus,
    pub last_indexed_commit: Option<String>,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub owner_id: Option<String>,
    pub shard_id: Option<String>,
    pub cold_storage_uri: Option<String>,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexMetadata {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub commit_sha: String,
    pub parent_commit_sha: Option<String>,
    pub files_indexed: i32,
    pub files_removed: Option<i32>,
    pub files_renamed: Option<i32>,
    pub indexing_duration_ms: Option<i32>,
    pub embedding_api_calls: Option<i32>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RepoRequest {
    pub id: Uuid,
    pub repository_id: Uuid,
    pub requester_id: String,
    pub status: RepoRequestStatus,
    pub policy_result: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CreateRepository {
    pub tenant_id: String,
    pub identity_id: Option<Uuid>,
    pub name: String,
    pub r#type: RepositoryType,
    pub remote_url: Option<String>,
    pub local_path: Option<String>,
    pub current_branch: Option<String>,
    pub tracked_branches: Option<Vec<String>>,
    pub sync_strategy: Option<SyncStrategy>,
    pub sync_interval_mins: Option<i32>,
    pub config: Option<serde_json::Value>,
}

pub struct CreateIdentity {
    pub tenant_id: String,
    pub name: String,
    pub provider: String,
    pub auth_type: String,
    pub secret_id: String,
    pub secret_provider: String,
    pub scopes: Option<Vec<String>>,
}

pub struct CreateRequest {
    pub repository_id: Uuid,
    pub requester_id: String,
}

pub struct RepoStorage {
    pool: PgPool,
}

impl RepoStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_repository(
        &self,
        repo: &CreateRepository,
    ) -> Result<Repository, sqlx::Error> {
        let row: Repository = sqlx::query_as(
            r#"
            INSERT INTO codesearch_repositories (
                tenant_id, identity_id, name, type, remote_url, local_path, 
                current_branch, tracked_branches, sync_strategy, 
                sync_interval_mins, config
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(&repo.tenant_id)
        .bind(repo.identity_id)
        .bind(&repo.name)
        .bind(&repo.r#type)
        .bind(&repo.remote_url)
        .bind(&repo.local_path)
        .bind(repo.current_branch.as_deref().unwrap_or("main"))
        .bind(repo.tracked_branches.as_deref().unwrap_or(&vec![]))
        .bind(repo.sync_strategy.as_ref().unwrap_or(&SyncStrategy::Manual))
        .bind(repo.sync_interval_mins.unwrap_or(15))
        .bind(repo.config.as_ref().unwrap_or(&serde_json::json!({})))
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn create_identity(
        &self,
        identity: &CreateIdentity,
    ) -> Result<Identity, sqlx::Error> {
        let row: Identity = sqlx::query_as(
            r#"
            INSERT INTO codesearch_identities (
                tenant_id, name, provider, auth_type, secret_id, secret_provider, scopes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(&identity.tenant_id)
        .bind(&identity.name)
        .bind(&identity.provider)
        .bind(&identity.auth_type)
        .bind(&identity.secret_id)
        .bind(&identity.secret_provider)
        .bind(identity.scopes.as_deref().unwrap_or(&vec![]))
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_identity(&self, id: Uuid) -> Result<Option<Identity>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_identities WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn update_identity(&self, id: Uuid, secret_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE codesearch_identities SET secret_id = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(secret_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_identity(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM codesearch_identities WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_repository(&self, id: Uuid) -> Result<Option<Repository>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_repositories WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get_repository_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<Repository>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_repositories WHERE tenant_id = $1 AND name = $2")
            .bind(tenant_id)
            .bind(name)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get_repository_by_url(
        &self,
        tenant_id: &str,
        url: &str,
    ) -> Result<Option<Repository>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_repositories WHERE tenant_id = $1 AND (remote_url = $2 OR remote_url = $3)")
            .bind(tenant_id)
            .bind(url)
            .bind(format!("{}.git", url))
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_repositories(&self, tenant_id: &str) -> Result<Vec<Repository>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_repositories WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: RepositoryStatus,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE codesearch_repositories SET status = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_repository(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM codesearch_repositories WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_request(&self, req: &CreateRequest) -> Result<RepoRequest, sqlx::Error> {
        let row: RepoRequest = sqlx::query_as(
            r#"
            INSERT INTO codesearch_requests (repository_id, requester_id)
            VALUES ($1, $2)
            RETURNING *
            "#,
        )
        .bind(req.repository_id)
        .bind(&req.requester_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn update_request_status(
        &self,
        id: Uuid,
        status: RepoRequestStatus,
        policy_result: Option<serde_json::Value>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE codesearch_requests SET status = $1, policy_result = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(status)
        .bind(policy_result)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_last_indexed(&self, id: Uuid, commit_sha: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE codesearch_repositories SET last_indexed_commit = $1, last_indexed_at = NOW(), updated_at = NOW() WHERE id = $2"
        )
        .bind(commit_sha)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_usage(
        &self,
        id: Uuid,
        branch: &str,
        is_search: bool,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Update repository last_used_at
        sqlx::query("UPDATE codesearch_repositories SET last_used_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        // Update granular metrics
        let query = if is_search {
            "INSERT INTO codesearch_usage_metrics (repository_id, branch, search_count, last_active_at) 
             VALUES ($1, $2, 1, NOW()) 
             ON CONFLICT (repository_id, branch) DO UPDATE SET search_count = codesearch_usage_metrics.search_count + 1, last_active_at = NOW()"
        } else {
            "INSERT INTO codesearch_usage_metrics (repository_id, branch, trace_count, last_active_at) 
             VALUES ($1, $2, 1, NOW()) 
             ON CONFLICT (repository_id, branch) DO UPDATE SET trace_count = codesearch_usage_metrics.trace_count + 1, last_active_at = NOW()"
        };

        // Note: The ON CONFLICT requires a unique index on (repository_id, branch).
        // I need to add that to the migration.

        sqlx::query(query)
            .bind(id)
            .bind(branch)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_request_by_id(&self, id: Uuid) -> Result<Option<RepoRequest>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM codesearch_requests WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }
}

pub struct RepoManager {
    storage: RepoStorage,
    base_path: std::path::PathBuf,
    secret_provider: Arc<dyn crate::secret_provider::SecretProvider>,
    policy_evaluator: Arc<dyn crate::policy_evaluator::PolicyEvaluator>,
    tracker_sender: Option<tokio::sync::mpsc::Sender<Uuid>>,
    shard_router: Option<Arc<crate::shard_router::ShardRouter>>,
    cold_storage: Option<Arc<crate::shard_router::ColdStorageManager>>,
}

impl RepoManager {
    pub fn new(
        storage: RepoStorage,
        base_path: std::path::PathBuf,
        secret_provider: Arc<dyn crate::secret_provider::SecretProvider>,
        policy_evaluator: Arc<dyn crate::policy_evaluator::PolicyEvaluator>,
    ) -> Self {
        Self {
            storage,
            base_path,
            secret_provider,
            policy_evaluator,
            tracker_sender: None,
            shard_router: None,
            cold_storage: None,
        }
    }

    pub fn with_shard_router(mut self, router: Arc<crate::shard_router::ShardRouter>) -> Self {
        self.shard_router = Some(router);
        self
    }

    pub fn with_cold_storage(
        mut self,
        cold_storage: Arc<crate::shard_router::ColdStorageManager>,
    ) -> Self {
        self.cold_storage = Some(cold_storage);
        self
    }

    pub fn set_tracker_sender(&mut self, sender: tokio::sync::mpsc::Sender<Uuid>) {
        self.tracker_sender = Some(sender);
    }

    pub async fn request_repository(
        &self,
        tenant_id: &str,
        requester_id: &str,
        requester_roles: Vec<String>,
        repo_data: CreateRepository,
    ) -> Result<Repository, CodeSearchError> {
        // Validation
        if self
            .storage
            .get_repository_by_name(tenant_id, &repo_data.name)
            .await
            .is_ok_and(|r| r.is_some())
        {
            return Err(CodeSearchError::DatabaseError {
                reason: "Repository already exists".to_string(),
            });
        }

        // Policy Evaluation
        let policy_ctx = crate::policy_evaluator::PolicyContext {
            principal_id: requester_id.to_string(),
            principal_roles: requester_roles,
            tenant_id: tenant_id.to_string(),
        };

        let temp_repo = Repository {
            id: Uuid::new_v4(),
            tenant_id: tenant_id.to_string(),
            identity_id: repo_data.identity_id,
            name: repo_data.name.clone(),
            r#type: repo_data.r#type.clone(),
            remote_url: repo_data.remote_url.clone(),
            local_path: repo_data.local_path.clone(),
            current_branch: repo_data
                .current_branch
                .clone()
                .unwrap_or_else(|| "main".to_string()),
            tracked_branches: repo_data.tracked_branches.clone().unwrap_or_default(),
            sync_strategy: repo_data
                .sync_strategy
                .clone()
                .unwrap_or(SyncStrategy::Manual),
            sync_interval_mins: repo_data.sync_interval_mins,
            status: RepositoryStatus::Requested,
            last_indexed_commit: None,
            last_indexed_at: None,
            last_used_at: None,
            owner_id: None,
            shard_id: None,
            cold_storage_uri: None,
            config: repo_data
                .config
                .clone()
                .unwrap_or_else(|| serde_json::json!({})),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        if !self
            .policy_evaluator
            .evaluate_request(&policy_ctx, "RequestRepository", &temp_repo)
            .await?
        {
            return Err(CodeSearchError::PolicyViolation {
                policy: "Cedar::RequestRepository".to_string(),
                reason: "Insufficient permissions to request repository indexing".to_string(),
            });
        }

        let mut repo = self
            .storage
            .create_repository(&repo_data)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Approval Logic: Local is auto-approved, others need Request
        let should_auto_approve = match repo.r#type {
            RepositoryType::Local => true,
            _ => false,
        };

        if should_auto_approve {
            self.storage
                .update_status(repo.id, RepositoryStatus::Approved)
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;
            repo.status = RepositoryStatus::Approved;

            // For local, we can move straight to READY if path exists
            if let Some(path) = &repo.local_path {
                if std::path::Path::new(path).exists() {
                    self.storage
                        .update_status(repo.id, RepositoryStatus::Ready)
                        .await
                        .map_err(|e| CodeSearchError::DatabaseError {
                            reason: e.to_string(),
                        })?;
                    repo.status = RepositoryStatus::Ready;
                }
            }
        } else {
            self.storage
                .create_request(&CreateRequest {
                    repository_id: repo.id,
                    requester_id: requester_id.to_string(),
                })
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;
        }

        Ok(repo)
    }

    pub async fn list_repositories(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<Repository>, CodeSearchError> {
        self.storage
            .list_repositories(tenant_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    pub async fn approve_request(
        &self,
        approver_id: &str,
        approver_roles: Vec<String>,
        request_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), CodeSearchError> {
        let request = self
            .storage
            .get_request_by_id(request_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Request not found".to_string(),
            })?;

        if request.status == RepoRequestStatus::Approved {
            return Ok(());
        }

        // Policy Evaluation
        let repo = self
            .storage
            .get_repository(request.repository_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let policy_ctx = crate::policy_evaluator::PolicyContext {
            principal_id: approver_id.to_string(),
            principal_roles: approver_roles,
            tenant_id: repo.tenant_id.clone(),
        };

        if !self
            .policy_evaluator
            .evaluate_approval(&policy_ctx, &request)
            .await?
        {
            return Err(CodeSearchError::PolicyViolation {
                policy: "Cedar::ApproveRepository".to_string(),
                reason: "Insufficient permissions to approve indexing request".to_string(),
            });
        }

        self.storage
            .update_request_status(
                request_id,
                RepoRequestStatus::Approved,
                reason.map(|r| serde_json::json!({"reason": r})),
            )
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Update repository status
        self.storage
            .update_status(request.repository_id, RepositoryStatus::Approved)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Start cloning/indexing in background
        let repo_id = request.repository_id;
        // For now, call it directly
        let this_clone = self.clone_repository(repo_id).await;
        if let Err(e) = this_clone {
            tracing::error!("Failed to clone repository {}: {:?}", repo_id, e);
        }

        Ok(())
    }

    pub async fn clone_repository(&self, id: Uuid) -> Result<(), CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        if repo_data.r#type == RepositoryType::Local {
            return Ok(());
        }

        let tenant_path = self.base_path.join(&repo_data.tenant_id);
        std::fs::create_dir_all(&tenant_path).map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;

        let local_path = tenant_path.join(&repo_data.name);
        let remote_url = repo_data
            .remote_url
            .ok_or_else(|| CodeSearchError::GitError {
                reason: "Remote URL required".to_string(),
            })?;

        // Assign to a shard if router is configured
        if let Some(router) = &self.shard_router {
            let shard_id =
                router
                    .assign_shard(id)
                    .await
                    .map_err(|e| CodeSearchError::DatabaseError {
                        reason: e.to_string(),
                    })?;
            tracing::info!("Assigned repository {} to shard {}", id, shard_id);

            // Check if this is not our local shard - if so, skip cloning here
            if !router.is_local(id).await.unwrap_or(false) {
                tracing::info!(
                    "Repository {} assigned to different shard, clone will happen there",
                    id
                );
                return Ok(());
            }
        }

        // Check for cold storage restore
        if let (Some(cold_storage), Some(uri)) = (&self.cold_storage, &repo_data.cold_storage_uri) {
            tracing::info!("Restoring repository {} from cold storage: {}", id, uri);
            cold_storage
                .restore_repo(uri, &local_path)
                .await
                .map_err(|e| CodeSearchError::GitError {
                    reason: format!("Cold restore failed: {}", e),
                })?;
        } else {
            // Retry logic for cloning
            use tokio_retry::Retry;
            use tokio_retry::strategy::ExponentialBackoff;

            let strategy = ExponentialBackoff::from_millis(100).take(3);

            Retry::spawn(strategy, || async {
                self.do_clone(&remote_url, &local_path).await
            })
            .await?;
        }

        // After clone, update status and trigger initial index
        self.storage
            .update_status(id, RepositoryStatus::Ready)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Trigger indexing
        self.reindex_repository("system", vec!["lead".to_string()], id, None, false)
            .await?;

        Ok(())
    }

    async fn do_clone(
        &self,
        remote_url: &str,
        local_path: &std::path::PathBuf,
    ) -> Result<(), CodeSearchError> {
        tracing::info!("Cloning {} to {:?}", remote_url, local_path);
        let cb = git2::RemoteCallbacks::new();
        // TODO: Credentials from Identity/SecretProvider

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(cb);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        if !local_path.exists()
            || std::fs::read_dir(local_path)
                .map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?
                .next()
                .is_none()
        {
            std::fs::create_dir_all(local_path).ok();
            builder
                .clone(remote_url, local_path)
                .map_err(|e| CodeSearchError::GitError {
                    reason: format!("Clone failed: {}", e),
                })?;
        }
        Ok(())
    }

    /// Fetch updates from remote and return new commit SHA if changed
    pub async fn fetch_updates(&self, id: Uuid) -> Result<Option<String>, CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        if repo_data.r#type == RepositoryType::Local {
            return Ok(None);
        }

        let local_path = self
            .base_path
            .join(repo_data.tenant_id.clone())
            .join(repo_data.name.clone());

        // Use Retry for the network operation
        use tokio_retry::Retry;
        use tokio_retry::strategy::ExponentialBackoff;
        let strategy = ExponentialBackoff::from_millis(100).take(3);

        let new_commit_id: Option<String> = Retry::spawn(strategy, || async {
            self.do_fetch(&local_path, &repo_data.current_branch).await
        })
        .await?;

        if let Some(new_sha) = new_commit_id {
            // Compare with current head
            let repo =
                git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?;
            let current_head = repo.head().ok();
            let current_sha = current_head.and_then(|h| h.target()).map(|t| t.to_string());

            if current_sha.as_deref() != Some(&new_sha) {
                return Ok(Some(new_sha));
            }
        }

        Ok(None)
    }

    async fn do_fetch(
        &self,
        local_path: &std::path::PathBuf,
        branch: &str,
    ) -> Result<Option<String>, CodeSearchError> {
        let repo = git2::Repository::open(local_path).map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;

        let mut fetch_options = git2::FetchOptions::new();
        // TODO: Auth from Identity

        remote
            .fetch(&[branch], Some(&mut fetch_options), None)
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Fetch failed: {}", e),
            })?;

        let fetch_head =
            repo.find_reference("FETCH_HEAD")
                .map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?;
        let commit = fetch_head
            .peel_to_commit()
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;

        Ok(Some(commit.id().to_string()))
    }

    /// Calculate the list of changed files between two commits
    pub async fn get_changed_files(
        &self,
        repo_id: Uuid,
        from_commit: &str,
        to_commit: &str,
    ) -> Result<Vec<String>, CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(repo_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let local_path = self
            .base_path
            .join(repo_data.tenant_id.clone())
            .join(repo_data.name.clone());
        let repo = git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
            reason: format!("Failed to open repo: {}", e),
        })?;

        let from_obj =
            repo.revparse_single(from_commit)
                .map_err(|e| CodeSearchError::GitError {
                    reason: format!("Invalid from_commit {}: {}", from_commit, e),
                })?;
        let to_obj = repo
            .revparse_single(to_commit)
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Invalid to_commit {}: {}", to_commit, e),
            })?;

        let from_tree = from_obj
            .peel_to_tree()
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;
        let to_tree = to_obj
            .peel_to_tree()
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;

        let mut diff_options = git2::DiffOptions::new();
        let diff = repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut diff_options))
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Diff failed: {}", e),
            })?;

        let mut changed_files = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    if let Some(path_str) = path.to_str() {
                        changed_files.push(path_str.to_string());
                    }
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;

        Ok(changed_files)
    }

    /// Trigger re-indexing for a repository
    pub async fn reindex_repository(
        &self,
        principal_id: &str,
        principal_roles: Vec<String>,
        id: Uuid,
        branch: Option<String>,
        incremental: bool,
    ) -> Result<(), CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let target_branch = branch.unwrap_or_else(|| repo_data.current_branch.clone());

        // Policy Evaluation
        let policy_ctx = crate::policy_evaluator::PolicyContext {
            principal_id: principal_id.to_string(),
            principal_roles,
            tenant_id: repo_data.tenant_id.clone(),
        };

        if !self
            .policy_evaluator
            .evaluate_request(&policy_ctx, "IndexRepository", &repo_data)
            .await?
        {
            return Err(CodeSearchError::PolicyViolation {
                policy: "Cedar::IndexRepository".to_string(),
                reason: "Insufficient permissions to trigger re-indexing".to_string(),
            });
        }

        if repo_data.status == RepositoryStatus::Indexing {
            return Ok(()); // Already indexing
        }

        let local_path = if repo_data.r#type == RepositoryType::Local {
            std::path::PathBuf::from(repo_data.local_path.as_ref().unwrap())
        } else {
            self.base_path
                .join(repo_data.tenant_id.clone())
                .join(repo_data.name.clone())
        };

        // Checkout branch if not correct
        if repo_data.r#type != RepositoryType::Local {
            let repo =
                git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?;

            let obj =
                repo.revparse_single(&target_branch)
                    .map_err(|e| CodeSearchError::GitError {
                        reason: format!("Branch {} not found: {}", target_branch, e),
                    })?;

            repo.checkout_tree(&obj, None)
                .map_err(|e| CodeSearchError::GitError {
                    reason: format!("Checkout failed: {}", e),
                })?;

            repo.set_head(&format!("refs/heads/{}", target_branch))
                .map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?;
        }

        self.storage
            .update_status(id, RepositoryStatus::Indexing)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        let mut cmd = std::process::Command::new("codesearch");
        cmd.arg("index")
            .arg(&local_path)
            .arg("--branch")
            .arg(&target_branch);

        let mut from_commit = None;
        if incremental {
            if let Some(last_commit) = &repo_data.last_indexed_commit {
                // Determine current head
                let repo =
                    git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
                        reason: e.to_string(),
                    })?;
                let head = repo.head().map_err(|e| CodeSearchError::GitError {
                    reason: e.to_string(),
                })?;
                let head_commit = head
                    .peel_to_commit()
                    .map_err(|e| CodeSearchError::GitError {
                        reason: e.to_string(),
                    })?;
                let head_sha = head_commit.id().to_string();

                if head_sha == *last_commit {
                    tracing::info!(
                        "Repository {} already up to date at {}",
                        repo_data.name,
                        head_sha
                    );
                    self.storage
                        .update_status(id, RepositoryStatus::Ready)
                        .await
                        .map_err(|e| CodeSearchError::DatabaseError {
                            reason: e.to_string(),
                        })?;
                    return Ok(());
                }

                from_commit = Some(last_commit.clone());
                cmd.arg("--incremental").arg("--from").arg(last_commit);
            }
        }

        tracing::info!(
            "Starting indexing for {}: incremental={}",
            repo_data.name,
            incremental
        );
        let start = std::time::Instant::now();

        // Execute indexing
        let output = cmd.output().map_err(|e| CodeSearchError::IndexingFailed {
            repo: repo_data.name.clone(),
            reason: format!("Failed to execute codesearch: {}", e),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            self.storage
                .update_status(id, RepositoryStatus::Error)
                .await
                .ok();
            return Err(CodeSearchError::IndexingFailed {
                repo: repo_data.name.clone(),
                reason: stderr.to_string(),
            });
        }

        // Get new head
        let repo = git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;
        let head = repo.head().map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;
        let head_commit = head
            .peel_to_commit()
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;
        let new_commit_sha = head_commit.id().to_string();

        // Calculate metadata
        let mut _files_indexed = 0;
        if let Some(from) = from_commit {
            if let Ok(changed) = self.get_changed_files(id, &from, &new_commit_sha).await {
                _files_indexed = changed.len() as i32;
            }
        }

        // Success!
        self.storage
            .update_last_indexed(id, &new_commit_sha)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;
        self.storage
            .update_status(id, RepositoryStatus::Ready)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Record metadata (Phase 1.4)
        // TODO: Insert into codesearch_index_metadata

        tracing::info!(
            "Finished indexing {} in {:?}",
            repo_data.name,
            start.elapsed()
        );
        Ok(())
    }

    /// Background job: Check all repositories for updates
    pub async fn check_all_repositories(&self) -> Result<(), CodeSearchError> {
        // In a real implementation, we would query repositories by strategy
        // For now, let's list all and filter
        let tenant_ids: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT tenant_id FROM codesearch_repositories")
                .fetch_all(&self.storage.pool)
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;

        for tenant_id in tenant_ids {
            let repos = self
                .storage
                .list_repositories(&tenant_id)
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;

            for repo in repos {
                if repo.sync_strategy == SyncStrategy::Job && repo.status == RepositoryStatus::Ready
                {
                    tracing::info!("Checking repository {} for updates...", repo.name);
                    match self.fetch_updates(repo.id).await {
                        Ok(Some(new_sha)) => {
                            tracing::info!(
                                "New commits detected for {}: {}. Triggering re-index.",
                                repo.name,
                                new_sha
                            );
                            if let Err(e) = self
                                .reindex_repository(
                                    "system",
                                    vec!["lead".to_string()],
                                    repo.id,
                                    None,
                                    true,
                                )
                                .await
                            {
                                tracing::error!(
                                    "Automatic re-index failed for {}: {:?}",
                                    repo.name,
                                    e
                                );
                            }
                        }
                        Ok(None) => {
                            tracing::debug!("Repository {} is up to date.", repo.name);
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch updates for {}: {:?}", repo.name, e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle incoming webhook events for 'hook' strategy
    pub async fn handle_webhook_event(
        &self,
        tenant_id: &str,
        provider: &str,
        payload: serde_json::Value,
    ) -> Result<(), CodeSearchError> {
        match provider {
            "github" => {
                // ... same extraction ...
                let repo_url = payload["repository"]["clone_url"].as_str().ok_or_else(|| {
                    CodeSearchError::GitError {
                        reason: "Missing clone_url in GitHub payload".to_string(),
                    }
                })?;

                let ref_str = payload["ref"]
                    .as_str()
                    .ok_or_else(|| CodeSearchError::GitError {
                        reason: "Missing ref in GitHub payload".to_string(),
                    })?;

                let branch = ref_str.strip_prefix("refs/heads/").unwrap_or(ref_str);

                if let Some(repo) = self
                    .storage
                    .get_repository_by_url(tenant_id, repo_url)
                    .await
                    .map_err(|e| CodeSearchError::DatabaseError {
                        reason: e.to_string(),
                    })?
                {
                    if repo.sync_strategy == SyncStrategy::Hook && repo.current_branch == branch {
                        tracing::info!(
                            "Webhook received for repository {}. Triggering incremental re-index.",
                            repo.name
                        );

                        // First fetch updates to update the local clone
                        if let Err(e) = self.fetch_updates(repo.id).await {
                            tracing::error!(
                                "Failed to fetch updates for {} from webhook: {:?}",
                                repo.name,
                                e
                            );
                            return Err(e);
                        }

                        // Then re-index (using 'agent' role for webhooks)
                        self.reindex_repository(
                            "webhook-agent",
                            vec!["lead".to_string()],
                            repo.id,
                            Some(branch.to_string()),
                            true,
                        )
                        .await?;
                    }
                }
            }
            _ => {
                return Err(CodeSearchError::GitError {
                    reason: format!("Unsupported webhook provider: {}", provider),
                });
            }
        }

        Ok(())
    }

    /// Record usage of a repository (search or trace)
    pub async fn record_usage(
        &self,
        id: Uuid,
        branch: &str,
        is_search: bool,
    ) -> Result<(), CodeSearchError> {
        self.storage
            .record_usage(id, branch, is_search)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    /// Backup a repository to cold storage (S3/GCS)
    pub async fn backup_to_cold_storage(&self, id: Uuid) -> Result<String, CodeSearchError> {
        let cold_storage =
            self.cold_storage
                .as_ref()
                .ok_or_else(|| CodeSearchError::DatabaseError {
                    reason: "Cold storage not configured".to_string(),
                })?;

        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let local_path = if repo_data.r#type == RepositoryType::Local {
            std::path::PathBuf::from(repo_data.local_path.as_ref().unwrap())
        } else {
            self.base_path
                .join(repo_data.tenant_id.clone())
                .join(repo_data.name.clone())
        };

        let uri = cold_storage
            .backup_repo(&repo_data.tenant_id, id, &local_path)
            .await
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;

        // Update the repository with the cold storage URI
        sqlx::query("UPDATE codesearch_repositories SET cold_storage_uri = $1, updated_at = NOW() WHERE id = $2")
            .bind(&uri)
            .bind(id)
            .execute(&self.storage.pool)
            .await
            .map_err(|e| CodeSearchError::DatabaseError { reason: e.to_string() })?;

        Ok(uri)
    }

    /// Prepare for pod shutdown - backup and release shard assignments
    pub async fn prepare_for_shutdown(&self, shard_id: &str) -> Result<i32, CodeSearchError> {
        let router = self
            .shard_router
            .as_ref()
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Shard router not configured".to_string(),
            })?;

        // Mark shard as draining
        router
            .drain_shard(shard_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?;

        // Get all repos assigned to this shard
        let repos: Vec<(Uuid,)> =
            sqlx::query_as("SELECT id FROM codesearch_repositories WHERE shard_id = $1")
                .bind(shard_id)
                .fetch_all(&self.storage.pool)
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;

        let mut backed_up = 0;
        for (repo_id,) in repos {
            // Backup to cold storage
            if let Err(e) = self.backup_to_cold_storage(repo_id).await {
                tracing::error!("Failed to backup repo {} to cold storage: {:?}", repo_id, e);
            } else {
                backed_up += 1;
            }
        }

        // Rebalance repos to other shards
        let migrated = router.rebalance_from_shard(shard_id).await.map_err(|e| {
            CodeSearchError::DatabaseError {
                reason: e.to_string(),
            }
        })?;

        tracing::info!(
            "Shutdown prep complete: {} repos backed up, {} reassigned",
            backed_up,
            migrated
        );
        Ok(backed_up)
    }

    /// Background job: Cleanup inactive repositories
    pub async fn perform_cleanup(&self) -> Result<i32, CodeSearchError> {
        // Find repositories inactive for more than 30 days
        let threshold = Utc::now() - chrono::Duration::days(30);

        // We'll query our own storage for this
        let stale_repos: Vec<Repository> = sqlx::query_as("SELECT * FROM codesearch_repositories WHERE last_used_at < $1 OR (last_used_at IS NULL AND created_at < $1)")
            .bind(threshold)
            .fetch_all(&self.storage.pool)
            .await
            .map_err(|e| CodeSearchError::DatabaseError { reason: e.to_string() })?;

        let mut count = 0;
        for repo in stale_repos {
            tracing::info!(
                "Cleaning up stale repository: {} (ID: {})",
                repo.name,
                repo.id
            );

            // 1. Delete local assets
            let local_path = if repo.r#type == RepositoryType::Local {
                None // Don't delete user's local path
            } else {
                Some(
                    self.base_path
                        .join(repo.tenant_id.clone())
                        .join(repo.name.clone()),
                )
            };

            if let Some(path) = local_path {
                if path.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&path) {
                        tracing::error!("Failed to delete local path for {}: {}", repo.name, e);
                    }
                }
            }

            // 2. Delete Code Search index via CLI
            let mut cmd = std::process::Command::new("codesearch");
            cmd.arg("cleanup")
                .arg("--repo")
                .arg(repo.id.to_string())
                .arg("--force");
            if let Err(e) = cmd.output() {
                tracing::error!(
                    "Failed to trigger Code Search cleanup for {}: {}",
                    repo.name,
                    e
                );
            }

            // 3. Log cleanup
            sqlx::query("INSERT INTO codesearch_cleanup_logs (repository_id, repository_name, reason, action_taken, performed_by) VALUES ($1, $2, $3, $4, $5)")
                .bind(repo.id)
                .bind(repo.name.clone())
                .bind("Inactivity (30+ days)")
                .bind("deleted_index_and_files")
                .bind("system-cleanup-job")
                .execute(&self.storage.pool)
                .await
                .ok();

            // 4. Delete from database
            self.storage.delete_repository(repo.id).await.map_err(|e| {
                CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;

            count += 1;
        }

        Ok(count)
    }

    /// Detect repository owners using CODEOWNERS file
    pub async fn detect_owners(&self, id: Uuid) -> Result<Vec<String>, CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let local_path = if repo_data.r#type == RepositoryType::Local {
            std::path::PathBuf::from(repo_data.local_path.as_ref().unwrap())
        } else {
            self.base_path
                .join(repo_data.tenant_id.clone())
                .join(repo_data.name.clone())
        };

        // Try standard locations for CODEOWNERS
        let possible_paths = vec![
            local_path.join("CODEOWNERS"),
            local_path.join(".github").join("CODEOWNERS"),
            local_path.join("docs").join("CODEOWNERS"),
        ];

        for path in possible_paths {
            if path.exists() {
                let content =
                    std::fs::read_to_string(path).map_err(|e| CodeSearchError::GitError {
                        reason: format!("Failed to read CODEOWNERS: {}", e),
                    })?;

                let mut owners = std::collections::HashSet::new();
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    // Pattern [whitespace] @owner [@owner2]
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() > 1 {
                        for owner in &parts[1..] {
                            if owner.starts_with('@') {
                                owners.insert(owner.to_string());
                            }
                        }
                    }
                }
                return Ok(owners.into_iter().collect());
            }
        }

        Ok(vec![])
    }

    pub async fn create_identity(
        &self,
        identity_data: CreateIdentity,
    ) -> Result<Identity, CodeSearchError> {
        self.storage
            .create_identity(&identity_data)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    pub async fn list_identities(&self, tenant_id: &str) -> Result<Vec<Identity>, CodeSearchError> {
        sqlx::query_as("SELECT * FROM codesearch_identities WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.storage.pool)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    pub async fn update_identity(&self, id: Uuid, secret_id: &str) -> Result<(), CodeSearchError> {
        self.storage
            .update_identity(id, secret_id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    pub async fn delete_identity(&self, id: Uuid) -> Result<(), CodeSearchError> {
        self.storage
            .delete_identity(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    /// Get changed files for a Pull Request (diff between branch and base)
    pub async fn get_pull_request_diff(
        &self,
        id: Uuid,
        base_branch: &str,
        head_branch: &str,
    ) -> Result<Vec<String>, CodeSearchError> {
        let repo_data = self
            .storage
            .get_repository(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })?
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Repository not found".to_string(),
            })?;

        let local_path = if repo_data.r#type == RepositoryType::Local {
            std::path::PathBuf::from(repo_data.local_path.as_ref().unwrap())
        } else {
            self.base_path
                .join(repo_data.tenant_id.clone())
                .join(repo_data.name.clone())
        };

        let repo = git2::Repository::open(&local_path).map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;

        let base = repo
            .revparse_single(base_branch)
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Base branch {} not found: {}", base_branch, e),
            })?;
        let head = repo
            .revparse_single(head_branch)
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Head branch {} not found: {}", head_branch, e),
            })?;

        let base_tree = base
            .as_commit()
            .ok_or_else(|| CodeSearchError::GitError {
                reason: "Base is not a commit".to_string(),
            })?
            .tree()
            .unwrap();
        let head_tree = head
            .as_commit()
            .ok_or_else(|| CodeSearchError::GitError {
                reason: "Head is not a commit".to_string(),
            })?
            .tree()
            .unwrap();

        let diff = repo
            .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)
            .map_err(|e| CodeSearchError::GitError {
                reason: e.to_string(),
            })?;

        let mut changed_files = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    changed_files.push(path.to_string_lossy().into_owned());
                }
                true
            },
            None,
            None,
            None,
        )
        .map_err(|e| CodeSearchError::GitError {
            reason: e.to_string(),
        })?;

        Ok(changed_files)
    }

    /// Perform a global rebalancing of shard assignments.
    /// This should be run as a background task or CronJob.
    pub async fn rebalance_shards(&self) -> Result<i32, CodeSearchError> {
        let router = self
            .shard_router
            .as_ref()
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Shard router not configured".to_string(),
            })?;

        let active_shards =
            router
                .get_active_shards()
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;

        if active_shards.is_empty() {
            return Err(CodeSearchError::DatabaseError {
                reason: "No active shards available for rebalancing".to_string(),
            });
        }

        // Find repositories assigned to offline or draining shards
        let outdated_repos: Vec<(Uuid,)> = sqlx::query_as(r#"
            SELECT r.id 
            FROM codesearch_repositories r
            LEFT JOIN codesearch_indexer_shards s ON r.shard_id = s.shard_id
            WHERE r.shard_id IS NULL OR s.status IN ('offline', 'draining') OR s.last_heartbeat < NOW() - INTERVAL '60 seconds'
        "#)
        .fetch_all(&self.storage.pool)
        .await
        .map_err(|e| CodeSearchError::DatabaseError { reason: e.to_string() })?;

        let mut rebalanced = 0;
        for (repo_id,) in outdated_repos {
            // Unassign first
            sqlx::query("UPDATE codesearch_repositories SET shard_id = NULL WHERE id = $1")
                .bind(repo_id)
                .execute(&self.storage.pool)
                .await
                .map_err(|e| CodeSearchError::DatabaseError {
                    reason: e.to_string(),
                })?;

            // Assign to a new healthy shard
            if let Ok(new_shard) = router.assign_shard(repo_id).await {
                tracing::info!("Rebalanced repo {} to shard {}", repo_id, new_shard);
                rebalanced += 1;
            }
        }

        Ok(rebalanced)
    }

    /// Check which shard a repository belongs to for external routing (middleware helper)
    pub async fn check_affinity(&self, id: Uuid) -> Result<Option<String>, CodeSearchError> {
        self.shard_router
            .as_ref()
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Shard router not configured".to_string(),
            })?
            .get_shard_for_repo(id)
            .await
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: e.to_string(),
            })
    }

    /// Start a file system watcher for local repositories with 'watch' strategy
    pub fn start_watcher(&self, id: Uuid, path: std::path::PathBuf) -> Result<(), CodeSearchError> {
        use notify::{Config, RecursiveMode, Watcher};

        let tx = self
            .tracker_sender
            .as_ref()
            .ok_or_else(|| CodeSearchError::DatabaseError {
                reason: "Tracker sender not configured".to_string(),
            })?
            .clone();

        let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(1);

        let mut watcher = notify::RecommendedWatcher::new(
            move |res| {
                if let Ok(_) = res {
                    let _ = watcher_tx.blocking_send(());
                }
            },
            Config::default(),
        )
        .map_err(|e| CodeSearchError::DatabaseError {
            reason: format!("Watcher failed: {}", e),
        })?;

        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| CodeSearchError::DatabaseError {
                reason: format!("Failed to watch {}: {}", path.display(), e),
            })?;

        // Debounce and signal re-index
        tokio::spawn(async move {
            let _watcher = watcher; // Keep watcher alive
            while let Some(_) = watcher_rx.recv().await {
                // Debounce - wait 5 seconds after last event
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                while let Ok(_) = watcher_rx.try_recv() {} // Clear queue

                tracing::info!(
                    "FileSystem change detected for repo {}. Signaling re-index.",
                    id
                );
                let _ = tx.send(id).await;
            }
        });

        Ok(())
    }

    /// Verify that the identity has the required permissions on the repository
    pub async fn verify_permissions(
        &self,
        repo_url: &str,
        identity: &Identity,
    ) -> Result<bool, CodeSearchError> {
        let token = self
            .secret_provider
            .get_secret(&identity.secret_id)
            .await
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("Failed to retrieve secret: {}", e),
            })?;

        match identity.provider.as_str() {
            "github" => self.verify_github_permissions(repo_url, &token).await,
            _ => {
                // Default to true for local or unknown for now, but in production we'd enforce it
                Ok(true)
            }
        }
    }

    async fn verify_github_permissions(
        &self,
        repo_url: &str,
        token: &str,
    ) -> Result<bool, CodeSearchError> {
        let client = reqwest::Client::new();

        // Extract owner/repo from URL (simplified)
        // https://github.com/owner/repo.git -> owner/repo
        let parts: Vec<&str> = repo_url.trim_end_matches(".git").split('/').collect();
        if parts.len() < 2 {
            return Err(CodeSearchError::GitError {
                reason: "Invalid GitHub URL".to_string(),
            });
        }
        let repo_path = format!("{}/{}", parts[parts.len() - 2], parts[parts.len() - 1]);

        let url = format!("https://api.github.com/repos/{}", repo_path);

        let response = client
            .get(&url)
            .header("Authorization", format!("token {}", token))
            .header("User-Agent", "aeterna-repo-manager")
            .send()
            .await
            .map_err(|e| CodeSearchError::GitError {
                reason: format!("GitHub API call failed: {}", e),
            })?;

        if response.status().is_success() {
            // Check if user has pull access
            let body: serde_json::Value =
                response.json::<serde_json::Value>().await.map_err(|e| {
                    CodeSearchError::GitError {
                        reason: e.to_string(),
                    }
                })?;

            let can_pull = body["permissions"]["pull"].as_bool().unwrap_or(false);
            if !can_pull {
                return Err(CodeSearchError::PolicyViolation {
                    policy: "GitPermission".to_string(),
                    reason: "Identity does not have pull access to this repository".to_string(),
                });
            }
            Ok(true)
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            Err(CodeSearchError::RepoNotFound {
                name: repo_url.to_string(),
            })
        } else {
            Err(CodeSearchError::GitError {
                reason: format!("GitHub API returned error: {}", response.status()),
            })
        }
    }
}
