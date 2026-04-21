//! Prometheus metrics for tenant wiring state (B2 task 5.6).
//!
//! Emitted from [`TenantRuntimeRegistry`] on every state transition.
//! Closes the observability loop:
//!
//!   5.3 `/ready`                  pod-level traffic gate
//!   5.4 `require_available_tenant` caller-visible 503
//!   5.5 `/admin/.../wiring`        PA-only detail (reasons)
//!   5.6 Prometheus metrics         fleet-level time series     ← this
//!
//! # Metric catalog
//!
//! | Metric                                    | Type      | Labels         |
//! |-------------------------------------------|-----------|----------------|
//! | `tenant_state`                            | gauge     | slug, state    |
//! | `tenant_wiring_duration_seconds`          | histogram | result         |
//! | `tenant_state_transitions_total`          | counter   | from, to       |
//!
//! ## `tenant_state{slug,state}` — gauge 0/1
//!
//! Emitted three series per slug — one for each of `loading`,
//! `available`, `loadingFailed`. Exactly one is `1`; the others are `0`.
//! This matches the kube-state-metrics idiom
//! (`kube_pod_status_phase{phase=...}`) and lets PromQL alerts be written
//! without hardcoded state enumerations:
//!
//! ```promql
//! # Alert if any tenant has been failing for ≥ 10m
//! max_over_time(tenant_state{state="loadingFailed"}[10m]) == 1
//! ```
//!
//! On `forget(slug)` all three series are reset to `0`. Prometheus has
//! no “delete series” API at write time; leaving them at `0` is the
//! recommended pattern (they will age out of the TSDB after a retention
//! window).
//!
//! ## `tenant_wiring_duration_seconds{result}` — histogram
//!
//! Recorded on every transition out of `Loading`:
//! `result=success` on → Available, `result=failure` on → LoadingFailed.
//! Duration is `now - since`. Slug is deliberately NOT a label here —
//! histogram cardinality grows fastest of the three metric types and
//! the distribution is a fleet-level concern. Operators who need per-
//! tenant timing look at traces, not Prometheus.
//!
//! ## `tenant_state_transitions_total{from,to}` — counter
//!
//! Rate of state transitions. Useful for flapping alerts without having
//! to derive them from the gauge:
//!
//! ```promql
//! # Flap alert: >5 failing rewires/minute across the whole fleet
//! sum(rate(tenant_state_transitions_total{to="loadingFailed"}[5m])) > 5/60
//! ```
//!
//! Labels are bounded enum values — no cardinality risk.
//!
//! # Performance
//!
//! Metric emission is fire-and-forget (`metrics` crate dispatches to the
//! installed recorder); calling it under the registry write lock adds
//! a handful of hashmap operations per transition, well under the
//! existing cost of the `RwLock` upgrade. We intentionally do NOT move
//! emission outside the lock because the prev→next transition has to be
//! observed atomically for the counter to be correct.

use std::time::SystemTime;

use super::tenant_runtime_state::TenantRuntimeState;

/// Stable label values for `tenant_state{state}`. Kept as `&'static str`
/// so no allocation per emission.
const STATE_LOADING: &str = "loading";
const STATE_AVAILABLE: &str = "available";
const STATE_LOADING_FAILED: &str = "loadingFailed";
const STATE_ABSENT: &str = "absent";

/// Map a state to its label. `absent` is reserved for `forget()` so the
/// transition counter can express “tenant removed” without a dedicated
/// metric.
fn state_label(s: Option<&TenantRuntimeState>) -> &'static str {
    match s {
        None => STATE_ABSENT,
        Some(TenantRuntimeState::Loading { .. }) => STATE_LOADING,
        Some(TenantRuntimeState::Available { .. }) => STATE_AVAILABLE,
        Some(TenantRuntimeState::LoadingFailed { .. }) => STATE_LOADING_FAILED,
    }
}

/// Emit metrics for a state transition.
///
/// Invariants upheld by this function:
/// * Exactly one `tenant_state{slug,state}` series is set to `1` after
///   the call (unless `next == None`, in which case all three are `0`).
/// * The transition counter is incremented exactly once.
/// * The wiring-duration histogram is recorded iff `prev` was `Loading`
///   and `next` is `Available` or `LoadingFailed`.
pub fn record_transition(
    slug: &str,
    prev: Option<&TenantRuntimeState>,
    next: Option<&TenantRuntimeState>,
) {
    let from = state_label(prev);
    let to = state_label(next);

    // 1. Transition counter — always.
    metrics::counter!(
        "tenant_state_transitions_total",
        "from" => from,
        "to" => to,
    )
    .increment(1);

    // 2. Gauges: set exactly one (or zero) to 1, the others to 0. We
    //    emit all three every time so a series that was previously
    //    `1` on a different state flips back to `0` atomically from
    //    the recorder's perspective.
    let slug_owned = slug.to_string();
    let set = |state_label: &'static str, val: f64| {
        metrics::gauge!(
            "tenant_state",
            "slug" => slug_owned.clone(),
            "state" => state_label,
        )
        .set(val);
    };
    match next {
        None => {
            set(STATE_LOADING, 0.0);
            set(STATE_AVAILABLE, 0.0);
            set(STATE_LOADING_FAILED, 0.0);
        }
        Some(TenantRuntimeState::Loading { .. }) => {
            set(STATE_LOADING, 1.0);
            set(STATE_AVAILABLE, 0.0);
            set(STATE_LOADING_FAILED, 0.0);
        }
        Some(TenantRuntimeState::Available { .. }) => {
            set(STATE_LOADING, 0.0);
            set(STATE_AVAILABLE, 1.0);
            set(STATE_LOADING_FAILED, 0.0);
        }
        Some(TenantRuntimeState::LoadingFailed { .. }) => {
            set(STATE_LOADING, 0.0);
            set(STATE_AVAILABLE, 0.0);
            set(STATE_LOADING_FAILED, 1.0);
        }
    }

    // 3. Duration histogram — only on transitions out of Loading.
    if let (Some(TenantRuntimeState::Loading { since }), Some(n)) = (prev, next) {
        let result = match n {
            TenantRuntimeState::Available { .. } => Some("success"),
            TenantRuntimeState::LoadingFailed { .. } => Some("failure"),
            // Loading → Loading: idempotent refresh, not a wiring
            // completion. Skip.
            TenantRuntimeState::Loading { .. } => None,
        };
        if let Some(result) = result {
            let elapsed = SystemTime::now()
                .duration_since(*since)
                .unwrap_or_default()
                .as_secs_f64();
            metrics::histogram!(
                "tenant_wiring_duration_seconds",
                "result" => result,
            )
            .record(elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
    use std::collections::HashMap;
    use std::time::Duration;

    /// Run `f` with a fresh `DebuggingRecorder` installed as the
    /// thread-local metrics recorder, returning the captured snapshot.
    /// Using `with_local_recorder` keeps emission assertions parallel-safe:
    /// the global recorder is untouched, so other tests running in parallel
    /// do not race ours.
    fn capture<F: FnOnce()>(f: F) -> Snapshotter {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, f);
        snapshotter
    }

    /// Collapse a snapshot to a map keyed by `(metric_name, sorted_labels)`
    /// so assertions can look up specific series by content regardless of
    /// emission order.
    fn index(snap: &Snapshotter) -> HashMap<(String, Vec<(String, String)>), DebugValue> {
        let mut out = HashMap::new();
        for (ck, _unit, _desc, val) in snap.snapshot().into_vec() {
            let key = ck.key();
            let name = key.name().to_string();
            let mut labels: Vec<(String, String)> = key
                .labels()
                .map(|l| (l.key().to_string(), l.value().to_string()))
                .collect();
            labels.sort();
            out.insert((name, labels), val);
        }
        out
    }

    fn labels<const N: usize>(pairs: [(&str, &str); N]) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn state_label_covers_all_variants() {
        assert_eq!(state_label(None), STATE_ABSENT);
        assert_eq!(
            state_label(Some(&TenantRuntimeState::loading_now())),
            STATE_LOADING
        );
        assert_eq!(
            state_label(Some(&TenantRuntimeState::available_now(1))),
            STATE_AVAILABLE
        );
        assert_eq!(
            state_label(Some(&TenantRuntimeState::failed_now("x"))),
            STATE_LOADING_FAILED
        );
    }

    /// Smoke test: emitting against the default (no-op) recorder must not
    /// panic. The real recorder is installed in main() via
    /// [`crate::server::metrics::create_recorder`]; test runs use the
    /// no-op default unless a test specifically installs one. This guard
    /// catches typos in format strings and nil-deref bugs.
    #[test]
    fn record_transition_does_not_panic_on_any_combination() {
        let states: [Option<TenantRuntimeState>; 4] = [
            None,
            Some(TenantRuntimeState::Loading {
                since: SystemTime::now() - Duration::from_millis(50),
            }),
            Some(TenantRuntimeState::available_now(1)),
            Some(TenantRuntimeState::failed_now("r")),
        ];
        for prev in states.iter() {
            for next in states.iter() {
                record_transition("test", prev.as_ref(), next.as_ref());
            }
        }
    }

    /// The histogram must be recorded only on Loading→Available or
    /// Loading→LoadingFailed. We verify the selection logic here —
    /// the metric recording itself is exercised by the
    /// does_not_panic test above.
    #[test]
    fn histogram_only_fires_on_completion_of_wiring() {
        // These are the two cases where we expect a histogram sample.
        // Verified indirectly: the function must NOT panic on any,
        // and the logic must match the match-arms. We simulate the
        // selection predicate directly.
        fn should_record(
            prev: Option<&TenantRuntimeState>,
            next: Option<&TenantRuntimeState>,
        ) -> bool {
            matches!(
                (prev, next),
                (
                    Some(TenantRuntimeState::Loading { .. }),
                    Some(TenantRuntimeState::Available { .. })
                        | Some(TenantRuntimeState::LoadingFailed { .. })
                )
            )
        }
        let loading = TenantRuntimeState::loading_now();
        let available = TenantRuntimeState::available_now(1);
        let failed = TenantRuntimeState::failed_now("r");

        assert!(should_record(Some(&loading), Some(&available)));
        assert!(should_record(Some(&loading), Some(&failed)));
        // Non-completion transitions do NOT fire:
        assert!(!should_record(None, Some(&loading)));
        assert!(!should_record(Some(&loading), Some(&loading))); // idempotent refresh
        assert!(!should_record(Some(&available), Some(&loading))); // rewire kickoff
        assert!(!should_record(Some(&failed), Some(&loading))); // retry kickoff
        assert!(!should_record(Some(&available), None)); // forget
    }

    // ========================================================================
    // Emission-value assertions (B2 5.6 followup)
    //
    // The three tests above verify SELECTION logic (which metric to emit
    // for which transition). The tests below install a `DebuggingRecorder`
    // and verify that the metric values + labels actually written match
    // the documented contract. Together they catch both logic bugs and
    // recorder-integration bugs (wrong metric name, wrong label key,
    // mis-ordered label args, accidental double-emit, ...).
    // ========================================================================

    /// Absent → Loading: the kickoff transition.
    /// Expected emissions:
    /// - counter `tenant_state_transitions_total{from=absent,to=loading}` = 1
    /// - gauge   `tenant_state{slug,state=loading}` = 1
    /// - gauge   `tenant_state{slug,state=available}` = 0
    /// - gauge   `tenant_state{slug,state=loadingFailed}` = 0
    /// - NO histogram sample (wiring has not completed yet)
    #[test]
    fn emits_kickoff_transition_absent_to_loading() {
        let snap = capture(|| {
            record_transition("acme", None, Some(&TenantRuntimeState::loading_now()));
        });
        let m = index(&snap);

        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "absent"), ("to", "loading")])
            )),
            Some(&DebugValue::Counter(1)),
            "kickoff counter absent→loading must be 1"
        );

        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "loading")])
            )),
            Some(&DebugValue::Gauge(1.0.into())),
            "loading gauge must be 1 after kickoff"
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "available")])
            )),
            Some(&DebugValue::Gauge(0.0.into())),
            "available gauge must be 0 after kickoff"
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "loadingFailed")])
            )),
            Some(&DebugValue::Gauge(0.0.into())),
            "loadingFailed gauge must be 0 after kickoff"
        );

        // No histogram: wiring incomplete.
        assert!(
            !m.keys().any(|(n, _)| n == "tenant_wiring_duration_seconds"),
            "histogram must NOT fire on absent→loading"
        );
    }

    /// Loading → Available: success path.
    /// Expected emissions:
    /// - counter `tenant_state_transitions_total{from=loading,to=available}` = 1
    /// - gauge   `tenant_state{slug,state=available}` = 1 (others 0)
    /// - histogram `tenant_wiring_duration_seconds{result=success}` = 1 sample
    ///   whose value is in the ballpark of the elapsed time since `since`
    #[test]
    fn emits_successful_wiring_completion() {
        let since = SystemTime::now() - Duration::from_millis(120);
        let snap = capture(|| {
            record_transition(
                "acme",
                Some(&TenantRuntimeState::Loading { since }),
                Some(&TenantRuntimeState::available_now(3)),
            );
        });
        let m = index(&snap);

        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "loading"), ("to", "available")])
            )),
            Some(&DebugValue::Counter(1))
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "available")])
            )),
            Some(&DebugValue::Gauge(1.0.into()))
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "loading")])
            )),
            Some(&DebugValue::Gauge(0.0.into()))
        );

        // Histogram fires exactly once on the success bucket.
        let hist = m
            .get(&(
                "tenant_wiring_duration_seconds".into(),
                labels([("result", "success")]),
            ))
            .expect("wiring duration success histogram must be recorded");
        match hist {
            DebugValue::Histogram(samples) => {
                assert_eq!(samples.len(), 1, "exactly one sample expected");
                let v = samples[0].into_inner();
                // Lower bound = the 120ms we slept; upper bound is
                // intentionally generous (test-clock jitter, CI noise).
                assert!(
                    (0.100..=5.0).contains(&v),
                    "duration sample {v}s should be ~0.12s"
                );
            }
            other => panic!("expected histogram, got {other:?}"),
        }

        // No failure histogram recorded.
        assert!(
            !m.contains_key(&(
                "tenant_wiring_duration_seconds".into(),
                labels([("result", "failure")]),
            )),
            "failure histogram must not fire on success path"
        );
    }

    /// Loading → LoadingFailed: resolver error path.
    /// Expected: failure histogram sample + loadingFailed gauge = 1.
    #[test]
    fn emits_failed_wiring_completion() {
        let since = SystemTime::now() - Duration::from_millis(40);
        let snap = capture(|| {
            record_transition(
                "acme",
                Some(&TenantRuntimeState::Loading { since }),
                Some(&TenantRuntimeState::failed_now("resolver timeout")),
            );
        });
        let m = index(&snap);

        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "loading"), ("to", "loadingFailed")])
            )),
            Some(&DebugValue::Counter(1))
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "loadingFailed")])
            )),
            Some(&DebugValue::Gauge(1.0.into()))
        );
        assert!(
            matches!(
                m.get(&(
                    "tenant_wiring_duration_seconds".into(),
                    labels([("result", "failure")])
                )),
                Some(DebugValue::Histogram(samples)) if samples.len() == 1
            ),
            "exactly one failure-bucket histogram sample expected"
        );
    }

    /// forget(slug): transition to `None`.
    /// Expected: all three `tenant_state` series for this slug reset to 0;
    /// no histogram; transition counter incremented with `to=absent`.
    /// This test guards against the cardinality leak that would happen if
    /// we stopped resetting the gauges — stale series would persist in the
    /// TSDB until retention evicts them.
    #[test]
    fn emits_forget_resets_all_gauges_to_zero() {
        let snap = capture(|| {
            record_transition("acme", Some(&TenantRuntimeState::available_now(1)), None);
        });
        let m = index(&snap);

        for state in ["loading", "available", "loadingFailed"] {
            assert_eq!(
                m.get(&(
                    "tenant_state".into(),
                    labels([("slug", "acme"), ("state", state)])
                )),
                Some(&DebugValue::Gauge(0.0.into())),
                "gauge state={state} must reset to 0 on forget()"
            );
        }
        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "available"), ("to", "absent")])
            )),
            Some(&DebugValue::Counter(1))
        );
        assert!(
            !m.keys().any(|(n, _)| n == "tenant_wiring_duration_seconds"),
            "histogram must NOT fire on forget()"
        );
    }

    /// Loading → Loading idempotent refresh: counter ticks, gauges stay,
    /// histogram does NOT fire (match-arm returns `None` for `result`).
    #[test]
    fn idempotent_refresh_does_not_record_histogram() {
        let snap = capture(|| {
            record_transition(
                "acme",
                Some(&TenantRuntimeState::loading_now()),
                Some(&TenantRuntimeState::loading_now()),
            );
        });
        let m = index(&snap);

        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "loading"), ("to", "loading")])
            )),
            Some(&DebugValue::Counter(1))
        );
        assert!(
            !m.keys().any(|(n, _)| n == "tenant_wiring_duration_seconds"),
            "histogram must NOT fire on idempotent Loading→Loading refresh"
        );
    }

    /// Fleet scenario: two slugs, multiple transitions — verifies the
    /// counter accumulates across slugs (labels identical on counter),
    /// while gauges are per-slug independent.
    #[test]
    fn accumulates_across_multiple_slugs_and_transitions() {
        let snap = capture(|| {
            // Slug A: full happy-path.
            record_transition("acme", None, Some(&TenantRuntimeState::loading_now()));
            record_transition(
                "acme",
                Some(&TenantRuntimeState::Loading {
                    since: SystemTime::now() - Duration::from_millis(10),
                }),
                Some(&TenantRuntimeState::available_now(1)),
            );
            // Slug B: kickoff then fail.
            record_transition("beta", None, Some(&TenantRuntimeState::loading_now()));
            record_transition(
                "beta",
                Some(&TenantRuntimeState::Loading {
                    since: SystemTime::now() - Duration::from_millis(15),
                }),
                Some(&TenantRuntimeState::failed_now("timeout")),
            );
        });
        let m = index(&snap);

        // Two absent→loading kickoffs.
        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "absent"), ("to", "loading")])
            )),
            Some(&DebugValue::Counter(2)),
            "kickoff counter accumulates across slugs"
        );
        // One each of the two completion transitions.
        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "loading"), ("to", "available")])
            )),
            Some(&DebugValue::Counter(1))
        );
        assert_eq!(
            m.get(&(
                "tenant_state_transitions_total".into(),
                labels([("from", "loading"), ("to", "loadingFailed")])
            )),
            Some(&DebugValue::Counter(1))
        );

        // Independent per-slug gauges.
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "acme"), ("state", "available")])
            )),
            Some(&DebugValue::Gauge(1.0.into()))
        );
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "beta"), ("state", "loadingFailed")])
            )),
            Some(&DebugValue::Gauge(1.0.into()))
        );
        // Cross-check: beta should NOT be marked available.
        assert_eq!(
            m.get(&(
                "tenant_state".into(),
                labels([("slug", "beta"), ("state", "available")])
            )),
            Some(&DebugValue::Gauge(0.0.into()))
        );

        // Histograms: one sample in each result bucket.
        for result in ["success", "failure"] {
            match m.get(&(
                "tenant_wiring_duration_seconds".into(),
                labels([("result", result)]),
            )) {
                Some(DebugValue::Histogram(samples)) => {
                    assert_eq!(samples.len(), 1, "one sample in {result} bucket");
                }
                other => panic!("expected histogram for {result}, got {other:?}"),
            }
        }
    }
}
