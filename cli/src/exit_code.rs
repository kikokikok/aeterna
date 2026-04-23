//! Standard CLI exit-code table for `aeterna`.
//!
//! B2 tasks 9.3 + 9.4 of `harden-tenant-provisioning`: before this module
//! landed, every failing command called `std::process::exit(1)` by hand
//! and callers could not distinguish "your manifest is bad" from "the
//! server is down" from "you have no permission". Scripts around the
//! CLI (CI jobs, GitOps reconcilers, bash wrappers) had nothing to
//! branch on.
//!
//! ### The table
//!
//! | Code | Variant                       | Meaning                                                     |
//! |------|-------------------------------|-------------------------------------------------------------|
//! | 0    | [`ExitCode::Success`]         | Command completed as requested.                             |
//! | 1    | [`ExitCode::Usage`]           | Client-side error: bad flags, local validation, schema mismatch, not-found. Retrying with the same input will fail the same way. |
//! | 2    | [`ExitCode::AuthDenied`]      | Authentication or authorization failed (HTTP 401 / 403). The caller must re-authenticate or be granted access. |
//! | 3    | [`ExitCode::Conflict`]        | Resource-state conflict (HTTP 409) — the caller's view of the resource is stale. Refresh and retry. |
//! | 4    | [`ExitCode::ServerTransient`] | Retryable server error (HTTP 502 / 503 / 504 or network-layer transport failure). Safe to retry with backoff. |
//! | 5    | [`ExitCode::ServerFatal`]     | Non-retryable server error (HTTP 500 / 501 or any other 5xx). Retrying will not help; the server itself is broken. |
//!
//! All 4xx codes not in the explicit table above collapse to
//! [`ExitCode::Usage`] — from the CLI's perspective a 422 validation
//! failure, a 400 parse failure, and a 404 not-found are all "the
//! request was rejected, fix the input".
//!
//! ### How to exit
//!
//! Commands that need to terminate the process with a specific code
//! should call [`ExitCode::exit`], which converts the enum into its
//! `u8` discriminant and hands it to `std::process::exit`. This keeps
//! the integer values out of call sites and makes `grep 'exit(' cli/`
//! a reliable audit surface.
//!
//! ```no_run
//! use aeterna::exit_code::ExitCode;
//! // Bad user input — tell the shell it was a usage error, not a crash.
//! ExitCode::Usage.exit();
//! ```

use std::process;

/// The complete set of exit codes the `aeterna` CLI emits.
///
/// See the module-level documentation for the full table and semantics.
/// The `u8` discriminant is the exit code itself — do not renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExitCode {
    /// `0` — success.
    Success = 0,
    /// `1` — client-side error (bad flags, schema, not-found, local
    /// validation, unclassified 4xx).
    Usage = 1,
    /// `2` — authentication or authorization failure (HTTP 401 / 403).
    AuthDenied = 2,
    /// `3` — resource-state conflict (HTTP 409).
    Conflict = 3,
    /// `4` — retryable server error (HTTP 502 / 503 / 504 or transport
    /// failure).
    ServerTransient = 4,
    /// `5` — non-retryable server error (HTTP 500 / 501 or other 5xx).
    ServerFatal = 5,
}

impl ExitCode {
    /// Numeric value of this exit code, suitable for handing to
    /// [`std::process::exit`] or a shell's `$?`.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Terminate the current process with this exit code.
    ///
    /// Never returns. Prefer this over `std::process::exit(N)` in the
    /// CLI so the code number stays in one place.
    pub fn exit(self) -> ! {
        process::exit(self.code())
    }

    /// Map an HTTP status code (as a `u16` so callers do not need to
    /// pull in `reqwest` / `http` here) to the CLI exit code a
    /// well-behaved consumer should surface.
    ///
    /// ### Mapping (B2 task 9.4)
    ///
    /// - `1xx` / `2xx` / `3xx` → [`ExitCode::Success`].  Redirects are
    ///   the HTTP layer's job; by the time a CLI sees one, the request
    ///   has already been followed or surfaced as an error on its own.
    /// - `401`, `403` → [`ExitCode::AuthDenied`].
    /// - `409` → [`ExitCode::Conflict`].
    /// - any other `4xx` → [`ExitCode::Usage`] (schema, parse, validation,
    ///   not-found, rate-limit, etc.).
    /// - `500`, `501` → [`ExitCode::ServerFatal`] — the server cannot
    ///   fulfil the request; retrying will not help.
    /// - `502`, `503`, `504` → [`ExitCode::ServerTransient`] — upstream
    ///   or overload; safe to retry with backoff.
    /// - any other `5xx` → [`ExitCode::ServerFatal`] — conservative
    ///   default for unknown server errors so scripts do not
    ///   hot-loop against a broken backend.
    /// - anything < 100 or >= 600 → [`ExitCode::ServerFatal`] — a
    ///   non-HTTP number reaching this function is itself a bug; fail
    ///   loudly rather than pretend it's fine.
    #[must_use]
    pub fn from_http_status(status: u16) -> Self {
        match status {
            100..=399 => Self::Success,
            401 | 403 => Self::AuthDenied,
            409 => Self::Conflict,
            400..=499 => Self::Usage,
            500 | 501 => Self::ServerFatal,
            502..=504 => Self::ServerTransient,
            505..=599 => Self::ServerFatal,
            _ => Self::ServerFatal,
        }
    }
}

impl From<ExitCode> for i32 {
    fn from(value: ExitCode) -> Self {
        value.code()
    }
}

impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- discriminant stability -----------------------------------------

    #[test]
    fn discriminants_match_documented_table() {
        // These values are a public contract with every script that
        // branches on `$?`. Renumbering silently would break CI
        // pipelines downstream. Pin every variant.
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::Usage as u8, 1);
        assert_eq!(ExitCode::AuthDenied as u8, 2);
        assert_eq!(ExitCode::Conflict as u8, 3);
        assert_eq!(ExitCode::ServerTransient as u8, 4);
        assert_eq!(ExitCode::ServerFatal as u8, 5);
    }

    #[test]
    fn code_matches_discriminant() {
        for code in [
            ExitCode::Success,
            ExitCode::Usage,
            ExitCode::AuthDenied,
            ExitCode::Conflict,
            ExitCode::ServerTransient,
            ExitCode::ServerFatal,
        ] {
            assert_eq!(code.code(), code as i32);
        }
    }

    #[test]
    fn display_renders_as_number() {
        assert_eq!(format!("{}", ExitCode::Success), "0");
        assert_eq!(format!("{}", ExitCode::ServerFatal), "5");
    }

    #[test]
    fn converts_into_i32() {
        let n: i32 = ExitCode::Conflict.into();
        assert_eq!(n, 3);
    }

    // ---- HTTP status mapping (task 9.4) ---------------------------------

    #[test]
    fn success_range_maps_to_success() {
        for s in [100, 101, 200, 201, 204, 301, 302, 304, 399] {
            assert_eq!(
                ExitCode::from_http_status(s),
                ExitCode::Success,
                "status {s} must map to Success"
            );
        }
    }

    #[test]
    fn auth_denied_covers_401_and_403_only() {
        assert_eq!(ExitCode::from_http_status(401), ExitCode::AuthDenied);
        assert_eq!(ExitCode::from_http_status(403), ExitCode::AuthDenied);
        // 402 Payment Required and 407 Proxy Authentication Required
        // are NOT our auth flow; they fall into the general 4xx bucket.
        assert_eq!(ExitCode::from_http_status(402), ExitCode::Usage);
        assert_eq!(ExitCode::from_http_status(407), ExitCode::Usage);
    }

    #[test]
    fn conflict_maps_only_409() {
        assert_eq!(ExitCode::from_http_status(409), ExitCode::Conflict);
        // 410 Gone and 412 Precondition Failed are closely related but
        // distinct; lumping them into Conflict would lie to scripts
        // that specifically retry on `$? == 3`.
        assert_eq!(ExitCode::from_http_status(410), ExitCode::Usage);
        assert_eq!(ExitCode::from_http_status(412), ExitCode::Usage);
    }

    #[test]
    fn other_4xx_maps_to_usage() {
        for s in [400, 404, 405, 406, 408, 411, 413, 415, 418, 422, 429, 499] {
            assert_eq!(
                ExitCode::from_http_status(s),
                ExitCode::Usage,
                "status {s} must map to Usage"
            );
        }
    }

    #[test]
    fn fatal_5xx_and_transient_5xx_split_by_retry_semantics() {
        // Non-retryable: the server itself rejected the request.
        assert_eq!(ExitCode::from_http_status(500), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(501), ExitCode::ServerFatal);
        // Retryable: upstream/overload. A well-behaved script can loop
        // with backoff on these.
        assert_eq!(ExitCode::from_http_status(502), ExitCode::ServerTransient);
        assert_eq!(ExitCode::from_http_status(503), ExitCode::ServerTransient);
        assert_eq!(ExitCode::from_http_status(504), ExitCode::ServerTransient);
        // Unknown 5xx: conservative default is Fatal so scripts do not
        // hot-loop against a broken backend.
        assert_eq!(ExitCode::from_http_status(505), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(511), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(599), ExitCode::ServerFatal);
    }

    #[test]
    fn out_of_range_maps_to_server_fatal() {
        // A non-HTTP number reaching this function is itself a bug —
        // fail loudly rather than silently return Success.
        assert_eq!(ExitCode::from_http_status(0), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(99), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(600), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(u16::MAX), ExitCode::ServerFatal);
    }

    #[test]
    fn boundaries_match_the_documented_comment_table() {
        // One assertion per row of the doc-comment table so a reviewer
        // can eyeball parity at a glance.
        assert_eq!(ExitCode::from_http_status(200), ExitCode::Success);
        assert_eq!(ExitCode::from_http_status(401), ExitCode::AuthDenied);
        assert_eq!(ExitCode::from_http_status(403), ExitCode::AuthDenied);
        assert_eq!(ExitCode::from_http_status(409), ExitCode::Conflict);
        assert_eq!(ExitCode::from_http_status(422), ExitCode::Usage);
        assert_eq!(ExitCode::from_http_status(500), ExitCode::ServerFatal);
        assert_eq!(ExitCode::from_http_status(503), ExitCode::ServerTransient);
    }
}
