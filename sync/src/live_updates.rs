use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::websocket::{Room, WsServer};

#[derive(Debug, Error)]
pub enum LiveUpdateError {
    #[error("Broadcast failed: {0}")]
    BroadcastFailed(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Channel closed")]
    ChannelClosed,
}

pub type LiveUpdateResult<T> = Result<T, LiveUpdateError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdateEvent {
    #[serde(rename_all = "camelCase")]
    MemoryAdded {
        memory_id: String,
        layer: String,
        tenant_id: String,
        content_preview: String,
        timestamp: i64,
    },
    #[serde(rename_all = "camelCase")]
    KnowledgeChanged {
        entry_id: String,
        change_type: KnowledgeChangeType,
        tenant_id: String,
        path: String,
        timestamp: i64,
    },
    #[serde(rename_all = "camelCase")]
    PolicyUpdated {
        policy_id: String,
        action: PolicyAction,
        tenant_id: String,
        affected_layers: Vec<String>,
        timestamp: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeChangeType {
    Created,
    Updated,
    Deleted,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PolicyAction {
    Created,
    Updated,
    Activated,
    Deactivated,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveUpdate {
    pub event: UpdateEvent,
    pub target_rooms: Vec<Room>,
    pub source_client: Option<String>,
}

pub struct LiveUpdateBroadcaster {
    ws_server: Arc<WsServer>,
    event_tx: broadcast::Sender<LiveUpdate>,
}

impl LiveUpdateBroadcaster {
    pub fn new(ws_server: Arc<WsServer>) -> Self {
        let (event_tx, _) = broadcast::channel(4096);
        Self {
            ws_server,
            event_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LiveUpdate> {
        self.event_tx.subscribe()
    }

    pub async fn broadcast_memory_added(
        &self,
        memory_id: String,
        layer: String,
        tenant_id: String,
        content_preview: String,
    ) -> LiveUpdateResult<usize> {
        let event = UpdateEvent::MemoryAdded {
            memory_id,
            layer: layer.clone(),
            tenant_id: tenant_id.clone(),
            content_preview,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let room = format!("tenant:{tenant_id}:layer:{layer}");
        self.broadcast_event(event, vec![room]).await
    }

    pub async fn broadcast_knowledge_changed(
        &self,
        entry_id: String,
        change_type: KnowledgeChangeType,
        tenant_id: String,
        path: String,
    ) -> LiveUpdateResult<usize> {
        let event = UpdateEvent::KnowledgeChanged {
            entry_id,
            change_type,
            tenant_id: tenant_id.clone(),
            path,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let room = format!("tenant:{tenant_id}:knowledge");
        self.broadcast_event(event, vec![room]).await
    }

    pub async fn broadcast_policy_updated(
        &self,
        policy_id: String,
        action: PolicyAction,
        tenant_id: String,
        affected_layers: Vec<String>,
    ) -> LiveUpdateResult<usize> {
        let rooms: Vec<Room> = affected_layers
            .iter()
            .map(|layer| format!("tenant:{tenant_id}:layer:{layer}"))
            .chain(std::iter::once(format!("tenant:{tenant_id}:policy")))
            .collect();

        let event = UpdateEvent::PolicyUpdated {
            policy_id,
            action,
            tenant_id,
            affected_layers,
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.broadcast_event(event, rooms).await
    }

    async fn broadcast_event(
        &self,
        event: UpdateEvent,
        target_rooms: Vec<Room>,
    ) -> LiveUpdateResult<usize> {
        let payload = serde_json::to_value(&event)?;

        let live_update = LiveUpdate {
            event,
            target_rooms: target_rooms.clone(),
            source_client: None,
        };

        let _ = self.event_tx.send(live_update);

        let mut total_sent = 0usize;
        for room in &target_rooms {
            match self
                .ws_server
                .broadcast_to_room(room, payload.clone())
                .await
            {
                Ok(count) => total_sent += count,
                Err(e) => {
                    tracing::warn!("Failed to broadcast to room {room}: {e}");
                }
            }
        }

        Ok(total_sent)
    }
}

pub fn tenant_layer_room(tenant_id: &str, layer: &str) -> Room {
    format!("tenant:{tenant_id}:layer:{layer}")
}

pub fn tenant_knowledge_room(tenant_id: &str) -> Room {
    format!("tenant:{tenant_id}:knowledge")
}

pub fn tenant_policy_room(tenant_id: &str) -> Room {
    format!("tenant:{tenant_id}:policy")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::{AuthToken, TokenValidator, WsError, WsResult};

    struct MockValidator;

    #[async_trait::async_trait]
    impl TokenValidator for MockValidator {
        async fn validate(&self, _token: &str) -> WsResult<AuthToken> {
            Err(WsError::AuthFailed("not used".into()))
        }
    }

    fn make_broadcaster() -> LiveUpdateBroadcaster {
        let ws_server = Arc::new(WsServer::new(Arc::new(MockValidator)));
        LiveUpdateBroadcaster::new(ws_server)
    }

    #[test]
    fn test_update_event_serialization() {
        let event = UpdateEvent::MemoryAdded {
            memory_id: "mem-123".into(),
            layer: "project".into(),
            tenant_id: "tenant-1".into(),
            content_preview: "Use PostgreSQL...".into(),
            timestamp: 1704067200,
        };

        let json = serde_json::to_string(&event).expect("serialize should succeed");
        let deserialized: UpdateEvent =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_knowledge_change_event_serialization() {
        let event = UpdateEvent::KnowledgeChanged {
            entry_id: "entry-456".into(),
            change_type: KnowledgeChangeType::Updated,
            tenant_id: "tenant-1".into(),
            path: "adr/001-database.md".into(),
            timestamp: 1704067200,
        };

        let json = serde_json::to_string(&event).expect("serialize should succeed");
        let deserialized: UpdateEvent =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_policy_update_event_serialization() {
        let event = UpdateEvent::PolicyUpdated {
            policy_id: "policy-789".into(),
            action: PolicyAction::Activated,
            tenant_id: "tenant-1".into(),
            affected_layers: vec!["project".into(), "team".into()],
            timestamp: 1704067200,
        };

        let json = serde_json::to_string(&event).expect("serialize should succeed");
        let deserialized: UpdateEvent =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(event, deserialized);
    }

    #[tokio::test]
    async fn test_broadcast_memory_added() {
        let broadcaster = make_broadcaster();

        let result = broadcaster
            .broadcast_memory_added(
                "mem-1".into(),
                "project".into(),
                "tenant-1".into(),
                "Test content".into(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("should be ok"), 0);
    }

    #[tokio::test]
    async fn test_broadcast_knowledge_changed() {
        let broadcaster = make_broadcaster();

        let result = broadcaster
            .broadcast_knowledge_changed(
                "entry-1".into(),
                KnowledgeChangeType::Created,
                "tenant-1".into(),
                "adr/new.md".into(),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_policy_updated() {
        let broadcaster = make_broadcaster();

        let result = broadcaster
            .broadcast_policy_updated(
                "policy-1".into(),
                PolicyAction::Updated,
                "tenant-1".into(),
                vec!["project".into(), "team".into()],
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_event_subscriber_receives() {
        let broadcaster = make_broadcaster();
        let mut rx = broadcaster.subscribe();

        broadcaster
            .broadcast_memory_added(
                "mem-1".into(),
                "project".into(),
                "tenant-1".into(),
                "Preview".into(),
            )
            .await
            .expect("broadcast should succeed");

        let update = rx.recv().await.expect("should receive update");
        match update.event {
            UpdateEvent::MemoryAdded { memory_id, .. } => {
                assert_eq!(memory_id, "mem-1");
            }
            other => panic!("Expected MemoryAdded, got {other:?}"),
        }
        assert_eq!(update.target_rooms, vec!["tenant:tenant-1:layer:project"]);
    }

    #[test]
    fn test_room_name_helpers() {
        assert_eq!(
            tenant_layer_room("acme", "project"),
            "tenant:acme:layer:project"
        );
        assert_eq!(tenant_knowledge_room("acme"), "tenant:acme:knowledge");
        assert_eq!(tenant_policy_room("acme"), "tenant:acme:policy");
    }

    #[test]
    fn test_knowledge_change_type_variants() {
        let types = vec![
            KnowledgeChangeType::Created,
            KnowledgeChangeType::Updated,
            KnowledgeChangeType::Deleted,
            KnowledgeChangeType::Merged,
        ];
        for ct in types {
            let json = serde_json::to_string(&ct).expect("serialize should succeed");
            let deserialized: KnowledgeChangeType =
                serde_json::from_str(&json).expect("deserialize should succeed");
            assert_eq!(ct, deserialized);
        }
    }

    #[test]
    fn test_policy_action_variants() {
        let actions = vec![
            PolicyAction::Created,
            PolicyAction::Updated,
            PolicyAction::Activated,
            PolicyAction::Deactivated,
            PolicyAction::Deleted,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).expect("serialize should succeed");
            let deserialized: PolicyAction =
                serde_json::from_str(&json).expect("deserialize should succeed");
            assert_eq!(action, deserialized);
        }
    }

    #[test]
    fn test_live_update_serialization() {
        let update = LiveUpdate {
            event: UpdateEvent::MemoryAdded {
                memory_id: "mem-1".into(),
                layer: "project".into(),
                tenant_id: "t-1".into(),
                content_preview: "test".into(),
                timestamp: 1234,
            },
            target_rooms: vec!["room-1".into()],
            source_client: Some("client-x".into()),
        };
        let json = serde_json::to_string(&update).expect("serialize should succeed");
        let deserialized: LiveUpdate =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(update, deserialized);
    }
}
