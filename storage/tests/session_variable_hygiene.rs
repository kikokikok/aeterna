//! H1 regression test — session-variable hygiene on `app.tenant_id`.
//!
//! Verifies the invariant documented in `PostgresBackend::activate_tenant_context`:
//! after a transaction that called `set_config('app.tenant_id', …, true)` commits,
//! subsequent connections drawn from the pool MUST NOT observe the setting.
//!
//! Fails loudly if anyone reverts the third argument to `false` (session-scoped)
//! or if anyone else introduces a session-scoped `set_config` on the app GUC.
//! This is the tripwire for hazard H1 in the RLS enforcement RFC (issue #58).

use storage::postgres::PostgresBackend;
use testing::postgres;

async fn create_test_backend() -> Option<PostgresBackend> {
    let fixture = postgres().await?;
    let backend = PostgresBackend::new(fixture.url()).await.ok()?;
    backend.initialize_schema().await.ok()?;
    Some(backend)
}

/// The tenant_id we set in the "poisoning" transaction. If H1 ever regresses
/// (session-scope set_config), this string leaks to subsequent connections.
const MARKER_TENANT: &str = "h1-regression-marker-tenant";

#[tokio::test]
async fn session_variable_does_not_leak_across_pooled_connections() {
    let Some(backend) = create_test_backend().await else {
        eprintln!("Skipping H1 regression test: Docker not available");
        return;
    };

    // Phase 1 — run a transaction that activates the tenant context, then
    // commits. On COMMIT, `SET LOCAL` must be discarded.
    {
        let mut tx = backend
            .pool()
            .begin()
            .await
            .expect("begin phase-1 transaction");
        PostgresBackend::activate_tenant_context(&mut tx, MARKER_TENANT)
            .await
            .expect("activate_tenant_context should succeed");

        // Sanity: inside the transaction, the setting IS visible.
        let (inside,): (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&mut *tx)
            .await
            .expect("inside-tx current_setting must succeed");
        assert_eq!(
            inside, MARKER_TENANT,
            "inside the transaction the GUC must reflect what we set"
        );

        tx.commit().await.expect("commit phase-1 transaction");
    }

    // Phase 2 — exhaust a batch of fresh connections. Every one of them
    // MUST observe an empty `app.tenant_id`. We loop because pool
    // assignment is non-deterministic: the guarantee we need is "no pooled
    // connection ever leaks this setting," not "the very next acquire."
    let pool = backend.pool().clone();
    for attempt in 0..16 {
        let (leaked,): (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&pool)
            .await
            .expect("phase-2 current_setting must succeed");
        assert_eq!(
            leaked, "",
            "H1 regression on attempt {attempt}: a pooled connection observed \
             `app.tenant_id = {leaked:?}` after the setting transaction had \
             committed. The third argument of set_config must be `true` \
             (transaction-scoped)."
        );
    }
}
