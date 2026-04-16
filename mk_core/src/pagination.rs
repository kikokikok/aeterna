//! Shared pagination primitives for all storage backends.
//!
//! Every `list_*` trait method should accept [`PaginationParams`] and return
//! [`PaginatedResult<T>`] so that pagination is enforced at the query level
//! (SQL LIMIT/OFFSET, Redis SSCAN, S3 MaxKeys, etc.) rather than by fetching
//! all rows and truncating in application code.

use serde::{Deserialize, Serialize};

/// Default page size when the caller does not specify a limit.
const DEFAULT_LIMIT: usize = 50;

/// Absolute maximum page size. Any requested limit above this is capped.
const MAX_LIMIT: usize = 200;

/// Storage-level pagination parameters.
///
/// Designed to be passed through from API-level extractors or constructed
/// directly by internal callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaginationParams {
    /// Maximum number of items to return. Capped at [`MAX_LIMIT`].
    pub limit: usize,
    /// Number of items to skip before returning results.
    pub offset: usize,
}

impl PaginationParams {
    /// Create new params with validation (limit is capped at MAX_LIMIT).
    pub fn new(limit: usize, offset: usize) -> Self {
        Self {
            limit: limit.min(MAX_LIMIT),
            offset,
        }
    }

    /// The maximum allowed limit value.
    pub const fn max_limit() -> usize {
        MAX_LIMIT
    }

    /// The default limit value.
    pub const fn default_limit() -> usize {
        DEFAULT_LIMIT
    }

    /// Helper: append `LIMIT $N OFFSET $M` to a SQL query string.
    ///
    /// `next_bind` is the 1-based bind parameter index to start from.
    /// Returns the SQL fragment and advances the bind index.
    pub fn sql_fragment(&self, next_bind: &mut usize) -> String {
        let limit_bind = *next_bind;
        let offset_bind = *next_bind + 1;
        *next_bind += 2;
        format!(" LIMIT ${limit_bind} OFFSET ${offset_bind}")
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            offset: 0,
        }
    }
}

/// A paginated result set returned by storage backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Grand total count of matching items (from `COUNT(*)` or equivalent).
    /// `None` if the backend cannot efficiently provide a total (e.g., vector search, S3).
    pub total: Option<u64>,
}

impl<T> PaginatedResult<T> {
    /// Create a paginated result from a complete items vec and optional total.
    pub fn new(items: Vec<T>, total: Option<u64>) -> Self {
        Self { items, total }
    }

    /// Convenience: create a result with a known total.
    pub fn with_total(items: Vec<T>, total: u64) -> Self {
        Self {
            items,
            total: Some(total),
        }
    }

    /// Convenience: create a result where total is unknown.
    pub fn without_total(items: Vec<T>) -> Self {
        Self {
            items,
            total: None,
        }
    }

    /// Map the items through a function, preserving the total.
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> PaginatedResult<U> {
        PaginatedResult {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params() {
        let p = PaginationParams::default();
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn limit_is_capped() {
        let p = PaginationParams::new(5000, 0);
        assert_eq!(p.limit, 200);
    }

    #[test]
    fn sql_fragment_bind_indices() {
        let p = PaginationParams::new(20, 40);
        let mut next = 3;
        let frag = p.sql_fragment(&mut next);
        assert_eq!(frag, " LIMIT $3 OFFSET $4");
        assert_eq!(next, 5);
    }

    #[test]
    fn paginated_result_map() {
        let result = PaginatedResult::with_total(vec![1, 2, 3], 100);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped.items, vec![2, 4, 6]);
        assert_eq!(mapped.total, Some(100));
    }

    #[test]
    fn paginated_result_without_total() {
        let result: PaginatedResult<i32> = PaginatedResult::without_total(vec![1]);
        assert!(result.total.is_none());
    }
}
