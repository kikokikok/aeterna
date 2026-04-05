use adapters::auth::cedar::CedarAuthorizer;
use mk_core::traits::AuthorizationService;
use mk_core::types::{Role, RoleIdentifier, TenantContext, TenantId, UserId};

const BASIC_SCHEMA: &str = r#"{
    "": {
        "entityTypes": {
            "User": {},
            "Unit": {},
            "Role": {}
        },
        "actions": {
            "View": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit"]
                }
            }
        }
    }
}"#;

fn create_ctx(tenant: &str, user: &str) -> TenantContext {
    TenantContext::new(
        TenantId::new(tenant.to_string()).unwrap(),
        UserId::new(user.to_string()).unwrap(),
    )
}

#[tokio::test]
async fn test_assign_role_persists_in_store_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-1");
    let user_id = UserId::new("user-1".to_string()).unwrap();

    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert!(roles.contains(&RoleIdentifier::Known(Role::Admin)));
}

#[tokio::test]
async fn test_remove_role_clears_from_store_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-2");
    let user_id = UserId::new("user-2".to_string()).unwrap();

    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();
    authorizer
        .remove_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_assign_multiple_roles_same_user_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-3");
    let user_id = UserId::new("user-3".to_string()).unwrap();

    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Developer))
        .await
        .unwrap();
    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert!(roles.contains(&RoleIdentifier::Known(Role::Developer)));
    assert!(roles.contains(&RoleIdentifier::Known(Role::Admin)));
}

#[tokio::test]
async fn test_remove_nonexistent_role_noop_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-4");
    let user_id = UserId::new("user-4".to_string()).unwrap();

    authorizer
        .remove_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert!(roles.is_empty());
}

#[tokio::test]
async fn test_assign_role_idempotent_hashset_dedup_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-5");
    let user_id = UserId::new("user-5".to_string()).unwrap();

    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();
    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Custom("admin".to_string()))
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert_eq!(roles.len(), 1);
}

#[tokio::test]
async fn test_assign_custom_role_works_expected() {
    let authorizer = CedarAuthorizer::new("", "{}").unwrap();
    let ctx = create_ctx("tenant-1", "user-6");
    let user_id = UserId::new("user-6".to_string()).unwrap();

    authorizer
        .assign_role(
            &ctx,
            &user_id,
            RoleIdentifier::Custom("billingOwner".to_string()),
        )
        .await
        .unwrap();

    let roles = authorizer.get_user_roles(&ctx).await.unwrap();
    assert!(roles.contains(&RoleIdentifier::Custom("billingOwner".to_string())));
}

#[tokio::test]
async fn test_authorization_pipeline_assign_role_then_check_permission_allowed_expected() {
    let policies = r#"
        permit(principal in Aeterna::Role::"Admin", action == Action::"View", resource == Unit::"unit-1");
    "#;
    let authorizer = CedarAuthorizer::new(policies, BASIC_SCHEMA).unwrap();
    let ctx = create_ctx("tenant-1", "user-7");
    let user_id = UserId::new("user-7".to_string()).unwrap();

    authorizer
        .assign_role(&ctx, &user_id, RoleIdentifier::Known(Role::Admin))
        .await
        .unwrap();

    let allowed = authorizer
        .check_permission(&ctx, "View", "Unit::\"unit-1\"")
        .await
        .unwrap();
    assert!(allowed);
}

#[tokio::test]
async fn test_authorization_pipeline_no_role_check_permission_denied_expected() {
    let policies = r#"
        permit(principal in Aeterna::Role::"Admin", action == Action::"View", resource == Unit::"unit-1");
    "#;
    let authorizer = CedarAuthorizer::new(policies, BASIC_SCHEMA).unwrap();
    let ctx = create_ctx("tenant-1", "user-8");

    let denied = authorizer
        .check_permission(&ctx, "View", "Unit::\"unit-1\"")
        .await
        .unwrap();
    assert!(!denied);
}

#[tokio::test]
async fn test_authorization_pipeline_custom_role_policy_check_permission_allowed_expected() {
    let policies = r#"
        permit(principal in Aeterna::Role::"billingOwner", action == Action::"View", resource == Unit::"finance-reports");
    "#;
    let authorizer = CedarAuthorizer::new(policies, BASIC_SCHEMA).unwrap();
    let ctx = create_ctx("tenant-1", "user-9");
    let user_id = UserId::new("user-9".to_string()).unwrap();

    authorizer
        .assign_role(
            &ctx,
            &user_id,
            RoleIdentifier::Custom("billingOwner".to_string()),
        )
        .await
        .unwrap();

    let allowed = authorizer
        .check_permission(&ctx, "View", "Unit::\"finance-reports\"")
        .await
        .unwrap();
    assert!(allowed);
}
