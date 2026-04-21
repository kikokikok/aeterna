//! Integration tests for the manifest-state read/write surface on
//! `TenantStore` (B2 task 1.5).
//!
//! These tests require Docker for the `postgres()` testcontainer fixture and
//! self-skip with a log line when it is not available, matching the pattern
//! used throughout `storage/tests/`.

use storage::tenant_store::TenantStore;
use testing::{postgres, unique_id};

async fn setup_pool() -> Option<sqlx::PgPool> {
    let fixture = postgres().await?;
    storage::postgres::PostgresBackend::new(fixture.url())
        .await
        .ok()?
        .initialize_schema()
        .await
        .ok()?;
    Some(
        sqlx::PgPool::connect(fixture.url())
            .await
            .expect("reconnect"),
    )
}

const VALID_HASH_A: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000001";
const VALID_HASH_B: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000002";

#[tokio::test]
async fn fresh_tenant_has_null_hash_and_zero_generation() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let slug = unique_id("ms-fresh");
    store.create_tenant(&slug, &slug).await.unwrap();

    let (hash, generation) = store.get_manifest_state(&slug).await.unwrap();
    assert!(
        hash.is_none(),
        "fresh tenant must have NULL hash so callers do not short-circuit"
    );
    assert_eq!(
        generation, 0,
        "fresh tenant must have generation = 0 (sentinel for 'never applied')"
    );
}

#[tokio::test]
async fn set_then_read_roundtrips_exactly() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let slug = unique_id("ms-rt");
    store.create_tenant(&slug, &slug).await.unwrap();

    store
        .set_manifest_state(&slug, VALID_HASH_A, 7)
        .await
        .unwrap();

    let (hash, gen_) = store.get_manifest_state(&slug).await.unwrap();
    assert_eq!(hash.as_deref(), Some(VALID_HASH_A));
    assert_eq!(gen_, 7);

    // Overwriting must update both fields atomically.
    store
        .set_manifest_state(&slug, VALID_HASH_B, 8)
        .await
        .unwrap();

    let (hash, gen_) = store.get_manifest_state(&slug).await.unwrap();
    assert_eq!(hash.as_deref(), Some(VALID_HASH_B));
    assert_eq!(gen_, 8);
}

#[tokio::test]
async fn get_manifest_state_returns_not_found_for_unknown_slug() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let err = store
        .get_manifest_state("this-slug-does-not-exist-ever")
        .await
        .expect_err("unknown slug must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("not found") || msg.to_lowercase().contains("not found"),
        "error must say not found, got: {msg}"
    );
}

#[tokio::test]
async fn set_manifest_state_returns_not_found_for_unknown_slug() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let err = store
        .set_manifest_state("also-does-not-exist", VALID_HASH_A, 1)
        .await
        .expect_err("unknown slug must error on UPDATE too");
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("not found"),
        "error must say not found, got: {msg}"
    );
}

#[tokio::test]
async fn db_rejects_malformed_hash_via_check_constraint() {
    // The column CHECK constraint tenants_manifest_hash_format enforces the
    // sha256:... shape. A caller that bypasses the application guard still
    // gets caught at the DB. We deliberately use a raw UPDATE to prove the
    // constraint is live, since set_manifest_state itself would reject the
    // bad string only if we added app-level validation (we haven't -- the
    // DB is the last line of defence on format).
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());

    let slug = unique_id("ms-badhash");
    store.create_tenant(&slug, &slug).await.unwrap();

    // "sha256:" + only 63 hex chars --> must fail the CHECK.
    let bad = "sha256:abcd";
    let res = sqlx::query("UPDATE tenants SET last_applied_manifest_hash = $1 WHERE slug = $2")
        .bind(bad)
        .bind(&slug)
        .execute(&pool)
        .await;
    assert!(
        res.is_err(),
        "DB must reject malformed manifest hash, but UPDATE succeeded"
    );
}

#[tokio::test]
async fn db_rejects_negative_generation_via_check_constraint() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());

    let slug = unique_id("ms-neg");
    store.create_tenant(&slug, &slug).await.unwrap();

    let res = sqlx::query("UPDATE tenants SET manifest_generation = -1 WHERE slug = $1")
        .bind(&slug)
        .execute(&pool)
        .await;
    assert!(
        res.is_err(),
        "DB must reject negative manifest_generation, but UPDATE succeeded"
    );
}

#[tokio::test]
async fn set_manifest_state_tx_commits_with_outer_transaction() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());

    let slug = unique_id("ms-tx");
    store.create_tenant(&slug, &slug).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    TenantStore::set_manifest_state_tx(&mut tx, &slug, VALID_HASH_A, 3)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (hash, gen_) = store.get_manifest_state(&slug).await.unwrap();
    assert_eq!(hash.as_deref(), Some(VALID_HASH_A));
    assert_eq!(gen_, 3);
}

#[tokio::test]
async fn set_manifest_state_tx_rollback_leaves_state_untouched() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());

    let slug = unique_id("ms-rb");
    store.create_tenant(&slug, &slug).await.unwrap();
    store
        .set_manifest_state(&slug, VALID_HASH_A, 1)
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    TenantStore::set_manifest_state_tx(&mut tx, &slug, VALID_HASH_B, 99)
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    let (hash, gen_) = store.get_manifest_state(&slug).await.unwrap();
    assert_eq!(
        hash.as_deref(),
        Some(VALID_HASH_A),
        "rollback must not persist the tx-scoped write"
    );
    assert_eq!(gen_, 1);
}
