//! Export/import API for the Aeterna backup-restore system.
//!
//! Provides endpoints for initiating, polling, downloading, cancelling,
//! and listing export jobs. Exports run as background Tokio tasks that
//! query PostgreSQL, write NDJSON files into a tar.gz archive via the
//! `aeterna-backup` crate, and optionally upload to S3.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aeterna_backup::archive::{ArchiveReader, ArchiveWriter};
use aeterna_backup::destination::ExportDestination;
use aeterna_backup::manifest::{BackupManifest, EntityCounts, ExportScope};
use aeterna_backup::s3;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use mk_core::traits::TenantConfigProvider;
use mk_core::types::Role;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{AppState, authenticated_tenant_context};

const JOB_STATE_TTL_SECS: u64 = 86400;
const TEMP_FILE_MAX_AGE_SECS: u64 = 7200;

// ---------------------------------------------------------------------------
// Job model
// ---------------------------------------------------------------------------

/// Status of an export job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Entity counts returned in the API response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityCountsResponse {
    pub memories: u64,
    pub knowledge_items: u64,
    pub policies: u64,
    pub org_units: u64,
    pub role_assignments: u64,
    pub governance_events: u64,
}

/// A single export job tracked in memory or Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportJob {
    pub job_id: String,
    pub status: JobStatus,
    pub scope: String,
    pub target: String,
    pub progress_pct: u8,
    pub entity_counts: EntityCountsResponse,
    pub archive_path: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// In-memory store for export jobs (V1).
pub struct ExportJobStore {
    jobs: RwLock<HashMap<String, ExportJob>>,
}

impl Default for ExportJobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportJobStore {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(&self, job: ExportJob) {
        self.jobs.write().await.insert(job.job_id.clone(), job);
    }

    async fn get(&self, job_id: &str) -> Option<ExportJob> {
        self.jobs.read().await.get(job_id).cloned()
    }

    async fn update_status(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = status;
        }
    }

    async fn update_progress(&self, job_id: &str, pct: u8) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.progress_pct = pct;
        }
    }

    async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.entity_counts = counts;
        }
    }

    async fn complete(&self, job_id: &str, archive_path: String) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = JobStatus::Completed;
            job.progress_pct = 100;
            job.archive_path = Some(archive_path);
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    async fn fail(&self, job_id: &str, error: String) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = JobStatus::Failed;
            job.error = Some(error);
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    async fn list_all(&self) -> Vec<ExportJob> {
        self.jobs.read().await.values().cloned().collect()
    }

    /// Remove archive files for jobs completed more than `max_age` ago.
    ///
    /// Returns the number of archive files deleted.
    async fn cleanup_archive_files(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let mut cleaned = 0usize;
        let jobs = self.jobs.read().await;
        for job in jobs.values() {
            if job.status != JobStatus::Completed {
                continue;
            }
            let completed_at = match &job.completed_at {
                Some(ts) => ts,
                None => continue,
            };
            let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at) else {
                continue;
            };
            let elapsed = now.signed_duration_since(completed);
            if elapsed.num_seconds() < max_age.as_secs() as i64 {
                continue;
            }
            if let Some(path_str) = &job.archive_path {
                let path = std::path::PathBuf::from(path_str);
                // Only remove local files, not S3 keys
                if path.exists()
                    && tokio::fs::remove_file(&path).await.is_ok() {
                        tracing::info!(
                            job_id = %job.job_id,
                            path = %path_str,
                            "Cleaned up local archive file"
                        );
                        cleaned += 1;
                    }
            }
        }
        cleaned
    }

    /// Remove job records for completed/failed jobs older than `max_age`,
    /// and cancelled jobs immediately (regardless of age).
    ///
    /// Returns the number of job records removed.
    async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();
        {
            let jobs = self.jobs.read().await;
            for job in jobs.values() {
                match job.status {
                    JobStatus::Cancelled => {
                        to_remove.push(job.job_id.clone());
                    }
                    JobStatus::Completed | JobStatus::Failed => {
                        let completed_at = match &job.completed_at {
                            Some(ts) => ts,
                            None => continue,
                        };
                        let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at)
                        else {
                            continue;
                        };
                        let elapsed = now.signed_duration_since(completed);
                        if elapsed.num_seconds() >= max_age.as_secs() as i64 {
                            to_remove.push(job.job_id.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        let count = to_remove.len();
        if !to_remove.is_empty() {
            let mut jobs = self.jobs.write().await;
            for id in &to_remove {
                jobs.remove(id);
            }
            tracing::info!(count, "Cleaned up expired/cancelled job records");
        }
        count
    }

    /// Mark a completed job for cleanup by setting `completed_at` to now if
    /// it was not already set.
    async fn mark_for_cleanup(&self, job_id: &str) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id)
            && job.completed_at.is_none() {
                job.completed_at = Some(chrono::Utc::now().to_rfc3339());
            }
    }
}

/// Redis-backed export job store for multi-instance deployments.
pub struct RedisExportJobStore {
    store: storage::RedisStore,
}

impl RedisExportJobStore {
    pub fn new(conn: std::sync::Arc<redis::aio::ConnectionManager>) -> Self {
        Self {
            store: storage::RedisStore::new(conn, "aeterna:export_jobs"),
        }
    }

    async fn insert(&self, job: ExportJob) {
        if let Err(e) = self
            .store
            .set(&job.job_id, &job, Some(JOB_STATE_TTL_SECS))
            .await
        {
            tracing::error!("Redis export job insert failed: {e}");
        }
    }

    async fn get(&self, job_id: &str) -> Option<ExportJob> {
        self.store.get(job_id).await.ok().flatten()
    }

    async fn update_status(&self, job_id: &str, status: JobStatus) {
        let _ = self
            .store
            .update::<ExportJob>(job_id, |j| j.status = status, Some(JOB_STATE_TTL_SECS))
            .await;
    }

    async fn update_progress(&self, job_id: &str, pct: u8) {
        let _ = self
            .store
            .update::<ExportJob>(job_id, |j| j.progress_pct = pct, Some(JOB_STATE_TTL_SECS))
            .await;
    }

    async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        let _ = self
            .store
            .update::<ExportJob>(
                job_id,
                |j| j.entity_counts = counts,
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn complete(&self, job_id: &str, archive_path: String) {
        let _ = self
            .store
            .update::<ExportJob>(
                job_id,
                |j| {
                    j.status = JobStatus::Completed;
                    j.progress_pct = 100;
                    j.archive_path = Some(archive_path);
                    j.completed_at = Some(chrono::Utc::now().to_rfc3339());
                },
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn fail(&self, job_id: &str, error: String) {
        let _ = self
            .store
            .update::<ExportJob>(
                job_id,
                |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(error);
                    j.completed_at = Some(chrono::Utc::now().to_rfc3339());
                },
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn list_all(&self) -> Vec<ExportJob> {
        self.store.list_all().await.unwrap_or_default()
    }

    async fn cleanup_archive_files(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let mut cleaned = 0usize;
        let jobs = self.list_all().await;
        for job in &jobs {
            if job.status != JobStatus::Completed {
                continue;
            }
            let completed_at = match &job.completed_at {
                Some(ts) => ts,
                None => continue,
            };
            let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at) else {
                continue;
            };
            let elapsed = now.signed_duration_since(completed);
            if elapsed.num_seconds() < max_age.as_secs() as i64 {
                continue;
            }
            if let Some(path_str) = &job.archive_path {
                let path = std::path::PathBuf::from(path_str);
                if path.exists()
                    && tokio::fs::remove_file(&path).await.is_ok() {
                        tracing::info!(
                            job_id = %job.job_id,
                            path = %path_str,
                            "Cleaned up local archive file"
                        );
                        cleaned += 1;
                    }
            }
        }
        cleaned
    }

    async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let jobs = self.list_all().await;
        let mut count = 0;
        for job in jobs {
            let should_remove = match job.status {
                JobStatus::Cancelled => true,
                JobStatus::Completed | JobStatus::Failed => {
                    let completed_at = match &job.completed_at {
                        Some(ts) => ts,
                        None => continue,
                    };
                    let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at) else {
                        continue;
                    };
                    let elapsed = now.signed_duration_since(completed);
                    elapsed.num_seconds() >= max_age.as_secs() as i64
                }
                _ => false,
            };
            if should_remove {
                let _ = self.store.delete(&job.job_id).await;
                count += 1;
            }
        }
        if count > 0 {
            tracing::info!(count, "Cleaned up expired/cancelled job records");
        }
        count
    }

    async fn mark_for_cleanup(&self, job_id: &str) {
        let _ = self
            .store
            .update::<ExportJob>(
                job_id,
                |j| {
                    if j.completed_at.is_none() {
                        j.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    }
                },
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }
}

/// Export job store backend: in-memory (single instance) or Redis (multi instance).
pub enum ExportJobStoreBackend {
    InMemory(ExportJobStore),
    Redis(RedisExportJobStore),
}

impl ExportJobStoreBackend {
    pub async fn insert(&self, job: ExportJob) {
        match self {
            Self::InMemory(s) => s.insert(job).await,
            Self::Redis(s) => s.insert(job).await,
        }
    }
    pub async fn get(&self, job_id: &str) -> Option<ExportJob> {
        match self {
            Self::InMemory(s) => s.get(job_id).await,
            Self::Redis(s) => s.get(job_id).await,
        }
    }
    pub async fn update_status(&self, job_id: &str, status: JobStatus) {
        match self {
            Self::InMemory(s) => s.update_status(job_id, status).await,
            Self::Redis(s) => s.update_status(job_id, status).await,
        }
    }
    pub async fn update_progress(&self, job_id: &str, pct: u8) {
        match self {
            Self::InMemory(s) => s.update_progress(job_id, pct).await,
            Self::Redis(s) => s.update_progress(job_id, pct).await,
        }
    }
    pub async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        match self {
            Self::InMemory(s) => s.update_counts(job_id, counts).await,
            Self::Redis(s) => s.update_counts(job_id, counts).await,
        }
    }
    pub async fn complete(&self, job_id: &str, archive_path: String) {
        match self {
            Self::InMemory(s) => s.complete(job_id, archive_path).await,
            Self::Redis(s) => s.complete(job_id, archive_path).await,
        }
    }
    pub async fn fail(&self, job_id: &str, error: String) {
        match self {
            Self::InMemory(s) => s.fail(job_id, error).await,
            Self::Redis(s) => s.fail(job_id, error).await,
        }
    }
    pub async fn list_all(&self) -> Vec<ExportJob> {
        match self {
            Self::InMemory(s) => s.list_all().await,
            Self::Redis(s) => s.list_all().await,
        }
    }
    pub async fn cleanup_archive_files(&self, max_age: std::time::Duration) -> usize {
        match self {
            Self::InMemory(s) => s.cleanup_archive_files(max_age).await,
            Self::Redis(s) => s.cleanup_archive_files(max_age).await,
        }
    }
    pub async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        match self {
            Self::InMemory(s) => s.cleanup_job_records(max_age).await,
            Self::Redis(s) => s.cleanup_job_records(max_age).await,
        }
    }
    pub async fn mark_for_cleanup(&self, job_id: &str) {
        match self {
            Self::InMemory(s) => s.mark_for_cleanup(job_id).await,
            Self::Redis(s) => s.mark_for_cleanup(job_id).await,
        }
    }
}

// The job store lives for the server lifetime.
// Initialized at startup via `init_job_stores` based on whether Redis is available.
static JOB_STORE: std::sync::OnceLock<ExportJobStoreBackend> = std::sync::OnceLock::new();

fn export_store() -> &'static ExportJobStoreBackend {
    JOB_STORE.get_or_init(|| ExportJobStoreBackend::InMemory(ExportJobStore::new()))
}

// ---------------------------------------------------------------------------
// Import job model
// ---------------------------------------------------------------------------

/// A single import job tracked in memory or Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub job_id: String,
    pub status: JobStatus,
    /// Import mode: `"merge"`, `"replace"`, or `"skip_existing"`.
    pub mode: String,
    pub dry_run: bool,
    pub progress_pct: u8,
    pub entity_counts: EntityCountsResponse,
    pub conflicts: Vec<ImportConflict>,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// A conflict detected during import (dry-run or real).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    pub entity_type: String,
    pub entity_id: String,
    pub reason: String,
    /// Resolution applied: `"kept_newer"`, `"overwritten"`, or `"skipped"`.
    pub resolution: String,
}

/// In-memory store for import jobs (V1).
pub struct ImportJobStore {
    jobs: RwLock<HashMap<String, ImportJob>>,
}

impl Default for ImportJobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportJobStore {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
        }
    }

    async fn insert(&self, job: ImportJob) {
        self.jobs.write().await.insert(job.job_id.clone(), job);
    }

    async fn get(&self, job_id: &str) -> Option<ImportJob> {
        self.jobs.read().await.get(job_id).cloned()
    }

    async fn update_status(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = status;
        }
    }

    async fn update_progress(&self, job_id: &str, pct: u8) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.progress_pct = pct;
        }
    }

    async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.entity_counts = counts;
        }
    }

    async fn set_conflicts(&self, job_id: &str, conflicts: Vec<ImportConflict>) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.conflicts = conflicts;
        }
    }

    async fn complete(&self, job_id: &str) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = JobStatus::Completed;
            job.progress_pct = 100;
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    async fn fail(&self, job_id: &str, error: String) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            job.status = JobStatus::Failed;
            job.error = Some(error);
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    async fn list_all(&self) -> Vec<ImportJob> {
        self.jobs.read().await.values().cloned().collect()
    }

    /// Remove import job records for completed/failed jobs older than `max_age`,
    /// and cancelled jobs immediately.
    ///
    /// Returns the number of job records removed.
    async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();
        {
            let jobs = self.jobs.read().await;
            for job in jobs.values() {
                match job.status {
                    JobStatus::Cancelled => {
                        to_remove.push(job.job_id.clone());
                    }
                    JobStatus::Completed | JobStatus::Failed => {
                        let completed_at = match &job.completed_at {
                            Some(ts) => ts,
                            None => continue,
                        };
                        let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at)
                        else {
                            continue;
                        };
                        let elapsed = now.signed_duration_since(completed);
                        if elapsed.num_seconds() >= max_age.as_secs() as i64 {
                            to_remove.push(job.job_id.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        let count = to_remove.len();
        if !to_remove.is_empty() {
            let mut jobs = self.jobs.write().await;
            for id in &to_remove {
                jobs.remove(id);
            }
            tracing::info!(count, "Cleaned up expired/cancelled import job records");
        }
        count
    }
}

/// Redis-backed import job store for multi-instance deployments.
pub struct RedisImportJobStore {
    store: storage::RedisStore,
}

impl RedisImportJobStore {
    pub fn new(conn: std::sync::Arc<redis::aio::ConnectionManager>) -> Self {
        Self {
            store: storage::RedisStore::new(conn, "aeterna:import_jobs"),
        }
    }

    async fn insert(&self, job: ImportJob) {
        if let Err(e) = self
            .store
            .set(&job.job_id, &job, Some(JOB_STATE_TTL_SECS))
            .await
        {
            tracing::error!("Redis import job insert failed: {e}");
        }
    }

    async fn get(&self, job_id: &str) -> Option<ImportJob> {
        self.store.get(job_id).await.ok().flatten()
    }

    async fn update_status(&self, job_id: &str, status: JobStatus) {
        let _ = self
            .store
            .update::<ImportJob>(job_id, |j| j.status = status, Some(JOB_STATE_TTL_SECS))
            .await;
    }

    async fn update_progress(&self, job_id: &str, pct: u8) {
        let _ = self
            .store
            .update::<ImportJob>(job_id, |j| j.progress_pct = pct, Some(JOB_STATE_TTL_SECS))
            .await;
    }

    async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        let _ = self
            .store
            .update::<ImportJob>(
                job_id,
                |j| j.entity_counts = counts,
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn set_conflicts(&self, job_id: &str, conflicts: Vec<ImportConflict>) {
        let _ = self
            .store
            .update::<ImportJob>(
                job_id,
                |j| j.conflicts = conflicts,
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn complete(&self, job_id: &str) {
        let _ = self
            .store
            .update::<ImportJob>(
                job_id,
                |j| {
                    j.status = JobStatus::Completed;
                    j.progress_pct = 100;
                    j.completed_at = Some(chrono::Utc::now().to_rfc3339());
                },
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn fail(&self, job_id: &str, error: String) {
        let _ = self
            .store
            .update::<ImportJob>(
                job_id,
                |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(error);
                    j.completed_at = Some(chrono::Utc::now().to_rfc3339());
                },
                Some(JOB_STATE_TTL_SECS),
            )
            .await;
    }

    async fn list_all(&self) -> Vec<ImportJob> {
        self.store.list_all().await.unwrap_or_default()
    }

    async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        let now = chrono::Utc::now();
        let jobs = self.list_all().await;
        let mut count = 0;
        for job in jobs {
            let should_remove = match job.status {
                JobStatus::Cancelled => true,
                JobStatus::Completed | JobStatus::Failed => {
                    let completed_at = match &job.completed_at {
                        Some(ts) => ts,
                        None => continue,
                    };
                    let Ok(completed) = chrono::DateTime::parse_from_rfc3339(completed_at) else {
                        continue;
                    };
                    let elapsed = now.signed_duration_since(completed);
                    elapsed.num_seconds() >= max_age.as_secs() as i64
                }
                _ => false,
            };
            if should_remove {
                let _ = self.store.delete(&job.job_id).await;
                count += 1;
            }
        }
        if count > 0 {
            tracing::info!(count, "Cleaned up expired/cancelled import job records");
        }
        count
    }
}

/// Import job store backend: in-memory (single instance) or Redis (multi instance).
pub enum ImportJobStoreBackend {
    InMemory(ImportJobStore),
    Redis(RedisImportJobStore),
}

impl ImportJobStoreBackend {
    pub async fn insert(&self, job: ImportJob) {
        match self {
            Self::InMemory(s) => s.insert(job).await,
            Self::Redis(s) => s.insert(job).await,
        }
    }
    pub async fn get(&self, job_id: &str) -> Option<ImportJob> {
        match self {
            Self::InMemory(s) => s.get(job_id).await,
            Self::Redis(s) => s.get(job_id).await,
        }
    }
    pub async fn update_status(&self, job_id: &str, status: JobStatus) {
        match self {
            Self::InMemory(s) => s.update_status(job_id, status).await,
            Self::Redis(s) => s.update_status(job_id, status).await,
        }
    }
    pub async fn update_progress(&self, job_id: &str, pct: u8) {
        match self {
            Self::InMemory(s) => s.update_progress(job_id, pct).await,
            Self::Redis(s) => s.update_progress(job_id, pct).await,
        }
    }
    pub async fn update_counts(&self, job_id: &str, counts: EntityCountsResponse) {
        match self {
            Self::InMemory(s) => s.update_counts(job_id, counts).await,
            Self::Redis(s) => s.update_counts(job_id, counts).await,
        }
    }
    pub async fn set_conflicts(&self, job_id: &str, conflicts: Vec<ImportConflict>) {
        match self {
            Self::InMemory(s) => s.set_conflicts(job_id, conflicts).await,
            Self::Redis(s) => s.set_conflicts(job_id, conflicts).await,
        }
    }
    pub async fn complete(&self, job_id: &str) {
        match self {
            Self::InMemory(s) => s.complete(job_id).await,
            Self::Redis(s) => s.complete(job_id).await,
        }
    }
    pub async fn fail(&self, job_id: &str, error: String) {
        match self {
            Self::InMemory(s) => s.fail(job_id, error).await,
            Self::Redis(s) => s.fail(job_id, error).await,
        }
    }
    pub async fn list_all(&self) -> Vec<ImportJob> {
        match self {
            Self::InMemory(s) => s.list_all().await,
            Self::Redis(s) => s.list_all().await,
        }
    }
    pub async fn cleanup_job_records(&self, max_age: std::time::Duration) -> usize {
        match self {
            Self::InMemory(s) => s.cleanup_job_records(max_age).await,
            Self::Redis(s) => s.cleanup_job_records(max_age).await,
        }
    }
}

static IMPORT_JOBS: std::sync::OnceLock<ImportJobStoreBackend> = std::sync::OnceLock::new();

fn import_store() -> &'static ImportJobStoreBackend {
    IMPORT_JOBS.get_or_init(|| ImportJobStoreBackend::InMemory(ImportJobStore::new()))
}

/// Initialize job stores with Redis backing when available.
///
/// Must be called once at server startup before any backup API handlers
/// are invoked. Falls back to in-memory stores if `redis_conn` is `None`.
pub fn init_job_stores(redis_conn: Option<&std::sync::Arc<redis::aio::ConnectionManager>>) {
    if let Some(conn) = redis_conn {
        let _ = JOB_STORE.set(ExportJobStoreBackend::Redis(RedisExportJobStore::new(
            conn.clone(),
        )));
        let _ = IMPORT_JOBS.set(ImportJobStoreBackend::Redis(RedisImportJobStore::new(
            conn.clone(),
        )));
        tracing::info!("Backup job stores initialized with Redis backend");
    } else {
        let _ = JOB_STORE.set(ExportJobStoreBackend::InMemory(ExportJobStore::new()));
        let _ = IMPORT_JOBS.set(ImportJobStoreBackend::InMemory(ImportJobStore::new()));
        tracing::info!("Backup job stores initialized with in-memory backend");
    }
}

// ---------------------------------------------------------------------------
// Periodic cleanup (called from background task)
// ---------------------------------------------------------------------------

/// Clean up expired export jobs: remove archive files > 1 hour old,
/// remove job records > 24 hours old, remove cancelled jobs immediately.
pub async fn cleanup_expired_export_jobs() {
    let archive_max_age = std::time::Duration::from_secs(3600); // 1 hour
    let record_max_age = std::time::Duration::from_secs(86_400); // 24 hours

    let files_cleaned = export_store().cleanup_archive_files(archive_max_age).await;
    let records_cleaned = export_store().cleanup_job_records(record_max_age).await;

    if files_cleaned > 0 || records_cleaned > 0 {
        tracing::info!(
            files_cleaned,
            records_cleaned,
            "Export job cleanup completed"
        );
    }
}

/// Clean up expired import jobs: remove job records > 24 hours old,
/// remove cancelled jobs immediately.
pub async fn cleanup_expired_import_jobs() {
    let record_max_age = std::time::Duration::from_secs(86_400); // 24 hours
    let records_cleaned = import_store().cleanup_job_records(record_max_age).await;

    if records_cleaned > 0 {
        tracing::info!(records_cleaned, "Import job cleanup completed");
    }
}

/// Clean up stale temp files in the backup export directory.
///
/// Removes any `.tar.gz` files that are not referenced by active export jobs.
pub async fn cleanup_temp_files() {
    let local_dir = std::env::var("AETERNA_BACKUP_LOCAL_DIR")
        .unwrap_or_else(|_| "/tmp/aeterna-exports".to_string());
    let dir = std::path::PathBuf::from(&local_dir);
    if !dir.exists() {
        return;
    }

    // Collect active archive paths
    let active_paths: std::collections::HashSet<String> = export_store()
        .list_all()
        .await
        .into_iter()
        .filter(|j| j.status == JobStatus::Running || j.status == JobStatus::Pending)
        .filter_map(|j| j.archive_path)
        .collect();

    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut cleaned = 0usize;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("gz") {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();
        if active_paths.contains(&path_str) {
            continue;
        }
        // Check file age: only clean up files older than 2 hours
        if let Ok(metadata) = tokio::fs::metadata(&path).await
            && let Ok(modified) = metadata.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age < std::time::Duration::from_secs(TEMP_FILE_MAX_AGE_SECS) {
                    continue;
                }
            }
        if tokio::fs::remove_file(&path).await.is_ok() {
            tracing::info!(path = %path_str, "Cleaned up orphaned temp file");
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        tracing::info!(cleaned, "Temp file cleanup completed");
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// What to export: `"all"`, `"memories"`, `"knowledge"`, `"governance"`.
    pub target: String,
    /// Export scope: `"tenant"` (current tenant) or `"full"` (all tenants).
    #[serde(default = "default_scope")]
    pub scope: String,
    /// Include governance audit events.
    #[serde(default)]
    pub include_audit: bool,
    /// ISO 8601 timestamp — only export records modified after this time.
    pub since: Option<String>,
}

fn default_scope() -> String {
    "tenant".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRequest {
    /// Path to the archive (local path or S3 key).
    pub archive_path: String,
    /// Import mode: `"merge"` (keep newer), `"replace"` (overwrite), `"skip_existing"`.
    #[serde(default = "default_import_mode")]
    pub mode: String,
    /// If `true`, perform a dry-run — detect conflicts but do not write data.
    #[serde(default)]
    pub dry_run: bool,
    /// Optional path to a JSON remap file for ID translation.
    pub remap_file: Option<String>,
}

fn default_import_mode() -> String {
    "merge".to_string()
}

/// Request to confirm a dry-run import and execute it for real.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfirmRequest {
    /// Override the import mode from the original dry-run (optional).
    pub mode: Option<String>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Export endpoints
        .route("/admin/export", post(handle_create_export))
        .route("/admin/export/{job_id}", get(handle_get_export))
        .route(
            "/admin/export/{job_id}/download",
            get(handle_download_export),
        )
        .route("/admin/export/{job_id}", delete(handle_cancel_export))
        .route("/admin/exports", get(handle_list_exports))
        // Import endpoints
        .route("/admin/import", post(handle_create_import))
        .route("/admin/import/{job_id}", get(handle_get_import))
        .route(
            "/admin/import/{job_id}/confirm",
            post(handle_confirm_import),
        )
        .route("/admin/imports", get(handle_list_imports))
        .route("/admin/stats", get(handle_admin_stats))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/v1/admin/export` — Initiate an export job.
#[tracing::instrument(skip_all)]
async fn handle_create_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> impl IntoResponse {
    let ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    // Authorization: PlatformAdmin for full scope, TenantAdmin for tenant scope
    if req.scope == "full" {
        if !ctx.has_known_role(&Role::PlatformAdmin) {
            return error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "PlatformAdmin role required for full-instance export",
            );
        }
    } else if !ctx.has_known_role(&Role::TenantAdmin) && !ctx.has_known_role(&Role::PlatformAdmin) {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "TenantAdmin role required for tenant export",
        );
    }

    // Validate target
    let valid_targets = ["all", "memories", "knowledge", "governance"];
    if !valid_targets.contains(&req.target.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_target",
            "target must be one of: all, memories, knowledge, governance",
        );
    }

    let job_id = Uuid::new_v4().to_string();
    let tenant_id = ctx.tenant_id.as_str().to_string();

    let job = ExportJob {
        job_id: job_id.clone(),
        status: JobStatus::Pending,
        scope: req.scope.clone(),
        target: req.target.clone(),
        progress_pct: 0,
        entity_counts: EntityCountsResponse::default(),
        archive_path: None,
        error: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };
    export_store().insert(job).await;

    // Spawn background task
    let bg_state = state.clone();
    let bg_job_id = job_id.clone();
    let bg_target = req.target.clone();
    let bg_scope = req.scope.clone();
    let bg_include_audit = req.include_audit;
    let bg_since = req.since.clone();
    let bg_tenant_id = tenant_id.clone();

    tokio::spawn(async move {
        let result = run_export(
            &bg_state,
            &bg_job_id,
            &bg_tenant_id,
            &bg_target,
            &bg_scope,
            bg_include_audit,
            bg_since,
        )
        .await;

        if let Err(e) = result {
            tracing::error!(job_id = %bg_job_id, error = %e, "Export job failed");
            export_store().fail(&bg_job_id, format!("{e:#}")).await;
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "jobId": job_id,
            "status": "pending"
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/export/{job_id}` — Poll export job status.
#[tracing::instrument(skip_all, fields(job_id))]
async fn handle_get_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    match export_store().get(&job_id).await {
        Some(job) => (StatusCode::OK, Json(json!(job))).into_response(),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "Export job not found"),
    }
}

/// `GET /api/v1/admin/export/{job_id}/download` — Download completed archive.
#[tracing::instrument(skip_all, fields(job_id))]
async fn handle_download_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    let job = match export_store().get(&job_id).await {
        Some(job) => job,
        None => return error_response(StatusCode::NOT_FOUND, "not_found", "Export job not found"),
    };

    if job.status != JobStatus::Completed {
        return error_response(
            StatusCode::CONFLICT,
            "not_completed",
            "Export job is not completed yet",
        );
    }

    let archive_path = match &job.archive_path {
        Some(path) => PathBuf::from(path),
        None => {
            return error_response(
                StatusCode::GONE,
                "no_archive",
                "Archive path not available (uploaded to S3?)",
            );
        }
    };

    if !archive_path.exists() {
        return error_response(
            StatusCode::GONE,
            "archive_missing",
            "Archive file no longer exists on disk",
        );
    }

    let bytes = match tokio::fs::read(&archive_path).await {
        Ok(b) => b,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                &format!("Failed to read archive: {e}"),
            );
        }
    };

    let body = Body::from(bytes);

    let filename = archive_path.file_name().map_or_else(
        || format!("{job_id}.tar.gz"),
        |n| n.to_string_lossy().to_string(),
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/gzip".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .unwrap(),
    );

    // Mark this job for cleanup so the periodic task picks it up sooner
    export_store().mark_for_cleanup(&job_id).await;

    (StatusCode::OK, headers, body).into_response()
}

/// `DELETE /api/v1/admin/export/{job_id}` — Cancel an export job.
#[tracing::instrument(skip_all, fields(job_id))]
async fn handle_cancel_export(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    match export_store().get(&job_id).await {
        Some(job) if job.status == JobStatus::Pending || job.status == JobStatus::Running => {
            export_store()
                .update_status(&job_id, JobStatus::Cancelled)
                .await;
            (
                StatusCode::OK,
                Json(json!({"jobId": job_id, "status": "cancelled"})),
            )
                .into_response()
        }
        Some(_) => error_response(
            StatusCode::CONFLICT,
            "not_cancellable",
            "Job is already completed, failed, or cancelled",
        ),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "Export job not found"),
    }
}

/// `GET /api/v1/admin/exports` — List recent export jobs.
#[tracing::instrument(skip_all)]
async fn handle_list_exports(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    let jobs = export_store().list_all().await;
    (StatusCode::OK, Json(json!({ "jobs": jobs }))).into_response()
}

// ---------------------------------------------------------------------------
// Import handlers
// ---------------------------------------------------------------------------

/// `POST /api/v1/admin/import` -- Start an import job.
#[tracing::instrument(skip_all)]
async fn handle_create_import(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ImportRequest>,
) -> impl IntoResponse {
    let ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    if !ctx.has_known_role(&Role::TenantAdmin) && !ctx.has_known_role(&Role::PlatformAdmin) {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "TenantAdmin role required for import",
        );
    }

    let valid_modes = ["merge", "replace", "skip_existing"];
    if !valid_modes.contains(&req.mode.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_mode",
            "mode must be one of: merge, replace, skip_existing",
        );
    }

    let job_id = Uuid::new_v4().to_string();
    let tenant_id = ctx.tenant_id.as_str().to_string();
    let is_platform_admin = ctx.has_known_role(&Role::PlatformAdmin);

    let job = ImportJob {
        job_id: job_id.clone(),
        status: JobStatus::Pending,
        mode: req.mode.clone(),
        dry_run: req.dry_run,
        progress_pct: 0,
        entity_counts: EntityCountsResponse::default(),
        conflicts: Vec::new(),
        error: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };
    import_store().insert(job).await;

    let bg_state = state.clone();
    let bg_job_id = job_id.clone();
    let bg_archive_path = req.archive_path.clone();
    let bg_mode = req.mode.clone();
    let bg_dry_run = req.dry_run;
    let bg_tenant_id = tenant_id.clone();

    tokio::spawn(async move {
        let result = run_import(
            &bg_state,
            &bg_job_id,
            &bg_tenant_id,
            &bg_archive_path,
            &bg_mode,
            bg_dry_run,
            is_platform_admin,
        )
        .await;

        if let Err(e) = result {
            tracing::error!(job_id = %bg_job_id, error = %e, "Import job failed");
            import_store().fail(&bg_job_id, format!("{e:#}")).await;
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "jobId": job_id,
            "status": "pending",
            "dryRun": req.dry_run
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/import/{job_id}` -- Poll import job status and conflicts.
#[tracing::instrument(skip_all, fields(job_id))]
async fn handle_get_import(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    match import_store().get(&job_id).await {
        Some(job) => (StatusCode::OK, Json(json!(job))).into_response(),
        None => error_response(StatusCode::NOT_FOUND, "not_found", "Import job not found"),
    }
}

/// `POST /api/v1/admin/import/{job_id}/confirm` -- Re-run a dry-run import for real.
#[tracing::instrument(skip_all, fields(job_id))]
async fn handle_confirm_import(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Json(req): Json<ImportConfirmRequest>,
) -> impl IntoResponse {
    let ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    if !ctx.has_known_role(&Role::TenantAdmin) && !ctx.has_known_role(&Role::PlatformAdmin) {
        return error_response(
            StatusCode::FORBIDDEN,
            "forbidden",
            "TenantAdmin role required for import confirmation",
        );
    }

    let original = match import_store().get(&job_id).await {
        Some(job) => job,
        None => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "Import job not found");
        }
    };

    if !original.dry_run || original.status != JobStatus::Completed {
        return error_response(
            StatusCode::CONFLICT,
            "not_confirmable",
            "Only completed dry-run jobs can be confirmed",
        );
    }

    let new_job_id = Uuid::new_v4().to_string();
    let mode = req.mode.unwrap_or(original.mode.clone());

    let new_job = ImportJob {
        job_id: new_job_id.clone(),
        status: JobStatus::Pending,
        mode,
        dry_run: false,
        progress_pct: 0,
        entity_counts: EntityCountsResponse::default(),
        conflicts: Vec::new(),
        error: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };
    import_store().insert(new_job).await;

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "jobId": new_job_id,
            "status": "pending",
            "dryRun": false,
            "note": "A new import job has been created with dry_run=false. Submit a new POST /admin/import to supply the archive path and start the real import."
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/imports` -- List recent import jobs.
#[tracing::instrument(skip_all)]
async fn handle_list_imports(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let _ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    let jobs = import_store().list_all().await;
    (StatusCode::OK, Json(json!({ "jobs": jobs }))).into_response()
}

#[tracing::instrument(skip_all)]
async fn handle_admin_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ctx = match authenticated_tenant_context(&state, &headers).await {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };
    let pool = state.postgres.pool();
    let tenant_id = ctx.tenant_id.as_str();

    let tenant_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE status != 'inactive'")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let user_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT u.id) FROM users u JOIN user_roles ur ON ur.user_id = u.id WHERE ur.tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let memory_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_entries WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let knowledge_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_items WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    (
        StatusCode::OK,
        Json(json!({
            "tenantCount": tenant_count,
            "userCount": user_count,
            "memoryCount": memory_count,
            "knowledgeCount": knowledge_count,
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Export execution
// ---------------------------------------------------------------------------

/// Resolve the export destination from tenant config, falling back to env
/// vars, then to local filesystem.
async fn resolve_export_destination(state: &AppState, tenant_id: &str) -> ExportDestination {
    // 1. Try tenant config
    if let Ok(Some(config)) = state
        .tenant_config_provider
        .get_config(&mk_core::types::TenantId::new(tenant_id.to_string()).unwrap_or_default())
        .await
    {
        let bucket = config
            .fields
            .get("backup_s3_bucket")
            .and_then(|f| f.value.as_str().map(String::from));
        let region = config
            .fields
            .get("backup_s3_region")
            .and_then(|f| f.value.as_str().map(String::from));
        let prefix = config
            .fields
            .get("backup_s3_prefix")
            .and_then(|f| f.value.as_str().map(String::from))
            .unwrap_or_default();
        let endpoint = config
            .fields
            .get("backup_s3_endpoint")
            .and_then(|f| f.value.as_str().map(String::from));

        if let Some(bucket) = bucket {
            let force_path_style = endpoint.is_some();
            return ExportDestination::S3 {
                bucket,
                prefix,
                region,
                endpoint,
                force_path_style,
            };
        }
    }

    // 2. Try platform-level env vars
    if let Ok(bucket) = std::env::var("AETERNA_BACKUP_S3_BUCKET") {
        let region = std::env::var("AETERNA_BACKUP_S3_REGION").ok();
        let prefix = std::env::var("AETERNA_BACKUP_S3_PREFIX").unwrap_or_default();
        let endpoint = std::env::var("AETERNA_BACKUP_S3_ENDPOINT").ok();
        return ExportDestination::S3 {
            bucket,
            prefix,
            region,
            endpoint: endpoint.clone(),
            force_path_style: endpoint.is_some(),
        };
    }

    // 3. Fall back to local filesystem
    let local_dir = std::env::var("AETERNA_BACKUP_LOCAL_DIR")
        .unwrap_or_else(|_| "/tmp/aeterna-exports".to_string());
    ExportDestination::Local {
        path: PathBuf::from(local_dir),
    }
}

/// Main export logic — runs inside a spawned Tokio task.
#[tracing::instrument(skip_all, fields(job_id, tenant_id, target, scope))]
async fn run_export(
    state: &AppState,
    job_id: &str,
    tenant_id: &str,
    target: &str,
    scope: &str,
    include_audit: bool,
    since: Option<String>,
) -> anyhow::Result<()> {
    export_store()
        .update_status(job_id, JobStatus::Running)
        .await;
    export_store().update_progress(job_id, 5).await;

    let destination = resolve_export_destination(state, tenant_id).await;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let scope_label = if scope == "full" { "full" } else { "tenant" };

    // Determine local archive path (always write locally first, then upload if S3)
    let local_dir = std::env::var("AETERNA_BACKUP_LOCAL_DIR")
        .unwrap_or_else(|_| "/tmp/aeterna-exports".to_string());
    let local_base = PathBuf::from(&local_dir);
    tokio::fs::create_dir_all(&local_base).await?;
    let archive_filename = format!("{timestamp}-{scope_label}-{target}.tar.gz");
    let local_archive_path = local_base.join(&archive_filename);

    let pool = state.postgres.pool();

    // NOTE (issue #57): a previous `set_config('app.tenant_id', $1, false)`
    // call lived here. It was broken in two ways: (a) it ran against the
    // pool without pinning a connection, so it landed on a random connection
    // and subsequent export queries using different connections never saw
    // the setting; (b) with `false` (session-scoped) it leaked tenant
    // context to the next user of that connection.
    //
    // The call has been removed because the export functions below do not
    // rely on RLS — they all filter with an explicit `WHERE tenant_id = $1`
    // clause. Proper RLS activation for cross-connection export paths
    // requires the architectural decision in issue #58.

    // Build the export scope for the manifest
    let manifest_scope = if scope == "full" {
        ExportScope::FullInstance
    } else {
        ExportScope::Tenant {
            tenant_id: tenant_id.to_string(),
        }
    };

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("AETERNA_INSTANCE_ID"))
        .unwrap_or_else(|_| "aeterna".to_string());
    let mut manifest = BackupManifest::new(hostname, manifest_scope);

    // Parse "since" timestamp if provided
    let since_epoch: Option<i64> = since.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.timestamp())
    });
    if since_epoch.is_some() {
        manifest.incremental = true;
        manifest.since_timestamp = since_epoch;
    }

    // Capture postgres txn start
    let txn_start: Option<(String,)> = sqlx::query_as("SELECT NOW()::text")
        .fetch_optional(pool)
        .await?;
    manifest.backend_snapshots.postgres_txn_start = txn_start.map(|(t,)| t);

    // Create archive writer
    let mut writer = ArchiveWriter::new(&local_archive_path)?;
    let mut counts = EntityCounts::default();

    export_store().update_progress(job_id, 10).await;

    // --- Export memories ---
    if target == "all" || target == "memories" {
        let memory_count =
            export_memories(&mut writer, pool, tenant_id, scope, since_epoch).await?;
        counts.memories = memory_count;
        export_store().update_progress(job_id, 30).await;
    }

    // Check for cancellation
    if is_cancelled(job_id).await {
        return Ok(());
    }

    // --- Export knowledge ---
    if target == "all" || target == "knowledge" {
        let knowledge_count =
            export_knowledge(&mut writer, pool, tenant_id, scope, since_epoch).await?;
        counts.knowledge_items = knowledge_count;
        export_store().update_progress(job_id, 50).await;
    }

    if is_cancelled(job_id).await {
        return Ok(());
    }

    // --- Export governance ---
    if target == "all" || target == "governance" {
        let (org_count, policy_count, role_count) =
            export_governance(&mut writer, pool, tenant_id, scope).await?;
        counts.org_units = org_count;
        counts.policies = policy_count;
        counts.role_assignments = role_count;
        export_store().update_progress(job_id, 70).await;

        if include_audit {
            let event_count =
                export_governance_events(&mut writer, pool, tenant_id, scope, since_epoch).await?;
            counts.governance_events = event_count;
        }
        export_store().update_progress(job_id, 80).await;
    }

    if is_cancelled(job_id).await {
        return Ok(());
    }

    // Update entity counts in the manifest
    manifest.entity_counts = counts.clone();

    // Write manifest and finalize
    writer.add_manifest(&manifest)?;
    let final_path = writer.finalize()?;

    export_store().update_progress(job_id, 90).await;

    // Update counts in the job store
    export_store()
        .update_counts(
            job_id,
            EntityCountsResponse {
                memories: counts.memories,
                knowledge_items: counts.knowledge_items,
                policies: counts.policies,
                org_units: counts.org_units,
                role_assignments: counts.role_assignments,
                governance_events: counts.governance_events,
            },
        )
        .await;

    // Upload to S3 if configured
    let result_path = match &destination {
        ExportDestination::S3 {
            bucket,
            prefix: _,
            region,
            endpoint,
            force_path_style,
        } => {
            let client =
                s3::create_s3_client(region.as_deref(), endpoint.as_deref(), *force_path_style)
                    .await?;

            let s3_key =
                destination.archive_key(tenant_id, &timestamp, &format!("{scope_label}-{target}"));
            s3::upload_archive(&client, bucket, &s3_key, &final_path).await?;

            tracing::info!(bucket, key = %s3_key, "Export archive uploaded to S3");

            // Clean up local file after S3 upload
            tokio::fs::remove_file(&final_path).await.ok();

            s3_key
        }
        ExportDestination::Local { .. } => final_path.to_string_lossy().to_string(),
    };

    export_store().complete(job_id, result_path).await;

    tracing::info!(
        job_id,
        memories = counts.memories,
        knowledge = counts.knowledge_items,
        policies = counts.policies,
        org_units = counts.org_units,
        roles = counts.role_assignments,
        "Export job completed"
    );

    Ok(())
}

/// Check if a job has been cancelled.
async fn is_cancelled(job_id: &str) -> bool {
    export_store()
        .get(job_id)
        .await
        .is_some_and(|j| j.status == JobStatus::Cancelled)
}

// ---------------------------------------------------------------------------
// Per-table export helpers
// ---------------------------------------------------------------------------

/// Export memory entries to `memories.ndjson`.
async fn export_memories(
    writer: &mut ArchiveWriter,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    scope: &str,
    since: Option<i64>,
) -> anyhow::Result<u64> {
    let rows: Vec<serde_json::Value> = if scope == "full" {
        if let Some(since_ts) = since {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at FROM memory_entries WHERE updated_at >= $1 ORDER BY created_at) t",
            )
            .bind(since_ts)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at FROM memory_entries ORDER BY created_at) t",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
    } else if let Some(since_ts) = since {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at FROM memory_entries WHERE tenant_id = $1 AND updated_at >= $2 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .bind(since_ts)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at FROM memory_entries WHERE tenant_id = $1 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let count = rows.len() as u64;
    if count > 0 {
        let mut ndjson = writer.create_ndjson_writer("memories.ndjson")?;
        for row in &rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    tracing::info!(count, "Exported memory entries");
    Ok(count)
}

/// Export knowledge items to `knowledge.ndjson`.
async fn export_knowledge(
    writer: &mut ArchiveWriter,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    scope: &str,
    since: Option<i64>,
) -> anyhow::Result<u64> {
    let rows: Vec<serde_json::Value> = if scope == "full" {
        if let Some(since_ts) = since {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, tenant_id, type, title, content, tags, properties, created_at, updated_at FROM knowledge_items WHERE updated_at >= $1 ORDER BY updated_at) t",
            )
            .bind(since_ts)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, tenant_id, type, title, content, tags, properties, created_at, updated_at FROM knowledge_items ORDER BY updated_at) t",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
    } else if let Some(since_ts) = since {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, tenant_id, type, title, content, tags, properties, created_at, updated_at FROM knowledge_items WHERE tenant_id = $1 AND updated_at >= $2 ORDER BY updated_at) t",
        )
        .bind(tenant_id)
        .bind(since_ts)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, tenant_id, type, title, content, tags, properties, created_at, updated_at FROM knowledge_items WHERE tenant_id = $1 ORDER BY updated_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let count = rows.len() as u64;
    if count > 0 {
        let mut ndjson = writer.create_ndjson_writer("knowledge.ndjson")?;
        for row in &rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    tracing::info!(count, "Exported knowledge items");
    Ok(count)
}

/// Export organizational units, policies, and role assignments.
async fn export_governance(
    writer: &mut ArchiveWriter,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    scope: &str,
) -> anyhow::Result<(u64, u64, u64)> {
    // Org units
    let org_rows: Vec<serde_json::Value> = if scope == "full" {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, name, type, parent_id, tenant_id, metadata, created_at FROM organizational_units ORDER BY created_at) t",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, name, type, parent_id, tenant_id, metadata, created_at FROM organizational_units WHERE tenant_id = $1 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let org_count = org_rows.len() as u64;
    if org_count > 0 {
        let mut ndjson = writer.create_ndjson_writer("org_units.ndjson")?;
        for row in &org_rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    // Policies
    let policy_rows: Vec<serde_json::Value> = if scope == "full" {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, unit_id, policy, created_at, updated_at FROM unit_policies ORDER BY created_at) t",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT p.id, p.unit_id, p.policy, p.created_at, p.updated_at FROM unit_policies p INNER JOIN organizational_units o ON p.unit_id = o.id WHERE o.tenant_id = $1 ORDER BY p.created_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let policy_count = policy_rows.len() as u64;
    if policy_count > 0 {
        let mut ndjson = writer.create_ndjson_writer("policies.ndjson")?;
        for row in &policy_rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    // Role assignments
    let role_rows: Vec<serde_json::Value> = if scope == "full" {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT user_id, tenant_id, unit_id, role, created_at FROM user_roles ORDER BY created_at) t",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT user_id, tenant_id, unit_id, role, created_at FROM user_roles WHERE tenant_id = $1 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let role_count = role_rows.len() as u64;
    if role_count > 0 {
        let mut ndjson = writer.create_ndjson_writer("role_assignments.ndjson")?;
        for row in &role_rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    tracing::info!(
        org_count,
        policy_count,
        role_count,
        "Exported governance data"
    );
    Ok((org_count, policy_count, role_count))
}

/// Export governance audit events.
async fn export_governance_events(
    writer: &mut ArchiveWriter,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    scope: &str,
    since: Option<i64>,
) -> anyhow::Result<u64> {
    let rows: Vec<serde_json::Value> = if scope == "full" {
        if let Some(since_ts) = since {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, event_type, tenant_id, payload, EXTRACT(EPOCH FROM created_at)::BIGINT AS timestamp FROM governance_events WHERE EXTRACT(EPOCH FROM created_at)::BIGINT >= $1 ORDER BY created_at) t",
            )
            .bind(since_ts)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_scalar(
                "SELECT row_to_json(t) FROM (SELECT id, event_type, tenant_id, payload, EXTRACT(EPOCH FROM created_at)::BIGINT AS timestamp FROM governance_events ORDER BY created_at) t",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
    } else if let Some(since_ts) = since {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, event_type, tenant_id, payload, EXTRACT(EPOCH FROM created_at)::BIGINT AS timestamp FROM governance_events WHERE tenant_id = $1 AND EXTRACT(EPOCH FROM created_at)::BIGINT >= $2 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .bind(since_ts)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT row_to_json(t) FROM (SELECT id, event_type, tenant_id, payload, EXTRACT(EPOCH FROM created_at)::BIGINT AS timestamp FROM governance_events WHERE tenant_id = $1 ORDER BY created_at) t",
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let count = rows.len() as u64;
    if count > 0 {
        let mut ndjson = writer.create_ndjson_writer("governance_events.ndjson")?;
        for row in &rows {
            ndjson.write_record(row)?;
        }
        ndjson.finish()?;
    }

    tracing::info!(count, "Exported governance events");
    Ok(count)
}

// ---------------------------------------------------------------------------
// Import execution
// ---------------------------------------------------------------------------

/// Resolve the archive to a local path, downloading from S3 if needed.
///
/// Returns `(local_path, should_cleanup)` — if downloaded from S3 the caller
/// should remove the temp file after use.
async fn resolve_archive_path(
    state: &AppState,
    tenant_id: &str,
    archive_path: &str,
) -> anyhow::Result<(PathBuf, bool)> {
    // If the path looks like an S3 key (starts with s3:// or lacks a leading /)
    let is_s3 = archive_path.starts_with("s3://") || archive_path.starts_with("s3:");
    let is_local = archive_path.starts_with('/') || archive_path.starts_with('.');

    if is_local {
        let p = PathBuf::from(archive_path);
        anyhow::ensure!(p.exists(), "Local archive not found: {archive_path}");
        return Ok((p, false));
    }

    if is_s3 {
        // Strip the s3:// prefix to get the key
        let key = archive_path
            .strip_prefix("s3://")
            .or_else(|| archive_path.strip_prefix("s3:"))
            .unwrap_or(archive_path);

        // Split bucket/key if the key contains the bucket
        let destination = resolve_export_destination(state, tenant_id).await;
        match &destination {
            ExportDestination::S3 {
                bucket,
                region,
                endpoint,
                force_path_style,
                ..
            } => {
                let client =
                    s3::create_s3_client(region.as_deref(), endpoint.as_deref(), *force_path_style)
                        .await?;

                let local_dir = std::env::var("AETERNA_BACKUP_LOCAL_DIR")
                    .unwrap_or_else(|_| "/tmp/aeterna-imports".to_string());
                let local_base = PathBuf::from(&local_dir);
                tokio::fs::create_dir_all(&local_base).await?;

                let filename = key.rsplit('/').next().unwrap_or("import-archive.tar.gz");
                let local_path = local_base.join(format!(
                    "import-{}-{filename}",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                ));

                s3::download_archive(&client, bucket, key, &local_path).await?;
                Ok((local_path, true))
            }
            ExportDestination::Local { .. } => {
                anyhow::bail!(
                    "S3 archive path provided but no S3 destination configured for tenant"
                );
            }
        }
    } else {
        // Treat as a local path
        let p = PathBuf::from(archive_path);
        anyhow::ensure!(p.exists(), "Archive not found: {archive_path}");
        Ok((p, false))
    }
}

/// Main import logic — runs inside a spawned Tokio task.
#[tracing::instrument(skip_all, fields(job_id, tenant_id, mode, dry_run))]
async fn run_import(
    state: &AppState,
    job_id: &str,
    tenant_id: &str,
    archive_path: &str,
    mode: &str,
    dry_run: bool,
    _is_platform_admin: bool,
) -> anyhow::Result<()> {
    import_store()
        .update_status(job_id, JobStatus::Running)
        .await;
    import_store().update_progress(job_id, 5).await;

    // Step 1: Resolve archive to local path (download from S3 if needed)
    let (local_path, cleanup_after) = resolve_archive_path(state, tenant_id, archive_path).await?;

    import_store().update_progress(job_id, 10).await;

    // Step 2: Open archive and validate checksums
    let reader = ArchiveReader::open(&local_path)?;
    let mismatches = reader.validate_checksums()?;
    if !mismatches.is_empty() {
        let details: Vec<String> = mismatches
            .iter()
            .map(|m| {
                format!(
                    "{}: expected={} actual={}",
                    m.filename, m.expected, m.actual
                )
            })
            .collect();
        anyhow::bail!(
            "Checksum validation failed for {} file(s): {}",
            mismatches.len(),
            details.join(", ")
        );
    }

    import_store().update_progress(job_id, 15).await;

    // Step 3: Read manifest to understand scope
    let manifest = reader.manifest();
    let entity_counts = &manifest.entity_counts;

    tracing::info!(
        memories = entity_counts.memories,
        knowledge = entity_counts.knowledge_items,
        policies = entity_counts.policies,
        org_units = entity_counts.org_units,
        role_assignments = entity_counts.role_assignments,
        governance_events = entity_counts.governance_events,
        "Archive manifest read"
    );

    let pool = state.postgres.pool();
    let mut all_conflicts: Vec<ImportConflict> = Vec::new();
    let mut counts = EntityCountsResponse::default();

    // Step 4: Import memories
    if entity_counts.memories > 0 {
        let (mem_count, mem_conflicts) =
            import_memories(&reader, pool, tenant_id, mode, dry_run).await?;
        counts.memories = mem_count;
        all_conflicts.extend(mem_conflicts);
        import_store().update_progress(job_id, 30).await;
    }

    // Step 5: Import knowledge
    if entity_counts.knowledge_items > 0 {
        let (ki_count, ki_conflicts) =
            import_knowledge(&reader, pool, tenant_id, mode, dry_run).await?;
        counts.knowledge_items = ki_count;
        all_conflicts.extend(ki_conflicts);
        import_store().update_progress(job_id, 50).await;
    }

    // Step 6: Import governance (org_units, policies, role_assignments)
    if entity_counts.org_units > 0
        || entity_counts.policies > 0
        || entity_counts.role_assignments > 0
    {
        let (gov_counts, gov_conflicts) =
            import_governance(&reader, pool, tenant_id, mode, dry_run).await?;
        counts.org_units = gov_counts.0;
        counts.policies = gov_counts.1;
        counts.role_assignments = gov_counts.2;
        all_conflicts.extend(gov_conflicts);
        import_store().update_progress(job_id, 70).await;
    }

    // Step 7: Import governance events
    if entity_counts.governance_events > 0 {
        let (ev_count, ev_conflicts) =
            import_governance_events(&reader, pool, tenant_id, mode, dry_run).await?;
        counts.governance_events = ev_count;
        all_conflicts.extend(ev_conflicts);
        import_store().update_progress(job_id, 85).await;
    }

    // Update job store with results
    import_store().update_counts(job_id, counts.clone()).await;
    import_store()
        .set_conflicts(job_id, all_conflicts.clone())
        .await;

    // Cleanup temp file if downloaded from S3
    if cleanup_after {
        tokio::fs::remove_file(&local_path).await.ok();
    }

    import_store().complete(job_id).await;

    tracing::info!(
        job_id,
        dry_run,
        memories = counts.memories,
        knowledge = counts.knowledge_items,
        policies = counts.policies,
        org_units = counts.org_units,
        roles = counts.role_assignments,
        conflicts = all_conflicts.len(),
        "Import job completed"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Per-table import helpers
// ---------------------------------------------------------------------------

/// Import memory entries from `memories.ndjson`.
async fn import_memories(
    reader: &ArchiveReader,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    mode: &str,
    dry_run: bool,
) -> anyhow::Result<(u64, Vec<ImportConflict>)> {
    let ndjson = match reader.open_ndjson::<serde_json::Value>("memories.ndjson") {
        Ok(r) => r,
        Err(_) => return Ok((0, Vec::new())),
    };

    let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();

    if records.is_empty() {
        return Ok((0, Vec::new()));
    }

    let mut conflicts = Vec::new();

    if dry_run {
        // Collect IDs and check for existing records
        let ids: Vec<String> = records
            .iter()
            .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        let existing: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT id::text, updated_at::text FROM memory_entries WHERE id = ANY($1) AND tenant_id = $2",
        )
        .bind(&ids)
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (existing_id, existing_updated) in &existing {
            let incoming_updated = records
                .iter()
                .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(existing_id))
                .and_then(|r| r.get("updated_at").and_then(|v| v.as_str()));

            let resolution = match mode {
                "merge" => {
                    if incoming_updated > existing_updated.as_deref() {
                        "overwritten"
                    } else {
                        "kept_newer"
                    }
                }
                "replace" => "overwritten",
                "skip_existing" => "skipped",
                _ => "skipped",
            };

            conflicts.push(ImportConflict {
                entity_type: "memory".into(),
                entity_id: existing_id.clone(),
                reason: "Record already exists".into(),
                resolution: resolution.into(),
            });
        }
    } else {
        // Real import
        for record in &records {
            let id = record
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let content = record
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let memory_layer = record
                .get("memory_layer")
                .and_then(|v| v.as_str())
                .unwrap_or("episodic");
            let importance_score: f64 = record
                .get("importance_score")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.5);
            let properties = record
                .get("properties")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let created_at = record
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let updated_at = record
                .get("updated_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let query = match mode {
                "merge" => sqlx::query(
                    "INSERT INTO memory_entries (id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz) \
                         ON CONFLICT (id) DO UPDATE SET \
                           content = EXCLUDED.content, \
                           memory_layer = EXCLUDED.memory_layer, \
                           importance_score = EXCLUDED.importance_score, \
                           properties = EXCLUDED.properties, \
                           updated_at = EXCLUDED.updated_at \
                         WHERE EXCLUDED.updated_at > memory_entries.updated_at",
                ),
                "replace" => sqlx::query(
                    "INSERT INTO memory_entries (id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz) \
                         ON CONFLICT (id) DO UPDATE SET \
                           content = EXCLUDED.content, \
                           memory_layer = EXCLUDED.memory_layer, \
                           importance_score = EXCLUDED.importance_score, \
                           properties = EXCLUDED.properties, \
                           updated_at = EXCLUDED.updated_at",
                ),
                _ => {
                    // skip_existing
                    sqlx::query(
                        "INSERT INTO memory_entries (id, tenant_id, content, memory_layer, importance_score, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz) \
                         ON CONFLICT (id) DO NOTHING",
                    )
                }
            };

            query
                .bind(id)
                .bind(tenant_id)
                .bind(content)
                .bind(memory_layer)
                .bind(importance_score)
                .bind(&properties)
                .bind(created_at)
                .bind(updated_at)
                .execute(pool)
                .await?;
        }
    }

    let count = records.len() as u64;
    tracing::info!(count, dry_run, mode, "Imported memory entries");
    Ok((count, conflicts))
}

/// Import knowledge items from `knowledge.ndjson`.
async fn import_knowledge(
    reader: &ArchiveReader,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    mode: &str,
    dry_run: bool,
) -> anyhow::Result<(u64, Vec<ImportConflict>)> {
    let ndjson = match reader.open_ndjson::<serde_json::Value>("knowledge.ndjson") {
        Ok(r) => r,
        Err(_) => return Ok((0, Vec::new())),
    };

    let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();
    if records.is_empty() {
        return Ok((0, Vec::new()));
    }

    let mut conflicts = Vec::new();

    if dry_run {
        let ids: Vec<String> = records
            .iter()
            .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();

        let existing: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT id::text, updated_at::text FROM knowledge_items WHERE id = ANY($1) AND tenant_id = $2",
        )
        .bind(&ids)
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (existing_id, existing_updated) in &existing {
            let incoming_updated = records
                .iter()
                .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(existing_id))
                .and_then(|r| r.get("updated_at").and_then(|v| v.as_str()));

            let resolution = match mode {
                "merge" => {
                    if incoming_updated > existing_updated.as_deref() {
                        "overwritten"
                    } else {
                        "kept_newer"
                    }
                }
                "replace" => "overwritten",
                _ => "skipped",
            };

            conflicts.push(ImportConflict {
                entity_type: "knowledge".into(),
                entity_id: existing_id.clone(),
                reason: "Record already exists".into(),
                resolution: resolution.into(),
            });
        }
    } else {
        for record in &records {
            let id = record
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let r#type = record
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let title = record
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let content = record
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let tags = record
                .get("tags")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let properties = record
                .get("properties")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let created_at = record
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let updated_at = record
                .get("updated_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let query = match mode {
                "merge" => sqlx::query(
                    "INSERT INTO knowledge_items (id, tenant_id, type, title, content, tags, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9::timestamptz) \
                         ON CONFLICT (id) DO UPDATE SET \
                           type = EXCLUDED.type, \
                           title = EXCLUDED.title, \
                           content = EXCLUDED.content, \
                           tags = EXCLUDED.tags, \
                           properties = EXCLUDED.properties, \
                           updated_at = EXCLUDED.updated_at \
                         WHERE EXCLUDED.updated_at > knowledge_items.updated_at",
                ),
                "replace" => sqlx::query(
                    "INSERT INTO knowledge_items (id, tenant_id, type, title, content, tags, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9::timestamptz) \
                         ON CONFLICT (id) DO UPDATE SET \
                           type = EXCLUDED.type, \
                           title = EXCLUDED.title, \
                           content = EXCLUDED.content, \
                           tags = EXCLUDED.tags, \
                           properties = EXCLUDED.properties, \
                           updated_at = EXCLUDED.updated_at",
                ),
                _ => sqlx::query(
                    "INSERT INTO knowledge_items (id, tenant_id, type, title, content, tags, properties, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::timestamptz, $9::timestamptz) \
                         ON CONFLICT (id) DO NOTHING",
                ),
            };

            query
                .bind(id)
                .bind(tenant_id)
                .bind(r#type)
                .bind(title)
                .bind(content)
                .bind(&tags)
                .bind(&properties)
                .bind(created_at)
                .bind(updated_at)
                .execute(pool)
                .await?;
        }
    }

    let count = records.len() as u64;
    tracing::info!(count, dry_run, mode, "Imported knowledge items");
    Ok((count, conflicts))
}

/// Import governance data (org_units, policies, role_assignments).
async fn import_governance(
    reader: &ArchiveReader,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    mode: &str,
    dry_run: bool,
) -> anyhow::Result<((u64, u64, u64), Vec<ImportConflict>)> {
    let mut all_conflicts = Vec::new();

    // --- Org units ---
    let org_count = {
        let ndjson = match reader.open_ndjson::<serde_json::Value>("org_units.ndjson") {
            Ok(r) => r,
            Err(_) => return Ok(((0, 0, 0), all_conflicts)),
        };
        let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();

        if dry_run {
            let ids: Vec<String> = records
                .iter()
                .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect();
            let existing: Vec<(String,)> = sqlx::query_as(
                "SELECT id::text FROM organizational_units WHERE id = ANY($1) AND tenant_id = $2",
            )
            .bind(&ids)
            .bind(tenant_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for (existing_id,) in &existing {
                let resolution = if mode == "skip_existing" {
                    "skipped"
                } else {
                    "overwritten"
                };
                all_conflicts.push(ImportConflict {
                    entity_type: "org_unit".into(),
                    entity_id: existing_id.clone(),
                    reason: "Record already exists".into(),
                    resolution: resolution.into(),
                });
            }
        } else {
            for record in &records {
                let id = record
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let name = record
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let r#type = record
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let parent_id = record.get("parent_id").and_then(|v| v.as_str());
                let metadata = record
                    .get("metadata")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let created_at = record
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                let query = match mode {
                    "skip_existing" => sqlx::query(
                        "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, created_at) \
                             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz) \
                             ON CONFLICT (id) DO NOTHING",
                    ),
                    _ => {
                        // merge and replace both overwrite for org units (no updated_at)
                        sqlx::query(
                            "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, created_at) \
                             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz) \
                             ON CONFLICT (id) DO UPDATE SET \
                               name = EXCLUDED.name, \
                               type = EXCLUDED.type, \
                               parent_id = EXCLUDED.parent_id, \
                               metadata = EXCLUDED.metadata",
                        )
                    }
                };

                query
                    .bind(id)
                    .bind(name)
                    .bind(r#type)
                    .bind(parent_id)
                    .bind(tenant_id)
                    .bind(&metadata)
                    .bind(created_at)
                    .execute(pool)
                    .await?;
            }
        }

        records.len() as u64
    };

    // --- Policies ---
    let policy_count = {
        let ndjson = match reader.open_ndjson::<serde_json::Value>("policies.ndjson") {
            Ok(r) => r,
            Err(_) => {
                return Ok(((org_count, 0, 0), all_conflicts));
            }
        };
        let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();

        if !dry_run {
            for record in &records {
                let id = record
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let unit_id = record
                    .get("unit_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let policy = record
                    .get("policy")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let created_at = record
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let updated_at = record
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                let query = match mode {
                    "merge" => sqlx::query(
                        "INSERT INTO unit_policies (id, unit_id, policy, created_at, updated_at) \
                             VALUES ($1, $2, $3, $4::timestamptz, $5::timestamptz) \
                             ON CONFLICT (id) DO UPDATE SET \
                               policy = EXCLUDED.policy, \
                               updated_at = EXCLUDED.updated_at \
                             WHERE EXCLUDED.updated_at > unit_policies.updated_at",
                    ),
                    "replace" => sqlx::query(
                        "INSERT INTO unit_policies (id, unit_id, policy, created_at, updated_at) \
                             VALUES ($1, $2, $3, $4::timestamptz, $5::timestamptz) \
                             ON CONFLICT (id) DO UPDATE SET \
                               policy = EXCLUDED.policy, \
                               updated_at = EXCLUDED.updated_at",
                    ),
                    _ => sqlx::query(
                        "INSERT INTO unit_policies (id, unit_id, policy, created_at, updated_at) \
                             VALUES ($1, $2, $3, $4::timestamptz, $5::timestamptz) \
                             ON CONFLICT (id) DO NOTHING",
                    ),
                };

                query
                    .bind(id)
                    .bind(unit_id)
                    .bind(&policy)
                    .bind(created_at)
                    .bind(updated_at)
                    .execute(pool)
                    .await?;
            }
        }

        records.len() as u64
    };

    // --- Role assignments ---
    let role_count = {
        let ndjson = match reader.open_ndjson::<serde_json::Value>("role_assignments.ndjson") {
            Ok(r) => r,
            Err(_) => {
                return Ok(((org_count, policy_count, 0), all_conflicts));
            }
        };
        let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();

        if !dry_run {
            for record in &records {
                let user_id = record
                    .get("user_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let unit_id = record.get("unit_id").and_then(|v| v.as_str());
                let role = record
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let created_at = record
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                // user_roles likely has a composite key (user_id, tenant_id, unit_id, role)
                let query = match mode {
                    "skip_existing" => sqlx::query(
                        "INSERT INTO user_roles (user_id, tenant_id, unit_id, role, created_at) \
                             VALUES ($1, $2, $3, $4, $5::timestamptz) \
                             ON CONFLICT DO NOTHING",
                    ),
                    _ => sqlx::query(
                        "INSERT INTO user_roles (user_id, tenant_id, unit_id, role, created_at) \
                             VALUES ($1, $2, $3, $4, $5::timestamptz) \
                             ON CONFLICT DO NOTHING",
                    ),
                };

                query
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(unit_id)
                    .bind(role)
                    .bind(created_at)
                    .execute(pool)
                    .await?;
            }
        }

        records.len() as u64
    };

    tracing::info!(
        org_count,
        policy_count,
        role_count,
        dry_run,
        "Imported governance data"
    );
    Ok(((org_count, policy_count, role_count), all_conflicts))
}

/// Import governance events from `governance_events.ndjson`.
async fn import_governance_events(
    reader: &ArchiveReader,
    pool: &sqlx::PgPool,
    tenant_id: &str,
    mode: &str,
    dry_run: bool,
) -> anyhow::Result<(u64, Vec<ImportConflict>)> {
    let ndjson = match reader.open_ndjson::<serde_json::Value>("governance_events.ndjson") {
        Ok(r) => r,
        Err(_) => return Ok((0, Vec::new())),
    };

    let records: Vec<serde_json::Value> = ndjson.filter_map(std::result::Result::ok).collect();
    if records.is_empty() {
        return Ok((0, Vec::new()));
    }

    let conflicts = Vec::new();

    if !dry_run {
        for record in &records {
            let id = record
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let event_type = record
                .get("event_type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let payload = record
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let timestamp = record
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let query = match mode {
                "skip_existing" => sqlx::query(
                    "INSERT INTO governance_events (id, event_type, tenant_id, payload, timestamp) \
                         VALUES ($1, $2, $3, $4, $5::timestamptz) \
                         ON CONFLICT (id) DO NOTHING",
                ),
                _ => sqlx::query(
                    "INSERT INTO governance_events (id, event_type, tenant_id, payload, timestamp) \
                         VALUES ($1, $2, $3, $4, $5::timestamptz) \
                         ON CONFLICT (id) DO UPDATE SET \
                           event_type = EXCLUDED.event_type, \
                           payload = EXCLUDED.payload, \
                           timestamp = EXCLUDED.timestamp",
                ),
            };

            query
                .bind(id)
                .bind(event_type)
                .bind(tenant_id)
                .bind(&payload)
                .bind(timestamp)
                .execute(pool)
                .await?;
        }
    }

    let count = records.len() as u64;
    tracing::info!(count, dry_run, mode, "Imported governance events");
    Ok((count, conflicts))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn error_response(status: StatusCode, error: &str, message: &str) -> axum::response::Response {
    (status, Json(json!({"error": error, "message": message}))).into_response()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_status_serialization() {
        let status = JobStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let back: JobStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, JobStatus::Running);
    }

    #[test]
    fn entity_counts_response_default() {
        let counts = EntityCountsResponse::default();
        assert_eq!(counts.memories, 0);
        assert_eq!(counts.knowledge_items, 0);
        assert_eq!(counts.policies, 0);
    }

    #[test]
    fn export_job_serialization() {
        let job = ExportJob {
            job_id: "test-123".into(),
            status: JobStatus::Pending,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T12:00:00Z".into(),
            completed_at: None,
        };
        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["jobId"], "test-123");
        assert_eq!(json["status"], "pending");
        assert_eq!(json["progressPct"], 0);
    }

    #[test]
    fn export_request_deserialization() {
        let json = r#"{"target":"memories","scope":"tenant","includeAudit":true}"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.target, "memories");
        assert_eq!(req.scope, "tenant");
        assert!(req.include_audit);
        assert!(req.since.is_none());
    }

    #[test]
    fn export_request_defaults() {
        let json = r#"{"target":"all"}"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.scope, "tenant");
        assert!(!req.include_audit);
    }

    #[tokio::test]
    async fn job_store_insert_and_get() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "j1".into(),
            status: JobStatus::Pending,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        let fetched = store.get("j1").await;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn job_store_update_status() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "j2".into(),
            status: JobStatus::Pending,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.update_status("j2", JobStatus::Running).await;

        let fetched = store.get("j2").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Running);
    }

    #[tokio::test]
    async fn job_store_complete() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "j3".into(),
            status: JobStatus::Running,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 50,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.complete("j3", "/tmp/archive.tar.gz".into()).await;

        let fetched = store.get("j3").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Completed);
        assert_eq!(fetched.progress_pct, 100);
        assert_eq!(fetched.archive_path, Some("/tmp/archive.tar.gz".into()));
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn job_store_fail() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "j4".into(),
            status: JobStatus::Running,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 30,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.fail("j4", "database error".into()).await;

        let fetched = store.get("j4").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Failed);
        assert_eq!(fetched.error, Some("database error".into()));
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn job_store_list_all() {
        let store = ExportJobStore::new();
        for i in 0..3 {
            let job = ExportJob {
                job_id: format!("list-{i}"),
                status: JobStatus::Pending,
                scope: "tenant".into(),
                target: "all".into(),
                progress_pct: 0,
                entity_counts: EntityCountsResponse::default(),
                archive_path: None,
                error: None,
                created_at: "2026-04-12T00:00:00Z".into(),
                completed_at: None,
            };
            store.insert(job).await;
        }
        let all = store.list_all().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn job_store_get_nonexistent() {
        let store = ExportJobStore::new();
        assert!(store.get("nonexistent").await.is_none());
    }

    #[test]
    fn error_response_helper() {
        let resp = error_response(StatusCode::FORBIDDEN, "forbidden", "Access denied");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn cleanup_removes_cancelled_jobs_immediately() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "cancelled-1".into(),
            status: JobStatus::Cancelled,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        let removed = store
            .cleanup_job_records(std::time::Duration::from_secs(86_400))
            .await;
        assert_eq!(removed, 1);
        assert!(store.get("cancelled-1").await.is_none());
    }

    #[tokio::test]
    async fn cleanup_retains_recent_completed_jobs() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "recent-1".into(),
            status: JobStatus::Completed,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 100,
            entity_counts: EntityCountsResponse::default(),
            archive_path: Some("/tmp/test.tar.gz".into()),
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        store.insert(job).await;

        // Use a 24-hour max age; the job was just completed so it should be retained
        let removed = store
            .cleanup_job_records(std::time::Duration::from_secs(86_400))
            .await;
        assert_eq!(removed, 0);
        assert!(store.get("recent-1").await.is_some());
    }

    #[tokio::test]
    async fn cleanup_removes_old_completed_jobs() {
        let store = ExportJobStore::new();
        // Simulate a job completed 25 hours ago
        let old_time = chrono::Utc::now() - chrono::Duration::hours(25);
        let job = ExportJob {
            job_id: "old-1".into(),
            status: JobStatus::Completed,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 100,
            entity_counts: EntityCountsResponse::default(),
            archive_path: None,
            error: None,
            created_at: "2026-04-11T00:00:00Z".into(),
            completed_at: Some(old_time.to_rfc3339()),
        };
        store.insert(job).await;

        let removed = store
            .cleanup_job_records(std::time::Duration::from_secs(86_400))
            .await;
        assert_eq!(removed, 1);
        assert!(store.get("old-1").await.is_none());
    }

    #[tokio::test]
    async fn mark_for_cleanup_sets_completed_at() {
        let store = ExportJobStore::new();
        let job = ExportJob {
            job_id: "mark-1".into(),
            status: JobStatus::Completed,
            scope: "tenant".into(),
            target: "all".into(),
            progress_pct: 100,
            entity_counts: EntityCountsResponse::default(),
            archive_path: Some("/tmp/test.tar.gz".into()),
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.mark_for_cleanup("mark-1").await;

        let fetched = store.get("mark-1").await.unwrap();
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn import_job_store_cleanup_removes_old_records() {
        let store = ImportJobStore::new();
        let old_time = chrono::Utc::now() - chrono::Duration::hours(25);
        let job = ImportJob {
            job_id: "imp-old-1".into(),
            status: JobStatus::Completed,
            mode: "merge".into(),
            dry_run: false,
            progress_pct: 100,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-11T00:00:00Z".into(),
            completed_at: Some(old_time.to_rfc3339()),
        };
        store.insert(job).await;

        let removed = store
            .cleanup_job_records(std::time::Duration::from_secs(86_400))
            .await;
        assert_eq!(removed, 1);
        assert!(store.get("imp-old-1").await.is_none());
    }

    // -----------------------------------------------------------------------
    // Import-specific tests
    // -----------------------------------------------------------------------

    #[test]
    fn import_request_deserialization() {
        let json = r#"{"archivePath":"/tmp/backup.tar.gz","mode":"merge","dryRun":true}"#;
        let req: ImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.archive_path, "/tmp/backup.tar.gz");
        assert_eq!(req.mode, "merge");
        assert!(req.dry_run);
        assert!(req.remap_file.is_none());
    }

    #[test]
    fn import_request_defaults() {
        let json = r#"{"archivePath":"/tmp/backup.tar.gz"}"#;
        let req: ImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, "merge");
        assert!(!req.dry_run);
    }

    #[test]
    fn import_request_with_remap() {
        let json = r#"{"archivePath":"s3://bucket/key.tar.gz","mode":"replace","dryRun":false,"remapFile":"/tmp/remap.json"}"#;
        let req: ImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.archive_path, "s3://bucket/key.tar.gz");
        assert_eq!(req.mode, "replace");
        assert_eq!(req.remap_file, Some("/tmp/remap.json".into()));
    }

    #[test]
    fn import_confirm_request_deserialization() {
        let json = r#"{"mode":"replace"}"#;
        let req: ImportConfirmRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, Some("replace".into()));

        let json_empty = r"{}";
        let req2: ImportConfirmRequest = serde_json::from_str(json_empty).unwrap();
        assert!(req2.mode.is_none());
    }

    #[test]
    fn import_job_serialization() {
        let job = ImportJob {
            job_id: "imp-123".into(),
            status: JobStatus::Running,
            mode: "merge".into(),
            dry_run: true,
            progress_pct: 42,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![ImportConflict {
                entity_type: "memory".into(),
                entity_id: "mem-1".into(),
                reason: "Record already exists".into(),
                resolution: "kept_newer".into(),
            }],
            error: None,
            created_at: "2026-04-12T12:00:00Z".into(),
            completed_at: None,
        };
        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["jobId"], "imp-123");
        assert_eq!(json["status"], "running");
        assert_eq!(json["mode"], "merge");
        assert_eq!(json["dryRun"], true);
        assert_eq!(json["progressPct"], 42);
        assert_eq!(json["conflicts"].as_array().unwrap().len(), 1);
        assert_eq!(json["conflicts"][0]["entityType"], "memory");
        assert_eq!(json["conflicts"][0]["resolution"], "kept_newer");
    }

    #[test]
    fn import_conflict_serialization() {
        let conflict = ImportConflict {
            entity_type: "knowledge".into(),
            entity_id: "ki-42".into(),
            reason: "Record already exists".into(),
            resolution: "overwritten".into(),
        };
        let json = serde_json::to_value(&conflict).unwrap();
        assert_eq!(json["entityType"], "knowledge");
        assert_eq!(json["entityId"], "ki-42");
        assert_eq!(json["resolution"], "overwritten");
    }

    #[tokio::test]
    async fn import_job_store_insert_and_get() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-1".into(),
            status: JobStatus::Pending,
            mode: "merge".into(),
            dry_run: false,
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        let fetched = store.get("imp-1").await;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn import_job_store_update_status_and_progress() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-2".into(),
            status: JobStatus::Pending,
            mode: "replace".into(),
            dry_run: true,
            progress_pct: 0,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        store.update_status("imp-2", JobStatus::Running).await;
        store.update_progress("imp-2", 50).await;

        let fetched = store.get("imp-2").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Running);
        assert_eq!(fetched.progress_pct, 50);
    }

    #[tokio::test]
    async fn import_job_store_set_conflicts() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-3".into(),
            status: JobStatus::Running,
            mode: "merge".into(),
            dry_run: true,
            progress_pct: 30,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        let conflicts = vec![
            ImportConflict {
                entity_type: "memory".into(),
                entity_id: "m1".into(),
                reason: "Record already exists".into(),
                resolution: "kept_newer".into(),
            },
            ImportConflict {
                entity_type: "memory".into(),
                entity_id: "m2".into(),
                reason: "Record already exists".into(),
                resolution: "overwritten".into(),
            },
        ];
        store.set_conflicts("imp-3", conflicts).await;

        let fetched = store.get("imp-3").await.unwrap();
        assert_eq!(fetched.conflicts.len(), 2);
        assert_eq!(fetched.conflicts[0].entity_id, "m1");
        assert_eq!(fetched.conflicts[1].resolution, "overwritten");
    }

    #[tokio::test]
    async fn import_job_store_complete() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-4".into(),
            status: JobStatus::Running,
            mode: "skip_existing".into(),
            dry_run: false,
            progress_pct: 80,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.complete("imp-4").await;

        let fetched = store.get("imp-4").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Completed);
        assert_eq!(fetched.progress_pct, 100);
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn import_job_store_fail() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-5".into(),
            status: JobStatus::Running,
            mode: "merge".into(),
            dry_run: false,
            progress_pct: 20,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;
        store.fail("imp-5", "checksum mismatch".into()).await;

        let fetched = store.get("imp-5").await.unwrap();
        assert_eq!(fetched.status, JobStatus::Failed);
        assert_eq!(fetched.error, Some("checksum mismatch".into()));
        assert!(fetched.completed_at.is_some());
    }

    #[tokio::test]
    async fn import_job_store_list_all() {
        let store = ImportJobStore::new();
        for i in 0..3 {
            let job = ImportJob {
                job_id: format!("imp-list-{i}"),
                status: JobStatus::Pending,
                mode: "merge".into(),
                dry_run: false,
                progress_pct: 0,
                entity_counts: EntityCountsResponse::default(),
                conflicts: vec![],
                error: None,
                created_at: "2026-04-12T00:00:00Z".into(),
                completed_at: None,
            };
            store.insert(job).await;
        }
        let all = store.list_all().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn import_job_store_get_nonexistent() {
        let store = ImportJobStore::new();
        assert!(store.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn import_job_store_update_counts() {
        let store = ImportJobStore::new();
        let job = ImportJob {
            job_id: "imp-6".into(),
            status: JobStatus::Running,
            mode: "merge".into(),
            dry_run: false,
            progress_pct: 50,
            entity_counts: EntityCountsResponse::default(),
            conflicts: vec![],
            error: None,
            created_at: "2026-04-12T00:00:00Z".into(),
            completed_at: None,
        };
        store.insert(job).await;

        let counts = EntityCountsResponse {
            memories: 42,
            knowledge_items: 10,
            policies: 3,
            org_units: 2,
            role_assignments: 5,
            governance_events: 7,
        };
        store.update_counts("imp-6", counts).await;

        let fetched = store.get("imp-6").await.unwrap();
        assert_eq!(fetched.entity_counts.memories, 42);
        assert_eq!(fetched.entity_counts.knowledge_items, 10);
        assert_eq!(fetched.entity_counts.governance_events, 7);
    }
}
