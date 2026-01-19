use async_trait::async_trait;
use distributed_lock::{
    DistributedLock, LockHandle, LockProvider, RedisDistributedLock, RedisLockHandle,
    RedisLockProvider,
};
use knowledge::governance::GovernanceEngine;
use knowledge::repository::GitRepository;
use memory::manager::MemoryManager;
use mk_core::types::{TenantContext, TenantId};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use sync::bridge::SyncManager;
use sync::state::SyncState;
use sync::state_persister::SyncStatePersister;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;

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
            match Redis::default().start().await {
                Ok(container) => {
                    let port = container.get_host_port_ipv4(6379).await.ok()?;
                    let url = format!("redis://localhost:{}", port);
                    Some(RedisFixture { container, url })
                }
                Err(_) => None,
            }
        })
        .await
        .as_ref()
}

async fn create_lock_provider() -> Option<Arc<RedisLockProvider>> {
    let fixture = get_redis_fixture().await?;
    RedisLockProvider::new(&fixture.url)
        .await
        .ok()
        .map(Arc::new)
}

fn unique_tenant_id() -> TenantId {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    TenantId::new(format!("lock-test-tenant-{}", id)).unwrap()
}

struct MockPersister;

#[async_trait]
impl SyncStatePersister for MockPersister {
    async fn load(
        &self,
        _tenant_id: &TenantId,
    ) -> Result<SyncState, Box<dyn std::error::Error + Send + Sync>> {
        Ok(SyncState::default())
    }

    async fn save(
        &self,
        _tenant_id: &TenantId,
        _s: &SyncState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn create_sync_manager(
    lock_provider: Option<Arc<RedisLockProvider>>,
) -> Result<(SyncManager, tempfile::TempDir), Box<dyn std::error::Error + Send + Sync>> {
    let repo_dir = tempfile::tempdir()?;
    let knowledge_repo = Arc::new(GitRepository::new(repo_dir.path())?);
    let governance_engine = Arc::new(GovernanceEngine::new());
    let memory_manager = Arc::new(MemoryManager::new());

    let sync_manager = SyncManager::new(
        memory_manager,
        knowledge_repo,
        governance_engine,
        config::config::DeploymentConfig::default(),
        None,
        Arc::new(MockPersister),
        lock_provider,
    )
    .await?;

    Ok((sync_manager, repo_dir))
}

#[tokio::test]
async fn test_acquire_and_release_lock_with_redis() {
    let Some(lock_provider) = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider))
        .await
        .expect("failed to create sync manager");

    let tenant_id = unique_tenant_id();

    let lock_handle = sync_manager
        .acquire_sync_lock(&tenant_id, "test_job")
        .await
        .expect("acquire should succeed");

    assert!(lock_handle.is_some(), "should acquire lock");

    sync_manager
        .release_sync_lock(lock_handle)
        .await
        .expect("release should succeed");
}

#[tokio::test]
async fn test_lock_contention_blocks_second_acquirer() {
    let Some(lock_provider): Option<Arc<RedisLockProvider>> = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let lock_key = format!("sync_lock:{}:contention_test", tenant_id.as_str());

    let lock: RedisDistributedLock = lock_provider.create_lock(&lock_key);
    let first_handle: RedisLockHandle = lock
        .acquire(Some(Duration::from_secs(5)))
        .await
        .expect("first acquire should succeed");

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider.clone()))
        .await
        .expect("failed to create sync manager");

    let second_result = sync_manager
        .acquire_sync_lock(&tenant_id, "contention_test")
        .await
        .expect("acquire call should not error");

    assert!(
        second_result.is_none(),
        "second acquire should fail due to contention"
    );

    first_handle
        .release()
        .await
        .expect("release should succeed");
}

#[tokio::test]
async fn test_lock_released_allows_reacquisition() {
    let Some(lock_provider) = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider))
        .await
        .expect("failed to create sync manager");

    let tenant_id = unique_tenant_id();

    let first_handle = sync_manager
        .acquire_sync_lock(&tenant_id, "reacquire_test")
        .await
        .expect("first acquire should succeed");
    assert!(first_handle.is_some());

    sync_manager
        .release_sync_lock(first_handle)
        .await
        .expect("release should succeed");

    let second_handle = sync_manager
        .acquire_sync_lock(&tenant_id, "reacquire_test")
        .await
        .expect("second acquire should succeed after release");
    assert!(second_handle.is_some(), "should reacquire after release");

    sync_manager
        .release_sync_lock(second_handle)
        .await
        .expect("final release should succeed");
}

#[tokio::test]
async fn test_different_tenants_can_acquire_locks_simultaneously() {
    let Some(lock_provider) = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider))
        .await
        .expect("failed to create sync manager");

    let tenant1 = unique_tenant_id();
    let tenant2 = unique_tenant_id();

    let handle1 = sync_manager
        .acquire_sync_lock(&tenant1, "multi_tenant")
        .await
        .expect("tenant1 acquire should succeed");
    assert!(handle1.is_some());

    let handle2 = sync_manager
        .acquire_sync_lock(&tenant2, "multi_tenant")
        .await
        .expect("tenant2 acquire should succeed");
    assert!(
        handle2.is_some(),
        "different tenants should not block each other"
    );

    sync_manager.release_sync_lock(handle1).await.unwrap();
    sync_manager.release_sync_lock(handle2).await.unwrap();
}

#[tokio::test]
async fn test_run_sync_cycle_with_redis_lock() {
    let Some(lock_provider) = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider))
        .await
        .expect("failed to create sync manager");

    let ctx = TenantContext::default();

    let result = sync_manager.run_sync_cycle(ctx, 60).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_sync_cycles_one_skipped() {
    let Some(lock_provider): Option<Arc<RedisLockProvider>> = create_lock_provider().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let lock_key = format!("sync_lock:{}:batch_sync", tenant_id.as_str());

    let lock: RedisDistributedLock = lock_provider.create_lock(&lock_key);
    let _holding_handle: RedisLockHandle = lock
        .acquire(Some(Duration::from_secs(30)))
        .await
        .expect("should acquire lock");

    let (sync_manager, _repo_dir) = create_sync_manager(Some(lock_provider.clone()))
        .await
        .expect("failed to create sync manager");

    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("test-user".to_string()).unwrap(),
        agent_id: None,
    };

    let result = sync_manager.run_sync_cycle(ctx, 60).await;
    assert!(result.is_ok(), "should succeed by skipping when lock held");
}
