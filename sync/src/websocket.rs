use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

pub type ClientId = Uuid;
pub type Room = String;

#[derive(Debug, Error)]
pub enum WsError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Send error: {0}")]
    Send(String),
}

pub type WsResult<T> = Result<T, WsError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthToken {
    pub user_id: String,
    pub tenant_id: String,
    pub permissions: Vec<String>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub client_id: ClientId,
    pub user_id: String,
    pub tenant_id: String,
    pub rooms: HashSet<Room>,
    pub connected_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsClientMessage {
    #[serde(rename_all = "camelCase")]
    Authenticate {
        token: String,
    },
    #[serde(rename_all = "camelCase")]
    Subscribe {
        room: Room,
    },
    #[serde(rename_all = "camelCase")]
    Unsubscribe {
        room: Room,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsServerMessage {
    #[serde(rename_all = "camelCase")]
    Authenticated {
        client_id: ClientId,
    },
    #[serde(rename_all = "camelCase")]
    Subscribed {
        room: Room,
    },
    #[serde(rename_all = "camelCase")]
    Unsubscribed {
        room: Room,
    },
    #[serde(rename_all = "camelCase")]
    RoomMessage {
        room: Room,
        payload: serde_json::Value,
    },
    #[serde(rename_all = "camelCase")]
    Error {
        message: String,
    },
    Pong,
}

#[async_trait::async_trait]
pub trait TokenValidator: Send + Sync {
    async fn validate(&self, token: &str) -> WsResult<AuthToken>;
}

pub struct RoomState {
    pub members: HashSet<ClientId>,
    pub broadcast_tx: broadcast::Sender<WsServerMessage>,
}

pub struct WsServer {
    clients: Arc<DashMap<ClientId, ClientInfo>>,
    rooms: Arc<DashMap<Room, RoomState>>,
    token_validator: Arc<dyn TokenValidator>,
    client_senders: Arc<DashMap<ClientId, mpsc::Sender<WsServerMessage>>>,
    shutdown: Arc<RwLock<bool>>,
}

impl WsServer {
    pub fn new(token_validator: Arc<dyn TokenValidator>) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            rooms: Arc::new(DashMap::new()),
            token_validator,
            client_senders: Arc::new(DashMap::new()),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn listen(self: &Arc<Self>, addr: SocketAddr) -> WsResult<()> {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("WebSocket server listening on {addr}");

        loop {
            if *self.shutdown.read().await {
                break;
            }

            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let server = Arc::clone(self);
                            tokio::spawn(async move {
                                if let Err(e) = server.handle_connection(stream, peer_addr).await {
                                    tracing::warn!("Connection from {peer_addr} error: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Accept error: {e}");
                        }
                    }
                }
                () = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}

            }
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        let mut flag = self.shutdown.write().await;
        *flag = true;
    }

    pub async fn handle_connection(
        self: &Arc<Self>,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> WsResult<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        tracing::debug!("New WebSocket connection from {peer_addr}");

        let auth_msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws_receiver.next())
            .await
            .map_err(|_| WsError::AuthFailed("Authentication timeout".into()))?
            .ok_or(WsError::ConnectionClosed)?
            .map_err(WsError::WebSocket)?;

        let client_msg: WsClientMessage = match auth_msg {
            Message::Text(text) => serde_json::from_str(&text)?,
            _ => return Err(WsError::AuthFailed("Expected text message".into())),
        };

        let token = match client_msg {
            WsClientMessage::Authenticate { token } => token,
            _ => {
                return Err(WsError::AuthFailed(
                    "First message must be Authenticate".into(),
                ));
            }
        };

        let auth_token = self.token_validator.validate(&token).await?;
        let client_id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();

        let client_info = ClientInfo {
            client_id,
            user_id: auth_token.user_id.clone(),
            tenant_id: auth_token.tenant_id.clone(),
            rooms: HashSet::new(),
            connected_at: now,
        };

        self.clients.insert(client_id, client_info);

        let auth_response = WsServerMessage::Authenticated { client_id };
        let auth_json = serde_json::to_string(&auth_response)?;
        ws_sender.send(Message::Text(auth_json.into())).await?;

        let (msg_tx, mut msg_rx) = mpsc::channel::<WsServerMessage>(256);
        self.client_senders.insert(client_id, msg_tx);

        let server = Arc::clone(self);
        let result: WsResult<()> = async {
            loop {
                tokio::select! {
                    incoming = ws_receiver.next() => {
                        match incoming {
                            Some(Ok(Message::Text(text))) => {
                                let msg: WsClientMessage = serde_json::from_str(&text)?;
                                let response = server.handle_client_message(client_id, msg).await?;
                                if let Some(resp) = response {
                                    let json = serde_json::to_string(&resp)?;
                                    ws_sender.send(Message::Text(json.into())).await?;
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                break;
                            }
                            Some(Ok(Message::Ping(data))) => {
                                ws_sender.send(Message::Pong(data)).await?;
                            }
                            Some(Err(e)) => {
                                tracing::warn!("WebSocket receive error for {client_id}: {e}");
                                break;
                            }
                            _ => {}
                        }
                    }
                    outgoing = msg_rx.recv() => {
                        match outgoing {
                            Some(msg) => {
                                let json = serde_json::to_string(&msg)?;
                                ws_sender.send(Message::Text(json.into())).await?;
                            }
                            None => break,
                        }
                    }
                }
            }
            Ok(())
        }
        .await;

        self.disconnect_client(client_id).await;
        result
    }

    async fn handle_client_message(
        &self,
        client_id: ClientId,
        msg: WsClientMessage,
    ) -> WsResult<Option<WsServerMessage>> {
        match msg {
            WsClientMessage::Subscribe { room } => {
                self.subscribe_to_room(client_id, &room).await?;
                Ok(Some(WsServerMessage::Subscribed { room }))
            }
            WsClientMessage::Unsubscribe { room } => {
                self.unsubscribe_from_room(client_id, &room).await;
                Ok(Some(WsServerMessage::Unsubscribed { room }))
            }
            WsClientMessage::Ping => Ok(Some(WsServerMessage::Pong)),
            WsClientMessage::Authenticate { .. } => Ok(Some(WsServerMessage::Error {
                message: "Already authenticated".into(),
            })),
        }
    }

    async fn subscribe_to_room(&self, client_id: ClientId, room: &str) -> WsResult<()> {
        self.rooms
            .entry(room.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(1024);
                RoomState {
                    members: HashSet::new(),
                    broadcast_tx: tx,
                }
            })
            .members
            .insert(client_id);

        if let Some(mut client) = self.clients.get_mut(&client_id) {
            client.rooms.insert(room.to_string());
        }

        tracing::debug!("Client {client_id} subscribed to room {room}");
        Ok(())
    }

    async fn unsubscribe_from_room(&self, client_id: ClientId, room: &str) {
        if let Some(mut room_state) = self.rooms.get_mut(room) {
            room_state.members.remove(&client_id);
        }
        if let Some(mut client) = self.clients.get_mut(&client_id) {
            client.rooms.remove(room);
        }
        tracing::debug!("Client {client_id} unsubscribed from room {room}");
    }

    async fn disconnect_client(&self, client_id: ClientId) {
        if let Some((_, client_info)) = self.clients.remove(&client_id) {
            for room in &client_info.rooms {
                if let Some(mut room_state) = self.rooms.get_mut(room) {
                    room_state.members.remove(&client_id);
                }
            }
        }
        self.client_senders.remove(&client_id);
        tracing::debug!("Client {client_id} disconnected");
    }

    pub async fn broadcast_to_room(
        &self,
        room: &str,
        payload: serde_json::Value,
    ) -> WsResult<usize> {
        let members: Vec<ClientId> = match self.rooms.get(room) {
            Some(room_state) => room_state.members.iter().copied().collect(),
            None => return Ok(0),
        };

        let msg = WsServerMessage::RoomMessage {
            room: room.to_string(),
            payload,
        };

        let mut sent = 0usize;
        for client_id in &members {
            if let Some(sender) = self.client_senders.get(client_id) {
                if sender.send(msg.clone()).await.is_ok() {
                    sent += 1;
                }
            }
        }
        Ok(sent)
    }

    pub fn connected_clients(&self) -> usize {
        self.clients.len()
    }

    pub fn room_members(&self, room: &str) -> Vec<ClientId> {
        self.rooms
            .get(room)
            .map(|r| r.members.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn client_rooms(&self, client_id: &ClientId) -> HashSet<Room> {
        self.clients
            .get(client_id)
            .map(|c| c.rooms.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockValidator {
        should_fail: AtomicBool,
    }

    impl MockValidator {
        fn new(should_fail: bool) -> Self {
            Self {
                should_fail: AtomicBool::new(should_fail),
            }
        }
    }

    #[async_trait::async_trait]
    impl TokenValidator for MockValidator {
        async fn validate(&self, token: &str) -> WsResult<AuthToken> {
            if self.should_fail.load(Ordering::Relaxed) {
                return Err(WsError::AuthFailed("Invalid token".into()));
            }
            Ok(AuthToken {
                user_id: format!("user-{token}"),
                tenant_id: "tenant-1".into(),
                permissions: vec!["read".into(), "write".into()],
                expires_at: chrono::Utc::now().timestamp() + 3600,
            })
        }
    }

    #[test]
    fn test_ws_server_creation() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        assert_eq!(server.connected_clients(), 0);
    }

    #[tokio::test]
    async fn test_room_subscription_lifecycle() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let client_info = ClientInfo {
            client_id,
            user_id: "user-1".into(),
            tenant_id: "tenant-1".into(),
            rooms: HashSet::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };
        server.clients.insert(client_id, client_info);

        server
            .subscribe_to_room(client_id, "layer:project:123")
            .await
            .expect("subscribe should succeed");
        assert!(
            server
                .room_members("layer:project:123")
                .contains(&client_id)
        );
        assert!(
            server
                .client_rooms(&client_id)
                .contains("layer:project:123")
        );

        server
            .unsubscribe_from_room(client_id, "layer:project:123")
            .await;
        assert!(
            !server
                .room_members("layer:project:123")
                .contains(&client_id)
        );
    }

    #[tokio::test]
    async fn test_broadcast_to_room() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let client_info = ClientInfo {
            client_id,
            user_id: "user-1".into(),
            tenant_id: "tenant-1".into(),
            rooms: HashSet::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };
        server.clients.insert(client_id, client_info);

        let (tx, mut rx) = mpsc::channel(256);
        server.client_senders.insert(client_id, tx);

        server
            .subscribe_to_room(client_id, "room-a")
            .await
            .expect("subscribe should succeed");

        let payload = serde_json::json!({"event": "test"});
        let sent = server
            .broadcast_to_room("room-a", payload.clone())
            .await
            .expect("broadcast should succeed");
        assert_eq!(sent, 1);

        let received = rx.recv().await.expect("should receive message");
        match received {
            WsServerMessage::RoomMessage { room, payload: p } => {
                assert_eq!(room, "room-a");
                assert_eq!(p, payload);
            }
            other => panic!("Expected RoomMessage, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_broadcast_to_empty_room() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);

        let sent = server
            .broadcast_to_room("nonexistent", serde_json::json!({}))
            .await
            .expect("broadcast should succeed");
        assert_eq!(sent, 0);
    }

    #[tokio::test]
    async fn test_disconnect_client_cleanup() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let client_info = ClientInfo {
            client_id,
            user_id: "user-1".into(),
            tenant_id: "tenant-1".into(),
            rooms: HashSet::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };
        server.clients.insert(client_id, client_info);

        let (tx, _rx) = mpsc::channel(256);
        server.client_senders.insert(client_id, tx);

        server
            .subscribe_to_room(client_id, "room-1")
            .await
            .expect("subscribe should succeed");
        server
            .subscribe_to_room(client_id, "room-2")
            .await
            .expect("subscribe should succeed");

        assert_eq!(server.connected_clients(), 1);

        server.disconnect_client(client_id).await;

        assert_eq!(server.connected_clients(), 0);
        assert!(!server.room_members("room-1").contains(&client_id));
        assert!(!server.room_members("room-2").contains(&client_id));
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = WsClientMessage::Subscribe {
            room: "layer:project:abc".into(),
        };
        let json = serde_json::to_string(&msg).expect("serialize should succeed");
        let deserialized: WsClientMessage =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = WsServerMessage::RoomMessage {
            room: "room-1".into(),
            payload: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&msg).expect("serialize should succeed");
        let deserialized: WsServerMessage =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(msg, deserialized);
    }

    #[tokio::test]
    async fn test_handle_client_message_subscribe() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let client_info = ClientInfo {
            client_id,
            user_id: "user-1".into(),
            tenant_id: "tenant-1".into(),
            rooms: HashSet::new(),
            connected_at: chrono::Utc::now().timestamp(),
        };
        server.clients.insert(client_id, client_info);

        let response = server
            .handle_client_message(
                client_id,
                WsClientMessage::Subscribe {
                    room: "room-x".into(),
                },
            )
            .await
            .expect("handle should succeed");

        assert_eq!(
            response,
            Some(WsServerMessage::Subscribed {
                room: "room-x".into()
            })
        );
    }

    #[tokio::test]
    async fn test_handle_client_message_ping() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let response = server
            .handle_client_message(client_id, WsClientMessage::Ping)
            .await
            .expect("handle should succeed");

        assert_eq!(response, Some(WsServerMessage::Pong));
    }

    #[tokio::test]
    async fn test_handle_duplicate_authenticate() {
        let validator = Arc::new(MockValidator::new(false));
        let server = WsServer::new(validator);
        let client_id = Uuid::new_v4();

        let response = server
            .handle_client_message(
                client_id,
                WsClientMessage::Authenticate {
                    token: "abc".into(),
                },
            )
            .await
            .expect("handle should succeed");

        match response {
            Some(WsServerMessage::Error { message }) => {
                assert_eq!(message, "Already authenticated");
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[test]
    fn test_ws_error_display() {
        let err = WsError::AuthFailed("bad token".into());
        assert_eq!(err.to_string(), "Authentication failed: bad token");

        let err = WsError::RoomNotFound("room-x".into());
        assert_eq!(err.to_string(), "Room not found: room-x");
    }

    #[tokio::test]
    async fn test_token_validator_success() {
        let validator = MockValidator::new(false);
        let result = validator.validate("test-token").await;
        assert!(result.is_ok());
        let token = result.expect("should be ok");
        assert_eq!(token.user_id, "user-test-token");
        assert_eq!(token.tenant_id, "tenant-1");
    }

    #[tokio::test]
    async fn test_token_validator_failure() {
        let validator = MockValidator::new(true);
        let result = validator.validate("test-token").await;
        assert!(result.is_err());
        match result {
            Err(WsError::AuthFailed(msg)) => assert_eq!(msg, "Invalid token"),
            other => panic!("Expected AuthFailed, got {other:?}"),
        }
    }
}
