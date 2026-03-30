use idp_sync::config::GitHubConfig;
use sqlx::PgPool;
use wiremock::MockServer;

pub fn test_pem_key() -> String {
    let key = include_str!("test_rsa_key.pem");
    key.to_string()
}

pub fn github_config(mock_server: &MockServer) -> GitHubConfig {
    GitHubConfig {
        org_name: "test-org".to_string(),
        app_id: 12345,
        installation_id: 67890,
        private_key_pem: test_pem_key(),
        team_filter: None,
        sync_repos_as_projects: false,
        api_base_url: Some(mock_server.uri()),
    }
}

pub async fn setup_db(pool: &PgPool) {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS pgcrypto")
        .execute(pool)
        .await
        .expect("pgcrypto");

    storage::postgres::PostgresBackend::from_pool(pool.clone())
        .initialize_schema()
        .await
        .expect("storage schema");

    idp_sync::github::initialize_github_sync_schema(pool)
        .await
        .expect("github sync schema");
}

pub fn mock_token_response() -> serde_json::Value {
    serde_json::json!({
        "token": "ghs_test_token_abc123",
        "expires_at": "2099-01-01T00:00:00Z"
    })
}

pub fn mock_org_members() -> serde_json::Value {
    serde_json::json!([
        {"login": "alice", "id": 1001, "type": "User", "site_admin": false},
        {"login": "bob", "id": 1002, "type": "User", "site_admin": false},
        {"login": "charlie", "id": 1003, "type": "User", "site_admin": true}
    ])
}

pub fn mock_teams_flat() -> serde_json::Value {
    serde_json::json!([
        {"id": 100, "slug": "platform", "name": "Platform", "description": "Platform team", "parent": null},
        {"id": 101, "slug": "security", "name": "Security", "description": "Security team", "parent": null}
    ])
}

pub fn mock_teams_nested() -> serde_json::Value {
    serde_json::json!([
        {"id": 100, "slug": "platform", "name": "Platform", "description": "Platform team", "parent": null},
        {"id": 101, "slug": "security", "name": "Security", "description": "Security team", "parent": null},
        {"id": 200, "slug": "api-team", "name": "API Team", "description": "API sub-team", "parent": {"id": 100, "slug": "platform", "name": "Platform"}},
        {"id": 201, "slug": "frontend-team", "name": "Frontend Team", "description": "Frontend sub-team", "parent": {"id": 100, "slug": "platform", "name": "Platform"}}
    ])
}

pub fn mock_teams_three_level() -> serde_json::Value {
    serde_json::json!([
        {"id": 100, "slug": "engineering", "name": "Engineering", "description": null, "parent": null},
        {"id": 200, "slug": "platform", "name": "Platform", "description": null, "parent": {"id": 100, "slug": "engineering", "name": "Engineering"}},
        {"id": 300, "slug": "api-team", "name": "API Team", "description": null, "parent": {"id": 200, "slug": "platform", "name": "Platform"}}
    ])
}

pub fn mock_team_members_alice_bob() -> serde_json::Value {
    serde_json::json!([
        {"login": "alice", "id": 1001},
        {"login": "bob", "id": 1002}
    ])
}

pub fn mock_team_members_charlie() -> serde_json::Value {
    serde_json::json!([
        {"login": "charlie", "id": 1003}
    ])
}

pub fn mock_user_alice() -> serde_json::Value {
    serde_json::json!({
        "login": "alice",
        "id": 1001,
        "name": "Alice Smith",
        "email": "alice@example.com",
        "created_at": "2023-01-15T10:30:00Z",
        "updated_at": "2024-06-20T14:00:00Z"
    })
}

pub fn mock_user_bob() -> serde_json::Value {
    serde_json::json!({
        "login": "bob",
        "id": 1002,
        "name": "Bob Jones",
        "email": null,
        "created_at": "2023-03-01T08:00:00Z",
        "updated_at": "2024-07-15T12:00:00Z"
    })
}

pub fn mock_user_charlie() -> serde_json::Value {
    serde_json::json!({
        "login": "charlie",
        "id": 1003,
        "name": "Charlie Brown",
        "email": "charlie@example.com",
        "created_at": "2022-06-01T00:00:00Z",
        "updated_at": "2024-08-01T00:00:00Z"
    })
}
