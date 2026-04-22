//! Integration tests for `TenantStore::list_domain_mappings`.
//!
//! The renderer (`cli/src/server/manifest_render.rs`) depends on the
//! documented invariants of this method:
//!
//! - empty vec for a tenant with no mappings,
//! - sorted ASC by domain (renderer does not re-sort),
//! - `NotFound`-class error for an unknown tenant ref.
//!
//! These tests lock all three so a future refactor cannot silently
//! break the manifest render contract.

use storage::postgres::PostgresError;
use storage::tenant_store::TenantStore;
use testing::{postgres, unique_id};

async fn setup_store() -> Option<TenantStore> {
    let fixture = postgres().await?;
    storage::postgres::PostgresBackend::new(fixture.url())
        .await
        .ok()?
        .initialize_schema()
        .await
        .ok()?;
    let pool = sqlx::PgPool::connect(fixture.url()).await.ok()?;
    Some(TenantStore::new(pool))
}

#[tokio::test]
async fn list_domain_mappings_returns_empty_for_tenant_with_no_mappings() {
    let Some(store) = setup_store().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let slug = unique_id("tenant-nodoms");
    store.create_tenant(&slug, &slug).await.unwrap();

    let domains = store.list_domain_mappings(&slug).await.unwrap();
    assert!(
        domains.is_empty(),
        "expected empty vec for tenant with no mappings, got {domains:?}"
    );
}

#[tokio::test]
async fn list_domain_mappings_returns_sorted_lexicographically() {
    let Some(store) = setup_store().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let slug = unique_id("tenant-sorted");
    store.create_tenant(&slug, &slug).await.unwrap();

    // Insert out of order deliberately so we know the ordering comes
    // from the SQL, not from insertion order.
    for d in [
        "zulu.example.com",
        "alpha.example.com",
        "mike.example.com",
        "bravo.example.com",
    ] {
        store.add_verified_domain_mapping(&slug, d).await.unwrap();
    }

    let domains = store.list_domain_mappings(&slug).await.unwrap();
    assert_eq!(
        domains,
        vec![
            "alpha.example.com".to_string(),
            "bravo.example.com".to_string(),
            "mike.example.com".to_string(),
            "zulu.example.com".to_string(),
        ],
        "list_domain_mappings must return ASC-sorted domains; the manifest \
         renderer relies on this for deterministic output across pods"
    );
}

#[tokio::test]
async fn list_domain_mappings_is_tenant_scoped() {
    let Some(store) = setup_store().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let slug_a = unique_id("tenant-scope-a");
    let slug_b = unique_id("tenant-scope-b");
    store.create_tenant(&slug_a, &slug_a).await.unwrap();
    store.create_tenant(&slug_b, &slug_b).await.unwrap();

    store
        .add_verified_domain_mapping(&slug_a, "only-a.example.com")
        .await
        .unwrap();
    store
        .add_verified_domain_mapping(&slug_b, "only-b.example.com")
        .await
        .unwrap();

    let a = store.list_domain_mappings(&slug_a).await.unwrap();
    let b = store.list_domain_mappings(&slug_b).await.unwrap();
    assert_eq!(a, vec!["only-a.example.com".to_string()]);
    assert_eq!(b, vec!["only-b.example.com".to_string()]);
}

#[tokio::test]
async fn list_domain_mappings_returns_not_found_for_unknown_tenant() {
    let Some(store) = setup_store().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let err = store
        .list_domain_mappings("no-such-tenant-slug")
        .await
        .expect_err("unknown tenant ref must error, not return empty");
    match err {
        PostgresError::NotFound(msg) => {
            assert!(
                msg.contains("no-such-tenant-slug"),
                "error must name the missing ref, got: {msg}"
            );
        }
        other => panic!("expected NotFound, got {other:?}"),
    }
}
