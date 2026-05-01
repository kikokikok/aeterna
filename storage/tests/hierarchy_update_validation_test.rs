//! Integration tests for the hierarchy invariants enforced by
//! `PostgresBackend::update_unit`.
//!
//! Pre-v1.5.0 `update_unit` skipped *all* validation — the matrix check
//! that `create_unit` performed was missing on the update path, so it was
//! possible to silently move a Project directly under a Company, or to
//! make a non-Company a root, or (in principle) to create a self-parent
//! cycle. The rc.9 sweep extracts a shared `validate_unit_invariants`
//! validator and routes both create and update through it; these tests
//! lock in the update-side coverage that did not previously exist.
//!
//! What's covered here:
//!   - matrix violation on update (the original latent bug)
//!   - self-parent on update (cycle rule 3a)
//!   - non-Company set as root on update (root rule)
//!   - legitimate reparent across same-type parents succeeds
//!   - dropping parent on a Company succeeds (root → root)
//!
//! What's *not* covered, by design:
//!   - multi-step ancestor cycle (rule 3b). With the strict-by-type
//!     matrix today, a multi-step cycle is structurally impossible: a
//!     Project can't have Project descendants because Project can't
//!     parent anything, etc. Reaching rule 3b requires either bypassing
//!     the matrix via direct SQL (gross, brittle) or relaxing the matrix
//!     to allow same-type recursion. The latter is the
//!     `add-legal-entity-tenant-grouping` proposal; if it lands, this
//!     test file should grow a `multistep_ancestor_cycle_rejected` case.

use mk_core::traits::StorageBackend;
use mk_core::types::{OrganizationalUnit, RecordSource, TenantId, UnitType};
use storage::postgres::PostgresBackend;
use testing::{postgres, unique_id};

async fn create_test_backend() -> Option<PostgresBackend> {
    let fixture = postgres().await?;
    let backend = PostgresBackend::new(fixture.url()).await.ok()?;
    backend.initialize_schema().await.ok()?;
    Some(backend)
}

fn unit(
    id: &str,
    name: &str,
    unit_type: UnitType,
    parent_id: Option<String>,
    tenant_id: &TenantId,
) -> OrganizationalUnit {
    OrganizationalUnit {
        id: id.to_string(),
        name: name.to_string(),
        unit_type,
        tenant_id: tenant_id.clone(),
        parent_id,
        metadata: std::collections::HashMap::new(),
        created_at: chrono::DateTime::from_timestamp(1000, 0).unwrap(),
        updated_at: chrono::DateTime::from_timestamp(1000, 0).unwrap(),
        source_owner: RecordSource::Admin,
    }
}

// ---------------------------------------------------------------------------
// Test: the original latent bug — update_unit used to allow a Project to
// be moved directly under a Company, which the matrix forbids on create.
// Now the same matrix runs on update.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn update_unit_rejects_matrix_violation() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let comp_id = unique_id("comp");
    let org_id = unique_id("org");
    let team_id = unique_id("team");
    let proj_id = unique_id("proj");

    storage
        .create_unit(&unit(&comp_id, "C", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();
    storage
        .create_unit(&unit(
            &org_id,
            "O",
            UnitType::Organization,
            Some(comp_id.clone()),
            &tenant_id,
        ))
        .await
        .unwrap();
    storage
        .create_unit(&unit(
            &team_id,
            "T",
            UnitType::Team,
            Some(org_id.clone()),
            &tenant_id,
        ))
        .await
        .unwrap();
    storage
        .create_unit(&unit(
            &proj_id,
            "P",
            UnitType::Project,
            Some(team_id.clone()),
            &tenant_id,
        ))
        .await
        .unwrap();

    // Try to move the Project directly under the Company. This was the
    // original silent-acceptance bug. Now must fail.
    let mut bad = unit(
        &proj_id,
        "P",
        UnitType::Project,
        Some(comp_id.clone()),
        &tenant_id,
    );
    bad.updated_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

    let result = storage.update_unit(&bad).await;
    assert!(
        result.is_err(),
        "update_unit must reject moving a Project directly under a Company"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("Invalid hierarchy"),
        "error must mention the matrix violation; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Test: rule 3a — a unit cannot be its own parent.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn update_unit_rejects_self_parent() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let comp_id = unique_id("comp");

    storage
        .create_unit(&unit(&comp_id, "C", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();

    // Try to set Company as its own parent.
    let mut bad = unit(
        &comp_id,
        "C",
        UnitType::Company,
        Some(comp_id.clone()),
        &tenant_id,
    );
    bad.updated_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

    let result = storage.update_unit(&bad).await;
    assert!(
        result.is_err(),
        "update_unit must reject a self-parent cycle"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("Cycle") || msg.contains("cycle"),
        "error must mention the cycle; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Test: root rule — only Company may be a root. Updating an Org to have
// no parent must be rejected.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn update_unit_rejects_non_company_as_root() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let comp_id = unique_id("comp");
    let org_id = unique_id("org");

    storage
        .create_unit(&unit(&comp_id, "C", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();
    storage
        .create_unit(&unit(
            &org_id,
            "O",
            UnitType::Organization,
            Some(comp_id.clone()),
            &tenant_id,
        ))
        .await
        .unwrap();

    // Try to make the Org a root.
    let mut bad = unit(&org_id, "O", UnitType::Organization, None, &tenant_id);
    bad.updated_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

    let result = storage.update_unit(&bad).await;
    assert!(
        result.is_err(),
        "update_unit must reject making a non-Company a root"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("Company") && msg.contains("root"),
        "error must mention the root rule; got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Test: a *legitimate* reparent (Organization moves between two Companies
// in the same tenant) must succeed.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn update_unit_allows_legitimate_reparent() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let c1 = unique_id("c1");
    let c2 = unique_id("c2");
    let org_id = unique_id("org");

    storage
        .create_unit(&unit(&c1, "C1", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();
    storage
        .create_unit(&unit(&c2, "C2", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();
    storage
        .create_unit(&unit(
            &org_id,
            "O",
            UnitType::Organization,
            Some(c1.clone()),
            &tenant_id,
        ))
        .await
        .unwrap();

    // Move Org from C1 to C2.
    let mut moved = unit(
        &org_id,
        "O",
        UnitType::Organization,
        Some(c2.clone()),
        &tenant_id,
    );
    moved.updated_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

    storage
        .update_unit(&moved)
        .await
        .expect("reparent between Companies in the same tenant must be allowed");
}

// ---------------------------------------------------------------------------
// Test: Company.parent_id may legitimately be None on update (root → root
// rewrite, e.g. to update name or metadata).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn update_unit_allows_company_root() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let comp_id = unique_id("comp");

    storage
        .create_unit(&unit(&comp_id, "C", UnitType::Company, None, &tenant_id))
        .await
        .unwrap();

    let mut renamed = unit(&comp_id, "C-renamed", UnitType::Company, None, &tenant_id);
    renamed.updated_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

    storage
        .update_unit(&renamed)
        .await
        .expect("renaming a root Company must be allowed");
}
