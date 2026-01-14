//! Integration tests for WriteCoordinator concurrent write scenarios
//!
//! Tests distributed lock behavior under contention using testcontainers Redis.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use storage::graph_duckdb::{ContentionAlertConfig, WriteCoordinator, WriteCoordinatorConfig};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use tokio::sync::Barrier;

async fn setup_redis_container()
-> Result<(ContainerAsync<Redis>, String), Box<dyn std::error::Error>> {
    let container = Redis::default().start().await?;
    let port = container.get_host_port_ipv4(6379).await?;
    let connection_url = format!("redis://localhost:{}", port);

    // Wait for Redis to be ready by attempting to connect with retries
    let client = redis::Client::open(connection_url.as_str())?;
    let mut retries = 10;
    loop {
        match client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                // Verify Redis is responding to commands
                let pong: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
                if pong.is_ok() {
                    break;
                }
            }
            Err(_) if retries > 0 => {
                retries -= 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    Ok((container, connection_url))
}

#[tokio::test]
async fn test_write_coordinator_single_lock_acquire_release() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig::default();
            let coordinator = WriteCoordinator::new(redis_url, config);

            let acquired_at = Instant::now();
            let lock_result = coordinator.acquire_lock("tenant-1").await;
            assert!(lock_result.is_ok(), "Should acquire lock successfully");

            let lock_value = lock_result.unwrap();
            assert!(
                !lock_value.is_empty(),
                "Lock value should be non-empty UUID"
            );

            let release_result = coordinator
                .release_lock("tenant-1", &lock_value, acquired_at)
                .await;
            assert!(release_result.is_ok(), "Should release lock successfully");
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_lock_blocks_second_acquirer() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig {
                lock_ttl_ms: 5000,
                max_retries: 2,
                base_backoff_ms: 50,
                max_backoff_ms: 100,
                alert_config: ContentionAlertConfig::default(),
            };
            let coordinator = Arc::new(WriteCoordinator::new(redis_url, config));

            let acquired_at = Instant::now();
            let first_lock = coordinator.acquire_lock("tenant-1").await;
            assert!(first_lock.is_ok(), "First lock should succeed");
            let first_lock_value = first_lock.unwrap();

            let coordinator_clone = coordinator.clone();
            let second_lock_handle =
                tokio::spawn(async move { coordinator_clone.acquire_lock("tenant-1").await });

            let second_result = second_lock_handle.await.unwrap();
            assert!(
                second_result.is_err(),
                "Second lock should timeout while first holds lock"
            );

            let release_result = coordinator
                .release_lock("tenant-1", &first_lock_value, acquired_at)
                .await;
            assert!(release_result.is_ok());
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_lock_acquired_after_release() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig::default();
            let coordinator = Arc::new(WriteCoordinator::new(redis_url, config));

            let acquired_at = Instant::now();
            let first_lock = coordinator.acquire_lock("tenant-1").await.unwrap();
            coordinator
                .release_lock("tenant-1", &first_lock, acquired_at)
                .await
                .unwrap();

            let second_lock = coordinator.acquire_lock("tenant-1").await;
            assert!(
                second_lock.is_ok(),
                "Should acquire lock after previous release"
            );

            let second_acquired_at = Instant::now();
            let second_lock_value = second_lock.unwrap();
            coordinator
                .release_lock("tenant-1", &second_lock_value, second_acquired_at)
                .await
                .unwrap();
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_tenant_isolation() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig::default();
            let coordinator = WriteCoordinator::new(redis_url, config);

            let acquired_at_1 = Instant::now();
            let lock_tenant_1 = coordinator.acquire_lock("tenant-1").await;
            assert!(lock_tenant_1.is_ok(), "Tenant-1 lock should succeed");

            let lock_tenant_2 = coordinator.acquire_lock("tenant-2").await;
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
                .release_lock("tenant-1", &lock_value_1, acquired_at_1)
                .await
                .unwrap();
            coordinator
                .release_lock("tenant-2", &lock_value_2, acquired_at_2)
                .await
                .unwrap();
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_concurrent_contention() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig {
                lock_ttl_ms: 200,
                max_retries: 10,
                base_backoff_ms: 10,
                max_backoff_ms: 50,
                alert_config: ContentionAlertConfig::default(),
            };
            let coordinator = Arc::new(WriteCoordinator::new(redis_url, config));

            let num_tasks = 5;
            let barrier = Arc::new(Barrier::new(num_tasks));
            let success_count = Arc::new(AtomicU32::new(0));

            let mut handles = vec![];

            for task_id in 0..num_tasks {
                let coord = coordinator.clone();
                let bar = barrier.clone();
                let count = success_count.clone();

                let handle = tokio::spawn(async move {
                    bar.wait().await;

                    let acquired_at = Instant::now();
                    match coord.acquire_lock("shared-tenant").await {
                        Ok(lock_value) => {
                            tokio::time::sleep(Duration::from_millis(20)).await;

                            let release_result = coord
                                .release_lock("shared-tenant", &lock_value, acquired_at)
                                .await;

                            if release_result.is_ok() {
                                count.fetch_add(1, Ordering::SeqCst);
                                (task_id, true)
                            } else {
                                (task_id, false)
                            }
                        }
                        Err(_) => (task_id, false),
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
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_lock_ttl_expiry() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig {
                lock_ttl_ms: 100,
                max_retries: 3,
                base_backoff_ms: 50,
                max_backoff_ms: 100,
                alert_config: ContentionAlertConfig::default(),
            };
            let coordinator = Arc::new(WriteCoordinator::new(redis_url, config));

            let _first_lock = coordinator.acquire_lock("tenant-1").await.unwrap();

            tokio::time::sleep(Duration::from_millis(150)).await;

            let second_lock = coordinator.acquire_lock("tenant-1").await;
            assert!(
                second_lock.is_ok(),
                "Should acquire lock after TTL expiry even without explicit release"
            );
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_exponential_backoff() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig {
                lock_ttl_ms: 10000,
                max_retries: 4,
                base_backoff_ms: 50,
                max_backoff_ms: 500,
                alert_config: ContentionAlertConfig::default(),
            };
            let coordinator = Arc::new(WriteCoordinator::new(redis_url, config));

            let acquired_at = Instant::now();
            let first_lock = coordinator.acquire_lock("tenant-1").await.unwrap();

            let coordinator_clone = coordinator.clone();
            let start = Instant::now();
            let second_result = coordinator_clone.acquire_lock("tenant-1").await;
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
                .release_lock("tenant-1", &first_lock, acquired_at)
                .await
                .unwrap();
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_write_coordinator_wrong_lock_value_release() {
    match setup_redis_container().await {
        Ok((_container, redis_url)) => {
            let config = WriteCoordinatorConfig {
                lock_ttl_ms: 5000,
                max_retries: 2,
                base_backoff_ms: 50,
                max_backoff_ms: 100,
                alert_config: ContentionAlertConfig::default(),
            };
            let coordinator = WriteCoordinator::new(redis_url, config);

            let acquired_at = Instant::now();
            let lock_value = coordinator.acquire_lock("tenant-1").await.unwrap();

            let wrong_release = coordinator
                .release_lock("tenant-1", "wrong-uuid-value", acquired_at)
                .await;
            assert!(
                wrong_release.is_ok(),
                "Release with wrong value should not error (Lua script returns 0)"
            );

            let second_lock_attempt = coordinator.acquire_lock("tenant-1").await;
            assert!(
                second_lock_attempt.is_err(),
                "Lock should still be held since wrong value didn't release it"
            );

            coordinator
                .release_lock("tenant-1", &lock_value, acquired_at)
                .await
                .unwrap();
        }
        Err(_) => {
            eprintln!("Skipping test: Docker not available");
        }
    }
}
