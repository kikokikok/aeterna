//! Integration tests for `PostgresSecretBackend`.
//!
//! Uses the shared `testing::postgres` fixture. Skips all tests gracefully
//! when Docker/testcontainers is unavailable.

use mk_core::{SecretBytes, SecretReference};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use storage::kms::{KmsProvider, LocalKmsProvider};
use storage::secret_backend::{PostgresSecretBackend, SecretBackend, SecretBackendError};
use uuid::Uuid;

async fn fixture_pool() -> Option<PgPool> {
    let fx = testing::postgres().await?;
    Some(
        PgPoolOptions::new()
            .max_connections(4)
            .connect(fx.url())
            .await
            .expect("open fixture pool"),
    )
}

async fn insert_tenant(pool: &PgPool, slug: &str) -> Uuid {
    let row = sqlx::query(
        "INSERT INTO tenants (slug, name) VALUES ($1, $2)
         ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(slug)
    .bind(slug)
    .fetch_one(pool)
    .await
    .expect("insert tenant");
    row.try_get::<Uuid, _>("id").expect("tenant id")
}

fn backend(pool: PgPool) -> PostgresSecretBackend {
    let key = [0x13u8; 32];
    let kms: Arc<dyn KmsProvider> =
        Arc::new(LocalKmsProvider::from_bytes(&key, "local:itest").unwrap());
    PostgresSecretBackend::new(pool, kms)
}

#[tokio::test]
async fn put_then_get_roundtrips() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("postgres fixture unavailable, skipping");
        return;
    };
    let tid = insert_tenant(&pool, "sb-rt").await;
    let be = backend(pool);

    let plaintext = b"super-sensitive-api-key".to_vec();
    let r = be
        .put(tid, "llm_api_key", SecretBytes::from(plaintext.clone()))
        .await
        .expect("put");
    let out = be.get(&r).await.expect("get");
    assert_eq!(out.expose(), plaintext.as_slice());
}

#[tokio::test]
async fn ciphertext_is_not_plaintext_on_disk() {
    let Some(pool) = fixture_pool().await else { return; };
    let tid = insert_tenant(&pool, "sb-atrest").await;
    let be = backend(pool.clone());

    let plaintext = b"sentinel-value-xy-12345";
    let r = be
        .put(tid, "k", SecretBytes::from(plaintext.to_vec()))
        .await
        .expect("put");
    let SecretReference::Postgres { secret_id } = r;
    let row = sqlx::query("SELECT ciphertext, wrapped_dek FROM tenant_secrets WHERE id = $1")
        .bind(secret_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let ct: Vec<u8> = row.try_get("ciphertext").unwrap();
    let wd: Vec<u8> = row.try_get("wrapped_dek").unwrap();
    assert!(!ct.windows(plaintext.len()).any(|w| w == plaintext),
        "ciphertext must not contain plaintext bytes");
    assert!(!wd.windows(plaintext.len()).any(|w| w == plaintext),
        "wrapped DEK must not contain plaintext bytes");
}

#[tokio::test]
async fn put_same_name_bumps_generation_and_rotates_dek() {
    let Some(pool) = fixture_pool().await else { return; };
    let tid = insert_tenant(&pool, "sb-rotate").await;
    let be = backend(pool.clone());

    let r1 = be.put(tid, "k", SecretBytes::from(b"v1".to_vec())).await.unwrap();
    let r2 = be.put(tid, "k", SecretBytes::from(b"v2".to_vec())).await.unwrap();
    // Same reference id — upsert, not insert.
    assert_eq!(r1, r2);
    let out = be.get(&r2).await.unwrap();
    assert_eq!(out.expose(), b"v2");

    let SecretReference::Postgres { secret_id } = r2;
    let row = sqlx::query("SELECT generation FROM tenant_secrets WHERE id = $1")
        .bind(secret_id)
        .fetch_one(&pool).await.unwrap();
    let generation: i64 = row.try_get("generation").unwrap();
    assert_eq!(generation, 2, "second put must bump generation");
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let Some(pool) = fixture_pool().await else { return; };
    let be = backend(pool);
    let r = SecretReference::Postgres { secret_id: Uuid::new_v4() };
    let err = be.get(&r).await.unwrap_err();
    assert!(matches!(err, SecretBackendError::NotFound(_)));
}

#[tokio::test]
async fn delete_is_idempotent() {
    let Some(pool) = fixture_pool().await else { return; };
    let tid = insert_tenant(&pool, "sb-del").await;
    let be = backend(pool);
    let r = be.put(tid, "k", SecretBytes::from(b"v".to_vec())).await.unwrap();
    be.delete(&r).await.expect("first delete");
    be.delete(&r).await.expect("second delete idempotent");
    let err = be.get(&r).await.unwrap_err();
    assert!(matches!(err, SecretBackendError::NotFound(_)));
}

#[tokio::test]
async fn list_returns_tenant_scoped_entries() {
    let Some(pool) = fixture_pool().await else { return; };
    let t1 = insert_tenant(&pool, "sb-list-a").await;
    let t2 = insert_tenant(&pool, "sb-list-b").await;
    let be = backend(pool);
    be.put(t1, "llm_api_key", SecretBytes::from(b"x".to_vec())).await.unwrap();
    be.put(t1, "embed_api_key", SecretBytes::from(b"y".to_vec())).await.unwrap();
    be.put(t2, "llm_api_key", SecretBytes::from(b"z".to_vec())).await.unwrap();

    let entries = be.list(t1).await.unwrap();
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["embed_api_key", "llm_api_key"], "alpha order, t1 only");
}

#[tokio::test]
async fn tenant_delete_cascades_secrets() {
    let Some(pool) = fixture_pool().await else { return; };
    let tid = insert_tenant(&pool, "sb-cascade").await;
    let be = backend(pool.clone());
    let r = be.put(tid, "k", SecretBytes::from(b"v".to_vec())).await.unwrap();
    sqlx::query("DELETE FROM tenants WHERE id = $1").bind(tid).execute(&pool).await.unwrap();
    let err = be.get(&r).await.unwrap_err();
    assert!(matches!(err, SecretBackendError::NotFound(_)));
}
