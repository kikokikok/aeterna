//! Tenant event stream — B2 §7.5.
//!
//! Server-Sent Events endpoint that streams tenant lifecycle events
//! (`provisioned`, `updated`, `deactivated`, `provisioning_step`) for a
//! single tenant slug. Powers `aeterna tenant watch <slug>` and the
//! `apply --watch` affordance which composes an apply with a live
//! progress stream.
//!
//! # Topology
//!
//! The endpoint is a thin adapter over the in-process broadcaster in
//! [`super::tenant_pubsub`]. One `broadcast::Receiver` per connection;
//! events broadcast *after* the subscription arrives are delivered in
//! order, filtered to the requested slug. Events that pre-date the
//! connection are NOT replayed — clients that need history should read
//! the governance audit log.
//!
//! # Wire shape
//!
//! Each SSE frame carries the serialised [`TenantChangeEvent`] JSON as
//! its `data:` payload, with a named event matching `TenantChangeKind`
//! so `EventSource` listeners can route by name. The untagged `kind`
//! variant renders as either a string (`"provisioned"`, `"updated"`,
//! `"deactivated"`) or an object (`{"provisioning_step":{...}}`).
//!
//! # Back-pressure
//!
//! The broadcaster capacity is `EVENT_BUS_CAPACITY = 256`. A slow
//! subscriber that falls behind receives a `Lagged(n)` error; we
//! surface that as a synthetic `event: lagged` frame and keep the
//! connection open. Clients should treat lagged as "disconnect and
//! reconnect" because ordered delivery is what `apply --watch` needs
//! to render a linear progress log.
//!
//! # Auth
//!
//! PlatformAdmin required — same gate as `tenant_wiring_api`, the
//! nearest-neighbour read-only observation endpoint in the
//! `/admin/tenants/...` namespace.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::stream::{self, Stream};
use mk_core::types::{Role, RoleIdentifier};
use serde_json::json;
use tokio::sync::broadcast;

use super::tenant_pubsub::{TenantChangeEvent, TenantChangeKind};
use super::{AppState, authenticated_platform_context};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/tenants/{slug}/events", get(stream_tenant_events))
        .with_state(state)
}

/// PA gate, mirroring `tenant_wiring_api::require_platform_admin`.
/// Duplicated rather than made `pub(crate)` to preserve the current
/// module boundary — each `/admin/tenants/...` endpoint owns its auth
/// surface so refactors in one do not silently open another.
async fn require_platform_admin(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let (_uid, roles) = authenticated_platform_context(state, headers).await?;
    let pa: RoleIdentifier = Role::PlatformAdmin.into();
    if !roles.contains(&pa) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "forbidden",
                "message": "PlatformAdmin role required",
            })),
        )
            .into_response());
    }
    Ok(())
}

/// Render one [`TenantChangeEvent`] as an SSE frame.
///
/// Extracted so it can be unit-tested without a live HTTP connection.
/// Returns `None` on the rare serialisation failure — callers drop the
/// frame rather than kill the stream.
fn render_event(ev: &TenantChangeEvent) -> Option<Event> {
    let data = serde_json::to_string(ev).ok()?;
    // Name each frame by the kind discriminator so EventSource clients
    // can `addEventListener("provisioning_step", …)`. `Unknown` is the
    // forward-compat catch-all from tenant_pubsub; we still surface it
    // to the client so debugging a rollout is not blind.
    let name = match &ev.kind {
        TenantChangeKind::Provisioned => "provisioned",
        TenantChangeKind::Updated => "updated",
        TenantChangeKind::Deactivated => "deactivated",
        TenantChangeKind::ProvisioningStep { .. } => "provisioning_step",
        TenantChangeKind::Unknown => "unknown",
    };
    Some(Event::default().event(name).data(data))
}

/// Synthetic `lagged` frame so the client knows it missed events and
/// can reconnect to re-sync.
fn render_lagged(skipped: u64) -> Event {
    Event::default()
        .event("lagged")
        .data(json!({ "skipped": skipped }).to_string())
}

#[tracing::instrument(skip_all, fields(slug = %slug))]
async fn stream_tenant_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if let Err(resp) = require_platform_admin(&state, &headers).await {
        return resp;
    }

    // Subscribe *before* constructing the response so the client starts
    // receiving events from this instant, not from the moment Axum
    // begins polling the body.
    let rx = super::tenant_pubsub::subscribe();

    // Filter-by-slug happens in the consumer. We do NOT tell the
    // broadcaster to filter because it is process-wide and pre-filter
    // would require a per-slug sub-channel (not worth it — active
    // watchers are few and filtering in the consumer is O(events)).
    let filtered = build_filtered_stream(rx, slug);

    Sse::new(filtered)
        // `KeepAlive::default()` = 15s comment ping. Critical for
        // infra that closes idle connections (ALB default 60 s, nginx
        // 75 s) — without it, a tenant sitting quietly between
        // provisions would see a silent disconnect after the LB
        // timeout.
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Build the filtered SSE stream. Extracted so it can be exercised in
/// unit tests without constructing an `AppState`.
fn build_filtered_stream(
    rx: broadcast::Receiver<TenantChangeEvent>,
    slug: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    // Hand-rolled stream over `broadcast::Receiver` via
    // `stream::unfold` — avoids adding `tokio-stream` as a workspace
    // dep just for `BroadcastStream`. State carried across yields:
    // the receiver (moves forward only) and the slug filter (clone on
    // entry once, reused each step).
    //
    // Loop semantics:
    //   * Matching slug  → yield the rendered frame.
    //   * Other  slug    → skip and continue recv'ing.
    //   * Lagged         → yield a synthetic `lagged` frame so the
    //                      client can reconnect to re-sync; keep the
    //                      receiver (broadcast docs: subsequent recv
    //                      resumes at the channel head).
    //   * Closed         → terminate the stream; Axum will close the
    //                      SSE connection cleanly.
    stream::unfold((rx, slug), |(mut rx, slug)| async move {
        loop {
            match rx.recv().await {
                Ok(ev) if ev.slug == slug => match render_event(&ev) {
                    Some(frame) => return Some((Ok(frame), (rx, slug))),
                    // serialisation failure is *so* rare (serde_json
                    // on an owned struct) that dropping the frame is
                    // strictly better than killing the stream — we
                    // continue to the next event.
                    None => continue,
                },
                Ok(_) => continue, // different tenant
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    return Some((Ok(render_lagged(n)), (rx, slug)));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::tenant_pubsub;

    #[test]
    fn render_event_names_unit_variants() {
        // axum's `Event` type is opaque but its `Debug` impl is stable
        // enough to smoke-check the name field — we just need to know
        // the correct SSE `event:` name was selected, not introspect
        // the full frame.
        let ev = TenantChangeEvent::new("acme", TenantChangeKind::Provisioned);
        let rendered = render_event(&ev).expect("serialises");
        let wire = format!("{:?}", rendered);
        assert!(wire.contains("provisioned"), "got: {wire}");
    }

    #[test]
    fn render_event_names_step_variant() {
        let ev = TenantChangeEvent::step("acme", "config", "ok", None);
        let rendered = render_event(&ev).expect("serialises");
        let wire = format!("{:?}", rendered);
        // The SSE event name must be the underscored form so
        // `EventSource.addEventListener` can target it without string
        // gymnastics on the client.
        assert!(wire.contains("provisioning_step"), "got: {wire}");
    }

    #[test]
    fn render_lagged_includes_skipped_count() {
        let ev = render_lagged(42);
        let wire = format!("{:?}", ev);
        assert!(wire.contains("lagged"));
        assert!(wire.contains("42"));
    }

    #[tokio::test]
    async fn filtered_stream_keeps_only_matching_slug() {
        // Subscribe *through the real broadcaster* so this test
        // exercises the same fan-out path the server uses.
        let rx = tenant_pubsub::subscribe();
        let stream = build_filtered_stream(rx, "events-filter-wanted".into());
        tokio::pin!(stream);

        // Push three events through the module-level broadcaster:
        //   1. matching slug
        //   2. *different* slug — must be filtered out
        //   3. matching slug
        // If the filter misbehaves we will see the middle event on the
        // stream; if it misses a real event we will time out on poll 2.
        tenant_pubsub::fan_out_local(&TenantChangeEvent::step(
            "events-filter-wanted",
            "tenant",
            "ok",
            None,
        ));
        tenant_pubsub::fan_out_local(&TenantChangeEvent::step(
            "events-filter-other",
            "tenant",
            "ok",
            None,
        ));
        tenant_pubsub::fan_out_local(&TenantChangeEvent::step(
            "events-filter-wanted",
            "config",
            "ok",
            None,
        ));

        let first = tokio::time::timeout(std::time::Duration::from_millis(200), stream.next())
            .await
            .expect("first event arrives within budget")
            .expect("stream not ended");
        let first_wire = format!("{:?}", first.unwrap());
        assert!(
            first_wire.contains("events-filter-wanted"),
            "first frame must carry the wanted slug: {first_wire}"
        );

        let second = tokio::time::timeout(std::time::Duration::from_millis(200), stream.next())
            .await
            .expect("second event arrives within budget")
            .expect("stream not ended");
        let second_wire = format!("{:?}", second.unwrap());
        assert!(
            second_wire.contains("events-filter-wanted"),
            "second frame must carry the wanted slug \
             (the `other` slug must have been filtered): {second_wire}"
        );

        // After both matches drain, a further poll with a tight
        // timeout must NOT resolve — the filter kept the `other` event
        // out, so the stream is idle.
        let third = tokio::time::timeout(std::time::Duration::from_millis(50), stream.next()).await;
        assert!(
            third.is_err(),
            "stream must stay quiet after matches drain; got: {third:?}"
        );
    }
}
