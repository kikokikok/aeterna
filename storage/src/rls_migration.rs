use sqlx::{AssertSqlSafe, PgPool};

const TENANT_TABLES: [&str; 3] = ["sync_states", "memory_entries", "knowledge_items"];

pub async fn run_rls_migration(pool: &PgPool) -> Result<(), sqlx::Error> {
    for table in TENANT_TABLES {
        enable_rls_for_table(pool, table).await?;
    }
    Ok(())
}

async fn enable_rls_for_table(pool: &PgPool, table: &str) -> Result<(), sqlx::Error> {
    let enable_rls = format!("ALTER TABLE {} ENABLE ROW LEVEL SECURITY", table);
    sqlx::query(AssertSqlSafe(enable_rls.as_str()))
        .execute(pool)
        .await
        .ok();

    let policy_name = format!("{}_tenant_isolation", table);
    let drop_policy = format!("DROP POLICY IF EXISTS {} ON {}", policy_name, table);
    sqlx::query(AssertSqlSafe(drop_policy.as_str()))
        .execute(pool)
        .await
        .ok();

    let create_policy = format!(
        "CREATE POLICY {} ON {} FOR ALL USING (tenant_id = current_setting('app.tenant_id', true)::text)",
        policy_name, table
    );
    sqlx::query(AssertSqlSafe(create_policy.as_str()))
        .execute(pool)
        .await
        .ok();

    Ok(())
}
