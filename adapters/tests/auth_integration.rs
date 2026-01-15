use adapters::auth::permit::PermitAuthorizationService;
use mk_core::traits::AuthorizationService;
use mk_core::types::{TenantContext, TenantId, UserId};
use serde_json::json;
use std::str::FromStr;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_permit_authorization_allow() {
    let mock_server = MockServer::start().await;
    let api_key = "test_key";
    let service = PermitAuthorizationService::new(api_key, &mock_server.uri());

    let ctx = TenantContext {
        tenant_id: TenantId::from_str("tenant-1").unwrap(),
        user_id: UserId::from_str("user-1").unwrap(),
        agent_id: None,
    };

    Mock::given(method("POST"))
        .and(path("/allowed"))
        .and(header("Authorization", &format!("Bearer {}", api_key)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "allow": true })))
        .mount(&mock_server)
        .await;

    let allowed = service
        .check_permission(&ctx, "memory:read", "hierarchical")
        .await
        .unwrap();
    assert!(allowed);
}

#[tokio::test]
async fn test_permit_authorization_deny() {
    let mock_server = MockServer::start().await;
    let api_key = "test_key";
    let service = PermitAuthorizationService::new(api_key, &mock_server.uri());

    let ctx = TenantContext {
        tenant_id: TenantId::from_str("tenant-1").unwrap(),
        user_id: UserId::from_str("user-1").unwrap(),
        agent_id: None,
    };

    Mock::given(method("POST"))
        .and(path("/allowed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "allow": false })))
        .mount(&mock_server)
        .await;

    let allowed = service
        .check_permission(&ctx, "memory:read", "hierarchical")
        .await
        .unwrap();
    assert!(!allowed);
}

#[tokio::test]
async fn test_permit_api_error() {
    let mock_server = MockServer::start().await;
    let api_key = "test_key";
    let service = PermitAuthorizationService::new(api_key, &mock_server.uri());

    let ctx = TenantContext {
        tenant_id: TenantId::from_str("tenant-1").unwrap(),
        user_id: UserId::from_str("user-1").unwrap(),
        agent_id: None,
    };

    Mock::given(method("POST"))
        .and(path("/allowed"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let result = service
        .check_permission(&ctx, "memory:read", "hierarchical")
        .await;
    assert!(result.is_err());
}
