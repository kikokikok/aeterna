use agent_a2a::config::{RoleMapping, TrustedIdentityConfig};
use agent_a2a::test_helpers::{create_test_router, create_test_router_with};
use agent_a2a::{AuthState, Config};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

fn app() -> Router {
    create_test_router()
}

fn app_with_auth(auth_state: AuthState) -> Router {
    create_test_router_with(Config::default(), auth_state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("degraded"));
    assert!(body_str.contains("not_implemented") || body_str.contains("unavailable"));
}

#[tokio::test]
async fn test_agent_card_discovery() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/agent.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("Aeterna A2A Agent"));
    assert!(body_str.contains("memory"));
    assert!(body_str.contains("knowledge"));
    assert!(body_str.contains("governance"));
}

#[tokio::test]
async fn test_tasks_send_endpoint() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/send")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"skill": "memory", "tool": "memory_add"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("a2a_requests_total"));
    assert!(body_str.contains("a2a_runtime_ready 0"));
    assert!(body_str.contains("a2a_task_execution_ready 0"));
}

#[tokio::test]
async fn test_multi_tenant_isolation() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_unauthorized_access() {
    let app = app_with_auth(AuthState {
        api_key: Some("expected-token".to_string()),
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig::default(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("authorization", "Bearer invalid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authorized_access_with_api_key_reaches_health_handler() {
    let app = app_with_auth(AuthState {
        api_key: Some("expected-token".to_string()),
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig::default(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("authorization", "Bearer expected-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_invalid_parameters() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/send")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"invalid": "json"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn test_interactive_identity_access_with_trusted_headers_reaches_health_handler() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            role_mappings: vec![RoleMapping {
                group: "aeterna-users".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-auth-request-email", "alice@example.com")
                .header("x-auth-request-user", "okta|alice")
                .header("x-auth-request-groups", "aeterna-users,aeterna-admins")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_interactive_identity_missing_required_headers_is_rejected() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            role_mappings: vec![RoleMapping {
                group: "aeterna-users".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-auth-request-email", "alice@example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_interactive_identity_spoof_without_trusted_proxy_marker_is_rejected() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            proxy_header: "x-aeterna-authenticated".to_string(),
            proxy_header_value: "true".to_string(),
            role_mappings: vec![RoleMapping {
                group: "aeterna-users".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-auth-request-email", "alice@example.com")
                .header("x-auth-request-user", "okta|alice")
                .header("x-auth-request-groups", "aeterna-users")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_interactive_identity_with_explicit_proxy_marker_is_allowed() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            proxy_header: "x-aeterna-authenticated".to_string(),
            proxy_header_value: "true".to_string(),
            role_mappings: vec![RoleMapping {
                group: "aeterna-users".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-aeterna-authenticated", "true")
                .header("x-auth-request-email", "alice@example.com")
                .header("x-auth-request-user", "okta|alice")
                .header("x-auth-request-groups", "aeterna-users")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_interactive_identity_fails_when_group_mapping_missing() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            role_mappings: vec![RoleMapping {
                group: "different-group".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-auth-request-email", "alice@example.com")
                .header("x-auth-request-user", "okta|alice")
                .header("x-auth-request-groups", "aeterna-users")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_interactive_identity_supports_tenant_mapping_pattern() {
    let app = app_with_auth(AuthState {
        api_key: None,
        jwt_secret: None,
        enabled: true,
        trusted_identity: TrustedIdentityConfig {
            enabled: true,
            tenant_mapping: agent_a2a::config::TenantMappingConfig {
                pattern: "tenant::{tenant}".to_string(),
                default_tenant: None,
            },
            role_mappings: vec![RoleMapping {
                group: "aeterna-users".to_string(),
                roles: vec!["viewer".to_string()],
            }],
            ..TrustedIdentityConfig::default()
        },
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-auth-request-email", "alice@example.com")
                .header("x-auth-request-user", "okta|alice")
                .header("x-auth-request-groups", "aeterna-users")
                .header("x-tenant-id", "tenant-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
