//! Request and response types for the Central Index Service API.

use serde::{Deserialize, Serialize};

/// Request payload sent when a repository index has been updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUpdateRequest {
    /// Full repository identifier (e.g. `owner/repo`).
    pub repository: String,
    /// Tenant identifier (typically the GitHub org or owner).
    pub tenant_id: String,
    /// Git commit SHA that was indexed.
    pub commit_sha: String,
    /// Branch name that was indexed.
    pub branch: String,
    /// Logical project name for grouping.
    pub project: String,
}

/// Response returned after an index update notification is accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUpdateResponse {
    /// Whether the update was accepted successfully.
    pub success: bool,
    /// Whether the update was queued for async processing.
    pub queued: bool,
    /// Unique job identifier for tracking.
    pub job_id: String,
}

/// Request to refresh the call graph for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRefreshRequest {
    /// Tenant identifier.
    pub tenant_id: String,
    /// Project name whose graph should be refreshed.
    pub project: String,
}

/// Query parameters for checking index status.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStatusQuery {
    /// Optional tenant filter.
    pub tenant_id: Option<String>,
    /// Optional project filter.
    pub project: Option<String>,
}

/// Request for cross-repository semantic search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRepoSearchRequest {
    /// Natural-language or keyword query.
    pub query: String,
    /// Tenant identifier for isolation.
    pub tenant_id: String,
    /// Optional list of projects to search within. `None` means all projects.
    pub projects: Option<Vec<String>>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
}

/// A single result from cross-repo search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRepoSearchResult {
    /// Project the result belongs to.
    pub project: String,
    /// File path within the project.
    pub file: String,
    /// Line number of the match.
    pub line: u32,
    /// Relevance score (0.0 – 1.0).
    pub score: f64,
    /// Code snippet around the match.
    pub snippet: String,
}

/// Response for cross-repo search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRepoSearchResponse {
    /// Matched results.
    pub results: Vec<CrossRepoSearchResult>,
    /// Total number of matches (may exceed `limit`).
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_update_request_roundtrip() {
        let req = IndexUpdateRequest {
            repository: "acme/api-server".into(),
            tenant_id: "acme".into(),
            commit_sha: "abc123".into(),
            branch: "main".into(),
            project: "acme/api-server".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: IndexUpdateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.repository, "acme/api-server");
        assert_eq!(parsed.tenant_id, "acme");
    }

    #[test]
    fn index_update_response_roundtrip() {
        let resp = IndexUpdateResponse {
            success: true,
            queued: true,
            job_id: "job-001".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IndexUpdateResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert!(parsed.queued);
        assert_eq!(parsed.job_id, "job-001");
    }

    #[test]
    fn cross_repo_search_request_optional_fields() {
        let json = r#"{"query":"auth","tenant_id":"acme"}"#;
        let req: CrossRepoSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "auth");
        assert!(req.projects.is_none());
        assert!(req.limit.is_none());
    }

    #[test]
    fn index_status_query_defaults() {
        let q = IndexStatusQuery::default();
        assert!(q.tenant_id.is_none());
        assert!(q.project.is_none());
    }
}
