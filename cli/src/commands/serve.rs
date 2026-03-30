use std::net::SocketAddr;

use clap::Args;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::server::{bootstrap, metrics, router};

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
    let app = router::build_router(state.clone());
    let metrics_handle = metrics::create_recorder();
    let metrics_app = metrics::router(metrics_handle);

    let app_addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let metrics_addr: SocketAddr = format!("{}:{}", args.bind, args.metrics_port).parse()?;

    tracing::info!(address = %app_addr, "Aeterna API server starting");
    tracing::info!(address = %metrics_addr, "Aeterna metrics server starting");

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
