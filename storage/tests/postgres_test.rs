//! Integration tests for PostgreSQL storage backend
//!
//! These tests use testcontainers to spin up a PostgreSQL instance.

use mk_core::traits::StorageBackend;
use mk_core::types::{TenantContext, TenantId, UserId};
use storage::postgres::{PostgresBackend, PostgresError};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn setup_postgres_container()
-> Result<(ContainerAsync<Postgres>, String), Box<dyn std::error::Error>> {
    let container = Postgres::default()
        .with_db_name("testdb")
        .with_user("testuser")
        .with_password("testpass")
        .start()
        .await?;

    let connection_url = format!(
        "postgres://testuser:testpass@localhost:{}/testdb",
        container.get_host_port_ipv4(5432).await?
    );

    Ok((container, connection_url))
}

#[tokio::test]
async fn test_postgres_backend_new() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await;
            assert!(backend.is_ok(), "Should connect to PostgreSQL");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_initialize_schema() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            let result = backend.initialize_schema().await;
            assert!(result.is_ok(), "Should initialize schema");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_store_and_retrieve() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "test_key";
            let value = b"{\"test\": \"data\"}";
            let store_result = backend.store(ctx.clone(), key, value).await;
            assert!(store_result.is_ok(), "Should store data");

            let retrieve_result = backend.retrieve(ctx, key).await;
            assert!(retrieve_result.is_ok(), "Should retrieve data");
            let retrieved = retrieve_result.unwrap();
            assert!(retrieved.is_some(), "Should have retrieved data");
            let retrieved_json: serde_json::Value =
                serde_json::from_slice(&retrieved.unwrap()).unwrap();
            let expected_json: serde_json::Value = serde_json::from_slice(value).unwrap();
            assert_eq!(
                retrieved_json, expected_json,
                "Retrieved JSON should match semantically"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_store_update() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "update_key";
            let value1 = b"{\"version\": 1}";
            backend.store(ctx.clone(), key, value1).await.unwrap();

            let value2 = b"{\"version\": 2}";
            backend.store(ctx.clone(), key, value2).await.unwrap();

            let retrieved = backend.retrieve(ctx, key).await.unwrap().unwrap();
            let retrieved_json: serde_json::Value = serde_json::from_slice(&retrieved).unwrap();
            let expected_json: serde_json::Value = serde_json::from_slice(value2).unwrap();
            assert_eq!(
                retrieved_json, expected_json,
                "Should retrieve updated value"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_delete() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "delete_key";
            let value = b"{\"to_delete\": true}";
            backend.store(ctx.clone(), key, value).await.unwrap();

            let exists_before = backend.exists(ctx.clone(), key).await.unwrap();
            assert!(exists_before, "Key should exist before delete");

            let delete_result = backend.delete(ctx.clone(), key).await;
            assert!(delete_result.is_ok(), "Should delete data");

            let exists_after = backend.exists(ctx.clone(), key).await.unwrap();
            assert!(!exists_after, "Key should not exist after delete");

            let retrieved = backend.retrieve(ctx, key).await.unwrap();
            assert!(retrieved.is_none(), "Should return None for deleted key");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_exists() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let exists = backend.exists(ctx.clone(), "nonexistent").await.unwrap();
            assert!(!exists, "Nonexistent key should not exist");

            let key = "exists_key";
            let value = b"{\"exists\": true}";
            backend.store(ctx.clone(), key, value).await.unwrap();

            let exists = backend.exists(ctx, key).await.unwrap();
            assert!(exists, "Stored key should exist");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_retrieve_nonexistent() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let result = backend.retrieve(ctx, "nonexistent_key").await;
            assert!(result.is_ok(), "Should handle nonexistent key");
            assert!(
                result.unwrap().is_none(),
                "Should return None for nonexistent key"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_invalid_json() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "invalid_json_key";
            let invalid_json = b"not valid json";
            let result = backend.store(ctx.clone(), key, invalid_json).await;
            assert!(result.is_ok(), "Should handle invalid JSON gracefully");

            let retrieved = backend.retrieve(ctx, key).await.unwrap();
            assert!(retrieved.is_some(), "Should retrieve something");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_tenant_isolation() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx1 = TenantContext::new(
                TenantId::new("tenant-1".to_string()).unwrap(),
                UserId::default(),
            );
            let ctx2 = TenantContext::new(
                TenantId::new("tenant-2".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "shared-key";
            let val1 = b"{\"tenant\": 1}";
            let val2 = b"{\"tenant\": 2}";

            // Tenant 1 stores data
            backend.store(ctx1.clone(), key, val1).await.unwrap();

            // Tenant 2 should NOT see it
            let res2 = backend.retrieve(ctx2.clone(), key).await.unwrap();
            assert!(res2.is_none(), "Tenant 2 should not see Tenant 1 data");

            // Tenant 2 stores different data for same key
            backend.store(ctx2.clone(), key, val2).await.unwrap();

            // Both should now see their own data
            let res1 = backend.retrieve(ctx1.clone(), key).await.unwrap();
            let res1_json: serde_json::Value = serde_json::from_slice(&res1.unwrap()).unwrap();
            let val1_json: serde_json::Value = serde_json::from_slice(val1).unwrap();
            assert_eq!(res1_json, val1_json);

            let res2 = backend.retrieve(ctx2.clone(), key).await.unwrap();
            let res2_json: serde_json::Value = serde_json::from_slice(&res2.unwrap()).unwrap();
            let val2_json: serde_json::Value = serde_json::from_slice(val2).unwrap();
            assert_eq!(res2_json, val2_json);

            // Deleting from Tenant 1 should NOT affect Tenant 2
            backend.delete(ctx1.clone(), key).await.unwrap();
            assert!(!backend.exists(ctx1, key).await.unwrap());
            assert!(backend.exists(ctx2, key).await.unwrap());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_connection_error() {
    let result = PostgresBackend::new("postgres://invalid:5432/invalid").await;
    assert!(result.is_err(), "Should fail with invalid connection");

    match result {
        Err(PostgresError::Database(_)) => {}
        _ => {
            panic!("Expected PostgresError::Database");
        }
    }
}
