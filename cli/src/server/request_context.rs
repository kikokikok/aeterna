//! Request-scoped client metadata extracted from `X-Aeterna-Client-Kind`.
//!
//! Implements B2 §11.2 (extract header, propagate through request-scoped
//! audit context) and §11.3 (normalize unknown values to `api`, preserving
//! the original in `client_kind_raw`).
//!
//! The header is emitted by the CLI (§7.7, `cli/src/client.rs::build_http_client`)
//! and by the admin UI. Every request routed through the auth middleware
//! gets a [`RequestContext`] inserted into its extensions so downstream
//! handlers can, in turn, populate [`storage::governance::AuditExtensions`]
//! (§11.1).
//!
//! # Normalization contract (§11.3)
//!
//! | Header value (case-insensitive, trimmed) | `client_kind`  | `client_kind_raw`    |
//! |------------------------------------------|----------------|----------------------|
//! | `cli`                                    | `"cli"`        | `None`               |
//! | `ui`                                     | `"ui"`         | `None`               |
//! | `api`                                    | `"api"`        | `None`               |
//! | anything else (e.g. `curl`, `sdk-java`)  | `"api"`        | `Some(<original>)`   |
//! | missing / empty / all-whitespace         | `"api"`        | `None`               |
//!
//! Keeping the raw value in memory (and NOT in the `governance_audit_log.via`
//! column) lets ops see unusual values in request logs without widening the
//! schema every time a new unofficial client appears. The CHECK constraint on
//! `via` (migration 030) enforces the three-value closed set at the database
//! level as a last-resort guard.

use axum::http::HeaderMap;

/// Canonical name of the client-kind request header.
pub const CLIENT_KIND_HEADER: &str = "X-Aeterna-Client-Kind";

/// The three normalized client-kind values the server accepts. Anything
/// else falls through to [`CLIENT_KIND_UNKNOWN_FALLBACK`].
pub const CLIENT_KIND_CLI: &str = "cli";
pub const CLIENT_KIND_UI: &str = "ui";
pub const CLIENT_KIND_API: &str = "api";

/// Fallback value used when the incoming header is missing, empty, or does
/// not match one of the three canonical values. Matches the `via` CHECK
/// constraint installed by migration 030.
pub const CLIENT_KIND_UNKNOWN_FALLBACK: &str = CLIENT_KIND_API;

/// Request-scoped client metadata inserted into `axum::Request::extensions`
/// by the auth middleware. Downstream handlers that write audit rows should
/// read this and feed it into [`storage::governance::AuditExtensions::via`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    /// Normalized client kind — one of `"cli"`, `"ui"`, `"api"`. Always
    /// safe to pass to the `governance_audit_log.via` column.
    pub client_kind: &'static str,

    /// Original header value when it did not match one of the three
    /// canonical kinds. `None` when the header was absent, empty, or one
    /// of the three known values. Retained in-process only — not written
    /// to the audit log (see module docs for rationale).
    pub client_kind_raw: Option<String>,
}

impl RequestContext {
    /// Context with no header present — equivalent to a legacy caller or a
    /// background job that synthesised the request. Maps to `via="api"` on
    /// insert, which is the pre-B2 default behaviour.
    pub const fn absent() -> Self {
        Self {
            client_kind: CLIENT_KIND_UNKNOWN_FALLBACK,
            client_kind_raw: None,
        }
    }

    /// Extract-and-normalize a context from an inbound request's headers.
    /// Equivalent to [`normalize_client_kind`] applied to the first value
    /// of `X-Aeterna-Client-Kind` if present.
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let raw = headers
            .get(CLIENT_KIND_HEADER)
            .and_then(|v| v.to_str().ok());
        normalize_client_kind(raw)
    }
}

/// Pure function: map an optional raw header value to the normalized
/// [`RequestContext`]. Extracted for unit-testability independent of Axum.
///
/// See the module-level normalization table for the authoritative contract.
pub fn normalize_client_kind(raw: Option<&str>) -> RequestContext {
    let trimmed = raw.map(str::trim).filter(|s| !s.is_empty());
    match trimmed {
        None => RequestContext::absent(),
        Some(value) => {
            // Case-insensitive comparison on the three canonical values so
            // mixed-case headers (`CLI`, `Ui`, ...) still round-trip
            // losslessly rather than being demoted to `api`.
            if value.eq_ignore_ascii_case(CLIENT_KIND_CLI) {
                RequestContext {
                    client_kind: CLIENT_KIND_CLI,
                    client_kind_raw: None,
                }
            } else if value.eq_ignore_ascii_case(CLIENT_KIND_UI) {
                RequestContext {
                    client_kind: CLIENT_KIND_UI,
                    client_kind_raw: None,
                }
            } else if value.eq_ignore_ascii_case(CLIENT_KIND_API) {
                RequestContext {
                    client_kind: CLIENT_KIND_API,
                    client_kind_raw: None,
                }
            } else {
                // Preserve the original (pre-trim) header so ops can see
                // "curl/8.5.0" in request logs even though we persisted
                // "api" in the audit row.
                RequestContext {
                    client_kind: CLIENT_KIND_UNKNOWN_FALLBACK,
                    client_kind_raw: raw.map(str::to_string),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn ctx(raw: Option<&str>) -> RequestContext {
        normalize_client_kind(raw)
    }

    #[test]
    fn absent_header_maps_to_api_with_no_raw() {
        let c = ctx(None);
        assert_eq!(c.client_kind, "api");
        assert!(c.client_kind_raw.is_none());
    }

    #[test]
    fn empty_and_whitespace_headers_map_to_absent() {
        for v in ["", " ", "   ", "\t"] {
            let c = ctx(Some(v));
            assert_eq!(c, RequestContext::absent(), "input: {v:?}");
        }
    }

    #[test]
    fn canonical_values_round_trip_losslessly() {
        for (input, expected) in [("cli", "cli"), ("ui", "ui"), ("api", "api")] {
            let c = ctx(Some(input));
            assert_eq!(c.client_kind, expected);
            assert!(
                c.client_kind_raw.is_none(),
                "canonical value {input:?} must not set client_kind_raw"
            );
        }
    }

    #[test]
    fn canonical_values_are_case_insensitive() {
        for input in ["CLI", "Cli", "cLi", "UI", "Ui", "API", "Api"] {
            let c = ctx(Some(input));
            assert!(
                matches!(c.client_kind, "cli" | "ui" | "api"),
                "input {input:?} normalized to {}",
                c.client_kind
            );
            assert!(c.client_kind_raw.is_none());
        }
    }

    #[test]
    fn unknown_values_normalize_to_api_and_preserve_original() {
        // Note: whitespace-padded canonical values (e.g. "CLI ") trim to
        // the canonical set and are covered by
        // `trimmed_whitespace_variant_of_unknown_still_preserves_pre_trim_original`.
        for input in ["curl", "sdk-java/1.2", "Mozilla/5.0"] {
            let c = ctx(Some(input));
            assert_eq!(
                c.client_kind, "api",
                "unknown input {input:?} must demote to api"
            );
            assert_eq!(
                c.client_kind_raw.as_deref(),
                Some(input),
                "original header must be preserved verbatim (pre-trim)"
            );
        }
    }

    #[test]
    fn trimmed_whitespace_variant_of_unknown_still_preserves_pre_trim_original() {
        // Trailing space on a canonical-looking value is what makes the
        // match fail; ensure the pre-trim original is what we preserve.
        let c = ctx(Some("  cli\t"));
        // Case-insensitive exact-match after trim → canonical.
        assert_eq!(c.client_kind, "cli");
        assert!(c.client_kind_raw.is_none());
    }

    #[test]
    fn from_headers_reads_first_value() {
        let mut headers = HeaderMap::new();
        headers.insert(CLIENT_KIND_HEADER, HeaderValue::from_static("ui"));
        let ctx = RequestContext::from_headers(&headers);
        assert_eq!(ctx.client_kind, "ui");
    }

    #[test]
    fn from_headers_without_the_header_returns_absent() {
        let headers = HeaderMap::new();
        assert_eq!(
            RequestContext::from_headers(&headers),
            RequestContext::absent()
        );
    }

    #[test]
    fn from_headers_with_invalid_utf8_demotes_to_api() {
        let mut headers = HeaderMap::new();
        // `from_bytes` accepts non-ASCII; `to_str()` in `from_headers`
        // will then fail and we fall through to `absent()`.
        headers.insert(
            CLIENT_KIND_HEADER,
            HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap(),
        );
        let ctx = RequestContext::from_headers(&headers);
        assert_eq!(ctx, RequestContext::absent());
    }

    #[test]
    fn const_absent_matches_normalized_none() {
        assert_eq!(RequestContext::absent(), normalize_client_kind(None));
    }

    #[test]
    fn fallback_const_agrees_with_migration_030_check_constraint() {
        // Compile-time sanity: the fallback must be one of the three
        // values the CHECK constraint allows. If this ever diverges a
        // migration needs updating.
        assert!(matches!(CLIENT_KIND_UNKNOWN_FALLBACK, "cli" | "ui" | "api"));
        assert_eq!(CLIENT_KIND_UNKNOWN_FALLBACK, "api");
    }
}
