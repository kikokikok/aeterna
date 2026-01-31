//! Section 13.4: WebSocket PubSub Reliability
//!
//! Implements resilient WebSocket connection with automatic reconnection,
//! exponential backoff, data consistency verification, and health metrics.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::time::{interval, sleep};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{debug, error, info, trace, warn};

/// WebSocket PubSub client with reliability features.
pub struct ReliableWebSocketClient {
    config: WebSocketConfig,
    state: Arc<RwLock<ConnectionState>>,
    message_tx: mpsc::UnboundedSender<PubSubMessage>,
    message_rx: Arc<RwLock<mpsc::UnboundedReceiver<PubSubMessage>>>,
    reconnect_attempts: Arc<RwLock<u32>>,
    last_checksum: Arc<RwLock<Option<String>>>,
    metrics: Arc<RwLock<ConnectionMetrics>>
}

/// Configuration for WebSocket client.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Server URL.
    pub url: String,
    /// Initial reconnect delay (seconds).
    pub initial_reconnect_delay_secs: u64,
    /// Maximum reconnect delay (seconds).
    pub max_reconnect_delay_secs: u64,
    /// Exponential backoff multiplier.
    pub backoff_multiplier: f64,
    /// Enable checksum verification.
    pub checksum_verification_enabled: bool,
    /// Health check interval (seconds).
    pub health_check_interval_secs: u64,
    /// Message buffer size.
    pub message_buffer_size: usize
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:7002/ws".to_string(),
            initial_reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 30,
            backoff_multiplier: 2.0,
            checksum_verification_enabled: true,
            health_check_interval_secs: 30,
            message_buffer_size: 1000
        }
    }
}

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting
}

/// PubSub message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubSubMessage {
    pub topic: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
    pub checksum: Option<String>
}

/// Connection health metrics.
#[derive(Debug, Clone, Default)]
pub struct ConnectionMetrics {
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub disconnections: u64,
    pub reconnection_attempts: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub checksum_failures: u64,
    pub average_latency_ms: f64,
    pub last_latency_ms: u64,
    pub connection_drops_last_hour: u64,
    pub latency_history: VecDeque<u64>
}

impl ReliableWebSocketClient {
    /// Create a new reliable WebSocket client.
    pub fn new(config: WebSocketConfig) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            last_checksum: Arc::new(RwLock::new(None)),
            metrics: Arc::new(RwLock::new(ConnectionMetrics::default()))
        }
    }

    /// Start the connection manager with automatic reconnection.
    pub async fn start(&self) -> Result<(), WebSocketError> {
        info!("Starting reliable WebSocket client to {}", self.config.url);

        let config = self.config.clone();
        let state = self.state.clone();
        let reconnect_attempts = self.reconnect_attempts.clone();
        let metrics = self.metrics.clone();
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            loop {
                // Attempt connection
                let connect_result = Self::connect(&config, &state, &metrics).await;

                match connect_result {
                    Ok(mut ws_stream) => {
                        // Reset reconnect attempts on successful connection
                        *reconnect_attempts.write().await = 0;

                        info!("WebSocket connected successfully");

                        // Handle connection
                        if let Err(e) =
                            Self::handle_connection(&mut ws_stream, &state, &message_tx, &metrics)
                                .await
                        {
                            warn!("Connection error: {}, will reconnect", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect: {}", e);
                    }
                }

                // Increment reconnect attempts
                let attempts = {
                    let mut attempts = reconnect_attempts.write().await;
                    *attempts += 1;
                    *attempts
                };

                // Calculate backoff delay (13.4.1: exponential backoff 1s→30s max)
                let delay = Self::calculate_backoff(
                    attempts,
                    config.initial_reconnect_delay_secs,
                    config.max_reconnect_delay_secs,
                    config.backoff_multiplier
                );

                warn!(
                    "Reconnecting in {} seconds (attempt {})",
                    delay.as_secs(),
                    attempts
                );

                *state.write().await = ConnectionState::Reconnecting;
                sleep(delay).await;
            }
        });

        // Start health metrics emitter
        self.start_health_emitter().await;

        Ok(())
    }

    /// Calculate exponential backoff delay.
    fn calculate_backoff(
        attempt: u32,
        initial_secs: u64,
        max_secs: u64,
        multiplier: f64
    ) -> Duration {
        let delay = initial_secs as f64 * multiplier.powi(attempt as i32 - 1);
        let clamped_delay = delay.min(max_secs as f64);
        Duration::from_secs(clamped_delay as u64)
    }

    /// Establish WebSocket connection.
    async fn connect(
        config: &WebSocketConfig,
        state: &Arc<RwLock<ConnectionState>>,
        metrics: &Arc<RwLock<ConnectionMetrics>>
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WebSocketError> {
        *state.write().await = ConnectionState::Connecting;
        metrics.write().await.connection_attempts += 1;

        let (ws_stream, _) = connect_async(&config.url)
            .await
            .map_err(|e| WebSocketError::ConnectionFailed(e.to_string()))?;

        *state.write().await = ConnectionState::Connected;
        metrics.write().await.successful_connections += 1;

        Ok(ws_stream)
    }

    /// Handle active connection with full resync on reconnect (13.4.2).
    async fn handle_connection(
        ws_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        state: &Arc<RwLock<ConnectionState>>,
        message_tx: &mpsc::UnboundedSender<PubSubMessage>,
        metrics: &Arc<RwLock<ConnectionMetrics>>
    ) -> Result<(), WebSocketError> {
        // Request full resync on connect/reconnect
        let resync_request = serde_json::json!({
            "type": "resync_request",
            "timestamp": chrono::Utc::now().timestamp()
        });

        ws_stream
            .send(Message::Text(resync_request.to_string()))
            .await
            .map_err(|e| WebSocketError::SendError(e.to_string()))?;

        loop {
            tokio::select! {
                msg = ws_stream.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<PubSubMessage>(&text) {
                                Ok(message) => {
                                    // Verify checksum (13.4.3)
                                    if let Some(expected_checksum) = &message.checksum {
                                        let calculated = Self::calculate_checksum(&message.payload);
                                        if &calculated != expected_checksum {
                                            warn!("Checksum mismatch for message on topic {}", message.topic);
                                            metrics.write().await.checksum_failures += 1;
                                            continue;
                                        }
                                    }

                                    metrics.write().await.messages_received += 1;

                                    if let Err(e) = message_tx.send(message) {
                                        error!("Failed to forward message: {}", e);
                                        metrics.write().await.messages_dropped += 1;
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse message: {}", e);
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            *state.write().await = ConnectionState::Disconnected;
                            metrics.write().await.disconnections += 1;
                            return Ok(());
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            *state.write().await = ConnectionState::Disconnected;
                            metrics.write().await.disconnections += 1;
                            return Err(WebSocketError::ConnectionError(e.to_string()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Calculate checksum for payload verification.
    fn calculate_checksum(payload: &serde_json::Value) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        payload.to_string().hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Start health metrics emitter (13.4.4).
    async fn start_health_emitter(&self) {
        let metrics = self.metrics.clone();
        let interval_secs = self.config.health_check_interval_secs;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                let metrics_snapshot = metrics.read().await.clone();

                // Emit metrics (would integrate with metrics system)
                trace!(
                    "WebSocket health: latency={}ms, drops_last_hour={}, received={}, sent={}",
                    metrics_snapshot.last_latency_ms,
                    metrics_snapshot.connection_drops_last_hour,
                    metrics_snapshot.messages_received,
                    metrics_snapshot.messages_sent
                );

                // Check for alerting conditions (13.4.5)
                if metrics_snapshot.connection_drops_last_hour > 10 {
                    warn!(
                        "ALERT: High connection drop rate: {} drops in last hour",
                        metrics_snapshot.connection_drops_last_hour
                    );
                }

                if metrics_snapshot.average_latency_ms > 1000.0 {
                    warn!(
                        "ALERT: High latency: {}ms average",
                        metrics_snapshot.average_latency_ms
                    );
                }
            }
        });
    }

    /// Get current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get connection metrics.
    pub async fn metrics(&self) -> ConnectionMetrics {
        self.metrics.read().await.clone()
    }
}

/// WebSocket errors.
#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Send error: {0}")]
    SendError(String),

    #[error("Receive error: {0}")]
    ReceiveError(String)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        // 13.4.1: Test exponential backoff 1s→30s max
        assert_eq!(
            ReliableWebSocketClient::calculate_backoff(1, 1, 30, 2.0),
            Duration::from_secs(1)
        );
        assert_eq!(
            ReliableWebSocketClient::calculate_backoff(2, 1, 30, 2.0),
            Duration::from_secs(2)
        );
        assert_eq!(
            ReliableWebSocketClient::calculate_backoff(5, 1, 30, 2.0),
            Duration::from_secs(16)
        );
        // Should cap at max
        assert_eq!(
            ReliableWebSocketClient::calculate_backoff(10, 1, 30, 2.0),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn test_checksum_calculation() {
        // 13.4.3: Test checksum verification
        let payload = serde_json::json!({"key": "value", "num": 42});
        let checksum1 = ReliableWebSocketClient::calculate_checksum(&payload);
        let checksum2 = ReliableWebSocketClient::calculate_checksum(&payload);

        assert_eq!(checksum1, checksum2);

        let different_payload = serde_json::json!({"key": "value", "num": 43});
        let checksum3 = ReliableWebSocketClient::calculate_checksum(&different_payload);

        assert_ne!(checksum1, checksum3);
    }
}
