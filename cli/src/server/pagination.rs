//! API-level pagination primitives for Axum route handlers.
//!
//! Provides [`ApiPaginationParams`] (an Axum `Query` extractor) and
//! [`PaginatedResponse<T>`] (the standard JSON envelope for list endpoints).
//!
//! These wrap the core [`mk_core::pagination::PaginationParams`] and
//! [`mk_core::pagination::PaginatedResult<T>`] to provide HTTP-layer concerns
//! (query string extraction, JSON serialization, validation errors).

use mk_core::pagination::{PaginatedResult, PaginationParams};
use serde::{Deserialize, Serialize};

/// Query-string pagination parameters extracted by Axum.
///
/// Usage in a handler:
/// ```ignore
/// async fn list_items(
///     Query(page): Query<ApiPaginationParams>,
/// ) -> impl IntoResponse {
///     let params: PaginationParams = page.into();
///     // ...
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPaginationParams {
    /// Page size. Defaults to 50, capped at 200.
    pub limit: Option<usize>,
    /// Items to skip. Defaults to 0.
    pub offset: Option<usize>,
}

impl From<ApiPaginationParams> for PaginationParams {
    fn from(api: ApiPaginationParams) -> Self {
        PaginationParams::new(
            api.limit.unwrap_or(PaginationParams::default_limit()),
            api.offset.unwrap_or(0),
        )
    }
}

impl Default for ApiPaginationParams {
    fn default() -> Self {
        Self {
            limit: None,
            offset: None,
        }
    }
}

/// Standard JSON response envelope for paginated list endpoints.
///
/// Serializes to:
/// ```json
/// {
///   "items": [...],
///   "total": 250,
///   "limit": 50,
///   "offset": 0
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T: Serialize> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Grand total count of matching items. May be `null` for expensive counts.
    pub total: Option<u64>,
    /// The page size that was used.
    pub limit: usize,
    /// The offset that was used.
    pub offset: usize,
}

impl<T: Serialize> PaginatedResponse<T> {
    /// Create from a core `PaginatedResult` and the params that produced it.
    pub fn from_result(result: PaginatedResult<T>, params: PaginationParams) -> Self {
        Self {
            items: result.items,
            total: result.total,
            limit: params.limit,
            offset: params.offset,
        }
    }

    /// Convenience: create directly from parts.
    pub fn new(items: Vec<T>, total: Option<u64>, limit: usize, offset: usize) -> Self {
        Self {
            items,
            total,
            limit,
            offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_params_defaults() {
        let api = ApiPaginationParams::default();
        let params: PaginationParams = api.into();
        assert_eq!(params.limit, 50);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn api_params_caps_limit() {
        let api = ApiPaginationParams {
            limit: Some(5000),
            offset: Some(10),
        };
        let params: PaginationParams = api.into();
        assert_eq!(params.limit, 200);
        assert_eq!(params.offset, 10);
    }

    #[test]
    fn paginated_response_serializes_camel_case() {
        let resp = PaginatedResponse::new(vec!["a", "b"], Some(100), 50, 0);
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("items").is_some());
        assert!(json.get("total").is_some());
        assert!(json.get("limit").is_some());
        assert!(json.get("offset").is_some());
        assert_eq!(json["total"], 100);
        assert_eq!(json["limit"], 50);
        assert_eq!(json["offset"], 0);
    }

    #[test]
    fn paginated_response_from_result() {
        let result = PaginatedResult::with_total(vec![1, 2, 3], 250);
        let params = PaginationParams::new(10, 20);
        let resp = PaginatedResponse::from_result(result, params);
        assert_eq!(resp.items, vec![1, 2, 3]);
        assert_eq!(resp.total, Some(250));
        assert_eq!(resp.limit, 10);
        assert_eq!(resp.offset, 20);
    }
}
