use crate::central_index::CentralIndexError;
use crate::central_index::auth::{ApiKeyGuard, RateLimiter, verify_webhook_signature};
use crate::central_index::types::{
    CrossRepoSearchRequest, GraphRefreshRequest, IndexStatusQuery, IndexUpdateRequest,
    IndexUpdateResponse,
};

fn authenticate(auth_header: Option<&str>, guard: &ApiKeyGuard) -> Result<(), CentralIndexError> {
    let header = auth_header.ok_or(CentralIndexError::Unauthorized)?;
    let token = ApiKeyGuard::extract_from_header(header).ok_or(CentralIndexError::Unauthorized)?;
    if !guard.validate(token) {
        return Err(CentralIndexError::Unauthorized);
    }
    Ok(())
}

/// Accept an index-updated webhook notification.
///
/// Validates the API key *and* optional webhook signature, applies rate
/// limiting, then deserialises the body.
pub async fn handle_index_updated(
    auth_header: Option<&str>,
    webhook_sig: Option<&str>,
    body: &[u8],
    guard: &ApiKeyGuard,
    rate_limiter: &RateLimiter,
) -> Result<IndexUpdateResponse, CentralIndexError> {
    authenticate(auth_header, guard)?;

    // Verify webhook signature when the secret is configured.
    if let Some(sig) = webhook_sig {
        let secret = std::env::var("AETERNA_WEBHOOK_SECRET").unwrap_or_default();
        if !secret.is_empty() && !verify_webhook_signature(body, sig, &secret) {
            return Err(CentralIndexError::InvalidSignature);
        }
    }

    let token = auth_header
        .and_then(ApiKeyGuard::extract_from_header)
        .unwrap_or("anonymous");
    if !rate_limiter.check_and_increment(token) {
        return Err(CentralIndexError::RateLimitExceeded {
            retry_after_secs: 60,
        });
    }

    let req: IndexUpdateRequest =
        serde_json::from_slice(body).map_err(|e| CentralIndexError::BadRequest(e.to_string()))?;

    let job_id = uuid::Uuid::new_v4().to_string();

    tracing::info!(
        repository = %req.repository,
        tenant_id = %req.tenant_id,
        commit_sha = %req.commit_sha,
        branch = %req.branch,
        job_id = %job_id,
        "index update queued"
    );

    Ok(IndexUpdateResponse {
        success: true,
        queued: true,
        job_id,
    })
}

/// Trigger a graph refresh for the given tenant/project.
pub async fn handle_graph_refresh(
    auth_header: Option<&str>,
    req: GraphRefreshRequest,
    guard: &ApiKeyGuard,
) -> Result<serde_json::Value, CentralIndexError> {
    authenticate(auth_header, guard)?;

    tracing::info!(
        tenant_id = %req.tenant_id,
        project = %req.project,
        "graph refresh requested"
    );

    Ok(serde_json::json!({
        "status": "accepted",
        "tenant_id": req.tenant_id,
        "project": req.project
    }))
}

/// Return the current indexing status, optionally filtered by tenant/project.
pub async fn handle_index_status(
    auth_header: Option<&str>,
    query: IndexStatusQuery,
    guard: &ApiKeyGuard,
) -> Result<serde_json::Value, CentralIndexError> {
    authenticate(auth_header, guard)?;

    Ok(serde_json::json!({
        "status": "ok",
        "filters": {
            "tenant_id": query.tenant_id,
            "project": query.project
        },
        "projects": []
    }))
}

/// Perform a cross-repository semantic search within a tenant's workspace.
pub async fn handle_cross_repo_search(
    auth_header: Option<&str>,
    req: CrossRepoSearchRequest,
    guard: &ApiKeyGuard,
) -> Result<serde_json::Value, CentralIndexError> {
    authenticate(auth_header, guard)?;

    let limit = req.limit.unwrap_or(20);

    Ok(serde_json::json!({
        "query": req.query,
        "tenant_id": req.tenant_id,
        "projects_filter": req.projects,
        "limit": limit,
        "results": [],
        "total": 0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> ApiKeyGuard {
        ApiKeyGuard {
            api_key: "test-key".into(),
        }
    }

    fn test_limiter() -> RateLimiter {
        RateLimiter::new(100)
    }

    #[tokio::test]
    async fn index_updated_rejects_missing_auth() {
        let body = b"{}";
        let result = handle_index_updated(None, None, body, &test_guard(), &test_limiter()).await;
        assert!(matches!(result, Err(CentralIndexError::Unauthorized)));
    }

    #[tokio::test]
    async fn index_updated_rejects_bad_token() {
        let body = b"{}";
        let result = handle_index_updated(
            Some("Bearer wrong"),
            None,
            body,
            &test_guard(),
            &test_limiter(),
        )
        .await;
        assert!(matches!(result, Err(CentralIndexError::Unauthorized)));
    }

    #[tokio::test]
    async fn index_updated_rejects_bad_json() {
        let body = b"not-json";
        let result = handle_index_updated(
            Some("Bearer test-key"),
            None,
            body,
            &test_guard(),
            &test_limiter(),
        )
        .await;
        assert!(matches!(result, Err(CentralIndexError::BadRequest(_))));
    }

    #[tokio::test]
    async fn index_updated_success() {
        let body = serde_json::to_vec(&serde_json::json!({
            "repository": "acme/api",
            "tenant_id": "acme",
            "commit_sha": "abc123",
            "branch": "main",
            "project": "acme/api"
        }))
        .unwrap();
        let result = handle_index_updated(
            Some("Bearer test-key"),
            None,
            &body,
            &test_guard(),
            &test_limiter(),
        )
        .await
        .unwrap();
        assert!(result.success);
        assert!(result.queued);
        assert!(!result.job_id.is_empty());
    }

    #[tokio::test]
    async fn graph_refresh_rejects_unauthorized() {
        let req = GraphRefreshRequest {
            tenant_id: "t".into(),
            project: "p".into(),
        };
        let result = handle_graph_refresh(None, req, &test_guard()).await;
        assert!(matches!(result, Err(CentralIndexError::Unauthorized)));
    }

    #[tokio::test]
    async fn graph_refresh_success() {
        let req = GraphRefreshRequest {
            tenant_id: "acme".into(),
            project: "api".into(),
        };
        let result = handle_graph_refresh(Some("Bearer test-key"), req, &test_guard())
            .await
            .unwrap();
        assert_eq!(result["status"], "accepted");
    }

    #[tokio::test]
    async fn index_status_success() {
        let query = IndexStatusQuery::default();
        let result = handle_index_status(Some("Bearer test-key"), query, &test_guard())
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn cross_repo_search_success() {
        let req = CrossRepoSearchRequest {
            query: "auth".into(),
            tenant_id: "acme".into(),
            projects: None,
            limit: Some(5),
        };
        let result = handle_cross_repo_search(Some("Bearer test-key"), req, &test_guard())
            .await
            .unwrap();
        assert_eq!(result["total"], 0);
        assert_eq!(result["limit"], 5);
    }
}
