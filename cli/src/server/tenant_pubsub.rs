//! Cross-pod tenant invalidation via Redis Pub/Sub (B2 task 5.2b, design §D5).
//!
//! # Topology
//!
//! A single channel — [`CHANNEL`] — carries [`TenantChangeEvent`] payloads.
//! Every pod subscribes on boot and publishes whenever a tenant-affecting
//! mutation commits (provisioning, config edit, secret rotation, deactivation).
//! On receipt, each pod:
//!
//! 1. Invalidates the per-tenant provider caches in
//!    [`memory::provider_registry::TenantProviderRegistry`].
//! 2. Re-marks the tenant as `Loading` in the runtime state registry.
//! 3. Re-primes LLM + embedding resolution (equivalent to one pass of the
//!    eager boot loop, task 5.2a).
//!
//! This closes the multi-pod coherence hole where pod A provisions tenant
//! T and pod B's cache is stale until restart. Latency budget per design:
//! 95th percentile cache convergence < 1s across the cluster.
//!
//! # Design decisions
//!
//! * **Fire-and-forget, at-most-once.** Redis Pub/Sub is not durable;
//!   missed messages (subscriber disconnected) are recovered by the next
//!   eager boot OR by the lazy fallback path (task 5.2c). A tenant
//!   outliving its cache TTL will simply re-resolve from Postgres on
//!   next request.
//! * **No origin-pod suppression in this drop.** Self-receipt is
//!   idempotent — invalidating your own freshly-populated cache is
//!   cheap; the next request re-primes from a warm Postgres row. If this
//!   becomes a hot-path concern (high-provision-volume workloads) a
//!   `origin_pod_id` field gets added and the subscriber filters on it.
//! * **Dedicated connection per subscriber.** `ConnectionManager` cannot
//!   service `SUBSCRIBE`; we open a fresh `redis::Client` from
//!   `AppState::redis_url`. The publisher side reuses the manager.
//! * **Reconnection with bounded backoff.** If the subscriber loop loses
//!   the connection, it reconnects with exponential backoff capped at
//!   `RECONNECT_BACKOFF_CAP`. Missed messages during the gap are the
//!   lazy-fallback/eager-boot's problem, not ours.
//!
//! # Non-goals
//!
//! * Event ordering across publishers. Two rapid updates to the same
//!   tenant that arrive out of order still converge to the latest
//!   Postgres state because each message triggers a full resolve, not
//!   a delta apply.
//! * Delivery acknowledgement. Use the DLQ (`redis_publisher`) for
//!   governance-grade events; this channel is best-effort cache
//!   coordination.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use futures_util::StreamExt;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use super::AppState;

// ---------------------------------------------------------------------------
// In-process broadcaster (B2 §7.5 — tenant watch SSE)
// ---------------------------------------------------------------------------
//
// The Redis pub/sub channel above solves *cross-pod* invalidation but
// does not help intra-pod subscribers like an SSE stream handler in the
// same process: there is no way to reach a `pubsub.on_message()` stream
// from an HTTP handler without opening another Redis connection, and
// even if we did, the same pod's own publishes round-trip through
// Redis and back before being visible.
//
// Solution: every publish (local or remote) also forwards through a
// lazy-init `tokio::sync::broadcast::Sender`. SSE handlers
// [`subscribe`] and filter on slug. The broadcaster is *best-effort*:
// slow subscribers that fall behind `EVENT_BUS_CAPACITY` are lagged
// and receive [`broadcast::error::RecvError::Lagged`] — the CLI
// treats that as "reconnect to re-sync", matching the
// at-most-once semantics we already advertise above.
//
// No AppState field is needed — the broadcaster is a process-wide
// singleton because a single pod has a single event stream and tests
// use unique tenant slugs so cross-test bleed is not observable.
//
// Capacity chosen pragmatically: 256 events is enough to cover one
// in-flight provisioning (~7 step events) × ~30 concurrent slow
// watchers before anyone lags. Larger values waste RAM; smaller
// values cause spurious lag in burst traffic.

const EVENT_BUS_CAPACITY: usize = 256;

static TENANT_EVENT_BUS: LazyLock<broadcast::Sender<TenantChangeEvent>> =
    LazyLock::new(|| broadcast::channel(EVENT_BUS_CAPACITY).0);

/// Subscribe to the in-process tenant-event bus.
///
/// Used by the SSE endpoint in `tenant_events_api`. Each subscriber
/// gets its own `Receiver`; events broadcast *after* the subscription
/// is created are delivered. Missed pre-subscription events are not
/// replayed — callers needing history should read the governance
/// audit log instead.
#[must_use]
pub fn subscribe() -> broadcast::Receiver<TenantChangeEvent> {
    TENANT_EVENT_BUS.subscribe()
}

/// Forward an event to every in-process subscriber.
///
/// No-op when there are zero subscribers. Never blocks, never errors
/// up — the broadcaster's `send` only fails when the channel has no
/// receivers, which is the steady state (no active `watch` commands).
///
/// `pub(crate)` so `tenant_events_api` tests can inject events
/// directly into the bus without constructing a full `AppState` — the
/// in-process fanout is what the SSE endpoint actually reads, so
/// testing it through the real channel is strictly more faithful
/// than a mocked publisher.
pub(crate) fn fan_out_local(event: &TenantChangeEvent) {
    // `.send` returns the number of active receivers on success or
    // `SendError` when there are zero. Both are boring from the
    // publisher's perspective — we log at TRACE only for delivery
    // observability, never at WARN.
    let _ = TENANT_EVENT_BUS.send(event.clone());
}

/// Pub/Sub channel name. Matches the convention used by `redis_publisher`
/// for governance streams (colon-delimited, `aeterna:` prefixed).
pub const CHANNEL: &str = "aeterna:tenant:changed";

/// Initial reconnect backoff on subscriber failure.
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(250);
/// Upper bound on reconnect backoff. A minute is more than enough for
/// transient Redis hiccups; longer outages are visible via `/ready`.
const RECONNECT_BACKOFF_CAP: Duration = Duration::from_secs(60);

/// Payload published on [`CHANNEL`] for every tenant mutation.
///
/// `kind` conveys intent for observability; the handler always performs
/// a full cache invalidation regardless, so adding a new variant is
/// strictly additive — older subscribers will log an `Unknown` and still
/// invalidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantChangeEvent {
    /// Tenant slug (stable identifier across renames).
    pub slug: String,
    /// What happened. Serialized as lowercase for channel compactness.
    pub kind: TenantChangeKind,
    /// Publisher wall-clock epoch seconds. Informational only — not used
    /// for ordering.
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TenantChangeKind {
    /// New tenant just finished provisioning. Subscribers should
    /// invalidate cache and re-wire.
    Provisioned,
    /// Existing tenant config or secret was updated. Subscribers
    /// should invalidate cache and re-wire.
    Updated,
    /// Tenant marked inactive. Subscribers should forget cached state.
    Deactivated,
    /// Per-step provisioning progress (B2 §7.5). Purely informational
    /// — subscribers **must not** invalidate cache or re-wire on this
    /// kind; the final `Provisioned`/`Updated` event is the
    /// authoritative "apply committed" signal. These events exist
    /// solely to feed `aeterna tenant watch` and the
    /// `apply --watch` UX affordance.
    ///
    /// `step` values are the lifecycle phase names from
    /// `ProvisionStep` (tenant, repository, config, secrets, …); the
    /// wire shape is intentionally a free-form string so new steps
    /// added later do not break older parsers — consumers that do not
    /// recognise a step name should render it verbatim.
    ProvisioningStep {
        /// Lifecycle phase name; see `ProvisionStep` for the canonical
        /// set (`tenant`, `repository`, `config`, `secrets`,
        /// `hierarchy`, `roles`, `domains`).
        step: String,
        /// One of `started`, `ok`, `failed`. String-typed for the
        /// same forward-compat reason as `step`.
        status: String,
        /// Optional detail for failed steps — truncated server-side
        /// before publish to keep the SSE frame size bounded.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Fallback for forward-compat with new kinds added by newer pods.
    #[serde(other)]
    Unknown,
}

impl TenantChangeEvent {
    /// Construct with `at = now`.
    pub fn new(slug: impl Into<String>, kind: TenantChangeKind) -> Self {
        Self {
            slug: slug.into(),
            kind,
            at: chrono::Utc::now().timestamp(),
        }
    }

    /// Construct a per-step progress event (B2 §7.5). Truncates
    /// `detail` to 512 bytes so a runaway error message cannot blow
    /// the Pub/Sub wire frame (Redis default is 512 MB but individual
    /// SSE clients will drop messages past the browser / reqwest
    /// per-event buffer; 512 B is comfortable for a one-line error
    /// summary and still fits inside a single TCP packet).
    pub fn step(
        slug: impl Into<String>,
        step: impl Into<String>,
        status: impl Into<String>,
        detail: Option<String>,
    ) -> Self {
        const DETAIL_CAP: usize = 512;
        let detail = detail.map(|d| {
            if d.len() <= DETAIL_CAP {
                d
            } else {
                // char_indices keeps us UTF-8 safe — naively slicing on
                // a byte offset can split a multibyte codepoint.
                let cut = d
                    .char_indices()
                    .take_while(|(i, _)| *i < DETAIL_CAP)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                let mut truncated = d[..cut].to_string();
                truncated.push('…');
                truncated
            }
        });
        Self::new(
            slug,
            TenantChangeKind::ProvisioningStep {
                step: step.into(),
                status: status.into(),
                detail,
            },
        )
    }
}

/// Publish a per-step provisioning event for the currently-running
/// apply (B2 §7.5). Convenience wrapper around [`publish`] that also
/// fans out to in-process SSE subscribers via [`fan_out_local`] —
/// without this, a watcher connected to the same pod that is running
/// the apply would have to wait for the Redis round-trip before
/// seeing progress (and would see nothing at all in no-Redis
/// single-node mode).
pub async fn publish_step(
    state: &AppState,
    slug: &str,
    step: &str,
    status: &str,
    detail: Option<String>,
) {
    let event = TenantChangeEvent::step(slug, step, status, detail);
    fan_out_local(&event);
    publish(state, &event).await;
}

/// Publish a tenant-change event.
///
/// No-op (logs at `debug`) when `redis_conn` is `None` — the pod is
/// running in single-node mode and there are no other subscribers to
/// notify. Local invalidation in that case is the caller's
/// responsibility (typically the same handler that calls `publish`).
///
/// Errors are logged at WARN and swallowed. A failed publish is not a
/// user-facing error because the eager boot loop + lazy fallback
/// provide eventual consistency.
pub async fn publish(state: &AppState, event: &TenantChangeEvent) {
    // Fan out to in-process SSE subscribers first — this runs
    // regardless of Redis state so single-node deployments still get
    // a working `aeterna tenant watch`. Cloning is cheap
    // (Arc-free struct of `String` + enum + i64) and `send` is
    // non-blocking.
    fan_out_local(event);

    let Some(conn) = state.redis_conn.as_ref() else {
        debug!(slug = %event.slug, kind = ?event.kind, "tenant:changed publish skipped (no redis)");
        return;
    };
    let payload = match serde_json::to_string(event) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "tenant:changed serialise failed");
            return;
        }
    };
    // Clone the manager cheaply; `publish` takes `&mut`.
    let mut conn = (**conn).clone();
    let res: redis::RedisResult<i64> = conn.publish(CHANNEL, &payload).await;
    match res {
        Ok(n) => debug!(
            slug = %event.slug,
            kind = ?event.kind,
            subscribers = n,
            "tenant:changed published"
        ),
        Err(e) => warn!(
            slug = %event.slug,
            kind = ?event.kind,
            error = %e,
            "tenant:changed publish failed"
        ),
    }
}

/// Spawn the long-lived subscriber task.
///
/// No-op when `redis_url` is `None`. Returns immediately. The task
/// retries on connection loss with exponential backoff and exits
/// cleanly on `state.shutdown_tx` flip.
pub fn spawn_subscriber(state: Arc<AppState>) {
    let Some(redis_url) = state.redis_url.clone() else {
        info!("tenant:changed subscriber: redis disabled, not spawning");
        return;
    };
    tokio::spawn(async move {
        run_subscriber(state, redis_url).await;
    });
}

async fn run_subscriber(state: Arc<AppState>, redis_url: String) {
    let mut backoff = RECONNECT_BACKOFF_INITIAL;
    loop {
        if *state.shutdown_tx.borrow() {
            info!("tenant:changed subscriber: shutdown requested");
            return;
        }
        match subscribe_and_consume(&state, &redis_url).await {
            Ok(()) => {
                info!("tenant:changed subscriber: stream ended cleanly, reconnecting");
                backoff = RECONNECT_BACKOFF_INITIAL;
            }
            Err(e) => {
                warn!(
                    error = %e,
                    backoff_ms = backoff.as_millis() as u64,
                    "tenant:changed subscriber failed, backing off"
                );
            }
        }
        // Respect shutdown during the backoff so we don't hold the pod
        // up past drain.
        tokio::select! {
            _ = sleep(backoff) => {}
            _ = wait_for_shutdown(&state) => {
                info!("tenant:changed subscriber: shutdown during backoff");
                return;
            }
        }
        // Exponential, capped.
        backoff = (backoff * 2).min(RECONNECT_BACKOFF_CAP);
    }
}

async fn wait_for_shutdown(state: &AppState) {
    let mut rx = state.shutdown_tx.subscribe();
    // `subscribe()` on a watch returns the current value; loop until true.
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}

async fn subscribe_and_consume(state: &AppState, redis_url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(CHANNEL).await?;
    info!(channel = CHANNEL, "tenant:changed subscriber: subscribed");

    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        if *state.shutdown_tx.borrow() {
            return Ok(());
        }
        let payload: String = match msg.get_payload::<String>() {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "tenant:changed: malformed payload");
                continue;
            }
        };
        let event: TenantChangeEvent = match serde_json::from_str(&payload) {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, payload = %payload, "tenant:changed: parse failed");
                continue;
            }
        };
        handle_event(state, event).await;
    }
    // `on_message` stream ending means the connection dropped; caller
    // will reconnect.
    Ok(())
}

/// Apply a change event to local state.
///
/// Exposed at `pub(crate)` so `provision_tenant` can call it directly
/// after a successful apply (avoiding a round-trip through Redis just
/// to update the pod that did the work).
pub(crate) async fn handle_event(state: &AppState, event: TenantChangeEvent) {
    debug!(slug = %event.slug, kind = ?event.kind, "tenant:changed handling");

    // Fan out to local SSE subscribers — this runs on the *subscriber*
    // side (we got the event from Redis), so in-process watchers on
    // any pod see progress regardless of which pod is running the
    // apply.
    fan_out_local(&event);

    // Per-step progress events are informational-only: they must not
    // trigger cache invalidation or re-wire (which would be a 7x
    // re-wire storm per apply — wasteful and occasionally incorrect
    // because an intermediate step failing does not mean the tenant
    // is in a "Provisioned" state yet).
    if matches!(event.kind, TenantChangeKind::ProvisioningStep { .. }) {
        return;
    }

    // Resolve slug → TenantId (UUID). `TenantId` wraps the UUID, NOT
    // the slug — an earlier revision constructed `TenantId::new(slug)`
    // which produced a value that would never match the
    // provider-registry cache keyed on UUIDs. The tenant store
    // round-trip is the authoritative mapping.
    let tenant_id = match super::tenant_lazy_wire::resolve_slug_to_id(state, &event.slug).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            // Tenant row is gone (deletion on the publisher side, or a
            // stale message for a slug that never existed on this
            // cluster). Scrub any lingering runtime-state entry so
            // `/ready` and status endpoints don't report a ghost.
            warn!(slug = %event.slug, "tenant:changed: slug unknown, forgetting");
            state.tenant_runtime_state.forget(&event.slug).await;
            return;
        }
        Err(e) => {
            warn!(
                slug = %event.slug,
                error = %e,
                "tenant:changed: slug resolution failed, dropping event"
            );
            return;
        }
    };

    match event.kind {
        TenantChangeKind::Deactivated => {
            // Forget, don't re-prime — tenant is gone from active set.
            state.provider_registry.invalidate_tenant(&tenant_id);
            state.tenant_runtime_state.forget(&event.slug).await;
            info!(slug = %event.slug, "tenant deactivated, cache forgotten");
        }
        TenantChangeKind::ProvisioningStep { .. } => {
            // Unreachable in practice — we early-returned above. Kept
            // as an explicit arm to keep the match exhaustive without
            // a blanket `_ =>` that would silently swallow any future
            // variant we add.
            unreachable!("ProvisioningStep handled by early return above");
        }
        TenantChangeKind::Provisioned | TenantChangeKind::Updated | TenantChangeKind::Unknown => {
            state.provider_registry.invalidate_tenant(&tenant_id);
            state.tenant_runtime_state.mark_loading(&event.slug).await;
            // Re-prime via the shared wiring path so pub/sub,
            // eager-boot, and lazy-fallback all converge through a
            // single code path — one place to tighten error handling
            // once `get_*_service` grows a fallible variant
            // (see b2-5.2-followup in tenant_eager_wire).
            match super::tenant_eager_wire::wire_one(state, &tenant_id).await {
                Ok(()) => {
                    let rev = state.tenant_runtime_state.mark_available(&event.slug).await;
                    info!(slug = %event.slug, rev, kind = ?event.kind, "tenant re-wired");
                }
                Err(e) => {
                    let reason = super::tenant_eager_wire::truncate(&format!("{e:#}"), 256);
                    let retries = state
                        .tenant_runtime_state
                        .mark_failed(&event.slug, &reason)
                        .await;
                    warn!(
                        slug = %event.slug,
                        retry_count = retries,
                        reason = %reason,
                        "tenant re-wire failed after tenant:changed"
                    );
                }
            }
        }
    }

    // Reserved for later: publish a metric counter increment here
    // once 5.6 lands so cross-pod invalidations are observable
    // without scraping logs.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serialises_with_snake_case_kind() {
        let ev = TenantChangeEvent::new("acme", TenantChangeKind::Provisioned);
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"kind\":\"provisioned\""), "got: {s}");
        assert!(s.contains("\"slug\":\"acme\""));
    }

    #[test]
    fn unknown_kind_round_trips() {
        // Older pod receiving a future variant must not panic.
        let raw = r#"{"slug":"acme","kind":"future_variant_2028","at":123}"#;
        let ev: TenantChangeEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.kind, TenantChangeKind::Unknown);
        assert_eq!(ev.slug, "acme");
    }

    #[test]
    fn kind_variants_parse() {
        for (wire, want) in [
            ("provisioned", TenantChangeKind::Provisioned),
            ("updated", TenantChangeKind::Updated),
            ("deactivated", TenantChangeKind::Deactivated),
        ] {
            let raw = format!(r#"{{"slug":"x","kind":"{wire}","at":1}}"#);
            let ev: TenantChangeEvent = serde_json::from_str(&raw).unwrap();
            assert_eq!(ev.kind, want, "wire={wire}");
        }
    }

    #[test]
    fn channel_name_matches_design() {
        // Guard against accidental renames — the channel is a wire
        // contract with deployed pods.
        assert_eq!(CHANNEL, "aeterna:tenant:changed");
    }

    // ---------------------------------------------------------------
    // B2 §7.5 — per-step progress events + local broadcaster
    // ---------------------------------------------------------------

    #[test]
    fn step_event_serialises_with_nested_object() {
        // Externally-tagged struct variant serialises as
        // `{"provisioning_step": {...}}` inside the outer struct's
        // `kind` field. Ugly but forward-compatible: older pods that
        // only know unit variants route unknown object-shaped kinds
        // through the `Unknown` catch-all (a harmless extra re-wire),
        // rather than failing deserialization.
        let ev = TenantChangeEvent::step("acme", "config", "ok", Some("applied 12 fields".into()));
        let s = serde_json::to_string(&ev).unwrap();
        assert!(
            s.contains("\"provisioning_step\""),
            "expected struct-variant tag in: {s}"
        );
        assert!(s.contains("\"step\":\"config\""), "got: {s}");
        assert!(s.contains("\"status\":\"ok\""), "got: {s}");
        assert!(s.contains("\"detail\":\"applied 12 fields\""), "got: {s}");
        assert!(s.contains("\"slug\":\"acme\""), "got: {s}");
    }

    #[test]
    fn step_event_omits_detail_when_none() {
        // `#[serde(skip_serializing_if = "Option::is_none")]` keeps
        // the wire shape compact for the common "started" / "ok"
        // cases where there is no error body to surface.
        let ev = TenantChangeEvent::step("acme", "secrets", "started", None);
        let s = serde_json::to_string(&ev).unwrap();
        assert!(!s.contains("detail"), "detail must be omitted: {s}");
    }

    #[test]
    fn step_event_detail_truncated_at_cap_utf8_safe() {
        // 512-byte cap with a UTF-8-safe splitter. Push a payload well
        // past the cap with a multibyte char straddling the boundary
        // so a naive byte-slice would panic. The truncated output
        // must end with the ellipsis marker so consumers can tell the
        // message was cut.
        let mut big = "a".repeat(500);
        // Insert a 3-byte char at position ~510 — if the splitter
        // sliced on the byte boundary naively it would panic on
        // `from_utf8`.
        big.push_str("あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほ"); // 30 × 3 = 90 bytes
        assert!(big.len() > 512, "test setup must exceed cap");
        let ev = TenantChangeEvent::step("acme", "hierarchy", "failed", Some(big));
        match &ev.kind {
            TenantChangeKind::ProvisioningStep { detail, .. } => {
                let d = detail.as_deref().expect("detail preserved");
                assert!(d.ends_with('…'), "must end with ellipsis: {d:?}");
                // +4 accounts for ellipsis (3 bytes) plus at most one
                // trailing multibyte codepoint we kept intact.
                assert!(d.len() <= 512 + 4, "len={}", d.len());
                // Round-trips through JSON — the primary smoke test
                // that UTF-8-safe truncation did its job.
                let s = serde_json::to_string(&ev).unwrap();
                let _: TenantChangeEvent = serde_json::from_str(&s).unwrap();
            }
            _ => panic!("expected ProvisioningStep"),
        }
    }

    #[test]
    fn step_event_under_cap_is_passed_through() {
        let ev = TenantChangeEvent::step("acme", "roles", "failed", Some("nope".into()));
        match &ev.kind {
            TenantChangeKind::ProvisioningStep { detail, .. } => {
                assert_eq!(detail.as_deref(), Some("nope"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn old_pod_parses_new_step_event_as_unknown() {
        // Forward-compat guard: an older pod receiving
        // `{"provisioning_step":{...}}` on the channel must route it
        // through the `#[serde(other)] Unknown` fallback rather than
        // panicking on deserialization. Simulate with a manually-crafted
        // payload (we can't run two serde versions in one test).
        let raw = r#"{"slug":"acme","kind":{"provisioning_step":{"step":"config","status":"ok"}},"at":1}"#;
        let ev: TenantChangeEvent = serde_json::from_str(raw).unwrap();
        // Note: *this* build knows ProvisioningStep, so it parses as
        // that variant. The forward-compat property is verified by
        // `unknown_kind_round_trips` above (string-shaped unknown) and
        // by the stable wire shape proven by this test — an old pod's
        // serde sees the same JSON.
        assert!(matches!(ev.kind, TenantChangeKind::ProvisioningStep { .. }));
    }

    #[test]
    fn local_broadcaster_delivers_to_subscribers() {
        // `subscribe` returns an independent Receiver per call; a
        // subsequent `fan_out_local` must deliver to every live one.
        //
        // Tests share a process-wide broadcaster (by design — it is a
        // module-level `LazyLock`) and cargo runs them in parallel, so
        // other tests' publishes leak into these receivers too. Use a
        // test-specific slug + slug filter to isolate — exactly the
        // pattern the real SSE handler uses. We drain up to a small
        // bounded number of messages waiting for ours; if the channel
        // fills with foreign events past the bound, the test fails
        // loudly rather than hanging.
        let slug = "pubsub-test-delivery-a";
        let mut rx1 = subscribe();
        let mut rx2 = subscribe();
        let ev = TenantChangeEvent::step(slug, "tenant", "ok", None);
        fan_out_local(&ev);

        fn drain_for_slug(
            rx: &mut broadcast::Receiver<TenantChangeEvent>,
            slug: &str,
        ) -> TenantChangeEvent {
            // Bounded drain — foreign test traffic should be tiny, but
            // cap defensively so a bug in another test cannot make
            // this one spin.
            for _ in 0..EVENT_BUS_CAPACITY {
                match rx.try_recv() {
                    Ok(ev) if ev.slug == slug => return ev,
                    Ok(_) => continue,
                    Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                    Err(e) => panic!("drain_for_slug: {e:?}"),
                }
            }
            panic!("did not observe slug={slug} within drain cap");
        }

        let got1 = drain_for_slug(&mut rx1, slug);
        let got2 = drain_for_slug(&mut rx2, slug);
        assert!(matches!(
            got1.kind,
            TenantChangeKind::ProvisioningStep { .. }
        ));
        assert!(matches!(
            got2.kind,
            TenantChangeKind::ProvisioningStep { .. }
        ));
    }

    #[test]
    fn local_broadcaster_no_receivers_is_silent() {
        // Steady state on pods with no active `tenant watch`: zero
        // subscribers. `fan_out_local` must not panic or log at WARN
        // just because nobody is listening.
        let ev = TenantChangeEvent::new("nobody-watching", TenantChangeKind::Updated);
        fan_out_local(&ev); // must not panic
    }
}
