//! On-demand tenant wiring (B2 task 5.2c — lazy-fallback half of design §D5).
//!
//! Closes the sub-second race between a tenant being provisioned on pod A
//! and the `tenant:changed` pub/sub message reaching pod B. Callers that
//! are about to serve a tenant-scoped request invoke [`ensure_wired`];
//! the function is idempotent, cheap when the tenant is already `Available`,
//! and does the minimum work needed otherwise.
//!
//! # State transitions
//!
//! | Current state           | Action                                                  |
//! |-------------------------|---------------------------------------------------------|
//! | `None` (unknown)        | Resolve UUID, mark `Loading`, wire, mark terminal       |
//! | `Loading { .. }`        | Return as-is (another task owns the wiring)             |
//! | `Available { .. }`      | Return as-is (cheap path)                               |
//! | `LoadingFailed { .. }`  | Retry iff `last_attempt_at` older than cooldown         |
//!
//! # Cooldown
//!
//! The `LoadingFailed` cooldown prevents a stampede against a tenant
//! with a genuinely broken config (bad secret, unreachable provider).
//! Default `30s`; overridable via `AETERNA_LAZY_RETRY_COOLDOWN_SECS`.
//! Inside the cooldown, `ensure_wired` returns the stale `LoadingFailed`
//! state so the caller can issue a `503 tenant_unavailable` immediately
//! without hammering the provider.
//!
//! # Concurrency
//!
//! Two concurrent first-touch requests to the same unknown tenant may
//! both enter the wiring branch. This is accepted:
//!
//! * The provider registry is cache-on-success; the second resolve hits
//!   the warm cache inside `get_llm_service` (not a duplicate provider
//!   build).
//! * The runtime-state registry serialises `mark_loading` / `mark_available`
//!   under its `RwLock`; `last_rev` is monotonic even under concurrency.
//!
//! Adding a per-slug `tokio::sync::Mutex` to strictly serialise lazy
//! wirings would reduce duplicate resolver work under a provisioning
//! storm but costs an extra allocation per tenant. Deferred until a
//! metric (5.6) shows the wasted resolver time matters.
//!
//! # Slug ↔ UUID
//!
//! `TenantRuntimeRegistry` is keyed by *slug* (stable, human-readable).
//! `TenantProviderRegistry` is keyed by *TenantId* which in this codebase
//! is the tenant's UUID string. Lazy wiring must resolve slug → UUID
//! via the tenant store before priming caches. Missing tenant rows are
//! surfaced as `LoadingFailed` with `reason = "tenant not found"` — the
//! caller should translate that to HTTP 404, not 503.

use std::time::{Duration, SystemTime};

use tracing::{debug, warn};

use super::AppState;
use super::tenant_eager_wire::{truncate, wire_one};
use super::tenant_runtime_state::TenantRuntimeState;

/// Default cooldown before retrying a `LoadingFailed` tenant.
///
/// Chosen to be longer than a typical provider cold-start (10s) and
/// shorter than a human-observable SLO hit (60s).
const DEFAULT_FAILED_RETRY_COOLDOWN: Duration = Duration::from_secs(30);

/// Resolve a tenant slug to its internal `TenantId` (UUID).
///
/// Returns `Ok(None)` when the slug is unknown to the tenant store;
/// callers should distinguish that from a real I/O error.
pub(super) async fn resolve_slug_to_id(
    state: &AppState,
    slug: &str,
) -> anyhow::Result<Option<mk_core::types::TenantId>> {
    match state.tenant_store.get_tenant(slug).await? {
        Some(record) => Ok(Some(record.id)),
        None => Ok(None),
    }
}

/// Ensure `slug` has a current wiring on this pod and return the
/// resulting state.
///
/// Never panics. On any error path the runtime state is transitioned
/// to `LoadingFailed` and that state is returned — the caller is
/// expected to map it to HTTP 503 (or 404 for the specific
/// `tenant not found` reason).
pub async fn ensure_wired(state: &AppState, slug: &str) -> TenantRuntimeState {
    // Fast path: already in a terminal state that doesn't warrant retry.
    if let Some(cur) = state.tenant_runtime_state.get(slug).await {
        match &cur {
            TenantRuntimeState::Available { .. } | TenantRuntimeState::Loading { .. } => {
                return cur;
            }
            TenantRuntimeState::LoadingFailed {
                last_attempt_at, ..
            } => {
                if !cooldown_elapsed(*last_attempt_at) {
                    debug!(tenant = %slug, "lazy wire: in cooldown, returning stale failure");
                    return cur;
                }
                debug!(tenant = %slug, "lazy wire: cooldown elapsed, retrying");
            }
        }
    }

    // Slow path: wire now.
    let tenant_id = match resolve_slug_to_id(state, slug).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            // Missing row — mark as not-found so repeated 404s don't
            // repeatedly hit Postgres inside the cooldown. The reason
            // string is stable so callers can pattern-match on it.
            state
                .tenant_runtime_state
                .mark_failed(slug, "tenant not found")
                .await;
            return failed_snapshot_or(state, slug, "tenant not found").await;
        }
        Err(e) => {
            let reason = truncate(&format!("tenant store lookup failed: {e:#}"), 256);
            warn!(tenant = %slug, error = %reason, "lazy wire: slug resolution errored");
            state.tenant_runtime_state.mark_failed(slug, &reason).await;
            return failed_snapshot_or(state, slug, &reason).await;
        }
    };

    state.tenant_runtime_state.mark_loading(slug).await;
    match wire_one(state, &tenant_id).await {
        Ok(()) => {
            let rev = state.tenant_runtime_state.mark_available(slug).await;
            debug!(tenant = %slug, rev, "lazy wire: available");
        }
        Err(e) => {
            let reason = truncate(&format!("{e:#}"), 256);
            let retry_count = state.tenant_runtime_state.mark_failed(slug, &reason).await;
            warn!(
                tenant = %slug,
                retry_count,
                reason = %reason,
                "lazy wire: failed"
            );
        }
    }

    state
        .tenant_runtime_state
        .get(slug)
        .await
        .unwrap_or_else(TenantRuntimeState::loading_now)
}

/// True iff the configured retry cooldown has elapsed since `last_attempt_at`.
///
/// Uses wall-clock time for consistency with [`TenantRuntimeState`]
/// timestamps. A clock going backwards (NTP step) is treated as "cooldown
/// still active" which errs on the side of not stampeding providers.
fn cooldown_elapsed(last_attempt_at: SystemTime) -> bool {
    let cooldown = configured_cooldown();
    match SystemTime::now().duration_since(last_attempt_at) {
        Ok(d) => d >= cooldown,
        Err(_) => false, // clock skew — stay in cooldown
    }
}

/// Current cooldown, honoring the `AETERNA_LAZY_RETRY_COOLDOWN_SECS`
/// override. Malformed values fall back to the default and log once
/// (in practice the override is set at pod start).
fn configured_cooldown() -> Duration {
    match std::env::var("AETERNA_LAZY_RETRY_COOLDOWN_SECS") {
        Ok(v) => match v.parse::<u64>() {
            Ok(secs) => Duration::from_secs(secs),
            Err(_) => DEFAULT_FAILED_RETRY_COOLDOWN,
        },
        Err(_) => DEFAULT_FAILED_RETRY_COOLDOWN,
    }
}

/// Return the current state for `slug`, falling back to a synthetic
/// `LoadingFailed{reason}` when the registry has somehow lost the entry
/// between our `mark_failed` and this read. That race is not expected
/// under the current registry implementation; the fallback is a belt-
/// and-braces so callers never have to deal with `None` after we
/// promised them a terminal state.
async fn failed_snapshot_or(state: &AppState, slug: &str, reason: &str) -> TenantRuntimeState {
    state
        .tenant_runtime_state
        .get(slug)
        .await
        .unwrap_or_else(|| TenantRuntimeState::failed_now(reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All env-mutation tests live in one function because `std::env` is
    /// process-global and `cargo test` parallelises by default. Splitting
    /// them into separate `#[test]` functions produced a race where one
    /// test's `remove_var` nuked another's `set_var` mid-assertion.
    /// Consolidation is strictly weaker than introducing a `Mutex` or
    /// pulling in `serial_test` \u2014 we just run sequentially.
    #[test]
    fn cooldown_env_behaviour_covers_default_override_and_malformed() {
        let prior = std::env::var("AETERNA_LAZY_RETRY_COOLDOWN_SECS").ok();

        // Default when unset.
        unsafe {
            std::env::remove_var("AETERNA_LAZY_RETRY_COOLDOWN_SECS");
        }
        assert_eq!(configured_cooldown(), Duration::from_secs(30), "default");

        // Valid override is honoured verbatim.
        unsafe {
            std::env::set_var("AETERNA_LAZY_RETRY_COOLDOWN_SECS", "5");
        }
        assert_eq!(
            configured_cooldown(),
            Duration::from_secs(5),
            "valid override"
        );

        // Malformed override falls back to the compiled-in default so
        // a typo'd manifest value doesn't uncap retry frequency.
        unsafe {
            std::env::set_var("AETERNA_LAZY_RETRY_COOLDOWN_SECS", "not a number");
        }
        assert_eq!(
            configured_cooldown(),
            DEFAULT_FAILED_RETRY_COOLDOWN,
            "malformed falls back to default"
        );

        // Restore prior state so co-resident tests in the same process
        // observe whatever the ambient CI / shell configured.
        match prior {
            Some(v) => unsafe {
                std::env::set_var("AETERNA_LAZY_RETRY_COOLDOWN_SECS", v);
            },
            None => unsafe {
                std::env::remove_var("AETERNA_LAZY_RETRY_COOLDOWN_SECS");
            },
        }
    }

    #[test]
    fn cooldown_elapsed_on_past_stamp() {
        let past = SystemTime::now() - Duration::from_secs(120);
        assert!(cooldown_elapsed(past));
    }

    #[test]
    fn cooldown_not_elapsed_on_recent_stamp() {
        let recent = SystemTime::now() - Duration::from_secs(1);
        assert!(!cooldown_elapsed(recent));
    }

    #[test]
    fn cooldown_not_elapsed_on_future_stamp() {
        // Clock skew: last_attempt_at is in the future. We must not
        // retry — that would invite a storm every time NTP re-syncs.
        let future = SystemTime::now() + Duration::from_secs(60);
        assert!(!cooldown_elapsed(future));
    }
}
