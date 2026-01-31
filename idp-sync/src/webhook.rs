use crate::config::IdpSyncConfig;
use crate::error::{IdpSyncError, IdpSyncResult};
use crate::okta::IdpClient;
use crate::sync::IdpSyncService;
use axum::{
    Router,
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::post
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

pub struct WebhookServer {
    config: IdpSyncConfig,
    sync_service: Arc<IdpSyncService>,
    client: Arc<dyn IdpClient>
}

#[derive(Clone)]
struct AppState {
    webhook_secret: Option<String>,
    sync_service: Arc<IdpSyncService>,
    client: Arc<dyn IdpClient>
}

impl WebhookServer {
    pub fn new(
        config: IdpSyncConfig,
        sync_service: Arc<IdpSyncService>,
        client: Arc<dyn IdpClient>
    ) -> Self {
        Self {
            config,
            sync_service,
            client
        }
    }

    pub async fn run(&self) -> IdpSyncResult<()> {
        let state = AppState {
            webhook_secret: self.config.webhook_secret.clone(),
            sync_service: self.sync_service.clone(),
            client: self.client.clone()
        };

        let app = Router::new()
            .route("/webhooks/okta", post(handle_okta_webhook))
            .route("/webhooks/azure", post(handle_azure_webhook))
            .route("/health", axum::routing::get(health_check))
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.config.webhook_port);
        info!(addr = %addr, "Starting webhook server");

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| IdpSyncError::ConfigError(format!("Failed to bind: {}", e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| IdpSyncError::ConfigError(format!("Server error: {}", e)))?;

        Ok(())
    }
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
struct OktaWebhookPayload {
    #[serde(rename = "eventType")]
    event_type: String,
    data: OktaEventData
}

#[derive(Debug, Deserialize)]
struct OktaEventData {
    events: Vec<OktaEvent>
}

#[derive(Debug, Deserialize)]
struct OktaEvent {
    #[serde(rename = "eventType")]
    event_type: String,
    target: Vec<OktaTarget>,
    published: DateTime<Utc>
}

#[derive(Debug, Deserialize)]
struct OktaTarget {
    id: String,
    #[serde(rename = "type")]
    target_type: String,
    #[serde(rename = "alternateId")]
    alternate_id: Option<String>
}

async fn handle_okta_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<OktaWebhookPayload>
) -> StatusCode {
    if let Some(ref secret) = state.webhook_secret {
        if !verify_okta_signature(&headers, secret) {
            warn!("Invalid Okta webhook signature");
            return StatusCode::UNAUTHORIZED;
        }
    }

    debug!(event_type = %payload.event_type, "Received Okta webhook");

    for event in payload.data.events {
        let result = process_okta_event(&state, &event).await;
        if let Err(e) = result {
            error!(error = %e, event_type = %event.event_type, "Failed to process Okta event");
        }
    }

    StatusCode::OK
}

async fn process_okta_event(state: &AppState, event: &OktaEvent) -> IdpSyncResult<()> {
    match event.event_type.as_str() {
        "user.lifecycle.create" | "user.lifecycle.activate" => {
            for target in &event.target {
                if target.target_type == "User" {
                    info!(user_id = %target.id, "Processing user create/activate event");
                    if let Ok(user) = state.client.get_user(&target.id).await {
                        info!(email = %user.email, "User synced from webhook");
                    }
                }
            }
        }
        "user.lifecycle.deactivate" | "user.lifecycle.suspend" => {
            for target in &event.target {
                if target.target_type == "User" {
                    info!(user_id = %target.id, "Processing user deactivate/suspend event");
                }
            }
        }
        "group.user_membership.add" => {
            info!("Processing group membership add event");
        }
        "group.user_membership.remove" => {
            info!("Processing group membership remove event");
        }
        _ => {
            debug!(event_type = %event.event_type, "Ignoring unhandled event type");
        }
    }

    Ok(())
}

fn verify_okta_signature(headers: &HeaderMap, secret: &str) -> bool {
    let signature = match headers.get("x-okta-request-signature") {
        Some(sig) => match sig.to_str() {
            Ok(s) => s,
            Err(_) => return false
        },
        None => return false
    };

    let timestamp = match headers.get("x-okta-request-timestamp") {
        Some(ts) => match ts.to_str() {
            Ok(s) => s,
            Err(_) => return false
        },
        None => return false
    };

    let expected = compute_okta_signature(secret, timestamp);
    signature == expected
}

fn compute_okta_signature(secret: &str, timestamp: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(timestamp.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Deserialize)]
struct AzureWebhookPayload {
    value: Vec<AzureNotification>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzureNotification {
    change_type: String,
    resource: String,
    resource_data: AzureResourceData
}

#[derive(Debug, Deserialize)]
struct AzureResourceData {
    id: String,
    #[serde(rename = "@odata.type")]
    odata_type: Option<String>
}

async fn handle_azure_webhook(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(payload): Json<AzureWebhookPayload>
) -> StatusCode {
    debug!("Received Azure AD webhook");

    for notification in payload.value {
        let result = process_azure_notification(&state, &notification).await;
        if let Err(e) = result {
            error!(error = %e, change_type = %notification.change_type, "Failed to process Azure notification");
        }
    }

    StatusCode::OK
}

async fn process_azure_notification(
    state: &AppState,
    notification: &AzureNotification
) -> IdpSyncResult<()> {
    let resource_type = notification
        .resource_data
        .odata_type
        .as_deref()
        .unwrap_or("unknown");

    match (notification.change_type.as_str(), resource_type) {
        ("created", "#microsoft.graph.user") | ("updated", "#microsoft.graph.user") => {
            info!(
                user_id = %notification.resource_data.id,
                change_type = %notification.change_type,
                "Processing user change"
            );
            if let Ok(user) = state.client.get_user(&notification.resource_data.id).await {
                info!(email = %user.email, "User synced from webhook");
            }
        }
        ("deleted", "#microsoft.graph.user") => {
            info!(
                user_id = %notification.resource_data.id,
                "Processing user deletion"
            );
        }
        _ => {
            debug!(
                change_type = %notification.change_type,
                resource_type = %resource_type,
                "Ignoring unhandled notification"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_okta_signature() {
        let secret = "test-secret";
        let timestamp = "1234567890";
        let sig = compute_okta_signature(secret, timestamp);
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn test_okta_payload_deserialization() {
        let json = r#"{
            "eventType": "com.okta.event_hook",
            "data": {
                "events": [
                    {
                        "eventType": "user.lifecycle.create",
                        "target": [
                            {
                                "id": "00u123",
                                "type": "User",
                                "alternateId": "test@example.com"
                            }
                        ],
                        "published": "2024-01-01T00:00:00Z"
                    }
                ]
            }
        }"#;

        let payload: OktaWebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.event_type, "com.okta.event_hook");
        assert_eq!(payload.data.events.len(), 1);
    }
}
