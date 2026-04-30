//! B1 — Tenant onboarding self-test.
//!
//! Closes the rc.9 §6 gap: validates that the **production wiring** for
//! tenant secret persistence works end-to-end against a real Postgres,
//! starting from the public factory [`storage::secret_backend::build_secret_backend_from_env`]
//! that the CLI bootstrap calls in production.
//!
//! Why a separate test file (rather than extending
//! `secret_backend_integration.rs`):
//!
//! * These tests **mutate process-global env vars** (`AETERNA_ENV`,
//!   `AETERNA_KMS_PROVIDER`, `AETERNA_LOCAL_KMS_KEY`). Without isolation
//!   they would race against the existing integration tests, which assume
//!   defaults. `serial_test::serial` keeps every case in this file
//!   strictly sequential.
//! * The other file exercises [`storage::secret_backend::PostgresSecretBackend`]
//!   directly with a hand-rolled KMS — useful for unit-style coverage but
//!   does **not** prove the production factory works.
//!
//! Coverage:
//!
//! 1. `factory_round_trip_in_development_with_local_kms` — the default
//!    dev path: `AETERNA_ENV=dev`, `AETERNA_KMS_PROVIDER=local`. Provisions
//!    a tenant row, stores its first secret, retrieves it, lists it,
//!    deletes it. This is what every dev workstation and CI run goes
//!    through.
//! 2. `factory_rejects_local_kms_in_production` — the A4 gate as wired
//!    through the real factory. `AETERNA_ENV=production` +
//!    `AETERNA_KMS_PROVIDER=local` must fail at construction time, before
//!    any tenant row can be touched.
//! 3. `factory_missing_aws_arn_in_production_returns_unsupported_reference` —
//!    `AETERNA_ENV=production` + `AETERNA_KMS_PROVIDER=aws` without
//!    `AETERNA_KMS_AWS_KEY_ARN` must surface a config error rather than
//!    silently fall through to the `local` branch.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use mk_core::SecretBytes;
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use storage::secret_backend::{SecretBackendError, build_secret_backend_from_env};
use uuid::Uuid;

/// Open a pool against the shared `testing::postgres` fixture. Returns
/// `None` when Docker / testcontainers is unavailable so the test skips
/// gracefully on developer laptops without Docker.
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

/// Insert a tenant row directly. Mirrors the side effect of a successful
/// `POST /admin/tenants/provision`, minus the auth and manifest-validation
/// layers that are covered by the `tenant_api` unit suite.
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

/// Set / clear the env vars the production factory reads. We use
/// `unsafe { std::env::set_var }` because Rust 2024 marks env mutation as
/// `unsafe` (background process safety); `serial_test::serial` ensures
/// no other test runs concurrently for the duration of the test.
fn set_env(env: Option<&str>, kms: Option<&str>, key: Option<&str>, arn: Option<&str>) {
    let pairs: [(&str, Option<&str>); 4] = [
        ("AETERNA_ENV", env),
        ("AETERNA_KMS_PROVIDER", kms),
        ("AETERNA_LOCAL_KMS_KEY", key),
        ("AETERNA_KMS_AWS_KEY_ARN", arn),
    ];
    for (name, value) in pairs {
        match value {
            Some(v) => unsafe { std::env::set_var(name, v) },
            None => unsafe { std::env::remove_var(name) },
        }
    }
}

/// 32 zero bytes, base64-encoded — a valid `AETERNA_LOCAL_KMS_KEY` for the
/// dev / CI factory path. Only used in tests; never appears in any
/// production fixture or seed.
fn dev_local_kms_key_b64() -> String {
    BASE64_STD.encode([0u8; 32])
}

#[tokio::test]
#[serial]
async fn factory_round_trip_in_development_with_local_kms() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("postgres fixture unavailable, skipping B1 round-trip");
        return;
    };

    set_env(
        Some("development"),
        Some("local"),
        Some(&dev_local_kms_key_b64()),
        None,
    );

    let backend = build_secret_backend_from_env(pool.clone())
        .await
        .expect("factory must succeed in development with a local KMS");

    let tid = insert_tenant(&pool, "b1-rt").await;
    let plaintext = b"first-secret-after-onboarding".to_vec();

    // Onboarding round-trip: put → get → list → delete.
    let reference = backend
        .put(tid, "git_token", SecretBytes::from(plaintext.clone()))
        .await
        .expect("first secret put after provisioning");

    let fetched = backend.get(&reference).await.expect("get");
    assert_eq!(fetched.expose(), plaintext.as_slice());

    let listed = backend.list(tid).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, "git_token");

    backend.delete(&reference).await.expect("delete");
    let after = backend.list(tid).await.expect("list-after-delete");
    assert!(after.is_empty(), "list must be empty after delete");

    set_env(None, None, None, None);
}

#[tokio::test]
#[serial]
async fn factory_rejects_local_kms_in_production() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("postgres fixture unavailable, skipping B1 prod-gate");
        return;
    };

    set_env(
        Some("production"),
        Some("local"),
        Some(&dev_local_kms_key_b64()),
        None,
    );

    let result = build_secret_backend_from_env(pool).await;
    set_env(None, None, None, None);

    match result {
        Err(SecretBackendError::ProductionSafety { selector, env }) => {
            assert_eq!(selector, "local");
            assert_eq!(env, "production");
        }
        Err(other) => panic!("wrong error variant: {other:?}"),
        Ok(_) => panic!("factory must reject local KMS in production"),
    }
}

#[tokio::test]
#[serial]
async fn factory_missing_aws_arn_in_production_returns_unsupported_reference() {
    let Some(pool) = fixture_pool().await else {
        eprintln!("postgres fixture unavailable, skipping B1 missing-arn");
        return;
    };

    set_env(
        Some("production"),
        Some("aws"),
        Some(&dev_local_kms_key_b64()), // ignored when selector=aws
        None,                           // ← the actual gap
    );

    let result = build_secret_backend_from_env(pool).await;
    set_env(None, None, None, None);

    match result {
        Err(SecretBackendError::UnsupportedReference("AETERNA_KMS_AWS_KEY_ARN")) => {}
        Err(other) => panic!(
            "missing ARN must surface as UnsupportedReference, not silently fall through to local; got: {other:?}"
        ),
        Ok(_) => panic!("factory must fail when AETERNA_KMS_AWS_KEY_ARN is unset"),
    }
}
