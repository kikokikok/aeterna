use std::net::SocketAddr;

use clap::Args;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::server::{bootstrap, lifecycle, metrics, router};

#[derive(Args)]
pub struct ServeArgs {
    /// Bind address (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// HTTP port for the API server
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    pub metrics_port: u16,

    /// Path to configuration file (defaults to AETERNA_CONFIG_PATH or ./config)
    #[arg(long, env = "AETERNA_CONFIG_PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

pub async fn run(args: ServeArgs) -> anyhow::Result<()> {
    if let Some(config_path) = &args.config
        && !config_path.exists()
    {
        anyhow::bail!(
            "Configuration directory not found: {}\n\nSet AETERNA_CONFIG_PATH to a valid config directory, or run:\n\n\taeterna setup\n\nto create an initial configuration.",
            config_path.display()
        );
    }

    let state = bootstrap::bootstrap().await?;

    // B2 task 6.4 — emit `BootstrapCompleted` governance event exactly
    // once per pod boot, immediately after the tracker is finalized.
    //
    // Non-fatal: a DB outage must not prevent the API from serving a
    // pod that otherwise bootstrapped cleanly. The event is best-effort;
    // `/admin/bootstrap/status` remains the authoritative per-pod
    // readout for ops, and the `governance_events` row is just the
    // async/durable copy for downstream consumers (audit dashboards,
    // pub/sub subscribers).
    emit_bootstrap_completed(state.as_ref()).await;

    let app = router::build_router(state.clone());
    let metrics_handle = metrics::create_recorder();
    let metrics_app = metrics::router(metrics_handle);

    let app_addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let metrics_addr: SocketAddr = format!("{}:{}", args.bind, args.metrics_port).parse()?;

    tracing::info!(address = %app_addr, "Aeterna API server starting");
    tracing::info!(address = %metrics_addr, "Aeterna metrics server starting");

    // Start the lifecycle manager — handles all periodic background tasks
    // (job cleanup, retention purge, remediation expiry, etc.)
    let lifecycle_mgr = lifecycle::LifecycleManager::new();
    lifecycle_mgr.start(state.clone());

    // Eager tenant wiring (B2 task 5.2 — design §D5 boot loop). Spawned
    // detached; the HTTP server binds immediately and `/ready` (task 5.3)
    // reports the progress. See
    // `cli/src/server/tenant_eager_wire.rs` for failure policy.
    crate::server::tenant_eager_wire::spawn_eager_wire(state.clone());

    // Cross-pod tenant invalidation subscriber (task 5.2b). No-op when
    // Redis is unavailable; in single-pod mode local invalidation is
    // handled directly by the mutating handler. See
    // `cli/src/server/tenant_pubsub.rs`.
    crate::server::tenant_pubsub::spawn_subscriber(state.clone());

    let app_listener = TcpListener::bind(app_addr).await?;
    let metrics_listener = TcpListener::bind(metrics_addr).await?;

    let shutdown_rx_http = state.shutdown_tx.subscribe();
    let shutdown_rx_metrics = state.shutdown_tx.subscribe();

    let shutdown_tx = state.shutdown_tx.clone();
    let signal_task = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    #[cfg(unix)]
    let shutdown_tx_sigterm = state.shutdown_tx.clone();
    #[cfg(unix)]
    let sigterm_task = tokio::spawn(async move {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            sigterm.recv().await;
            let _ = shutdown_tx_sigterm.send(true);
        }
    });

    let app_server =
        axum::serve(app_listener, app).with_graceful_shutdown(await_shutdown(shutdown_rx_http));

    let metrics_server = axum::serve(metrics_listener, metrics_app)
        .with_graceful_shutdown(await_shutdown(shutdown_rx_metrics));

    tokio::try_join!(app_server, metrics_server)?;

    lifecycle_mgr.shutdown();

    signal_task.abort();
    #[cfg(unix)]
    sigterm_task.abort();

    Ok(())
}

async fn await_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while shutdown_rx.changed().await.is_ok() {
        if *shutdown_rx.borrow() {
            break;
        }
    }
}

/// B2 task 6.4 — fire a one-shot `BootstrapCompleted` governance event
/// after the tracker finalizes.
///
/// ## Scope
///
/// Platform-level: uses [`INSTANCE_SCOPE_TENANT_ID`] (`__root__`) as the
/// tenant scope because bootstrap is not tenant-scoped. The payload is
/// the full per-phase snapshot, byte-for-byte identical to what
/// `/api/v1/admin/bootstrap/status` returns on the same pod.
///
/// ## Failure handling
///
/// Best-effort. Any error in writing the event is logged at `warn!`
/// level and swallowed; the server continues to start normally. The
/// tracker already populates `/admin/bootstrap/status` synchronously —
/// the governance event is a durable copy for downstream consumers, not
/// the authoritative record.
///
/// ## Idempotency
///
/// `PersistentEvent::new` generates a fresh UUID `event_id`, and the
/// row's `idempotency_key` is `sha256(event_id:timestamp:tenant_id)`.
/// Every pod boot therefore produces a unique row — we do not suppress
/// repeat emissions on pod restart, which is the desired behaviour:
/// each boot is independently observable.
///
/// ## No-op cases
///
/// - Tracker not ready (failed bootstrap): the server never reaches
///   this call because `bootstrap()` returned `Err` upstream and the
///   process exits. The `if` guard is belt-and-braces against future
///   refactors that decouple tracker state from the `Result`.
async fn emit_bootstrap_completed(state: &crate::server::AppState) {
    use mk_core::types::{GovernanceEvent, INSTANCE_SCOPE_TENANT_ID, PersistentEvent, TenantId};

    if !state.bootstrap_tracker.is_completed() {
        // Defensive: bootstrap reported partial failure. Do not claim
        // success in the event log.
        tracing::debug!(
            "skipping BootstrapCompleted emission — tracker is not in a completed state"
        );
        return;
    }

    let snapshot = state.bootstrap_tracker.snapshot();
    let Some(tenant_id) = TenantId::new(INSTANCE_SCOPE_TENANT_ID.to_string()) else {
        tracing::warn!(
            "BootstrapCompleted emission skipped: INSTANCE_SCOPE_TENANT_ID did not satisfy \
             TenantId::new (this should not happen — it is a compile-time constant)"
        );
        return;
    };

    let event = GovernanceEvent::BootstrapCompleted {
        tenant_id,
        timestamp: chrono::Utc::now().timestamp(),
        snapshot,
    };

    use mk_core::traits::StorageBackend;
    match state
        .postgres
        .persist_event(PersistentEvent::new(event))
        .await
    {
        Ok(()) => tracing::info!("BootstrapCompleted governance event persisted"),
        Err(err) => tracing::warn!(
            error = %err,
            "Failed to persist BootstrapCompleted governance event \
             (server continues; /admin/bootstrap/status remains the primary readout)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_args_defaults() {
        let args = ServeArgs {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            metrics_port: 9090,
            config: None,
            log_level: "info".to_string(),
        };
        assert_eq!(args.bind, "0.0.0.0");
        assert_eq!(args.port, 8080);
        assert_eq!(args.metrics_port, 9090);
        assert!(args.config.is_none());
        assert_eq!(args.log_level, "info");
    }

    #[test]
    fn test_serve_args_custom_port() {
        let args = ServeArgs {
            bind: "127.0.0.1".to_string(),
            port: 3000,
            metrics_port: 3001,
            config: Some(std::path::PathBuf::from("/etc/aeterna")),
            log_level: "debug".to_string(),
        };
        assert_eq!(args.port, 3000);
        assert_eq!(args.metrics_port, 3001);
        assert_eq!(args.bind, "127.0.0.1");
        assert!(args.config.is_some());
        assert_eq!(args.log_level, "debug");
    }

    #[tokio::test]
    async fn test_serve_missing_config_path_returns_error() {
        let args = ServeArgs {
            bind: "0.0.0.0".to_string(),
            port: 8080,
            metrics_port: 9090,
            config: Some(std::path::PathBuf::from(
                "/nonexistent/path/that/does/not/exist",
            )),
            log_level: "info".to_string(),
        };
        let result = run(args).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Configuration directory not found"));
    }

    #[tokio::test]
    async fn await_shutdown_returns_when_flag_becomes_true() {
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(await_shutdown(rx));
        tx.send(true).unwrap();
        task.await.unwrap();
    }
}
