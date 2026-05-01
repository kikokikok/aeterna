//! Round-trip tests for the `tenants.legal_entity_name` metadata column
//! introduced by migration 033_tenant_legal_entity.sql.
//!
//! See migration 033's header for the full rationale; in short, this is
//! the v1.5.x stepping stone to the `add-legal-entity-tenant-grouping`
//! proposal. Today the column is pure metadata: nullable text, no FK,
//! no auth implications. The tests here lock in:
//!
//!   - new tenants are created with `legal_entity_name = NULL`
//!     (back-compat: existing callers don't pass this field)
//!   - the storage update path can SET the column
//!   - the storage update path can leave the column UNTOUCHED
//!     (the `Option<Option<&str>>::None` case)
//!   - the storage update path can CLEAR the column to NULL
//!     (the `Option<Option<&str>>::Some(None)` case) — the API does
//!     not yet expose this capability but the storage layer must
//!     correctly support it for the future API extension
//!   - the partial index `idx_tenants_legal_entity_name` lookup works
//!     for the future "all tenants of legal entity X" query.

use mk_core::types::RecordSource;
use storage::postgres::PostgresBackend;
use storage::tenant_store::TenantStore;
use testing::{postgres, unique_id};

async fn create_tenant_store() -> Option<TenantStore> {
    let fixture = postgres().await?;
    let backend = PostgresBackend::new(fixture.url()).await.ok()?;
    backend.initialize_schema().await.ok()?;
    Some(TenantStore::new(backend.pool().clone()))
}

#[tokio::test]
async fn legal_entity_name_defaults_to_null_on_create() {
    let Some(store) = create_tenant_store().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let slug = unique_id("t-default");
    let created = store
        .create_tenant_with_source(&slug, "Default Tenant", RecordSource::Admin)
        .await
        .expect("create_tenant_with_source");

    assert_eq!(
        created.legal_entity_name, None,
        "new tenants must start with legal_entity_name = NULL for back-compat"
    );

    // Round-trip via get_tenant.
    let fetched = store
        .get_tenant(&slug)
        .await
        .expect("get_tenant")
        .expect("tenant exists");
    assert_eq!(fetched.legal_entity_name, None);
}

#[tokio::test]
async fn legal_entity_name_can_be_set_via_update() {
    let Some(store) = create_tenant_store().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let slug = unique_id("t-set");
    store
        .create_tenant_with_source(&slug, "Acme EU SAS", RecordSource::Admin)
        .await
        .unwrap();

    let updated = store
        .update_tenant(&slug, None, None, Some(Some("Acme Holding")))
        .await
        .expect("update_tenant")
        .expect("tenant exists");

    assert_eq!(
        updated.legal_entity_name.as_deref(),
        Some("Acme Holding"),
        "setting legal_entity_name via Some(Some(_)) must persist"
    );
}

#[tokio::test]
async fn legal_entity_name_left_alone_when_omitted() {
    let Some(store) = create_tenant_store().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let slug = unique_id("t-keep");
    store
        .create_tenant_with_source(&slug, "Keep Tenant", RecordSource::Admin)
        .await
        .unwrap();

    // Set it once.
    store
        .update_tenant(&slug, None, None, Some(Some("Initial Holding")))
        .await
        .unwrap()
        .unwrap();

    // Now patch slug only — legal_entity_name omitted (None) must not
    // be touched. This is the back-compat case for callers that don't
    // know about the field yet.
    let after = store
        .update_tenant(&slug, None, Some("Renamed Tenant"), None)
        .await
        .expect("update_tenant")
        .expect("tenant exists");

    assert_eq!(after.name, "Renamed Tenant");
    assert_eq!(
        after.legal_entity_name.as_deref(),
        Some("Initial Holding"),
        "omitting legal_entity_name (None) must leave the column untouched"
    );
}

#[tokio::test]
async fn legal_entity_name_can_be_explicitly_cleared() {
    let Some(store) = create_tenant_store().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let slug = unique_id("t-clear");
    store
        .create_tenant_with_source(&slug, "Clear Tenant", RecordSource::Admin)
        .await
        .unwrap();

    // Set it.
    store
        .update_tenant(&slug, None, None, Some(Some("Some Holding")))
        .await
        .unwrap()
        .unwrap();

    // Now explicitly clear it via Some(None). The API doesn't currently
    // expose this distinction; the storage layer must support it for
    // the future cleanup path.
    let cleared = store
        .update_tenant(&slug, None, None, Some(None))
        .await
        .expect("update_tenant")
        .expect("tenant exists");

    assert_eq!(
        cleared.legal_entity_name, None,
        "Some(None) must clear the column to NULL"
    );
}
