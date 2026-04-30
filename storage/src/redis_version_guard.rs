//! Server-version guard against stale Redis state across upgrades.
//!
//! When the aeterna server binary is rolled to a new version, items already
//! cached in Redis from a previous version may have schemas that the new
//! binary cannot deserialise. The most recent example surfaced in rc.8 (rc.9
//! triage item B1): stale `git_provider_connections` records carrying
//! `providerKind: gitHubApp` (camelCase) failed deserialise on the new
//! binary, which expected the PascalCase `GitHubApp` enum tag. Items with
//! that shape stuck around forever because Redis is shared across
//! deployments and nothing was purging them on upgrade.
//!
//! [`RedisVersionGuard::ensure_version`] is a startup-time guard that
//! compares the *expected* server version against a sentinel stored in
//! Redis under [`SERVER_VERSION_KEY`]. When the values differ, the matching
//! key prefix is flushed (via `SCAN`+`UNLINK`, never `FLUSHDB`) so the new
//! binary never reads stale items it cannot parse, and the sentinel is
//! updated to the new version.
//!
//! ## Concurrency model
//!
//! During a Kubernetes rolling deploy, every fresh replica racing into
//! `bootstrap()` would otherwise call this guard concurrently. A naive
//! implementation would have N replicas each running `SCAN`+`UNLINK` against
//! the same prefix while clients are issuing writes to keys the new code
//! considers valid — a write/purge race that could corrupt fresh data.
//!
//! To prevent that, the guard uses Redis itself as the coordinator:
//! [`PURGE_LOCK_KEY`] is acquired with `SET NX PX` (set-if-not-exists with
//! millisecond TTL). Exactly one replica wins the lock and performs the
//! purge; the rest observe the post-purge sentinel and proceed without
//! touching the data plane. The lock TTL ([`PURGE_LOCK_TTL`]) is the
//! deliberately-long upper bound on a full prefix flush; if the leader
//! crashes mid-purge, the TTL ensures the next deploy can still acquire it.
//!
//! ## What gets flushed
//!
//! Only keys whose names start with the configured prefix (default
//! `aeterna:`). `FLUSHDB` is intentionally not used — the deployment
//! cluster's Dragonfly / Redis instance may hold non-aeterna keys (e.g.
//! Helm-managed prereqs, OPAL fetcher cache) that must survive an aeterna
//! upgrade.
//!
//! ## Failure semantics
//!
//! The guard is **fail-open** for I/O errors: if Redis is briefly
//! unreachable, we log the failure and let bootstrap continue rather than
//! refusing to start the server. Stale data is a correctness concern but
//! not a security one, and refusing to boot would make Redis a hard
//! dependency for *every* aeterna pod. A loud `tracing::warn!` surfaces the
//! failure in deployment logs.

use std::sync::Arc;
use std::time::Duration;

use redis::AsyncCommands;
use thiserror::Error;

/// Redis key under which the version sentinel is stored.
pub const SERVER_VERSION_KEY: &str = "aeterna:server:version";

/// Redis key used as a leader-election lock for the purge operation.
pub const PURGE_LOCK_KEY: &str = "aeterna:server:bootstrap:purge-lock";

/// TTL of the purge lock. Long enough that a full prefix flush of a busy
/// production Dragonfly completes within the budget, short enough that a
/// crashed leader does not block the next deploy more than briefly.
pub const PURGE_LOCK_TTL: Duration = Duration::from_secs(30);

/// Default prefix the guard scans for purge candidates. Aligns with the
/// `aeterna:*` prefix used by every `RedisStore` instance in the codebase.
pub const DEFAULT_PURGE_PREFIX: &str = "aeterna:";

/// Page size for `SCAN MATCH`. 1000 is the same value used by
/// `redis-cli --scan` and offers a reasonable compromise between round-trip
/// count and per-call work.
const SCAN_PAGE_SIZE: usize = 1000;

/// Outcome of a single [`RedisVersionGuard::ensure_version`] call. Logged
/// at boot so operators can see in deployment output exactly which path
/// fired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionAction {
    /// The sentinel matched the expected version. No purge occurred.
    Match,
    /// No sentinel was present (first deploy, or sentinel manually wiped).
    /// The sentinel was written; nothing was purged.
    Initialized,
    /// The sentinel did not match. Held keys under the prefix were unlinked
    /// and the sentinel was rewritten.
    Purged {
        /// The previous sentinel value.
        from: String,
        /// Number of keys removed by the `UNLINK` calls.
        keys_removed: u64,
    },
    /// Another replica won the leader-election lock and is performing the
    /// purge. This replica observed a stale sentinel and did not touch the
    /// data plane; it will see the post-purge sentinel on the next request.
    SkippedNotLeader { current: String },
}

/// Errors raised by the version guard.
#[derive(Debug, Error)]
pub enum VersionGuardError {
    #[error("redis i/o error: {0}")]
    Redis(#[from] redis::RedisError),
}

/// The decision returned by the pure version-comparison helper. Separated
/// from the I/O-bearing [`RedisVersionGuard::ensure_version`] so it can be
/// unit-tested without a Redis connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionDecision {
    /// Sentinel and expected match — no work required.
    NoOp,
    /// Sentinel absent — write the expected value.
    InitialiseSentinel,
    /// Sentinel present but differs — acquire the lock and purge.
    PurgeRequired { from: String },
}

/// Pure decision function: given the current sentinel value (`Some(s)` if
/// Redis returned a value, `None` if missing) and the expected value, what
/// should the guard do?
///
/// Pulled out as a free function so the matrix of (present/absent) ×
/// (match/differ) cases is unit-testable without spinning up a Redis.
pub fn decide_action(current: Option<&str>, expected: &str) -> VersionDecision {
    match current {
        Some(v) if v == expected => VersionDecision::NoOp,
        Some(v) => VersionDecision::PurgeRequired { from: v.to_string() },
        None => VersionDecision::InitialiseSentinel,
    }
}

/// Server-version guard wrapping a Redis [`ConnectionManager`].
///
/// Holds the connection plus the configurable prefix used both for the
/// purge scan and for any non-default version-key namespacing (test
/// isolation, primarily). The default constructor uses
/// [`DEFAULT_PURGE_PREFIX`].
pub struct RedisVersionGuard {
    conn: Arc<redis::aio::ConnectionManager>,
    prefix: String,
}

impl RedisVersionGuard {
    /// Construct a guard with the default `aeterna:` prefix.
    pub fn new(conn: Arc<redis::aio::ConnectionManager>) -> Self {
        Self {
            conn,
            prefix: DEFAULT_PURGE_PREFIX.to_string(),
        }
    }

    /// Construct a guard with a caller-specified key prefix. Intended for
    /// integration tests that share a Redis instance and need isolation.
    pub fn with_prefix(conn: Arc<redis::aio::ConnectionManager>, prefix: &str) -> Self {
        Self {
            conn,
            prefix: prefix.to_string(),
        }
    }

    /// Compare the stored sentinel against `expected_version`; on mismatch,
    /// acquire the leader-election lock and purge keys whose names start
    /// with the configured prefix; on absence, write the sentinel.
    ///
    /// Returns the [`VersionAction`] that fired. The caller (typically
    /// bootstrap) should log the result so deployment operators can see
    /// whether a purge ran on this rollout.
    pub async fn ensure_version(
        &self,
        expected_version: &str,
    ) -> Result<VersionAction, VersionGuardError> {
        let mut conn = (*self.conn).clone();

        let current: Option<String> = conn.get(SERVER_VERSION_KEY).await?;
        match decide_action(current.as_deref(), expected_version) {
            VersionDecision::NoOp => Ok(VersionAction::Match),
            VersionDecision::InitialiseSentinel => {
                conn.set::<_, _, ()>(SERVER_VERSION_KEY, expected_version)
                    .await?;
                Ok(VersionAction::Initialized)
            }
            VersionDecision::PurgeRequired { from } => {
                if !self.acquire_purge_lock(&mut conn).await? {
                    return Ok(VersionAction::SkippedNotLeader { current: from });
                }
                let keys_removed = self.flush_prefix(&mut conn).await?;
                conn.set::<_, _, ()>(SERVER_VERSION_KEY, expected_version)
                    .await?;
                Ok(VersionAction::Purged { from, keys_removed })
            }
        }
    }

    /// Acquire the purge lock via `SET NX PX`. Returns `true` if this
    /// caller won the lock, `false` if another replica holds it.
    async fn acquire_purge_lock(
        &self,
        conn: &mut redis::aio::ConnectionManager,
    ) -> Result<bool, VersionGuardError> {
        let acquired: Option<String> = redis::cmd("SET")
            .arg(PURGE_LOCK_KEY)
            .arg("held")
            .arg("NX")
            .arg("PX")
            .arg(PURGE_LOCK_TTL.as_millis() as u64)
            .query_async(conn)
            .await?;
        Ok(acquired.is_some())
    }

    /// Iterate `SCAN MATCH {prefix}*` and `UNLINK` every batch. Returns the
    /// total number of keys removed. The version sentinel and the purge
    /// lock both live under the prefix; deleting the lock would release it
    /// prematurely and the sentinel is about to be rewritten anyway, so we
    /// excluded both from the unlink set explicitly.
    async fn flush_prefix(
        &self,
        conn: &mut redis::aio::ConnectionManager,
    ) -> Result<u64, VersionGuardError> {
        let pattern = format!("{}*", self.prefix);
        let mut cursor: u64 = 0;
        let mut removed: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(SCAN_PAGE_SIZE)
                .query_async(conn)
                .await?;

            let to_unlink: Vec<&String> = keys
                .iter()
                .filter(|k| k.as_str() != SERVER_VERSION_KEY && k.as_str() != PURGE_LOCK_KEY)
                .collect();

            if !to_unlink.is_empty() {
                let n: u64 = redis::cmd("UNLINK")
                    .arg(&to_unlink)
                    .query_async(conn)
                    .await?;
                removed += n;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // decide_action — pure decision matrix.
    // -------------------------------------------------------------------------

    #[test]
    fn decide_noop_when_sentinel_matches() {
        assert_eq!(
            decide_action(Some("0.8.0-rc.9"), "0.8.0-rc.9"),
            VersionDecision::NoOp
        );
    }

    #[test]
    fn decide_initialise_when_sentinel_absent() {
        assert_eq!(
            decide_action(None, "0.8.0-rc.9"),
            VersionDecision::InitialiseSentinel
        );
    }

    #[test]
    fn decide_purge_when_sentinel_differs() {
        assert_eq!(
            decide_action(Some("0.8.0-rc.8"), "0.8.0-rc.9"),
            VersionDecision::PurgeRequired {
                from: "0.8.0-rc.8".to_string()
            }
        );
    }

    #[test]
    fn decide_treats_empty_sentinel_as_present_and_different() {
        // A blank sentinel should NOT be treated as 'absent' — someone
        // explicitly wrote an empty string, and the comparison logic should
        // surface that as a mismatch (not silently re-initialise).
        assert_eq!(
            decide_action(Some(""), "0.8.0-rc.9"),
            VersionDecision::PurgeRequired { from: String::new() }
        );
    }

    #[test]
    fn decide_is_case_sensitive() {
        // Git SHAs are hex, version strings are lowercase by convention.
        // Defensive: do not normalise case, since 'RC.9' and 'rc.9' as
        // sentinels would represent different deploys.
        assert_eq!(
            decide_action(Some("0.8.0-RC.9"), "0.8.0-rc.9"),
            VersionDecision::PurgeRequired {
                from: "0.8.0-RC.9".to_string()
            }
        );
    }

    // -------------------------------------------------------------------------
    // Constants — these names are part of the operator-visible contract.
    // Any rename should be a deliberate, breaking change.
    // -------------------------------------------------------------------------

    #[test]
    fn key_constants_use_the_aeterna_prefix() {
        assert!(SERVER_VERSION_KEY.starts_with(DEFAULT_PURGE_PREFIX));
        assert!(PURGE_LOCK_KEY.starts_with(DEFAULT_PURGE_PREFIX));
        // Lock and version keys MUST be distinct — the purge logic relies
        // on excluding both from the SCAN-then-UNLINK set.
        assert_ne!(SERVER_VERSION_KEY, PURGE_LOCK_KEY);
    }

    #[test]
    fn purge_lock_ttl_is_long_enough_for_a_realistic_flush() {
        // 30s is the agreed lower bound for a full Dragonfly flush of a
        // production-sized aeterna prefix. Shorter TTLs risk the leader
        // losing the lock mid-purge.
        assert!(PURGE_LOCK_TTL >= Duration::from_secs(30));
        // But not so long that a crashed leader blocks the next deploy
        // for an absurd duration.
        assert!(PURGE_LOCK_TTL <= Duration::from_secs(120));
    }
}
