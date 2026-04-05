use mk_core::types::RecordSource;
use storage::tenant_store::TenantStore;
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

async fn mapping_source(
    pool: &sqlx::PgPool,
    tenant_id: &mk_core::types::TenantId,
    domain: &str,
) -> String {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT source
        FROM tenant_domain_mappings
        WHERE tenant_id = $1::uuid AND lower(domain) = lower($2)
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(domain)
    .fetch_one(pool)
    .await
    .expect("mapping source exists")
}

#[tokio::test]
async fn test_sync_mapping_create_tags_new_mapping_as_sync() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let slug = unique_id("domain-sync-new");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();

    store
        .add_verified_domain_mapping_with_source(&slug, "sync.example.com", RecordSource::Sync)
        .await
        .unwrap();

    let source = mapping_source(&pool, &tenant.id, "sync.example.com").await;
    assert_eq!(source, "sync");
}

#[tokio::test]
async fn test_sync_mapping_upsert_does_not_overwrite_admin_mapping() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let slug = unique_id("domain-guard");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();

    store
        .add_verified_domain_mapping(&slug, "guard.example.com")
        .await
        .unwrap();
    store
        .add_verified_domain_mapping_with_source(&slug, "guard.example.com", RecordSource::Sync)
        .await
        .unwrap();

    let source = mapping_source(&pool, &tenant.id, "guard.example.com").await;
    assert_eq!(source, "admin");
}

#[tokio::test]
async fn test_admin_mapping_overwrites_existing_sync_mapping() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let slug = unique_id("domain-admin-wins");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();

    store
        .add_verified_domain_mapping_with_source(
            &slug,
            "admin-wins.example.com",
            RecordSource::Sync,
        )
        .await
        .unwrap();
    store
        .add_verified_domain_mapping(&slug, "admin-wins.example.com")
        .await
        .unwrap();

    let source = mapping_source(&pool, &tenant.id, "admin-wins.example.com").await;
    assert_eq!(source, "admin");
}

#[tokio::test]
async fn test_sync_mapping_upsert_does_not_affect_other_tenant() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let slug_a = unique_id("domain-cross-a");
    let slug_b = unique_id("domain-cross-b");
    let tenant_a = store.create_tenant(&slug_a, &slug_a).await.unwrap();
    let tenant_b = store.create_tenant(&slug_b, &slug_b).await.unwrap();

    store
        .add_verified_domain_mapping(&slug_a, "a.example.com")
        .await
        .unwrap();
    store
        .add_verified_domain_mapping(&slug_b, "b.example.com")
        .await
        .unwrap();

    store
        .add_verified_domain_mapping_with_source(&slug_a, "a.example.com", RecordSource::Sync)
        .await
        .unwrap();

    let source_a = mapping_source(&pool, &tenant_a.id, "a.example.com").await;
    let source_b = mapping_source(&pool, &tenant_b.id, "b.example.com").await;
    assert_eq!(source_a, "admin");
    assert_eq!(source_b, "admin");

    let resolved = store
        .resolve_verified_tenant(None, Some("user@b.example.com"))
        .await
        .unwrap();
    assert_eq!(resolved.tenant.id, tenant_b.id);
}
