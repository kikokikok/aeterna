use storage::tenant_store::{TenantResolutionError, TenantStore};
use testing::{postgres, unique_id};

async fn setup_pool() -> Option<sqlx::PgPool> {
    let fixture = postgres().await?;
    let _pool = sqlx::PgPool::connect(fixture.url())
        .await
        .expect("connect to testcontainer");

    storage::postgres::PostgresBackend::new(fixture.url())
        .await
        .ok()?
        .initialize_schema()
        .await
        .ok()?;

    let pool = sqlx::PgPool::connect(fixture.url())
        .await
        .expect("reconnect");
    Some(pool)
}

#[tokio::test]
async fn resolve_verified_tenant_fails_closed_when_domain_mapping_is_ambiguous() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let slug_a = unique_id("tenant-ambig-a");
    let slug_b = unique_id("tenant-ambig-b");
    store.create_tenant(&slug_a, &slug_a).await.unwrap();
    store.create_tenant(&slug_b, &slug_b).await.unwrap();

    store
        .add_verified_domain_mapping(&slug_a, "shared.example.com")
        .await
        .unwrap();
    store
        .add_verified_domain_mapping(&slug_b, "shared.example.com")
        .await
        .unwrap();

    let err = store
        .resolve_verified_tenant(None, Some("user@shared.example.com"))
        .await
        .expect_err("ambiguous verified mappings must fail closed");

    match err {
        TenantResolutionError::AmbiguousVerifiedMapping(tenant_ids) => {
            assert_eq!(tenant_ids.len(), 2);
        }
        other => panic!("expected AmbiguousVerifiedMapping, got {other:?}"),
    }
}

#[tokio::test]
async fn resolve_verified_tenant_reports_missing_mapping_when_absent() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);

    let err = store
        .resolve_verified_tenant(None, Some("user@missing.example.com"))
        .await
        .expect_err("missing verified mappings must fail closed");

    match err {
        TenantResolutionError::MissingVerifiedMapping => {}
        other => panic!("expected MissingVerifiedMapping, got {other:?}"),
    }
}
