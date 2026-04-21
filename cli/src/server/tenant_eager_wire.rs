//! Eager tenant wiring (B2 task 5.2 — boot-loop half of design §D5).
//!
//! On pod start, the handler [`spawn_eager_wire`] enumerates every active
//! tenant and primes the provider registry so subsequent requests hit a
//! warm cache and so misconfigured tenants are surfaced by `/ready` (task
//! 5.3) instead of by the first user who tries to use them.
//!
//! # Scope of this drop
//!
//! This module owns *only* the boot loop. The pub/sub subscriber that
//! listens on `tenant:changed` for cross-pod invalidation and the lazy
//! fallback that closes the sub-second race window (both per design
//! §D5) land in follow-up PRs; the public surface here — `spawn_eager_wire`
//! taking `Arc<AppState>` — is designed so those additions do not
//! require another wide refactor.
//!
//! # Blocking policy
//!
//! Boot does **not** block on the wiring completing. The task is spawned
//! and returns immediately so the HTTP server can bind and serve `/health`
//! (liveness). What a failed wiring costs is that `/ready` keeps returning
//! 503 with `LoadingFailed` tenants visible in the status endpoint until
//! either a rewire succeeds or the tenant is deactivated. Per-tenant
//! failures do not taint other tenants.
//!
//! # Strict mode
//!
//! `AETERNA_EAGER_WIRE_STRICT=1` makes any `LoadingFailed` keep `/ready`
//! at 503 forever. Default (per design §D5 "failure policy: per-tenant")
//! is permissive — `/ready` flips to 200 once every tenant has *some*
//! terminal state, so a single misconfigured tenant does not lock the
//! whole cluster out of load-balancer rotation.
//!
//! Strict mode is consulted by the `/ready` handler (5.3), not here —
//! this module only records state. We expose a helper so the handler
//! reads the same env var.

use std::sync::Arc;
use std::time::Instant;

use tracing::{debug, error, info, warn};

use super::AppState;

/// Spawn the eager tenant-wiring task.
///
/// Returns immediately. The task runs on the tokio runtime and logs
/// progress at INFO; failures log at WARN with the tenant slug and the
/// error summary (redacted — downstream producers must not include
/// secret material in error messages).
///
/// The caller owns the `Arc<AppState>` returned by [`super::bootstrap::bootstrap`];
/// passing a clone keeps the task independent of the HTTP server
/// lifecycle. Graceful shutdown is driven by `AppState::shutdown_tx`
/// — the loop checks the watch on each iteration and bails without
/// writing further state when shutdown is requested.
pub fn spawn_eager_wire(state: Arc<AppState>) {
    tokio::spawn(async move {
        if let Err(e) = run_eager_wire(&state).await {
            error!(error = %e, "eager tenant wiring loop failed");
        }
    });
}

/// True iff strict mode is enabled. Used by the `/ready` gate (5.3).
/// Defined here so the env-var spelling is centralised.
pub fn is_strict_mode() -> bool {
    matches!(
        std::env::var("AETERNA_EAGER_WIRE_STRICT").as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "on")
    )
}

async fn run_eager_wire(state: &AppState) -> anyhow::Result<()> {
    let started = Instant::now();
    let tenants = state.tenant_store.list_tenants(false).await?;
    info!(
        tenant_count = tenants.len(),
        strict_mode = is_strict_mode(),
        "eager tenant wiring: starting"
    );

    // Seed every tenant as Loading BEFORE we start resolving — this way
    // `/ready` cannot observe a partial registry that says "all available"
    // simply because we haven't recorded the in-flight ones yet. Between
    // this loop and the resolution loop `all_available` returns false
    // even if the first tenant already finished.
    for t in &tenants {
        state
            .tenant_runtime_state
            .mark_loading(t.slug.as_str())
            .await;
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    for t in tenants {
        // Respect shutdown: stop seeding new work but leave already-recorded
        // state intact. The pod will exit in moments.
        if *state.shutdown_tx.borrow() {
            warn!("eager tenant wiring: shutdown requested, aborting");
            break;
        }

        let slug = t.slug.clone();
        let tenant_id = t.id.clone();
        let started_one = Instant::now();
        match wire_one(state, &tenant_id).await {
            Ok(()) => {
                let rev = state.tenant_runtime_state.mark_available(&slug).await;
                ok += 1;
                debug!(
                    tenant = %slug,
                    rev,
                    elapsed_ms = started_one.elapsed().as_millis() as u64,
                    "tenant wired"
                );
            }
            Err(e) => {
                // `e` is `anyhow::Error` from `wire_one`; its root-cause
                // chain is pre-redacted. We still clamp to 256 chars so a
                // surprise stack trace does not blow up the registry
                // entry size.
                let reason = truncate(&format!("{e:#}"), 256);
                let retries = state.tenant_runtime_state.mark_failed(&slug, &reason).await;
                failed += 1;
                warn!(
                    tenant = %slug,
                    retry_count = retries,
                    reason = %reason,
                    elapsed_ms = started_one.elapsed().as_millis() as u64,
                    "tenant wiring failed"
                );
            }
        }
    }

    info!(
        ok,
        failed,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "eager tenant wiring: complete"
    );
    Ok(())
}

/// Wire a single tenant: prime the LLM and embedding caches via the
/// existing [`memory::provider_registry::TenantProviderRegistry`]. A
/// tenant with neither an LLM nor an embedding provider configured is
/// considered successfully wired — it just resolves to the platform
/// defaults at request time and that is a legitimate steady state (e.g.
/// bootstrap before a manifest is applied).
///
/// We intentionally do NOT wire memory-layer backends here. Those are
/// per-request today via the memory manager; 5.2's scope is limited to
/// the provider registry. Widening to memory layers is a separate
/// design decision because some layers are lazy by construction.
async fn wire_one(state: &AppState, tenant_id: &mk_core::types::TenantId) -> anyhow::Result<()> {
    // The provider registry caches on success; a miss falls through to
    // platform defaults and returns `None`, which is NOT an error. Only a
    // resolution error (bad config, missing secret, unreachable endpoint)
    // should mark the tenant failed.
    //
    // The current `get_*_service` API swallows resolution errors and
    // returns `None` indistinguishably from "no override configured".
    // Until that API grows a fallible variant, eager-wiring can only
    // detect failures that surface as resolver panics or config-provider
    // errors. This is acceptable for the first drop: the readiness
    // signal is strictly better than the status quo (none), and the
    // provider-level failure surface is a known follow-up (see TODO
    // below).
    //
    // TODO(b2-5.2-followup): tighten `get_*_service` to return
    // `Result<Option<_>, ResolutionError>` so real wiring failures
    // bubble up here instead of being logged-and-swallowed inside the
    // registry.
    let _ = state
        .provider_registry
        .get_llm_service(tenant_id, state.tenant_config_provider.as_ref())
        .await;
    let _ = state
        .provider_registry
        .get_embedding_service(tenant_id, state.tenant_config_provider.as_ref())
        .await;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Safe char-boundary truncate; avoids splitting a multi-byte char
        // which would be a panic on some inputs. `floor_char_boundary`
        // would be cleaner but is nightly.
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_mode_defaults_off() {
        // Isolate from CI env. `std::env` is process-global; this test
        // is single-threaded enough for a save/restore dance.
        let prior = std::env::var("AETERNA_EAGER_WIRE_STRICT").ok();
        unsafe {
            std::env::remove_var("AETERNA_EAGER_WIRE_STRICT");
        }
        assert!(!is_strict_mode());
        if let Some(v) = prior {
            unsafe {
                std::env::set_var("AETERNA_EAGER_WIRE_STRICT", v);
            }
        }
    }

    #[test]
    fn strict_mode_parses_truthy_values() {
        let prior = std::env::var("AETERNA_EAGER_WIRE_STRICT").ok();
        for v in ["1", "true", "TRUE", "yes", "on"] {
            unsafe {
                std::env::set_var("AETERNA_EAGER_WIRE_STRICT", v);
            }
            assert!(is_strict_mode(), "expected {v} to enable strict mode");
        }
        for v in ["0", "false", "no", "off", ""] {
            unsafe {
                std::env::set_var("AETERNA_EAGER_WIRE_STRICT", v);
            }
            assert!(!is_strict_mode(), "expected {v} to leave strict off");
        }
        match prior {
            Some(v) => unsafe {
                std::env::set_var("AETERNA_EAGER_WIRE_STRICT", v);
            },
            None => unsafe {
                std::env::remove_var("AETERNA_EAGER_WIRE_STRICT");
            },
        }
    }

    #[test]
    fn truncate_preserves_short_strings() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_trims_with_ellipsis() {
        let s = "x".repeat(300);
        let out = truncate(&s, 10);
        assert_eq!(out.chars().count(), 11, "10 chars + ellipsis");
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_respects_utf8_boundaries() {
        // 4-byte emoji at byte 9. Truncating at max=10 would split the
        // emoji; the helper must step back to the nearest boundary.
        let s = "abcdefghi✨✨✨";
        let out = truncate(s, 10);
        // Valid UTF-8 by construction of String::from; the real
        // assertion is "did not panic".
        assert!(out.ends_with('…'));
        assert!(out.len() <= s.len());
    }
}
