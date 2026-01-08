use crate::tools::Tool;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use sync::bridge::SyncManager;
use validator::Validate;

pub struct SyncNowTool {
    sync_manager: Arc<SyncManager>,
}

impl SyncNowTool {
    pub fn new(sync_manager: Arc<SyncManager>) -> Self {
        Self { sync_manager }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct SyncNowparams {
    #[serde(default)]
    pub force: bool,
}

#[async_trait]
impl Tool for SyncNowTool {
    fn name(&self) -> &str {
        "sync_now"
    }

    fn description(&self) -> &str {
        "Trigger manual synchronization between memory and knowledge systems."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": {
                    "type": "boolean",
                    "description": "Force full sync (ignore delta detection)",
                    "default": false
                }
            }
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: SyncNowparams = serde_json::from_value(params)?;
        p.validate()?;

        if p.force {
            self.sync_manager.sync_all().await?;
        } else {
            self.sync_manager.sync_incremental().await?;
        }

        Ok(json!({
            "success": true,
            "message": "Synchronization completed"
        }))
    }
}

pub struct SyncStatusTool {
    sync_manager: Arc<SyncManager>,
}

impl SyncStatusTool {
    pub fn new(sync_manager: Arc<SyncManager>) -> Self {
        Self { sync_manager }
    }
}

#[async_trait]
impl Tool for SyncStatusTool {
    fn name(&self) -> &str {
        "sync_status"
    }

    fn description(&self) -> &str {
        "Check the current sync status, including last sync time and health."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.sync_manager.get_state().await;

        Ok(json!({
            "success": true,
            "healthy": state.failed_items.is_empty(),
            "lastSyncAt": state.last_sync_at,
            "failedItems": state.failed_items.len(),
            "stats": {
                "totalSyncs": state.stats.total_syncs,
                "totalItemsSynced": state.stats.total_items_synced,
                "avgSyncDurationMs": state.stats.avg_sync_duration_ms
            }
        }))
    }
}
