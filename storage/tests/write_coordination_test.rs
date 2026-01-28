//! Integration tests for WriteCoordinator concurrent write scenarios
//!
//! Tests distributed lock behavior under contention using testcontainers Redis.

use std::sync::Arc;
use std::time::{Duration, Instant};

use std::sync::atomic::{AtomicU32, Ordering};
use storage::graph_duckdb::{ContentionAlertConfig, WriteCoordinator, WriteCoordinatorConfig};
use testing::{redis, unique_id};
use tokio::sync::Barrier;

/// Generate a unique tenant ID for each test to avoid lock collisions
fn unique_tenant_id(prefix: &str) -> String {
    unique_id(prefix)
}

#[tokio::test]
async fn test_write_coordinator_single_lock_acquire_release() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig::default();
    let coordinator = WriteCoordinator::new(fixture.url().to_string(), config);

    let acquired_at = Instant::now();
    let lock_result = coordinator.acquire_lock(&tenant).await;
    assert!(lock_result.is_ok(), "Should acquire lock successfully");

    let lock_value = lock_result.unwrap();
    assert!(
        !lock_value.is_empty(),
        "Lock value should be non-empty UUID"
    );

    let release_result = coordinator
        .release_lock(&tenant, &lock_value, acquired_at)
        .await;
    assert!(release_result.is_ok(), "Should release lock successfully");
}

#[tokio::test]
async fn test_write_coordinator_lock_blocks_second_acquirer() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig {
        lock_ttl_ms: 5000,
        max_retries: 2,
        base_backoff_ms: 50,
        max_backoff_ms: 100,
        alert_config: ContentionAlertConfig::default()
    };
    let coordinator = Arc::new(WriteCoordinator::new(fixture.url().to_string(), config));

    let acquired_at = Instant::now();
    let first_lock = coordinator.acquire_lock(&tenant).await;
    assert!(first_lock.is_ok(), "First lock should succeed");
    let first_lock_value = first_lock.unwrap();

    let coordinator_clone = coordinator.clone();
    let tenant_clone = tenant.clone();
    let second_lock_handle =
        tokio::spawn(async move { coordinator_clone.acquire_lock(&tenant_clone).await });

    let second_result = second_lock_handle.await.unwrap();
    assert!(
        second_result.is_err(),
        "Second lock should timeout while first holds lock"
    );

    let release_result = coordinator
        .release_lock(&tenant, &first_lock_value, acquired_at)
        .await;
    assert!(release_result.is_ok());
}

#[tokio::test]
async fn test_write_coordinator_lock_acquired_after_release() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig::default();
    let coordinator = Arc::new(WriteCoordinator::new(fixture.url().to_string(), config));

    let acquired_at = Instant::now();
    let first_lock = coordinator.acquire_lock(&tenant).await.unwrap();
    coordinator
        .release_lock(&tenant, &first_lock, acquired_at)
        .await
        .unwrap();

    let second_lock = coordinator.acquire_lock(&tenant).await;
    assert!(
        second_lock.is_ok(),
        "Should acquire lock after previous release"
    );

    let second_acquired_at = Instant::now();
    let second_lock_value = second_lock.unwrap();
    coordinator
        .release_lock(&tenant, &second_lock_value, second_acquired_at)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_write_coordinator_tenant_isolation() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant1 = unique_tenant_id("tenant");
    let tenant2 = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig::default();
    let coordinator = WriteCoordinator::new(fixture.url().to_string(), config);

    let acquired_at_1 = Instant::now();
    let lock_tenant_1 = coordinator.acquire_lock(&tenant1).await;
    assert!(lock_tenant_1.is_ok(), "Tenant-1 lock should succeed");

    let lock_tenant_2 = coordinator.acquire_lock(&tenant2).await;
    assert!(
        lock_tenant_2.is_ok(),
        "Tenant-2 lock should succeed (isolated from tenant-1)"
    );

    let lock_value_1 = lock_tenant_1.unwrap();
    let lock_value_2 = lock_tenant_2.unwrap();

    assert_ne!(
        lock_value_1, lock_value_2,
        "Lock values should be different"
    );

    let acquired_at_2 = Instant::now();
    coordinator
        .release_lock(&tenant1, &lock_value_1, acquired_at_1)
        .await
        .unwrap();
    coordinator
        .release_lock(&tenant2, &lock_value_2, acquired_at_2)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_write_coordinator_concurrent_contention() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("shared-tenant");
    let config = WriteCoordinatorConfig {
        lock_ttl_ms: 200,
        max_retries: 10,
        base_backoff_ms: 10,
        max_backoff_ms: 50,
        alert_config: ContentionAlertConfig::default()
    };
    let coordinator = Arc::new(WriteCoordinator::new(fixture.url().to_string(), config));

    let num_tasks = 5;
    let barrier = Arc::new(Barrier::new(num_tasks));
    let success_count = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    for task_id in 0..num_tasks {
        let coord = coordinator.clone();
        let bar = barrier.clone();
        let count = success_count.clone();
        let tenant_clone = tenant.clone();

        let handle = tokio::spawn(async move {
            bar.wait().await;

            let acquired_at = Instant::now();
            match coord.acquire_lock(&tenant_clone).await {
                Ok(lock_value) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;

                    let release_result = coord
                        .release_lock(&tenant_clone, &lock_value, acquired_at)
                        .await;

                    if release_result.is_ok() {
                        count.fetch_add(1, Ordering::SeqCst);
                        (task_id, true)
                    } else {
                        (task_id, false)
                    }
                }
                Err(_) => (task_id, false)
            }
        });

        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    let successes = success_count.load(Ordering::SeqCst);

    assert!(
        successes >= 1,
        "At least one task should successfully acquire and release lock"
    );
    assert!(
        successes <= num_tasks as u32,
        "Cannot have more successes than tasks"
    );

    let successful_tasks: Vec<_> = results.iter().filter(|(_, success)| *success).collect();
    assert!(
        !successful_tasks.is_empty(),
        "At least one task should succeed"
    );
}

#[tokio::test]
async fn test_write_coordinator_lock_ttl_expiry() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig {
        lock_ttl_ms: 100,
        max_retries: 3,
        base_backoff_ms: 50,
        max_backoff_ms: 100,
        alert_config: ContentionAlertConfig::default()
    };
    let coordinator = Arc::new(WriteCoordinator::new(fixture.url().to_string(), config));

    let _first_lock = coordinator.acquire_lock(&tenant).await.unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;

    let second_lock = coordinator.acquire_lock(&tenant).await;
    assert!(
        second_lock.is_ok(),
        "Should acquire lock after TTL expiry even without explicit release"
    );
}

#[tokio::test]
async fn test_write_coordinator_exponential_backoff() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig {
        lock_ttl_ms: 10000,
        max_retries: 4,
        base_backoff_ms: 50,
        max_backoff_ms: 500,
        alert_config: ContentionAlertConfig::default()
    };
    let coordinator = Arc::new(WriteCoordinator::new(fixture.url().to_string(), config));

    let acquired_at = Instant::now();
    let first_lock = coordinator.acquire_lock(&tenant).await.unwrap();

    let coordinator_clone = coordinator.clone();
    let tenant_clone = tenant.clone();
    let start = Instant::now();
    let second_result = coordinator_clone.acquire_lock(&tenant_clone).await;
    let elapsed = start.elapsed();

    assert!(second_result.is_err(), "Second lock should timeout");

    let expected_min_wait = Duration::from_millis(50 + 100 + 200);
    assert!(
        elapsed >= expected_min_wait,
        "Should wait at least {:?} due to exponential backoff, but only waited {:?}",
        expected_min_wait,
        elapsed
    );

    coordinator
        .release_lock(&tenant, &first_lock, acquired_at)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_write_coordinator_wrong_lock_value_release() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let tenant = unique_tenant_id("tenant");
    let config = WriteCoordinatorConfig {
        lock_ttl_ms: 5000,
        max_retries: 2,
        base_backoff_ms: 50,
        max_backoff_ms: 100,
        alert_config: ContentionAlertConfig::default()
    };
    let coordinator = WriteCoordinator::new(fixture.url().to_string(), config);

    let acquired_at = Instant::now();
    let lock_value = coordinator.acquire_lock(&tenant).await.unwrap();

    let wrong_release = coordinator
        .release_lock(&tenant, "wrong-uuid-value", acquired_at)
        .await;
    assert!(
        wrong_release.is_ok(),
        "Release with wrong value should not error (Lua script returns 0)"
    );

    let second_lock_attempt = coordinator.acquire_lock(&tenant).await;
    assert!(
        second_lock_attempt.is_err(),
        "Lock should still be held since wrong value didn't release it"
    );

    coordinator
        .release_lock(&tenant, &lock_value, acquired_at)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_write_coordinator_induced_redis_failure() {
    let Some(fixture) = redis().await else {
        eprintln!("Skipping test: Docker not available");
        return;
    };

    let config = WriteCoordinatorConfig::default();
    let coordinator = WriteCoordinator::new(fixture.url().to_string(), config);

    let result = coordinator.acquire_lock("TRIGGER_REDIS_ERROR").await;
    assert!(result.is_err());
    match result {
        Err(storage::graph_duckdb::GraphError::S3(msg)) => {
            assert!(msg.contains("Induced Redis failure"));
        }
        _ => panic!("Expected induced Redis failure, got {:?}", result)
    }
}
