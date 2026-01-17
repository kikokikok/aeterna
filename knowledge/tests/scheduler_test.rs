use async_trait::async_trait;
use config::config::{DeploymentConfig, JobConfig};
use knowledge::governance::GovernanceEngine;
use knowledge::scheduler::GovernanceScheduler;
use mk_core::types::{KnowledgeEntry, KnowledgeLayer, TenantContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use storage::redis::RedisStorage;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tokio::sync::{OnceCell, RwLock};

struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String,
}

static REDIS: OnceCell<Option<RedisFixture>> = OnceCell::const_new();
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn get_redis_fixture() -> Option<&'static RedisFixture> {
    REDIS
        .get_or_init(|| async {
            let container = match Redis::default().start().await {
                Ok(c) => c,
                Err(_) => return None,
            };
            let host = match container.get_host().await {
                Ok(h) => h,
                Err(_) => return None,
            };
            let port = match container.get_host_port_ipv4(6379).await {
                Ok(p) => p,
                Err(_) => return None,
            };
            let url = format!("redis://{}:{}", host, port);
            Some(RedisFixture { container, url })
        })
        .await
        .as_ref()
}

fn unique_id(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}-{}", prefix, id)
}

struct MockRepository {
    entries: RwLock<Vec<KnowledgeEntry>>,
}

impl MockRepository {
    fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl mk_core::traits::KnowledgeRepository for MockRepository {
    type Error = knowledge::repository::RepositoryError;

    async fn get(
        &self,
        _ctx: TenantContext,
        _layer: KnowledgeLayer,
        path: &str,
    ) -> Result<Option<KnowledgeEntry>, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries.iter().find(|e| e.path == path).cloned())
    }

    async fn store(
        &self,
        _ctx: TenantContext,
        entry: KnowledgeEntry,
        _message: &str,
    ) -> Result<String, Self::Error> {
        self.entries.write().await.push(entry);
        Ok("hash123".to_string())
    }

    async fn list(
        &self,
        _ctx: TenantContext,
        layer: KnowledgeLayer,
        _prefix: &str,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|e| e.layer == layer)
            .cloned()
            .collect())
    }

    async fn delete(
        &self,
        _ctx: TenantContext,
        _layer: KnowledgeLayer,
        _path: &str,
        _message: &str,
    ) -> Result<String, Self::Error> {
        Ok("hash123".to_string())
    }

    async fn get_head_commit(&self, _ctx: TenantContext) -> Result<Option<String>, Self::Error> {
        Ok(Some("head123".to_string()))
    }

    async fn get_affected_items(
        &self,
        _ctx: TenantContext,
        _since_commit: &str,
    ) -> Result<Vec<(KnowledgeLayer, String)>, Self::Error> {
        Ok(Vec::new())
    }

    async fn search(
        &self,
        _ctx: TenantContext,
        _query: &str,
        _layers: Vec<KnowledgeLayer>,
        _limit: usize,
    ) -> Result<Vec<KnowledgeEntry>, Self::Error> {
        Ok(Vec::new())
    }

    fn root_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

#[tokio::test]
async fn test_scheduler_locked_job_execution() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let redis = Arc::new(RedisStorage::new(&fixture.url).await.unwrap());
    let engine = Arc::new(GovernanceEngine::new());
    let repo: Arc<
        dyn mk_core::traits::KnowledgeRepository<Error = knowledge::repository::RepositoryError>,
    > = Arc::new(MockRepository::new());
    let config = DeploymentConfig::default();

    let mut job_config = JobConfig::default();
    job_config.lock_ttl_seconds = 10;

    let job_name = unique_id("test_locked_job");
    let scheduler = GovernanceScheduler::new(
        engine.clone(),
        repo.clone(),
        config,
        Duration::from_secs(300),
        Duration::from_secs(3600),
        Duration::from_secs(86400),
    )
    .with_redis(redis.clone())
    .with_job_config(job_config.clone());

    let result: anyhow::Result<()> = scheduler
        .run_job(&job_name, "tenant-1", async { Ok(()) })
        .await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Storage not configured")
    );

    let lock_key = job_config.lock_key(&job_name);
    let lock_attempt = redis.acquire_lock(&lock_key, 10).await.unwrap();
    assert!(lock_attempt.is_some());
}

#[tokio::test]
async fn test_scheduler_deduplication() {
    let Some(fixture) = get_redis_fixture().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let redis = Arc::new(RedisStorage::new(&fixture.url).await.unwrap());
    let engine = Arc::new(GovernanceEngine::new());
    let repo: Arc<
        dyn mk_core::traits::KnowledgeRepository<Error = knowledge::repository::RepositoryError>,
    > = Arc::new(MockRepository::new());
    let config = DeploymentConfig::default();

    let mut job_config = JobConfig::default();
    job_config.deduplication_window_seconds = 60;

    let job_name = unique_id("dedup_job");
    let scheduler = GovernanceScheduler::new(
        engine.clone(),
        repo.clone(),
        config,
        Duration::from_secs(300),
        Duration::from_secs(3600),
        Duration::from_secs(86400),
    )
    .with_redis(redis.clone())
    .with_job_config(job_config);

    redis.record_job_completion(&job_name, 60).await.unwrap();

    let result: anyhow::Result<()> = scheduler
        .run_job(&job_name, "tenant-1", async { Ok(()) })
        .await;

    assert!(result.is_ok());
}
