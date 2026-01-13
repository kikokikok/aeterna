//! Integration tests for Redis storage backend
//!
//! These tests use testcontainers to spin up a Redis instance.

use errors::StorageError;
use storage::redis::RedisStorage;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

async fn setup_redis_container()
-> Result<(ContainerAsync<Redis>, String), Box<dyn std::error::Error>> {
    let container = Redis::default().start().await?;

    let port = container.get_host_port_ipv4(6379).await?;
    let connection_url = format!("redis://localhost:{}", port);

    Ok((container, connection_url))
}

#[tokio::test]
async fn test_redis_basic_operations() {
    match setup_redis_container().await {
        Ok((_container, connection_url)) => {
            let redis = RedisStorage::new(&connection_url)
                .await
                .expect("Failed to create Redis storage");

            let set_result = redis.set("test_key", "test_value", Some(60)).await;
            assert!(set_result.is_ok(), "Set operation should succeed");

            let get_result = redis.get("test_key").await;
            assert!(get_result.is_ok(), "Get operation should succeed");
            assert_eq!(
                get_result.unwrap(),
                Some("test_value".to_string()),
                "Retrieved value should match"
            );

            let exists_result = redis.exists_key("test_key").await;
            assert!(exists_result.is_ok(), "Exists operation should succeed");
            assert!(exists_result.unwrap(), "Key should exist");

            let delete_result = redis.delete_key("test_key").await;
            assert!(delete_result.is_ok(), "Delete operation should succeed");

            let exists_after_delete = redis.exists_key("test_key").await;
            assert!(
                exists_after_delete.is_ok(),
                "Exists operation should succeed"
            );
            assert!(
                !exists_after_delete.unwrap(),
                "Key should not exist after delete"
            );
        }
        Err(_) => {
            eprintln!("Skipping Redis test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_redis_ttl_expiration() {
    match setup_redis_container().await {
        Ok((_container, connection_url)) => {
            let redis = RedisStorage::new(&connection_url)
                .await
                .expect("Failed to create Redis storage");

            let set_result = redis.set("ttl_key", "ttl_value", Some(1)).await;
            assert!(set_result.is_ok(), "Set with TTL should succeed");

            let exists_immediately = redis.exists_key("ttl_key").await;
            assert!(
                exists_immediately.is_ok() && exists_immediately.unwrap(),
                "Key should exist immediately"
            );

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            let exists_after_ttl = redis.exists_key("ttl_key").await;
            assert!(
                exists_after_ttl.is_ok() && !exists_after_ttl.unwrap(),
                "Key should not exist after TTL"
            );
        }
        Err(_) => {
            eprintln!("Skipping Redis test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_redis_without_ttl() {
    match setup_redis_container().await {
        Ok((_container, connection_url)) => {
            let redis = RedisStorage::new(&connection_url)
                .await
                .expect("Failed to create Redis storage");

            let set_result = redis.set("no_ttl_key", "persistent_value", None).await;
            assert!(set_result.is_ok(), "Set without TTL should succeed");

            let exists_result = redis.exists_key("no_ttl_key").await;
            assert!(
                exists_result.is_ok() && exists_result.unwrap(),
                "Key should exist"
            );
        }
        Err(_) => {
            eprintln!("Skipping Redis test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_redis_get_nonexistent_key() {
    match setup_redis_container().await {
        Ok((_container, connection_url)) => {
            let redis = RedisStorage::new(&connection_url)
                .await
                .expect("Failed to create Redis storage");

            let get_result = redis.get("nonexistent_key").await;
            assert!(get_result.is_ok(), "Get operation should succeed");
            assert_eq!(
                get_result.unwrap(),
                None,
                "Should return None for nonexistent key"
            );
        }
        Err(_) => {
            eprintln!("Skipping Redis test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_redis_connection_error() {
    let result = RedisStorage::new("redis://invalid:6379").await;

    assert!(result.is_err(), "Should fail with invalid connection");

    match result {
        Err(StorageError::ConnectionError { backend, .. }) => {
            assert_eq!(backend, "Redis", "Error should be for Redis backend");
        }
        _ => {
            panic!("Expected ConnectionError");
        }
    }
}
