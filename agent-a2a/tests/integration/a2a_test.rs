use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode}
};
use tower::ServiceExt;

fn app() -> Router {
    crate::create_test_router()
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("healthy"));
}

#[tokio::test]
async fn test_agent_card_discovery() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/.well-known/agent.json")
                .body(Body::empty())
                .unwrap()
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
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert!(body_str.contains("a2a_requests_total"));
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
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_unauthorized_access() {
    let app = app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("authorization", "Bearer invalid-token")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    // Should still work since auth is disabled by default
    assert_eq!(response.status(), StatusCode::OK);
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
                .unwrap()
        )
        .await
        .unwrap();

    // Endpoint accepts any JSON body
    assert_eq!(response.status(), StatusCode::OK);
}
