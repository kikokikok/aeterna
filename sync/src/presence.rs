use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::websocket::{ClientId, Room};

pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 90;

#[derive(Debug, Error)]
pub enum PresenceError {
    #[error("Client not found: {0}")]
    ClientNotFound(ClientId),
    #[error("Broadcast error: {0}")]
    Broadcast(String),
}

pub type PresenceResult<T> = Result<T, PresenceError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PresenceState {
    Online,
    Away,
    Offline,
}

impl std::fmt::Display for PresenceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "online"),
            Self::Away => write!(f, "away"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientPresence {
    pub client_id: ClientId,
    pub user_id: String,
    pub state: PresenceState,
    pub last_heartbeat: i64,
    pub connected_at: i64,
    pub rooms: HashSet<Room>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PresenceEvent {
    #[serde(rename_all = "camelCase")]
    Connected {
        client_id: ClientId,
        user_id: String,
        room: Room,
    },
    #[serde(rename_all = "camelCase")]
    Disconnected {
        client_id: ClientId,
        user_id: String,
        room: Room,
    },
    #[serde(rename_all = "camelCase")]
    StateChanged {
        client_id: ClientId,
        user_id: String,
        previous: PresenceState,
        current: PresenceState,
    },
}

pub struct PresenceTracker {
    presences: Arc<DashMap<ClientId, ClientPresence>>,
    event_tx: broadcast::Sender<PresenceEvent>,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
}

impl PresenceTracker {
    pub fn new() -> Self {
        Self::with_intervals(
            Duration::from_secs(HEARTBEAT_INTERVAL_SECS),
            Duration::from_secs(HEARTBEAT_TIMEOUT_SECS),
        )
    }

    pub fn with_intervals(heartbeat_interval: Duration, heartbeat_timeout: Duration) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            presences: Arc::new(DashMap::new()),
            event_tx,
            heartbeat_interval,
            heartbeat_timeout,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PresenceEvent> {
        self.event_tx.subscribe()
    }

    pub fn track_connect(
        &self,
        client_id: ClientId,
        user_id: String,
        room: Room,
    ) -> PresenceResult<()> {
        let now = Utc::now().timestamp();

        let mut rooms = HashSet::new();
        rooms.insert(room.clone());

        if let Some(mut existing) = self.presences.get_mut(&client_id) {
            existing.rooms.insert(room.clone());
            existing.state = PresenceState::Online;
            existing.last_heartbeat = now;
        } else {
            let presence = ClientPresence {
                client_id,
                user_id: user_id.clone(),
                state: PresenceState::Online,
                last_heartbeat: now,
                connected_at: now,
                rooms,
            };
            self.presences.insert(client_id, presence);
        }

        let _ = self.event_tx.send(PresenceEvent::Connected {
            client_id,
            user_id,
            room,
        });

        Ok(())
    }

    pub fn track_disconnect(&self, client_id: ClientId, room: &str) -> PresenceResult<()> {
        let user_id = if let Some(mut presence) = self.presences.get_mut(&client_id) {
            presence.rooms.remove(room);
            let uid = presence.user_id.clone();
            if presence.rooms.is_empty() {
                presence.state = PresenceState::Offline;
            }
            uid
        } else {
            return Err(PresenceError::ClientNotFound(client_id));
        };

        if self
            .presences
            .get(&client_id)
            .is_some_and(|p| p.rooms.is_empty())
        {
            self.presences.remove(&client_id);
        }

        let _ = self.event_tx.send(PresenceEvent::Disconnected {
            client_id,
            user_id,
            room: room.to_string(),
        });

        Ok(())
    }

    pub fn record_heartbeat(&self, client_id: ClientId) -> PresenceResult<()> {
        let mut presence = self
            .presences
            .get_mut(&client_id)
            .ok_or(PresenceError::ClientNotFound(client_id))?;
        let now = Utc::now().timestamp();
        let previous_state = presence.state;
        presence.last_heartbeat = now;

        if previous_state == PresenceState::Away {
            presence.state = PresenceState::Online;
            let uid = presence.user_id.clone();
            drop(presence);
            let _ = self.event_tx.send(PresenceEvent::StateChanged {
                client_id,
                user_id: uid,
                previous: PresenceState::Away,
                current: PresenceState::Online,
            });
        }

        Ok(())
    }

    pub fn check_heartbeats(&self) -> Vec<PresenceEvent> {
        let now = Utc::now().timestamp();
        let timeout_secs = self.heartbeat_timeout.as_secs() as i64;
        let away_secs = (self.heartbeat_interval.as_secs() * 2) as i64;
        let mut events = Vec::new();

        let client_ids: Vec<ClientId> = self.presences.iter().map(|r| *r.key()).collect();

        for client_id in client_ids {
            if let Some(mut presence) = self.presences.get_mut(&client_id) {
                let elapsed = now - presence.last_heartbeat;

                if elapsed > timeout_secs && presence.state != PresenceState::Offline {
                    let prev = presence.state;
                    presence.state = PresenceState::Offline;
                    events.push(PresenceEvent::StateChanged {
                        client_id,
                        user_id: presence.user_id.clone(),
                        previous: prev,
                        current: PresenceState::Offline,
                    });
                } else if elapsed > away_secs && presence.state == PresenceState::Online {
                    presence.state = PresenceState::Away;
                    events.push(PresenceEvent::StateChanged {
                        client_id,
                        user_id: presence.user_id.clone(),
                        previous: PresenceState::Online,
                        current: PresenceState::Away,
                    });
                }
            }
        }

        for event in &events {
            let _ = self.event_tx.send(event.clone());
        }

        events
    }

    pub async fn start_heartbeat_checker(
        self: &Arc<Self>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let tracker = Arc::clone(self);
        let interval = tracker.heartbeat_interval;

        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        tracker.check_heartbeats();
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Heartbeat checker shutting down");
                        break;
                    }
                }
            }
        });
    }

    pub fn get_presence(&self, client_id: &ClientId) -> Option<ClientPresence> {
        self.presences.get(client_id).map(|r| r.clone())
    }

    pub fn get_room_presences(&self, room: &str) -> Vec<ClientPresence> {
        self.presences
            .iter()
            .filter(|r| r.rooms.contains(room))
            .map(|r| r.clone())
            .collect()
    }

    pub fn online_count(&self) -> usize {
        self.presences
            .iter()
            .filter(|r| r.state == PresenceState::Online)
            .count()
    }

    pub fn total_tracked(&self) -> usize {
        self.presences.len()
    }
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client_id() -> ClientId {
        Uuid::new_v4()
    }

    #[test]
    fn test_presence_tracker_creation() {
        let tracker = PresenceTracker::new();
        assert_eq!(tracker.online_count(), 0);
        assert_eq!(tracker.total_tracked(), 0);
    }

    #[test]
    fn test_track_connect_and_disconnect() {
        let tracker = PresenceTracker::new();
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("connect should succeed");

        assert_eq!(tracker.online_count(), 1);
        assert_eq!(tracker.total_tracked(), 1);

        let presence = tracker.get_presence(&client_id).expect("should exist");
        assert_eq!(presence.state, PresenceState::Online);
        assert!(presence.rooms.contains("room-a"));

        tracker
            .track_disconnect(client_id, "room-a")
            .expect("disconnect should succeed");

        assert_eq!(tracker.online_count(), 0);
        assert_eq!(tracker.total_tracked(), 0);
    }

    #[test]
    fn test_track_connect_multiple_rooms() {
        let tracker = PresenceTracker::new();
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("connect should succeed");
        tracker
            .track_connect(client_id, "user-1".into(), "room-b".into())
            .expect("connect should succeed");

        let presence = tracker.get_presence(&client_id).expect("should exist");
        assert_eq!(presence.rooms.len(), 2);

        tracker
            .track_disconnect(client_id, "room-a")
            .expect("disconnect should succeed");
        assert_eq!(tracker.total_tracked(), 1);

        tracker
            .track_disconnect(client_id, "room-b")
            .expect("disconnect should succeed");
        assert_eq!(tracker.total_tracked(), 0);
    }

    #[test]
    fn test_disconnect_unknown_client() {
        let tracker = PresenceTracker::new();
        let result = tracker.track_disconnect(make_client_id(), "room-a");
        assert!(result.is_err());
    }

    #[test]
    fn test_heartbeat_recording() {
        let tracker = PresenceTracker::new();
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("connect should succeed");

        let result = tracker.record_heartbeat(client_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_heartbeat_unknown_client() {
        let tracker = PresenceTracker::new();
        let result = tracker.record_heartbeat(make_client_id());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_heartbeats_marks_away() {
        let tracker =
            PresenceTracker::with_intervals(Duration::from_secs(1), Duration::from_secs(5));
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("connect should succeed");

        if let Some(mut p) = tracker.presences.get_mut(&client_id) {
            p.last_heartbeat = Utc::now().timestamp() - 3;
        }

        let events = tracker.check_heartbeats();
        assert_eq!(events.len(), 1);
        match &events[0] {
            PresenceEvent::StateChanged {
                previous, current, ..
            } => {
                assert_eq!(*previous, PresenceState::Online);
                assert_eq!(*current, PresenceState::Away);
            }
            other => panic!("Expected StateChanged, got {other:?}"),
        }
    }

    #[test]
    fn test_check_heartbeats_marks_offline() {
        let tracker =
            PresenceTracker::with_intervals(Duration::from_secs(1), Duration::from_secs(5));
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("connect should succeed");

        if let Some(mut p) = tracker.presences.get_mut(&client_id) {
            p.last_heartbeat = Utc::now().timestamp() - 10;
        }

        let events = tracker.check_heartbeats();
        assert_eq!(events.len(), 1);
        match &events[0] {
            PresenceEvent::StateChanged {
                previous, current, ..
            } => {
                assert_eq!(*previous, PresenceState::Online);
                assert_eq!(*current, PresenceState::Offline);
            }
            other => panic!("Expected StateChanged, got {other:?}"),
        }
    }

    #[test]
    fn test_get_room_presences() {
        let tracker = PresenceTracker::new();
        let c1 = make_client_id();
        let c2 = make_client_id();
        let c3 = make_client_id();

        tracker
            .track_connect(c1, "user-1".into(), "room-a".into())
            .expect("ok");
        tracker
            .track_connect(c2, "user-2".into(), "room-a".into())
            .expect("ok");
        tracker
            .track_connect(c3, "user-3".into(), "room-b".into())
            .expect("ok");

        let room_a = tracker.get_room_presences("room-a");
        assert_eq!(room_a.len(), 2);

        let room_b = tracker.get_room_presences("room-b");
        assert_eq!(room_b.len(), 1);
    }

    #[test]
    fn test_presence_state_display() {
        assert_eq!(PresenceState::Online.to_string(), "online");
        assert_eq!(PresenceState::Away.to_string(), "away");
        assert_eq!(PresenceState::Offline.to_string(), "offline");
    }

    #[test]
    fn test_presence_event_serialization() {
        let event = PresenceEvent::Connected {
            client_id: Uuid::nil(),
            user_id: "user-1".into(),
            room: "room-a".into(),
        };
        let json = serde_json::to_string(&event).expect("serialize should succeed");
        let deserialized: PresenceEvent =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_heartbeat_restores_online_from_away() {
        let tracker =
            PresenceTracker::with_intervals(Duration::from_secs(1), Duration::from_secs(10));
        let client_id = make_client_id();

        tracker
            .track_connect(client_id, "user-1".into(), "room-a".into())
            .expect("ok");

        if let Some(mut p) = tracker.presences.get_mut(&client_id) {
            p.state = PresenceState::Away;
        }

        tracker.record_heartbeat(client_id).expect("ok");

        let presence = tracker.get_presence(&client_id).expect("should exist");
        assert_eq!(presence.state, PresenceState::Online);
    }
}
