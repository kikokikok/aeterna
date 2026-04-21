//! Per-tenant runtime readiness state (B2 task 5.1).
//!
//! This module defines [`TenantRuntimeState`], the state machine every
//! provisioned tenant passes through inside a running pod, and
//! [`TenantRuntimeRegistry`], the pod-local, async-safe map from tenant slug
//! to state.
//!
//! # Where this fits
//!
//! Task 5.1 is type-only: no boot loop, no pub/sub, no `/ready` wiring yet.
//! The enum and registry land first so downstream tasks (5.2 eager boot,
//! 5.3 `/ready` gate, 5.4 per-route 503, 5.5 status endpoint surfacing,
//! 5.6 Prometheus metrics) have a single, already-tested data type to
//! read and write. Keeping the drop this small also means the PR can
//! merge on its own without holding back other B2 work.
//!
//! # State machine
//!
//! ```text
//!               ┌──────────────┐
//!    (insert) → │   Loading    │
//!               └──────┬───────┘
//!                      │
//!           ┌──────────┴──────────┐
//!           ▼                     ▼
//!     ┌───────────┐         ┌───────────────┐
//!     │ Available │ ◄────── │ LoadingFailed │
//!     └─────┬─────┘  retry  └───────┬───────┘
//!           │                       │
//!           └───────► Loading ◄─────┘     (rewire on provision/update)
//! ```
//!
//! A slug with no entry in the registry is *not* the same as `Loading`: it
//! means the pod has never been asked about the tenant. Callers that
//! require an explicit answer (`/ready`, per-route 503) must treat
//! "absent" and "Loading" separately — see [`TenantRuntimeRegistry::get`]
//! and the lookup helpers below.
//!
//! # Concurrency model
//!
//! The registry is backed by a `tokio::sync::RwLock<HashMap<String, _>>`.
//! Readers outnumber writers by many orders of magnitude on the hot path
//! (every tenant-scoped request hits `get`; writes happen on boot, on
//! pub/sub invalidation, and on provision). We chose an async RwLock over
//! `parking_lot` so holders can `.await` across internal operations in
//! future tasks without deadlocking — eager boot (5.2) will want to hold
//! a write guard while awaiting a DB fetch, for example.
//!
//! # Why not `Arc<DashMap<..>>`?
//!
//! `dashmap` is already a transitive dep, but its sharded locks don't
//! integrate cleanly with async code and the per-slug contention here is
//! bounded by tenant count, not QPS: a single `RwLock<HashMap>` holds up
//! easily into the thousands of tenants. We can swap later if a profile
//! demands it; the public API is the only thing downstream tasks bind to.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Serialize;
use tokio::sync::RwLock;

/// Per-tenant readiness state in the pod-local registry.
///
/// The variants intentionally carry only the minimum needed by the
/// `/ready` gate (5.3) and the status endpoint (5.5): a reason string on
/// failure, a `since` timestamp on `Loading`, and a monotonically-growing
/// `rev` on `Available` so we can detect stale observations after a
/// rewire. Richer diagnostics (attempt counts, last-seen errors) belong in
/// metrics and structured logs, not in the state itself — that keeps the
/// enum cheap to clone and cheap to serialize.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TenantRuntimeState {
    /// Wiring is in progress. Entered on boot, on a `tenant:changed`
    /// pub/sub notification, and on synchronous rewire inside
    /// `provision_tenant`.
    Loading {
        /// Wall-clock time wiring started, UTC epoch seconds.
        #[serde(with = "serde_epoch")]
        since: SystemTime,
    },

    /// Providers (LLM / embedding / memory) are registered on this pod
    /// and the tenant is serving traffic.
    Available {
        /// Monotonic revision bumped on every successful rewire. Lets
        /// callers distinguish "Pod A saw rev=7 but the handler ran against
        /// rev=8" — useful in the status endpoint and in metrics labels.
        rev: u64,
        /// Wall-clock time the current wiring landed.
        #[serde(with = "serde_epoch")]
        wired_at: SystemTime,
    },

    /// The last wiring attempt failed. Requests to this tenant return
    /// `503 tenant_unavailable` (5.4) and `/ready` flips to 503 when
    /// strict mode is on (5.3).
    LoadingFailed {
        /// Human-readable summary suitable for surfacing in error bodies
        /// and logs. Never leaks secret material — producers must redact
        /// before calling.
        reason: String,
        /// When the last failing attempt finished.
        #[serde(with = "serde_epoch")]
        last_attempt_at: SystemTime,
        /// How many consecutive failing attempts have happened since the
        /// last `Available`. Reset to 0 on any successful rewire.
        retry_count: u32,
    },
}

impl TenantRuntimeState {
    /// Convenience constructor for the initial `Loading` transition,
    /// stamped with `SystemTime::now()`. Helps keep call sites short.
    pub fn loading_now() -> Self {
        Self::Loading {
            since: SystemTime::now(),
        }
    }

    /// Convenience constructor for the success transition.
    pub fn available_now(rev: u64) -> Self {
        Self::Available {
            rev,
            wired_at: SystemTime::now(),
        }
    }

    /// Convenience constructor for a fresh failure (retry_count = 1).
    /// Callers that need to increment retry_count must go through
    /// [`TenantRuntimeRegistry::mark_failed`] so the increment reads the
    /// prior state under the registry lock.
    pub fn failed_now(reason: impl Into<String>) -> Self {
        Self::LoadingFailed {
            reason: reason.into(),
            last_attempt_at: SystemTime::now(),
            retry_count: 1,
        }
    }

    /// True iff the tenant can currently serve traffic on this pod.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }

    /// True iff the last wiring attempt failed and hasn't been retried
    /// into success.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::LoadingFailed { .. })
    }
}

/// Pod-local, async-safe registry keyed by tenant slug.
///
/// Wrap in `Arc` and store on `AppState`. `Clone` is cheap (Arc bump).
#[derive(Debug, Default)]
struct RegistryInner {
    /// Current observable state per slug.
    state: HashMap<String, TenantRuntimeState>,
    /// Highest `rev` ever assigned to each slug. Kept separately from
    /// the state enum so `Loading` and `LoadingFailed` transitions don't
    /// lose the monotonic counter — the next `Available` still gets
    /// `last_rev + 1`. This preserves "rev only ever increases" across
    /// any number of rewire cycles.
    last_rev: HashMap<String, u64>,
}

#[derive(Debug, Clone, Default)]
pub struct TenantRuntimeRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl TenantRuntimeRegistry {
    /// Create an empty registry. Pre-sized for typical single-digit-thousands
    /// tenant counts; the `RwLock<HashMap>` grows on demand regardless.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the state of a single tenant. Returns `None` when no entry
    /// exists — callers MUST treat this as "unknown to this pod" rather
    /// than "available", otherwise a missing-entry race on boot would let
    /// unwireable tenants serve traffic.
    pub async fn get(&self, slug: &str) -> Option<TenantRuntimeState> {
        self.inner.read().await.state.get(slug).cloned()
    }

    /// Upsert a state for a slug. Returns the previous state if any.
    /// This is the low-level primitive; most callers should use the
    /// `mark_*` helpers which carry the correct state-machine transitions.
    ///
    /// If `state` is `Available { rev, .. }`, the registry's internal
    /// `last_rev` is advanced to `max(last_rev, rev)` so a subsequent
    /// `mark_available` starts from at least `rev + 1`.
    pub async fn set(&self, slug: &str, state: TenantRuntimeState) -> Option<TenantRuntimeState> {
        let mut guard = self.inner.write().await;
        if let TenantRuntimeState::Available { rev, .. } = &state {
            let entry = guard.last_rev.entry(slug.to_string()).or_insert(0);
            if *rev > *entry {
                *entry = *rev;
            }
        }
        guard.state.insert(slug.to_string(), state)
    }

    /// Transition `slug` to `Loading { since = now }`. Idempotent —
    /// re-calling while already `Loading` refreshes the `since` stamp so
    /// long-running wirings can show progress via the status endpoint.
    pub async fn mark_loading(&self, slug: &str) {
        self.inner
            .write()
            .await
            .state
            .insert(slug.to_string(), TenantRuntimeState::loading_now());
    }

    /// Transition `slug` to `Available`. Increments `last_rev` under the
    /// write lock so the counter is monotonic across concurrent rewires
    /// and survives intermediate `Loading`/`LoadingFailed` transitions.
    pub async fn mark_available(&self, slug: &str) -> u64 {
        let mut guard = self.inner.write().await;
        let next_rev = guard
            .last_rev
            .get(slug)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        guard.last_rev.insert(slug.to_string(), next_rev);
        guard.state.insert(
            slug.to_string(),
            TenantRuntimeState::available_now(next_rev),
        );
        next_rev
    }

    /// Transition `slug` to `LoadingFailed`. Increments `retry_count` if
    /// the prior state was already `LoadingFailed`, else starts at 1.
    /// Reading the prior state under the write lock guarantees the count
    /// is monotonic under concurrent failures.
    pub async fn mark_failed(&self, slug: &str, reason: impl Into<String>) -> u32 {
        let reason = reason.into();
        let mut guard = self.inner.write().await;
        let retry_count = match guard.state.get(slug) {
            Some(TenantRuntimeState::LoadingFailed { retry_count, .. }) => {
                retry_count.saturating_add(1)
            }
            _ => 1,
        };
        guard.state.insert(
            slug.to_string(),
            TenantRuntimeState::LoadingFailed {
                reason,
                last_attempt_at: SystemTime::now(),
                retry_count,
            },
        );
        retry_count
    }

    /// Remove a tenant's entry entirely. Used on tenant deletion so the
    /// slug is not reported in `/admin/.../status` or `/ready` after the
    /// row is gone. Also clears the stored `last_rev` so a future
    /// re-creation of the same slug starts rev counting from 1 — slug
    /// reuse after deletion is an operator-initiated event, and leaking
    /// the prior count would confuse status dashboards.
    pub async fn forget(&self, slug: &str) -> Option<TenantRuntimeState> {
        let mut guard = self.inner.write().await;
        guard.last_rev.remove(slug);
        guard.state.remove(slug)
    }

    /// Snapshot the whole registry as `(slug, state)` pairs. Used by the
    /// `/ready` gate (5.3) and the status endpoint (5.5) to render a
    /// consistent view without holding the lock during serialization.
    ///
    /// The returned vector is unsorted; callers that need a deterministic
    /// order should sort by slug at the call site.
    pub async fn snapshot(&self) -> Vec<(String, TenantRuntimeState)> {
        self.inner
            .read()
            .await
            .state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// True iff every tenant in the registry is `Available`. Used by the
    /// `/ready` gate (5.3). An empty registry returns `true` — the pod
    /// has not been told about any tenant and therefore has nothing to
    /// fail; the eager boot loop is responsible for populating the
    /// registry before `/ready` can legitimately return 200.
    pub async fn all_available(&self) -> bool {
        self.inner
            .read()
            .await
            .state
            .values()
            .all(TenantRuntimeState::is_available)
    }

    /// Count of tenants in `LoadingFailed`. Cheap gauge-friendly number
    /// for 5.6 metrics; a full snapshot isn't required just to report
    /// the cardinality.
    pub async fn failed_count(&self) -> usize {
        self.inner
            .read()
            .await
            .state
            .values()
            .filter(|s| s.is_failed())
            .count()
    }
}

/// Serialize `SystemTime` as UTC epoch seconds. Kept module-private; the
/// enum is the public surface.
mod serde_epoch {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde::{Serialize, Serializer};

    pub fn serialize<S>(t: &SystemTime, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = t
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        secs.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_registry_is_all_available_vacuously() {
        // An empty registry is "all available" because there is nothing
        // to be not-available. The boot-loop contract (task 5.2) is what
        // prevents `/ready` from returning 200 prematurely by ensuring
        // at least one tenant is present before the gate flips.
        let r = TenantRuntimeRegistry::new();
        assert!(r.all_available().await);
        assert_eq!(r.failed_count().await, 0);
        assert!(r.snapshot().await.is_empty());
    }

    #[tokio::test]
    async fn absent_tenant_is_not_available() {
        // Absence ≠ Available. The lookup helpers used by per-route 503
        // (task 5.4) rely on this distinction to avoid serving traffic
        // on pods that haven't heard of a tenant yet.
        let r = TenantRuntimeRegistry::new();
        assert!(r.get("ghost").await.is_none());
    }

    #[tokio::test]
    async fn mark_loading_then_available_flows() {
        let r = TenantRuntimeRegistry::new();
        r.mark_loading("acme").await;
        match r.get("acme").await {
            Some(TenantRuntimeState::Loading { .. }) => {}
            other => panic!("expected Loading, got {other:?}"),
        }
        let rev = r.mark_available("acme").await;
        assert_eq!(rev, 1, "first Available must be rev=1");
        assert!(r.all_available().await);
    }

    #[tokio::test]
    async fn mark_available_is_monotonic_across_rewires() {
        // Rewire on provision should bump `rev` so callers can detect
        // "the value I observed is stale". We do NOT reset rev on a
        // transient Loading state in between — rev only ever increases.
        let r = TenantRuntimeRegistry::new();
        assert_eq!(r.mark_available("acme").await, 1);
        r.mark_loading("acme").await;
        assert_eq!(r.mark_available("acme").await, 2);
        r.mark_loading("acme").await;
        assert_eq!(r.mark_available("acme").await, 3);
    }

    #[tokio::test]
    async fn mark_failed_increments_retry_count() {
        let r = TenantRuntimeRegistry::new();
        assert_eq!(r.mark_failed("acme", "boom 1").await, 1);
        assert_eq!(r.mark_failed("acme", "boom 2").await, 2);
        assert_eq!(r.mark_failed("acme", "boom 3").await, 3);
        assert_eq!(r.failed_count().await, 1, "failed_count is per-slug");
        match r.get("acme").await {
            Some(TenantRuntimeState::LoadingFailed {
                reason,
                retry_count,
                ..
            }) => {
                assert_eq!(retry_count, 3);
                assert_eq!(reason, "boom 3", "latest reason wins");
            }
            other => panic!("expected LoadingFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn success_resets_retry_count_on_next_failure() {
        // The contract: retry_count counts CONSECUTIVE failures. A
        // successful rewire in between must reset the counter, otherwise
        // operators lose the "currently flapping" signal.
        let r = TenantRuntimeRegistry::new();
        assert_eq!(r.mark_failed("acme", "a").await, 1);
        assert_eq!(r.mark_failed("acme", "b").await, 2);
        r.mark_available("acme").await;
        assert_eq!(
            r.mark_failed("acme", "c").await,
            1,
            "must reset after success"
        );
    }

    #[tokio::test]
    async fn all_available_requires_every_tenant_available() {
        let r = TenantRuntimeRegistry::new();
        r.mark_available("a").await;
        r.mark_available("b").await;
        assert!(r.all_available().await);
        r.mark_failed("b", "rot").await;
        assert!(!r.all_available().await, "one failure taints the pod");
    }

    #[tokio::test]
    async fn forget_removes_the_entry() {
        let r = TenantRuntimeRegistry::new();
        r.mark_available("doomed").await;
        let prior = r.forget("doomed").await;
        assert!(prior.is_some());
        assert!(r.get("doomed").await.is_none());
        assert!(r.forget("doomed").await.is_none(), "forget is idempotent");
    }

    #[tokio::test]
    async fn snapshot_is_consistent_with_underlying_state() {
        let r = TenantRuntimeRegistry::new();
        r.mark_loading("a").await;
        r.mark_available("b").await;
        r.mark_failed("c", "kaboom").await;
        let mut snap = r.snapshot().await;
        snap.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].0, "a");
        assert!(matches!(snap[0].1, TenantRuntimeState::Loading { .. }));
        assert_eq!(snap[1].0, "b");
        assert!(snap[1].1.is_available());
        assert_eq!(snap[2].0, "c");
        assert!(snap[2].1.is_failed());
    }

    #[test]
    fn state_serializes_with_tag_discriminator() {
        // Status endpoint (5.5) will emit these over JSON. Lock the
        // serde tag shape now so a future renamer trips a test, not a
        // dashboard.
        let s = TenantRuntimeState::Available {
            rev: 7,
            wired_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["state"], "available");
        assert_eq!(v["rev"], 7);
        assert_eq!(v["wiredAt"], 1_700_000_000);
    }

    #[test]
    fn loading_and_failed_serialize_distinguishably() {
        let loading = TenantRuntimeState::Loading {
            since: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100),
        };
        let failed = TenantRuntimeState::LoadingFailed {
            reason: "missing secret".into(),
            last_attempt_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200),
            retry_count: 3,
        };
        let v1 = serde_json::to_value(&loading).unwrap();
        assert_eq!(v1["state"], "loading");
        assert_eq!(v1["since"], 100);
        let v2 = serde_json::to_value(&failed).unwrap();
        assert_eq!(v2["state"], "loadingFailed");
        assert_eq!(v2["reason"], "missing secret");
        assert_eq!(v2["lastAttemptAt"], 200);
        assert_eq!(v2["retryCount"], 3);
    }
}
