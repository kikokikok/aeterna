use chrono::{DateTime, Utc};
use mk_core::types::MemoryLayer;
use serde_json::json;
use sqlx::{Pool, Postgres};
use storage::budget_storage::{BudgetStorage, BudgetStorageError, StoredBudget, StoredUsage};
use testing::{postgres, unique_id};

async fn create_test_budget_storage() -> Option<BudgetStorage> {
    let fixture = postgres().await?;
    let pool = Pool::<Postgres>::connect(fixture.url()).await.ok()?;
    let storage = BudgetStorage::new(pool);
    storage.initialize_schema().await.ok()?;
    Some(storage)
}

fn unique_tenant_id() -> String {
    unique_id("test-tenant")
}

fn create_test_budget(tenant_id: &str) -> StoredBudget {
    StoredBudget {
        tenant_id: tenant_id.to_string(),
        daily_token_limit: 1_000_000,
        hourly_token_limit: 100_000,
        per_layer_limits: json!({
            "session": 50_000,
            "project": 100_000,
            "team": 200_000
        }),
        warning_threshold_percent: 80,
        critical_threshold_percent: 90,
        exhausted_action: "reject".to_string(),
        created_at: Utc::now().timestamp(),
        updated_at: Utc::now().timestamp()
    }
}

#[tokio::test]
async fn test_budget_storage_initialize_schema() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let result = storage.initialize_schema().await;
    assert!(result.is_ok(), "Schema initialization should be idempotent");
}

#[tokio::test]
async fn test_budget_crud_operations() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);

    let upsert_result = storage.upsert_budget(&budget).await;
    assert!(upsert_result.is_ok(), "Should upsert budget");

    let retrieved = storage.get_budget(&tenant_id).await;
    assert!(retrieved.is_ok(), "Should retrieve budget");
    let retrieved_budget = retrieved.unwrap();
    assert!(retrieved_budget.is_some(), "Budget should exist");
    let budget = retrieved_budget.unwrap();
    assert_eq!(budget.tenant_id, tenant_id);
    assert_eq!(budget.daily_token_limit, 1_000_000);
    assert_eq!(budget.hourly_token_limit, 100_000);
    assert_eq!(budget.warning_threshold_percent, 80);
    assert_eq!(budget.critical_threshold_percent, 90);
    assert_eq!(budget.exhausted_action, "reject");

    let mut updated_budget = budget.clone();
    updated_budget.daily_token_limit = 2_000_000;
    updated_budget.updated_at = Utc::now().timestamp();

    let update_result = storage.upsert_budget(&updated_budget).await;
    assert!(update_result.is_ok(), "Should update budget");

    let retrieved_updated = storage.get_budget(&tenant_id).await.unwrap().unwrap();
    assert_eq!(retrieved_updated.daily_token_limit, 2_000_000);

    let delete_result = storage.delete_budget(&tenant_id).await;
    assert!(delete_result.is_ok(), "Should delete budget");
    let was_deleted = delete_result.unwrap();
    assert!(was_deleted, "Budget should have been deleted");

    let retrieved_after_delete = storage.get_budget(&tenant_id).await.unwrap();
    assert!(
        retrieved_after_delete.is_none(),
        "Budget should not exist after deletion"
    );
}

#[tokio::test]
async fn test_budget_not_found() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let non_existent_tenant = "non-existent-tenant-123";
    let result = storage.get_budget(non_existent_tenant).await;
    assert!(
        result.is_ok(),
        "Should handle non-existent tenant gracefully"
    );
    assert!(
        result.unwrap().is_none(),
        "Should return None for non-existent tenant"
    );
}

#[tokio::test]
async fn test_record_and_get_usage() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let window_start = now - (now % 3600);

    let result = storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            5000,
            window_start
        )
        .await;
    assert!(result.is_ok(), "Should record usage");

    let usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await;
    assert!(usage.is_ok(), "Should get usage");
    assert_eq!(usage.unwrap(), 5000, "Usage should match recorded amount");

    let total_usage = storage
        .get_usage(&tenant_id, None, "hourly", window_start)
        .await;
    assert!(total_usage.is_ok(), "Should get total usage");
    assert_eq!(total_usage.unwrap(), 5000, "Total usage should match");

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            3000,
            window_start
        )
        .await
        .unwrap();

    let updated_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(
        updated_usage, 8000,
        "Usage should accumulate in same window"
    );

    let new_window_start = window_start + 3600;
    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            2000,
            new_window_start
        )
        .await
        .unwrap();

    let new_window_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            new_window_start
        )
        .await
        .unwrap();
    assert_eq!(new_window_usage, 2000, "New window should have fresh usage");

    let old_window_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(old_window_usage, 8000, "Old window usage should persist");
}

#[tokio::test]
async fn test_get_all_layer_usage() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let window_start = now - (now % 3600);

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            5000,
            window_start
        )
        .await
        .unwrap();

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Project,
            "hourly",
            15000,
            window_start
        )
        .await
        .unwrap();

    storage
        .record_usage(&tenant_id, MemoryLayer::Team, "hourly", 25000, window_start)
        .await
        .unwrap();

    let all_usage = storage
        .get_all_layer_usage(&tenant_id, "hourly", window_start)
        .await;
    assert!(all_usage.is_ok(), "Should get all layer usage");

    let usage_map: std::collections::HashMap<String, i64> =
        all_usage.unwrap().into_iter().collect();

    assert_eq!(usage_map.len(), 3, "Should have usage for 3 layers");
    assert_eq!(
        usage_map.get("session").copied().unwrap_or(0),
        5000,
        "Session layer usage should match"
    );
    assert_eq!(
        usage_map.get("project").copied().unwrap_or(0),
        15000,
        "Project layer usage should match"
    );
    assert_eq!(
        usage_map.get("team").copied().unwrap_or(0),
        25000,
        "Team layer usage should match"
    );
}

#[tokio::test]
async fn test_reset_usage() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let window_start = now - (now % 3600);

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            5000,
            window_start
        )
        .await
        .unwrap();

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "daily",
            15000,
            window_start
        )
        .await
        .unwrap();

    let hourly_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(hourly_usage, 5000);

    let daily_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "daily",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(daily_usage, 15000);

    let reset_result = storage.reset_usage(&tenant_id, "hourly").await;
    assert!(reset_result.is_ok(), "Should reset usage");

    let hourly_usage_after_reset = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(hourly_usage_after_reset, 0, "Hourly usage should be reset");

    let daily_usage_after_reset = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "daily",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(daily_usage_after_reset, 15000, "Daily usage should persist");
}

#[tokio::test]
async fn test_cleanup_old_usage() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let current_window = now - (now % 3600);
    let old_window = current_window - (24 * 3600);

    storage
        .record_usage(&tenant_id, MemoryLayer::Session, "hourly", 5000, old_window)
        .await
        .unwrap();

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            3000,
            current_window
        )
        .await
        .unwrap();

    let cleanup_threshold = current_window - (12 * 3600);
    let cleanup_result = storage.cleanup_old_usage(cleanup_threshold).await;
    assert!(cleanup_result.is_ok(), "Should cleanup old usage");

    let rows_deleted = cleanup_result.unwrap();
    assert_eq!(rows_deleted, 1, "Should delete 1 old usage record");

    let old_usage = storage
        .get_usage(&tenant_id, Some(MemoryLayer::Session), "hourly", old_window)
        .await
        .unwrap();
    assert_eq!(old_usage, 0, "Old usage should be cleaned up");

    let current_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            current_window
        )
        .await
        .unwrap();
    assert_eq!(current_usage, 3000, "Current usage should persist");
}

#[tokio::test]
async fn test_usage_with_different_window_types() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let hourly_window = now - (now % 3600);
    let daily_window = now - (now % (24 * 3600));

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            5000,
            hourly_window
        )
        .await
        .unwrap();

    storage
        .record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "daily",
            15000,
            daily_window
        )
        .await
        .unwrap();

    let hourly_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            hourly_window
        )
        .await
        .unwrap();
    assert_eq!(hourly_usage, 5000, "Hourly usage should match");

    let daily_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "daily",
            daily_window
        )
        .await
        .unwrap();
    assert_eq!(daily_usage, 15000, "Daily usage should match");

    let all_hourly_usage = storage
        .get_all_layer_usage(&tenant_id, "hourly", hourly_window)
        .await
        .unwrap();
    assert_eq!(all_hourly_usage.len(), 1, "Should have hourly usage");

    let all_daily_usage = storage
        .get_all_layer_usage(&tenant_id, "daily", daily_window)
        .await
        .unwrap();
    assert_eq!(all_daily_usage.len(), 1, "Should have daily usage");
}

#[tokio::test]
async fn test_concurrent_usage_updates() {
    let Some(storage) = create_test_budget_storage().await else {
        eprintln!("Skipping budget storage test: Docker not available");
        return;
    };

    let tenant_id = unique_tenant_id();
    let budget = create_test_budget(&tenant_id);
    storage.upsert_budget(&budget).await.unwrap();

    let now = Utc::now().timestamp();
    let window_start = now - (now % 3600);

    let (result1, result2, result3) = tokio::join!(
        storage.record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            1000,
            window_start
        ),
        storage.record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            2000,
            window_start
        ),
        storage.record_usage(
            &tenant_id,
            MemoryLayer::Session,
            "hourly",
            3000,
            window_start
        ),
    );

    assert!(result1.is_ok(), "Concurrent update 1 should succeed");
    assert!(result2.is_ok(), "Concurrent update 2 should succeed");
    assert!(result3.is_ok(), "Concurrent update 3 should succeed");

    let total_usage = storage
        .get_usage(
            &tenant_id,
            Some(MemoryLayer::Session),
            "hourly",
            window_start
        )
        .await
        .unwrap();
    assert_eq!(
        total_usage, 6000,
        "Should accumulate all concurrent updates"
    );
}
