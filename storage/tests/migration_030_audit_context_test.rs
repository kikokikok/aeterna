//! Migration 030 — `governance_audit_log` request-context columns.
//!
//! Verifies the five new nullable columns from B2 §11.1 are present, that
//! the `via` CHECK constraint rejects out-of-enum values, that old
//! `log_audit` callers continue to work unchanged, and that the new
//! `log_audit_with_extensions` method round-trips every field.
//!
//! Docker-gated via the shared `testing::postgres()` fixture.

#![cfg(test)]

use serde_json::json;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use storage::PrincipalType;
use storage::governance::{AuditExtensions, GovernanceStorage};
use uuid::Uuid;

async fn pool() -> Option<sqlx::PgPool> {
    let fixture = testing::postgres().await?;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(fixture.url())
        .await
        .ok()
}

#[tokio::test]
async fn new_columns_exist_with_expected_types() {
    let Some(pool) = pool().await else { return };

    // Query information_schema to confirm every column and its nullability.
    // Types are checked via `udt_name` (Postgres short type names).
    let rows = sqlx::query(
        r#"
        SELECT column_name, udt_name, is_nullable
        FROM information_schema.columns
        WHERE table_name = 'governance_audit_log'
          AND column_name IN ('via','client_version','manifest_hash','generation','dry_run')
        ORDER BY column_name
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("information_schema query");

    let found: Vec<(String, String, String)> = rows
        .iter()
        .map(|r| {
            (
                r.get::<String, _>("column_name"),
                r.get::<String, _>("udt_name"),
                r.get::<String, _>("is_nullable"),
            )
        })
        .collect();

    assert_eq!(
        found,
        vec![
            ("client_version".into(), "text".into(), "YES".into()),
            ("dry_run".into(), "bool".into(), "YES".into()),
            ("generation".into(), "int8".into(), "YES".into()),
            ("manifest_hash".into(), "text".into(), "YES".into()),
            ("via".into(), "text".into(), "YES".into()),
        ],
        "migration 030 must install exactly these five nullable columns"
    );
}

#[tokio::test]
async fn via_check_constraint_rejects_bogus_values() {
    let Some(pool) = pool().await else { return };

    // Direct INSERT bypassing the Rust layer — simulates a malicious or
    // buggy caller that skips normalization. The CHECK constraint should
    // catch it.
    let result = sqlx::query(
        r#"INSERT INTO governance_audit_log (action, actor_type, details, via)
           VALUES ('test', 'system', '{}', 'bogus-value')"#,
    )
    .execute(&pool)
    .await;

    let err = result.expect_err("CHECK constraint must reject 'bogus-value'");
    let msg = format!("{err}");
    assert!(
        msg.contains("governance_audit_log_via_check") || msg.contains("check constraint"),
        "expected CHECK violation, got: {msg}"
    );
}

#[tokio::test]
async fn via_check_accepts_each_canonical_value() {
    let Some(pool) = pool().await else { return };
    for via in ["cli", "ui", "api"] {
        sqlx::query(
            r#"INSERT INTO governance_audit_log (action, actor_type, details, via)
               VALUES ('test', 'system', '{}', $1)"#,
        )
        .bind(via)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("CHECK must accept canonical value {via:?}: {e}"));
    }
}

#[tokio::test]
async fn via_check_accepts_null() {
    let Some(pool) = pool().await else { return };
    // Pre-migration rows and non-provision actions legitimately leave
    // `via` NULL. The constraint must not regress that.
    sqlx::query(
        r#"INSERT INTO governance_audit_log (action, actor_type, details)
           VALUES ('test', 'system', '{}')"#,
    )
    .execute(&pool)
    .await
    .expect("NULL via must be allowed");
}

#[tokio::test]
async fn log_audit_preserves_pre_migration_030_semantics() {
    let Some(pool) = pool().await else { return };
    let store = GovernanceStorage::new(pool.clone());

    // Call the legacy 10-arg `log_audit` — every new column must land as NULL.
    let id = store
        .log_audit(
            "legacy.action",
            None,
            Some("tenant"),
            Some("t-legacy"),
            PrincipalType::System,
            None,
            None,
            json!({}),
            None,
        )
        .await
        .expect("legacy log_audit must succeed");

    let row = sqlx::query(
        r#"SELECT via, client_version, manifest_hash, generation, dry_run
           FROM governance_audit_log WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(row.get::<Option<String>, _>("via").is_none());
    assert!(row.get::<Option<String>, _>("client_version").is_none());
    assert!(row.get::<Option<String>, _>("manifest_hash").is_none());
    assert!(row.get::<Option<i64>, _>("generation").is_none());
    assert!(row.get::<Option<bool>, _>("dry_run").is_none());
}

#[tokio::test]
async fn log_audit_with_extensions_round_trips_every_field() {
    let Some(pool) = pool().await else { return };
    let store = GovernanceStorage::new(pool.clone());

    let ext = AuditExtensions {
        via: Some("cli".into()),
        client_version: Some("aeterna-cli/0.8.0-rc.3".into()),
        manifest_hash: Some("sha256:deadbeef".into()),
        generation: Some(7),
        dry_run: Some(true),
    };

    let id = store
        .log_audit_with_extensions(
            "tenant.provision.dry_run",
            None,
            Some("tenant"),
            Some("t-new"),
            PrincipalType::User,
            Some(Uuid::new_v4()),
            Some("ops@example.com"),
            json!({"plan": "noop"}),
            None,
            ext.clone(),
        )
        .await
        .expect("extension-aware insert must succeed");

    let row = sqlx::query(
        r#"SELECT via, client_version, manifest_hash, generation, dry_run
           FROM governance_audit_log WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<Option<String>, _>("via"), ext.via);
    assert_eq!(
        row.get::<Option<String>, _>("client_version"),
        ext.client_version
    );
    assert_eq!(
        row.get::<Option<String>, _>("manifest_hash"),
        ext.manifest_hash
    );
    assert_eq!(row.get::<Option<i64>, _>("generation"), ext.generation);
    assert_eq!(row.get::<Option<bool>, _>("dry_run"), ext.dry_run);
}

#[tokio::test]
async fn empty_extensions_is_equivalent_to_legacy_log_audit() {
    let Some(pool) = pool().await else { return };
    let store = GovernanceStorage::new(pool.clone());

    let id = store
        .log_audit_with_extensions(
            "action.empty_ext",
            None,
            None,
            None,
            PrincipalType::System,
            None,
            None,
            json!({}),
            None,
            AuditExtensions::empty(),
        )
        .await
        .unwrap();

    let row = sqlx::query(
        r#"SELECT via, client_version, manifest_hash, generation, dry_run
           FROM governance_audit_log WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(row.get::<Option<String>, _>("via").is_none());
    assert!(row.get::<Option<String>, _>("client_version").is_none());
    assert!(row.get::<Option<String>, _>("manifest_hash").is_none());
    assert!(row.get::<Option<i64>, _>("generation").is_none());
    assert!(row.get::<Option<bool>, _>("dry_run").is_none());
}
