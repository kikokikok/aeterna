//! Server setup and lifecycle for the OPAL Data Fetcher.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;

use crate::error::{FetcherError, Result};
use crate::listener::ReferentialListener;
use crate::routes::create_router;
use crate::state::{AppState, FetcherConfig};

/// The OPAL Data Fetcher server.
pub struct OpalFetcherServer {
    state: Arc<AppState>,
    listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl OpalFetcherServer {
    /// Creates a new server instance with the given configuration.
    pub async fn new(config: FetcherConfig) -> Result<Self> {
        let state = Arc::new(AppState::new(config).await?);
        Ok(Self {
            state,
            listener_handle: None,
        })
    }

    /// Creates a server instance from an existing `AppState`.
    pub fn with_state(state: Arc<AppState>) -> Self {
        Self {
            state,
            listener_handle: None,
        }
    }

    /// Starts the PostgreSQL LISTEN/NOTIFY listener in the background.
    pub async fn start_listener(&mut self) -> Result<()> {
        if !self.state.config.enable_listener {
            tracing::info!("PostgreSQL listener disabled by configuration");
            return Ok(());
        }

        let listener = ReferentialListener::new(self.state.clone()).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = listener.run().await {
                tracing::error!(error = %e, "Referential listener error");
            }
        });

        self.listener_handle = Some(handle);
        tracing::info!("PostgreSQL LISTEN/NOTIFY listener started");

        Ok(())
    }

    /// Runs the HTTP server.
    ///
    /// This method blocks until the server is shut down (e.g., via Ctrl+C).
    pub async fn run(mut self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.state.config.host, self.state.config.port)
            .parse()
            .map_err(|e| FetcherError::Configuration(format!("Invalid address: {e}")))?;

        // Start the listener if enabled
        self.start_listener().await?;

        // Create router
        let router = create_router(self.state.clone());

        // Bind to address
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| FetcherError::Server(format!("Failed to bind to {addr}: {e}")))?;

        tracing::info!(%addr, "OPAL Data Fetcher server starting");

        // Run server with graceful shutdown
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| FetcherError::Server(format!("Server error: {e}")))?;

        // Cleanup
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }

        tracing::info!("OPAL Data Fetcher server stopped");
        Ok(())
    }

    /// Returns a reference to the application state.
    #[must_use]
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }
}

/// Signal handler for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        },
        () = terminate => {
            tracing::info!("Received terminate signal, initiating graceful shutdown");
        },
    }
}

/// Entry point for running the server from configuration.
///
/// This is a convenience function that creates and runs the server.
pub async fn run_server(config: FetcherConfig) -> Result<()> {
    let server = OpalFetcherServer::new(config).await?;
    server.run().await
}

/// Entry point for running the server from environment variables.
///
/// This is a convenience function for containerized deployments.
pub async fn run_from_env() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config = FetcherConfig::from_env()?;
    run_server(config).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_shutdown_signal_exists() {}
}
