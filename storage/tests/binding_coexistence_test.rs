use mk_core::types::{BranchPolicy, CredentialKind, RecordSource, RepositoryKind};
use storage::tenant_store::{
    TenantRepositoryBindingStore, TenantStore, UpsertTenantRepositoryBinding,
};
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

fn local_binding(
    tenant_id: &mk_core::types::TenantId,
    source: RecordSource,
    branch: &str,
) -> UpsertTenantRepositoryBinding {
    UpsertTenantRepositoryBinding {
        tenant_id: tenant_id.clone(),
        kind: RepositoryKind::Local,
        local_path: Some("/repo/knowledge".to_string()),
        remote_url: None,
        branch: branch.to_string(),
        branch_policy: BranchPolicy::DirectCommit,
        credential_kind: CredentialKind::None,
        credential_ref: None,
        github_owner: None,
        github_repo: None,
        source_owner: source,
        git_provider_connection_id: None,
    }
}

#[tokio::test]
async fn test_create_tenant_defaults_to_admin_source() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);
    let slug = unique_id("coex-admin");
    let record = store.create_tenant(&slug, &slug).await.unwrap();
    assert_eq!(
        record.source_owner,
        RecordSource::Admin,
        "create_tenant() must tag new tenants as Admin"
    );
}

#[tokio::test]
async fn test_ensure_tenant_with_sync_source_tags_new_tenant() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);
    let slug = unique_id("coex-sync");
    let record = store
        .ensure_tenant_with_source(&slug, RecordSource::Sync)
        .await
        .unwrap();
    assert_eq!(
        record.source_owner,
        RecordSource::Sync,
        "ensure_tenant_with_source(Sync) must tag new tenant as Sync"
    );
}

#[tokio::test]
async fn test_ensure_tenant_with_sync_source_does_not_change_existing_admin_tenant() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool);
    let slug = unique_id("coex-existing");

    let original = store.create_tenant(&slug, &slug).await.unwrap();
    assert_eq!(original.source_owner, RecordSource::Admin);

    let returned = store
        .ensure_tenant_with_source(&slug, RecordSource::Sync)
        .await
        .unwrap();
    assert_eq!(
        returned.source_owner,
        RecordSource::Admin,
        "ensure_tenant_with_source must not overwrite an existing Admin tenant's source_owner"
    );
    assert_eq!(returned.id, original.id, "must return the same tenant id");
}

#[tokio::test]
async fn test_sync_upsert_does_not_overwrite_admin_binding() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let binding_store = TenantRepositoryBindingStore::new(pool);

    let slug = unique_id("coex-guard");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();
    let tenant_id = &tenant.id;

    let admin_req = local_binding(tenant_id, RecordSource::Admin, "main");
    let admin_binding = binding_store.upsert_binding(admin_req).await.unwrap();
    assert_eq!(admin_binding.source_owner, RecordSource::Admin);
    assert_eq!(admin_binding.branch, "main");

    let sync_req = local_binding(tenant_id, RecordSource::Sync, "sync-branch");
    let after_sync = binding_store.upsert_binding(sync_req).await.unwrap();

    assert_eq!(
        after_sync.source_owner,
        RecordSource::Admin,
        "source_owner must remain Admin after a Sync upsert attempt"
    );
    assert_eq!(
        after_sync.branch, "main",
        "branch must NOT be overwritten by a Sync upsert when existing row is Admin-owned"
    );
}

#[tokio::test]
async fn test_admin_upsert_overwrites_existing_admin_binding() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let binding_store = TenantRepositoryBindingStore::new(pool);

    let slug = unique_id("coex-overwrite");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();
    let tenant_id = &tenant.id;

    binding_store
        .upsert_binding(local_binding(tenant_id, RecordSource::Admin, "main"))
        .await
        .unwrap();

    let updated = binding_store
        .upsert_binding(local_binding(tenant_id, RecordSource::Admin, "release"))
        .await
        .unwrap();

    assert_eq!(
        updated.branch, "release",
        "Admin upsert must overwrite an existing Admin-owned binding"
    );
}

#[tokio::test]
async fn test_admin_upsert_overwrites_sync_binding() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let binding_store = TenantRepositoryBindingStore::new(pool);

    let slug = unique_id("coex-admin-wins");
    let tenant = store.create_tenant(&slug, &slug).await.unwrap();
    let tenant_id = &tenant.id;

    binding_store
        .upsert_binding(local_binding(tenant_id, RecordSource::Sync, "sync-branch"))
        .await
        .unwrap();

    let admin_result = binding_store
        .upsert_binding(local_binding(
            tenant_id,
            RecordSource::Admin,
            "admin-branch",
        ))
        .await
        .unwrap();

    assert_eq!(admin_result.branch, "admin-branch");
    assert_eq!(admin_result.source_owner, RecordSource::Admin);
}

#[tokio::test]
async fn test_sync_binding_upsert_does_not_affect_other_tenant() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let binding_store = TenantRepositoryBindingStore::new(pool);

    let slug_a = unique_id("coex-cross-a");
    let slug_b = unique_id("coex-cross-b");
    let tenant_a = store.create_tenant(&slug_a, &slug_a).await.unwrap();
    let tenant_b = store.create_tenant(&slug_b, &slug_b).await.unwrap();

    binding_store
        .upsert_binding(local_binding(&tenant_a.id, RecordSource::Admin, "branch-a"))
        .await
        .unwrap();
    binding_store
        .upsert_binding(local_binding(&tenant_b.id, RecordSource::Admin, "branch-b"))
        .await
        .unwrap();

    binding_store
        .upsert_binding(local_binding(
            &tenant_a.id,
            RecordSource::Sync,
            "sync-branch",
        ))
        .await
        .unwrap();

    let b_binding = binding_store
        .get_binding(&tenant_b.id)
        .await
        .unwrap()
        .expect("tenant B binding must still exist");

    assert_eq!(
        b_binding.branch, "branch-b",
        "Tenant B's binding must not be affected by an upsert targeting tenant A"
    );
    assert_eq!(b_binding.source_owner, RecordSource::Admin);

    let a_binding = binding_store
        .get_binding(&tenant_a.id)
        .await
        .unwrap()
        .expect("tenant A binding must still exist");
    assert_eq!(
        a_binding.branch, "branch-a",
        "Tenant A's Admin binding must not be overwritten by Sync upsert"
    );
}

#[tokio::test]
async fn test_missing_binding_returns_none_not_another_tenants_binding() {
    let Some(pool) = setup_pool().await else {
        eprintln!("Skipping: Docker not available");
        return;
    };
    let store = TenantStore::new(pool.clone());
    let binding_store = TenantRepositoryBindingStore::new(pool);

    let slug_configured = unique_id("coex-configured");
    let slug_empty = unique_id("coex-empty");
    let configured = store
        .create_tenant(&slug_configured, &slug_configured)
        .await
        .unwrap();
    store.create_tenant(&slug_empty, &slug_empty).await.unwrap();
    let empty_id = store.get_tenant(&slug_empty).await.unwrap().unwrap().id;

    binding_store
        .upsert_binding(local_binding(&configured.id, RecordSource::Admin, "main"))
        .await
        .unwrap();

    let result = binding_store.get_binding(&empty_id).await.unwrap();
    assert!(
        result.is_none(),
        "A tenant with no binding must return None, never another tenant's binding"
    );
}
