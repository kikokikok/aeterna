//! Integration tests for Redis storage backend using testcontainers.

use errors::StorageError;
use mk_core::types::{PartialJobResult, TenantContext, TenantId, UserId};
use std::sync::atomic::{AtomicU32, Ordering};
use storage::redis::RedisStorage;
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

async fn create_test_redis() -> Option<RedisStorage> {
    let fixture = get_redis_fixture().await?;
    RedisStorage::new(&fixture.url).await.ok()
}

fn unique_key(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}:{}", prefix, id)
}

fn unique_tenant_id() -> TenantId {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    TenantId::new(format!("test-tenant-{}", id)).unwrap()
}

#[tokio::test]
async fn test_redis_basic_operations() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("basic");
    let set_result = redis.set(&key, "test_value", Some(60)).await;
    assert!(set_result.is_ok(), "Set operation should succeed");

    let get_result = redis.get(&key).await;
    assert!(get_result.is_ok(), "Get operation should succeed");
    assert_eq!(
        get_result.unwrap(),
        Some("test_value".to_string()),
        "Retrieved value should match"
    );

    let exists_result = redis.exists_key(&key).await;
    assert!(exists_result.is_ok(), "Exists operation should succeed");
    assert!(exists_result.unwrap(), "Key should exist");

    let delete_result = redis.delete_key(&key).await;
    assert!(delete_result.is_ok(), "Delete operation should succeed");

    let exists_after_delete = redis.exists_key(&key).await;
    assert!(exists_after_delete.is_ok());
    assert!(
        !exists_after_delete.unwrap(),
        "Key should not exist after delete"
    );
}

#[tokio::test]
async fn test_redis_ttl_expiration() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("ttl");
    let set_result = redis.set(&key, "ttl_value", Some(1)).await;
    assert!(set_result.is_ok(), "Set with TTL should succeed");

    let exists_immediately = redis.exists_key(&key).await;
    assert!(exists_immediately.is_ok() && exists_immediately.unwrap());

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let exists_after_ttl = redis.exists_key(&key).await;
    assert!(exists_after_ttl.is_ok() && !exists_after_ttl.unwrap());
}

#[tokio::test]
async fn test_redis_without_ttl() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("no_ttl");
    let set_result = redis.set(&key, "persistent_value", None).await;
    assert!(set_result.is_ok(), "Set without TTL should succeed");

    let exists_result = redis.exists_key(&key).await;
    assert!(exists_result.is_ok() && exists_result.unwrap());
}

#[tokio::test]
async fn test_redis_get_nonexistent_key() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("nonexistent");
    let get_result = redis.get(&key).await;
    assert!(get_result.is_ok(), "Get operation should succeed");
    assert_eq!(get_result.unwrap(), None);
}

#[tokio::test]
async fn test_redis_connection_error() {
    let result = RedisStorage::new("redis://invalid:6379").await;
    assert!(result.is_err(), "Should fail with invalid connection");

    match result {
        Err(StorageError::ConnectionError { backend, .. }) => {
            assert_eq!(backend, "Redis");
        }
        _ => panic!("Expected ConnectionError"),
    }
}

#[tokio::test]
async fn test_redis_scoped_key() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let ctx = TenantContext::new(
        TenantId::new("tenant-123".to_string()).unwrap(),
        UserId::default(),
    );
    let scoped = redis.scoped_key(&ctx, "my_key");
    assert_eq!(scoped, "tenant-123:my_key");
}

#[tokio::test]
async fn test_redis_acquire_and_release_lock() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("lock");
    let lock_result = redis.acquire_lock(&lock_key, 60).await;
    assert!(lock_result.is_ok());
    let lock = lock_result.unwrap();
    assert!(lock.is_some(), "Should acquire lock");

    let lock_info = lock.unwrap();
    assert_eq!(lock_info.lock_key, lock_key);
    assert_eq!(lock_info.ttl_seconds, 60);
    assert!(!lock_info.lock_token.is_empty());

    let release_result = redis.release_lock(&lock_key, &lock_info.lock_token).await;
    assert!(release_result.is_ok());
    assert!(release_result.unwrap(), "Should release lock successfully");

    let exists = redis.check_lock_exists(&lock_key).await;
    assert!(exists.is_ok());
    assert!(!exists.unwrap(), "Lock should not exist after release");
}

#[tokio::test]
async fn test_redis_lock_contention() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("contention");

    let first_lock = redis.acquire_lock(&lock_key, 60).await.unwrap();
    assert!(first_lock.is_some(), "First lock should succeed");

    let second_lock = redis.acquire_lock(&lock_key, 60).await.unwrap();
    assert!(
        second_lock.is_none(),
        "Second lock should fail (contention)"
    );

    let first_info = first_lock.unwrap();
    redis
        .release_lock(&lock_key, &first_info.lock_token)
        .await
        .unwrap();

    let third_lock = redis.acquire_lock(&lock_key, 60).await.unwrap();
    assert!(
        third_lock.is_some(),
        "Third lock should succeed after release"
    );
}

#[tokio::test]
async fn test_redis_release_lock_wrong_token() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("wrong_token");
    let lock = redis.acquire_lock(&lock_key, 60).await.unwrap().unwrap();

    let release_wrong = redis.release_lock(&lock_key, "wrong-token").await;
    assert!(release_wrong.is_ok());
    assert!(
        !release_wrong.unwrap(),
        "Should not release with wrong token"
    );

    let still_exists = redis.check_lock_exists(&lock_key).await.unwrap();
    assert!(still_exists, "Lock should still exist");

    redis
        .release_lock(&lock_key, &lock.lock_token)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_extend_lock() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("extend");
    let lock = redis.acquire_lock(&lock_key, 5).await.unwrap().unwrap();

    let extend_result = redis.extend_lock(&lock_key, &lock.lock_token, 60).await;
    assert!(extend_result.is_ok());
    assert!(extend_result.unwrap(), "Should extend lock");

    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;

    let still_exists = redis.check_lock_exists(&lock_key).await.unwrap();
    assert!(still_exists, "Lock should still exist after extension");

    redis
        .release_lock(&lock_key, &lock.lock_token)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_extend_lock_wrong_token() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("extend_wrong");
    let lock = redis.acquire_lock(&lock_key, 60).await.unwrap().unwrap();

    let extend_wrong = redis.extend_lock(&lock_key, "wrong-token", 120).await;
    assert!(extend_wrong.is_ok());
    assert!(!extend_wrong.unwrap(), "Should not extend with wrong token");

    redis
        .release_lock(&lock_key, &lock.lock_token)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_record_job_completion() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let job_name = unique_key("job");

    let record_result = redis.record_job_completion(&job_name, 3600).await;
    assert!(record_result.is_ok());

    let was_completed = redis.check_job_recently_completed(&job_name).await;
    assert!(was_completed.is_ok());
    assert!(was_completed.unwrap(), "Job should be marked as completed");
}

#[tokio::test]
async fn test_redis_job_not_recently_completed() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let job_name = unique_key("never_ran");

    let was_completed = redis.check_job_recently_completed(&job_name).await;
    assert!(was_completed.is_ok());
    assert!(
        !was_completed.unwrap(),
        "Job should not be marked as completed"
    );
}

#[tokio::test]
async fn test_redis_save_and_get_checkpoint() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job_name = unique_key("checkpoint");

    let checkpoint = PartialJobResult::new(job_name.clone(), tenant_id.clone())
        .with_progress(50, Some(100))
        .with_last_id("item-123".to_string());

    let save_result = redis.save_job_checkpoint(&checkpoint, 3600).await;
    assert!(save_result.is_ok());

    let get_result = redis.get_job_checkpoint(&job_name, &tenant_id).await;
    assert!(get_result.is_ok());
    let retrieved = get_result.unwrap();
    assert!(retrieved.is_some());

    let retrieved_checkpoint = retrieved.unwrap();
    assert_eq!(retrieved_checkpoint.job_name, job_name);
    assert_eq!(retrieved_checkpoint.tenant_id, tenant_id);
    assert_eq!(retrieved_checkpoint.processed_count, 50);
    assert_eq!(retrieved_checkpoint.total_count, Some(100));
    assert_eq!(
        retrieved_checkpoint.last_processed_id,
        Some("item-123".to_string())
    );
}

#[tokio::test]
async fn test_redis_get_checkpoint_nonexistent() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job_name = unique_key("no_checkpoint");

    let get_result = redis.get_job_checkpoint(&job_name, &tenant_id).await;
    assert!(get_result.is_ok());
    assert!(get_result.unwrap().is_none());
}

#[tokio::test]
async fn test_redis_delete_checkpoint() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job_name = unique_key("delete_checkpoint");

    let checkpoint = PartialJobResult::new(job_name.clone(), tenant_id.clone());
    redis.save_job_checkpoint(&checkpoint, 3600).await.unwrap();

    let delete_result = redis.delete_job_checkpoint(&job_name, &tenant_id).await;
    assert!(delete_result.is_ok());

    let get_result = redis.get_job_checkpoint(&job_name, &tenant_id).await;
    assert!(get_result.unwrap().is_none());
}

#[tokio::test]
async fn test_redis_publish_governance_event() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let event = mk_core::types::GovernanceEvent::DriftDetected {
        project_id: "proj-1".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.75,
        timestamp: chrono::Utc::now().timestamp(),
    };

    use mk_core::traits::EventPublisher;
    let publish_result = redis.publish(event).await;
    assert!(publish_result.is_ok());
}

#[tokio::test]
async fn test_redis_publish_unit_created_event() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let event = mk_core::types::GovernanceEvent::UnitCreated {
        unit_id: "unit-123".to_string(),
        unit_type: mk_core::types::UnitType::Team,
        tenant_id: tenant_id.clone(),
        parent_id: Some("parent-456".to_string()),
        timestamp: chrono::Utc::now().timestamp(),
    };

    use mk_core::traits::EventPublisher;
    let publish_result = redis.publish(event).await;
    assert!(publish_result.is_ok());
}

#[tokio::test]
async fn test_redis_publish_role_assigned_event() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let event = mk_core::types::GovernanceEvent::RoleAssigned {
        user_id: UserId::new("user-1".to_string()).unwrap(),
        unit_id: "unit-1".to_string(),
        role: mk_core::types::Role::Developer,
        tenant_id: tenant_id.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    use mk_core::traits::EventPublisher;
    let publish_result = redis.publish(event).await;
    assert!(publish_result.is_ok());
}

#[tokio::test]
async fn test_redis_publish_policy_updated_event() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let event = mk_core::types::GovernanceEvent::PolicyUpdated {
        policy_id: "policy-1".to_string(),
        layer: mk_core::types::KnowledgeLayer::Org,
        tenant_id: tenant_id.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    use mk_core::traits::EventPublisher;
    let publish_result = redis.publish(event).await;
    assert!(publish_result.is_ok());
}

#[tokio::test]
async fn test_redis_tenant_isolation_locks() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant1 = unique_tenant_id();
    let tenant2 = unique_tenant_id();
    let job_name = "shared_job";

    let lock_key1 = format!("{}:lock:{}", tenant1, job_name);
    let lock_key2 = format!("{}:lock:{}", tenant2, job_name);

    let lock1 = redis.acquire_lock(&lock_key1, 60).await.unwrap();
    assert!(lock1.is_some(), "Tenant 1 should get lock");

    let lock2 = redis.acquire_lock(&lock_key2, 60).await.unwrap();
    assert!(lock2.is_some(), "Tenant 2 should get separate lock");

    redis
        .release_lock(&lock_key1, &lock1.unwrap().lock_token)
        .await
        .unwrap();
    redis
        .release_lock(&lock_key2, &lock2.unwrap().lock_token)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_redis_tenant_isolation_checkpoints() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant1 = unique_tenant_id();
    let tenant2 = unique_tenant_id();
    let job_name = unique_key("shared_checkpoint_job");

    let checkpoint1 =
        PartialJobResult::new(job_name.clone(), tenant1.clone()).with_progress(10, Some(100));
    redis.save_job_checkpoint(&checkpoint1, 3600).await.unwrap();

    let tenant1_checkpoint = redis.get_job_checkpoint(&job_name, &tenant1).await.unwrap();
    assert!(tenant1_checkpoint.is_some());
    assert_eq!(tenant1_checkpoint.unwrap().processed_count, 10);

    let tenant2_checkpoint = redis.get_job_checkpoint(&job_name, &tenant2).await.unwrap();
    assert!(
        tenant2_checkpoint.is_none(),
        "Tenant 2 should not see tenant 1's checkpoint"
    );
}

#[tokio::test]
async fn test_redis_overwrite_value() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("overwrite");

    redis.set(&key, "value1", None).await.unwrap();
    let v1 = redis.get(&key).await.unwrap();
    assert_eq!(v1, Some("value1".to_string()));

    redis.set(&key, "value2", None).await.unwrap();
    let v2 = redis.get(&key).await.unwrap();
    assert_eq!(v2, Some("value2".to_string()));
}

#[tokio::test]
async fn test_redis_empty_value() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("empty");

    redis.set(&key, "", None).await.unwrap();
    let result = redis.get(&key).await.unwrap();
    assert_eq!(result, Some("".to_string()));
}

#[tokio::test]
async fn test_redis_large_value() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("large");
    let large_value = "x".repeat(1024 * 100);

    redis.set(&key, &large_value, None).await.unwrap();
    let result = redis.get(&key).await.unwrap();
    assert_eq!(result, Some(large_value));
}

#[tokio::test]
async fn test_redis_special_characters_in_key() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("special:chars:with:colons");

    redis.set(&key, "value", None).await.unwrap();
    let result = redis.get(&key).await.unwrap();
    assert_eq!(result, Some("value".to_string()));
}

#[tokio::test]
async fn test_redis_delete_nonexistent_key() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let key = unique_key("never_existed");
    let delete_result = redis.delete_key(&key).await;
    assert!(
        delete_result.is_ok(),
        "Deleting nonexistent key should not error"
    );
}

#[tokio::test]
async fn test_redis_lock_ttl_expiry() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let lock_key = unique_key("lock_ttl");

    let lock = redis.acquire_lock(&lock_key, 1).await.unwrap().unwrap();

    let exists_before = redis.check_lock_exists(&lock_key).await.unwrap();
    assert!(exists_before, "Lock should exist initially");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let exists_after = redis.check_lock_exists(&lock_key).await.unwrap();
    assert!(!exists_after, "Lock should expire after TTL");

    let release_expired = redis
        .release_lock(&lock_key, &lock.lock_token)
        .await
        .unwrap();
    assert!(
        !release_expired,
        "Releasing expired lock should return false"
    );
}

#[tokio::test]
async fn test_redis_job_completion_ttl_expiry() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let job_name = unique_key("job_ttl");

    redis.record_job_completion(&job_name, 1).await.unwrap();

    let completed_before = redis.check_job_recently_completed(&job_name).await.unwrap();
    assert!(completed_before, "Job should be marked completed initially");

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let completed_after = redis.check_job_recently_completed(&job_name).await.unwrap();
    assert!(!completed_after, "Job completion should expire after TTL");
}

#[tokio::test]
async fn test_redis_checkpoint_with_partial_data() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job_name = unique_key("partial_data");

    let partial_data = serde_json::json!({
        "failed_items": ["item-1", "item-2"],
        "retries": 3,
        "errors": [{"code": 500, "message": "Server error"}]
    });

    let mut checkpoint = PartialJobResult::new(job_name.clone(), tenant_id.clone());
    checkpoint.partial_data = partial_data.clone();

    redis.save_job_checkpoint(&checkpoint, 3600).await.unwrap();

    let retrieved = redis
        .get_job_checkpoint(&job_name, &tenant_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.partial_data, partial_data);
}

#[tokio::test]
async fn test_redis_multiple_checkpoints_different_jobs() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job1 = unique_key("job1");
    let job2 = unique_key("job2");

    let checkpoint1 =
        PartialJobResult::new(job1.clone(), tenant_id.clone()).with_progress(10, None);
    let checkpoint2 =
        PartialJobResult::new(job2.clone(), tenant_id.clone()).with_progress(20, None);

    redis.save_job_checkpoint(&checkpoint1, 3600).await.unwrap();
    redis.save_job_checkpoint(&checkpoint2, 3600).await.unwrap();

    let retrieved1 = redis
        .get_job_checkpoint(&job1, &tenant_id)
        .await
        .unwrap()
        .unwrap();
    let retrieved2 = redis
        .get_job_checkpoint(&job2, &tenant_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(retrieved1.processed_count, 10);
    assert_eq!(retrieved2.processed_count, 20);
}

#[tokio::test]
async fn test_redis_checkpoint_update() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let job_name = unique_key("update_checkpoint");

    let checkpoint1 =
        PartialJobResult::new(job_name.clone(), tenant_id.clone()).with_progress(10, Some(100));
    redis.save_job_checkpoint(&checkpoint1, 3600).await.unwrap();

    let checkpoint2 = PartialJobResult::new(job_name.clone(), tenant_id.clone())
        .with_progress(50, Some(100))
        .with_last_id("item-50".to_string());
    redis.save_job_checkpoint(&checkpoint2, 3600).await.unwrap();

    let retrieved = redis
        .get_job_checkpoint(&job_name, &tenant_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.processed_count, 50);
    assert_eq!(retrieved.last_processed_id, Some("item-50".to_string()));
}

#[tokio::test]
async fn test_redis_storage_backend_trait() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    use mk_core::traits::StorageBackend;

    let tenant_id = unique_tenant_id();
    let user_id = UserId::new("user-1".to_string()).unwrap();
    let ctx = TenantContext::new(tenant_id, user_id);

    let key = unique_key("storage_backend");
    let value = b"binary data";

    redis.store(ctx.clone(), &key, value).await.unwrap();

    let result = redis.retrieve(ctx.clone(), &key).await.unwrap();
    assert_eq!(result, Some(value.to_vec()));

    let exists = redis.exists(ctx.clone(), &key).await.unwrap();
    assert!(exists);

    redis.delete(ctx.clone(), &key).await.unwrap();

    let exists_after = redis.exists(ctx, &key).await.unwrap();
    assert!(!exists_after);
}

#[tokio::test]
async fn test_redis_subscribe_receives_governance_events() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let stream_key = format!("governance:events:{}", tenant_id);

    use mk_core::traits::EventPublisher;
    let mut rx = redis.subscribe(&[&stream_key]).await.unwrap();

    let event = mk_core::types::GovernanceEvent::DriftDetected {
        project_id: "proj-subscribe-test".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.85,
        timestamp: chrono::Utc::now().timestamp(),
    };

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    redis.publish(event.clone()).await.unwrap();

    let received = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await;

    assert!(
        received.is_ok(),
        "Should receive event within timeout period"
    );
    let received_event = received.unwrap();
    assert!(received_event.is_some(), "Channel should not be closed");

    if let mk_core::types::GovernanceEvent::DriftDetected {
        project_id,
        drift_score,
        ..
    } = received_event.unwrap()
    {
        assert_eq!(project_id, "proj-subscribe-test");
        assert!((drift_score - 0.85).abs() < 0.01);
    } else {
        panic!("Expected DriftDetected event");
    }
}

#[tokio::test]
async fn test_redis_subscribe_multiple_channels() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id1 = unique_tenant_id();
    let tenant_id2 = unique_tenant_id();
    let stream1 = format!("governance:events:{}", tenant_id1);
    let stream2 = format!("governance:events:{}", tenant_id2);

    use mk_core::traits::EventPublisher;

    let mut received_count = 0;
    for attempt in 1..=3 {
        let mut rx = redis.subscribe(&[&stream1, &stream2]).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(300 * attempt as u64)).await;

        let event1 = mk_core::types::GovernanceEvent::PolicyUpdated {
            policy_id: format!("policy-{}", attempt),
            layer: mk_core::types::KnowledgeLayer::Company,
            tenant_id: tenant_id1.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        redis.publish(event1).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let event2 = mk_core::types::GovernanceEvent::UnitCreated {
            unit_id: format!("unit-{}", attempt),
            unit_type: mk_core::types::UnitType::Team,
            tenant_id: tenant_id2.clone(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp(),
        };
        redis.publish(event2).await.unwrap();

        received_count = 0;
        for _ in 0..2 {
            if let Ok(Some(_)) =
                tokio::time::timeout(tokio::time::Duration::from_secs(3), rx.recv()).await
            {
                received_count += 1;
            }
        }

        if received_count == 2 {
            break;
        }
    }

    assert_eq!(received_count, 2, "Should receive events from both streams");
}

#[tokio::test]
async fn test_redis_subscribe_multiple_events_in_sequence() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let stream_key = format!("governance:events:{}", tenant_id);

    use mk_core::traits::EventPublisher;
    let mut rx = redis.subscribe(&[&stream_key]).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    for i in 0..5 {
        let event = mk_core::types::GovernanceEvent::DriftDetected {
            project_id: format!("proj-seq-{}", i),
            tenant_id: tenant_id.clone(),
            drift_score: 0.1 * (i as f32 + 1.0),
            timestamp: chrono::Utc::now().timestamp(),
        };
        redis.publish(event).await.unwrap();
    }

    let mut received_projects = Vec::new();
    for _ in 0..5 {
        let received = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await;
        if let Ok(Some(mk_core::types::GovernanceEvent::DriftDetected { project_id, .. })) =
            received
        {
            received_projects.push(project_id);
        }
    }

    assert_eq!(received_projects.len(), 5, "Should receive all 5 events");
    for i in 0..5 {
        assert!(
            received_projects.contains(&format!("proj-seq-{}", i)),
            "Should contain proj-seq-{}",
            i
        );
    }
}

#[tokio::test]
async fn test_redis_subscribe_channel_closed_on_drop() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let stream_key = format!("governance:events:{}", tenant_id);

    use mk_core::traits::EventPublisher;
    let rx = redis.subscribe(&[&stream_key]).await.unwrap();

    drop(rx);

    let event = mk_core::types::GovernanceEvent::DriftDetected {
        project_id: "proj-drop-test".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.3,
        timestamp: chrono::Utc::now().timestamp(),
    };

    let result = redis.publish(event).await;
    assert!(
        result.is_ok(),
        "Publish should succeed even if no subscribers"
    );
}

#[tokio::test]
async fn test_redis_subscribe_receives_all_event_types() {
    let Some(redis) = create_test_redis().await else {
        eprintln!("Skipping Redis test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let user_id = UserId::new("user-event-types".to_string()).unwrap();
    let stream_key = format!("governance:events:{}", tenant_id);

    use mk_core::traits::EventPublisher;
    let mut rx = redis.subscribe(&[&stream_key]).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let events = vec![
        mk_core::types::GovernanceEvent::DriftDetected {
            project_id: "proj-1".to_string(),
            tenant_id: tenant_id.clone(),
            drift_score: 0.5,
            timestamp: chrono::Utc::now().timestamp(),
        },
        mk_core::types::GovernanceEvent::UnitCreated {
            unit_id: "unit-1".to_string(),
            unit_type: mk_core::types::UnitType::Team,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: chrono::Utc::now().timestamp(),
        },
        mk_core::types::GovernanceEvent::RoleAssigned {
            user_id: user_id.clone(),
            unit_id: "unit-1".to_string(),
            role: mk_core::types::Role::Developer,
            tenant_id: tenant_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        },
        mk_core::types::GovernanceEvent::PolicyUpdated {
            policy_id: "policy-1".to_string(),
            layer: mk_core::types::KnowledgeLayer::Team,
            tenant_id: tenant_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        },
    ];

    for event in &events {
        redis.publish(event.clone()).await.unwrap();
    }

    let mut received_count = 0;
    for _ in 0..events.len() {
        let received = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await;
        if received.is_ok() && received.unwrap().is_some() {
            received_count += 1;
        }
    }

    assert_eq!(
        received_count,
        events.len(),
        "Should receive all published event types"
    );
}
