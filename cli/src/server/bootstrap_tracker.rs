//! Bootstrap phase tracker (B2 task 6.1).
//!
//! Records per-phase timing and outcomes during `bootstrap()` so ops can
//! answer "why did my pod take 40s to start" and "did the admin seed
//! no-op or actually run" via a single admin endpoint instead of hunting
//! through log tails.
//!
//! # Architecture note (task 6.2)
//!
//! Today `bootstrap()` runs **synchronously** in `serve::run` before the
//! HTTP listener binds. That means no request can observe a mid-bootstrap
//! state — the pod either completes boot and begins serving, or exits
//! (kubelet restart). `/ready` is therefore **structurally** gated on
//! bootstrap completion without any explicit check: a pod that failed
//! bootstrap has no listener at all, which the kubelet reads as 503.
//!
//! This module still provides [`BootstrapTracker::is_completed`] so the
//! `/ready` handler can opt in to explicit gating if bootstrap is ever
//! refactored to run async-post-bind. Until then the method returns
//! `true` by the time any request is possible.
//!
//! # Wire contract (`/api/v1/admin/bootstrap/status`)
//!
//! PlatformAdmin-gated. Returns the in-memory snapshot — per-pod, not
//! cluster-wide. Operators investigating a boot regression across a
//! rolling deploy hit each pod directly.
//!
//! # Redaction
//!
//! Step error strings can carry upstream detail (DB error messages,
//! K8s-API failures, etc.). The endpoint is PA-gated precisely so the
//! raw text is only reachable behind auth. There is no unauthenticated
//! surface for this data.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Snapshot of the bootstrap progress for the wire / tests.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatus {
    pub state: &'static str,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub steps: Vec<StepStatus>,
}

/// Single-phase record. One per call to [`BootstrapTracker::begin`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepStatus {
    pub name: &'static str,
    /// `running` until terminated; then `success` or `failure`.
    pub state: &'static str,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Non-None only when `state == "failure"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug)]
struct Inner {
    started_at_wall: DateTime<Utc>,
    started_at_mono: Instant,
    completed_at_wall: Option<DateTime<Utc>>,
    completed_at_mono: Option<Instant>,
    /// True once [`BootstrapTracker::mark_ready`] was called.
    completed: bool,
    /// True if any step was marked failure.
    failed: bool,
    steps: Vec<StepRecord>,
}

#[derive(Debug)]
struct StepRecord {
    name: &'static str,
    started_at_wall: DateTime<Utc>,
    started_at_mono: Instant,
    /// None while running. `Some(Ok(()))` on success, `Some(Err(msg))`
    /// on failure.
    outcome: Option<Result<(), String>>,
    completed_at_wall: Option<DateTime<Utc>>,
    completed_at_mono: Option<Instant>,
}

/// Thread-safe, append-only bootstrap progress log.
///
/// Shared via `Arc` from `bootstrap()` into `AppState` so the admin
/// endpoint can observe it after the fact.
#[derive(Debug)]
pub struct BootstrapTracker {
    inner: Mutex<Inner>,
}

impl Default for BootstrapTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BootstrapTracker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                started_at_wall: Utc::now(),
                started_at_mono: Instant::now(),
                completed_at_wall: None,
                completed_at_mono: None,
                completed: false,
                failed: false,
                steps: Vec::new(),
            }),
        }
    }

    /// Record the start of a phase. Name must be `'static` so the wire
    /// contract stays stable.
    pub fn begin(&self, name: &'static str) {
        let mut g = self.inner.lock().expect("bootstrap tracker poisoned");
        g.steps.push(StepRecord {
            name,
            started_at_wall: Utc::now(),
            started_at_mono: Instant::now(),
            outcome: None,
            completed_at_wall: None,
            completed_at_mono: None,
        });
    }

    /// Mark the most recently started step with matching name as
    /// succeeded. Silently no-ops on unknown or already-terminated
    /// names — the tracker must never panic bootstrap.
    pub fn complete(&self, name: &'static str) {
        self.terminate(name, Ok(()));
    }

    /// Mark the most recently started step with matching name as
    /// failed. The tracker is flagged as failed overall.
    pub fn fail(&self, name: &'static str, error: impl Into<String>) {
        self.terminate(name, Err(error.into()));
    }

    fn terminate(&self, name: &'static str, outcome: Result<(), String>) {
        let mut g = self.inner.lock().expect("bootstrap tracker poisoned");
        if outcome.is_err() {
            g.failed = true;
        }
        // Walk backwards so the most recent running step is matched.
        if let Some(rec) = g
            .steps
            .iter_mut()
            .rev()
            .find(|r| r.name == name && r.outcome.is_none())
        {
            rec.outcome = Some(outcome);
            rec.completed_at_wall = Some(Utc::now());
            rec.completed_at_mono = Some(Instant::now());
        }
    }

    /// Finalize the tracker as completed-successfully. Idempotent.
    pub fn mark_ready(&self) {
        let mut g = self.inner.lock().expect("bootstrap tracker poisoned");
        if g.completed {
            return;
        }
        g.completed = true;
        g.completed_at_wall = Some(Utc::now());
        g.completed_at_mono = Some(Instant::now());
    }

    /// True iff [`mark_ready`] has been called and no step failed.
    pub fn is_completed(&self) -> bool {
        let g = self.inner.lock().expect("bootstrap tracker poisoned");
        g.completed && !g.failed
    }

    /// Deep-copy snapshot for the wire / tests.
    pub fn snapshot(&self) -> BootstrapStatus {
        let g = self.inner.lock().expect("bootstrap tracker poisoned");
        let state = if g.failed {
            "failed"
        } else if g.completed {
            "completed"
        } else {
            "running"
        };
        let duration_ms = g
            .completed_at_mono
            .map(|end| duration_to_ms(end.duration_since(g.started_at_mono)));
        let steps = g
            .steps
            .iter()
            .map(|r| {
                let state = match &r.outcome {
                    None => "running",
                    Some(Ok(())) => "success",
                    Some(Err(_)) => "failure",
                };
                let duration_ms = r
                    .completed_at_mono
                    .map(|end| duration_to_ms(end.duration_since(r.started_at_mono)));
                let error = match &r.outcome {
                    Some(Err(msg)) => Some(msg.clone()),
                    _ => None,
                };
                StepStatus {
                    name: r.name,
                    state,
                    started_at: r.started_at_wall,
                    completed_at: r.completed_at_wall,
                    duration_ms,
                    error,
                }
            })
            .collect();
        BootstrapStatus {
            state,
            started_at: g.started_at_wall,
            completed_at: g.completed_at_wall,
            duration_ms,
            steps,
        }
    }
}

fn duration_to_ms(d: Duration) -> u64 {
    // Saturating: a bootstrap measured in centuries is a bug we don't
    // want the tracker to panic on.
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_is_running_with_no_steps() {
        let t = BootstrapTracker::new();
        let s = t.snapshot();
        assert_eq!(s.state, "running");
        assert!(s.steps.is_empty());
        assert!(s.completed_at.is_none());
        assert!(s.duration_ms.is_none());
        assert!(!t.is_completed());
    }

    #[test]
    fn begin_complete_records_success() {
        let t = BootstrapTracker::new();
        t.begin("database");
        t.complete("database");
        let s = t.snapshot();
        assert_eq!(s.steps.len(), 1);
        assert_eq!(s.steps[0].name, "database");
        assert_eq!(s.steps[0].state, "success");
        assert!(s.steps[0].completed_at.is_some());
        assert!(s.steps[0].duration_ms.is_some());
        assert!(s.steps[0].error.is_none());
    }

    #[test]
    fn begin_fail_records_failure_and_flags_tracker() {
        let t = BootstrapTracker::new();
        t.begin("seed");
        t.fail("seed", "pg down");
        let s = t.snapshot();
        assert_eq!(s.steps[0].state, "failure");
        assert_eq!(s.steps[0].error.as_deref(), Some("pg down"));
        // Tracker overall is "failed" even before mark_ready() is called.
        assert_eq!(s.state, "failed");
        // And stays failed after mark_ready — the success marker cannot
        // paper over a failed step.
        t.mark_ready();
        assert_eq!(t.snapshot().state, "failed");
        assert!(!t.is_completed());
    }

    #[test]
    fn mark_ready_flips_state_to_completed_only_when_no_failure() {
        let t = BootstrapTracker::new();
        t.begin("a");
        t.complete("a");
        assert_eq!(t.snapshot().state, "running"); // not yet marked ready
        t.mark_ready();
        let s = t.snapshot();
        assert_eq!(s.state, "completed");
        assert!(s.completed_at.is_some());
        assert!(s.duration_ms.is_some());
        assert!(t.is_completed());
    }

    #[test]
    fn mark_ready_is_idempotent() {
        let t = BootstrapTracker::new();
        t.mark_ready();
        let first = t.snapshot().completed_at;
        t.mark_ready();
        let second = t.snapshot().completed_at;
        // Exact equality — second call must not overwrite the timestamp.
        assert_eq!(first, second);
    }

    #[test]
    fn complete_unknown_step_is_silent_noop() {
        let t = BootstrapTracker::new();
        // Must not panic, must not add a phantom record.
        t.complete("never-started");
        assert!(t.snapshot().steps.is_empty());
    }

    #[test]
    fn complete_of_already_terminated_step_is_noop() {
        let t = BootstrapTracker::new();
        t.begin("x");
        t.complete("x");
        // A second complete() should not mutate the timestamp.
        let ts = t.snapshot().steps[0].completed_at;
        std::thread::sleep(Duration::from_millis(2));
        t.complete("x");
        assert_eq!(t.snapshot().steps[0].completed_at, ts);
    }

    #[test]
    fn multiple_steps_retain_order_of_insertion() {
        let t = BootstrapTracker::new();
        for name in ["a", "b", "c", "d"] {
            t.begin(name);
            t.complete(name);
        }
        let s = t.snapshot();
        let names: Vec<&str> = s.steps.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn overlapping_steps_match_most_recent_unfinished() {
        // Not that bootstrap() ever nests, but document the rule.
        let t = BootstrapTracker::new();
        t.begin("pair");
        t.begin("pair");
        t.complete("pair");
        let s = t.snapshot();
        assert_eq!(s.steps.len(), 2);
        // Second (inner) started is the one completed.
        assert_eq!(s.steps[0].state, "running");
        assert_eq!(s.steps[1].state, "success");
    }

    #[test]
    fn snapshot_is_a_deep_copy_not_a_view() {
        let t = BootstrapTracker::new();
        t.begin("x");
        let snap_before = t.snapshot();
        t.complete("x");
        // The earlier snapshot must still show running.
        assert_eq!(snap_before.steps[0].state, "running");
        assert_eq!(t.snapshot().steps[0].state, "success");
    }

    #[test]
    fn wire_shape_uses_camel_case() {
        // Dashboards key on camelCase — guard against a rename.
        let t = BootstrapTracker::new();
        t.begin("database");
        t.complete("database");
        t.mark_ready();
        let wire = serde_json::to_value(t.snapshot()).unwrap();
        for key in ["state", "startedAt", "completedAt", "durationMs", "steps"] {
            assert!(
                wire.get(key).is_some(),
                "missing top-level key {key} in {wire}"
            );
        }
        let step = &wire["steps"][0];
        for key in ["name", "state", "startedAt", "completedAt", "durationMs"] {
            assert!(step.get(key).is_some(), "missing step key {key} in {step}");
        }
    }

    #[test]
    fn error_is_elided_on_success_and_present_on_failure() {
        let t = BootstrapTracker::new();
        t.begin("ok");
        t.complete("ok");
        t.begin("bad");
        t.fail("bad", "boom");
        let wire = serde_json::to_value(t.snapshot()).unwrap();
        assert!(wire["steps"][0].get("error").is_none());
        assert_eq!(wire["steps"][1]["error"], "boom");
    }

    #[test]
    fn default_and_new_are_equivalent() {
        let a = BootstrapTracker::new().snapshot();
        let b = BootstrapTracker::default().snapshot();
        assert_eq!(a.state, b.state);
        assert_eq!(a.steps.len(), b.steps.len());
    }

    #[test]
    fn is_completed_is_false_during_running_and_failure_states() {
        let t = BootstrapTracker::new();
        assert!(!t.is_completed());
        t.begin("x");
        assert!(!t.is_completed());
        t.fail("x", "nope");
        assert!(!t.is_completed());
        t.mark_ready(); // mark_ready after failure does NOT flip to completed
        assert!(!t.is_completed());
    }
}
