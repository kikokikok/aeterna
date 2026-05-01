use crate::graph_event_log::{GraphEvent, GraphEventLog, GraphEventLogError};
use duckdb::{Connection, params};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::watch;
use tokio::time::{self, Duration};
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum ProjectorError {
    #[error("Event log error: {0}")]
    EventLog(#[from] GraphEventLogError),
    #[error("DuckDB error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("Projector not running for tenant: {0}")]
    NotRunning(String),
}

#[derive(Debug, Clone)]
pub struct ProjectorConfig {
    pub poll_interval: Duration,
    pub batch_size: i64,
    pub lag_threshold: i64,
    pub checkpoint_interval: Duration,
}

impl Default for ProjectorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            batch_size: 1000,
            lag_threshold: 100,
            checkpoint_interval: Duration::from_secs(10),
        }
    }
}

struct TenantProjectorState {
    last_applied_seq: i64,
    _task_handle: tokio::task::JoinHandle<()>,
}

pub struct GraphProjector {
    event_log: Arc<GraphEventLog>,
    writer: Arc<Mutex<Connection>>,
    config: ProjectorConfig,
    tenants: Arc<Mutex<HashMap<String, TenantProjectorState>>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl GraphProjector {
    pub fn new(
        event_log: Arc<GraphEventLog>,
        writer: Arc<Mutex<Connection>>,
        config: ProjectorConfig,
    ) -> Self {
        Self::ensure_projector_state_table(&writer.lock());

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            event_log,
            writer,
            config,
            tenants: Arc::new(Mutex::new(HashMap::new())),
            shutdown_tx,
            shutdown_rx,
        }
    }

    fn ensure_projector_state_table(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS graph_projector_state (
                tenant_id VARCHAR PRIMARY KEY,
                last_applied_seq BIGINT NOT NULL DEFAULT 0
            )
            "#,
        )
        .expect("Failed to create graph_projector_state table");
    }

    fn load_last_applied_seq(conn: &Connection, tenant_id: &str) -> i64 {
        conn.query_row(
            "SELECT last_applied_seq FROM graph_projector_state WHERE tenant_id = ?",
            params![tenant_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    fn save_last_applied_seq(conn: &Connection, tenant_id: &str, seq: i64) {
        conn.execute(
            r#"
            INSERT INTO graph_projector_state (tenant_id, last_applied_seq)
            VALUES (?, ?)
            ON CONFLICT (tenant_id) DO UPDATE SET last_applied_seq = excluded.last_applied_seq
            "#,
            params![tenant_id, seq],
        )
        .ok();
    }

    /// Apply a single event idempotently into DuckDB.
    /// Uses INSERT OR IGNORE keyed on (id) to prevent double-apply.
    pub fn apply_event(conn: &Connection, event: &GraphEvent) -> Result<(), duckdb::Error> {
        match event.kind.as_str() {
            "add_node" => {
                let id = event.payload["id"].as_str().unwrap_or_default();
                let label = event.payload["label"].as_str().unwrap_or_default();
                let properties = event.payload["properties"].to_string();
                let tenant_id = event.payload["tenant_id"]
                    .as_str()
                    .unwrap_or(&event.tenant_id);

                conn.execute(
                    r#"
                    INSERT OR IGNORE INTO memory_nodes (id, label, properties, tenant_id, seq)
                    VALUES (?, ?, ?, ?, ?)
                    "#,
                    params![id, label, properties, tenant_id, event.seq],
                )?;
            }
            "add_edge" => {
                let id = event.payload["id"].as_str().unwrap_or_default();
                let source_id = event.payload["source_id"].as_str().unwrap_or_default();
                let target_id = event.payload["target_id"].as_str().unwrap_or_default();
                let relation = event.payload["relation"].as_str().unwrap_or_default();
                let properties = event.payload["properties"].to_string();
                let tenant_id = event.payload["tenant_id"]
                    .as_str()
                    .unwrap_or(&event.tenant_id);

                conn.execute(
                    r#"
                    INSERT OR IGNORE INTO memory_edges (id, source_id, target_id, relation, properties, tenant_id, seq)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                    params![id, source_id, target_id, relation, properties, tenant_id, event.seq],
                )?;
            }
            "soft_delete_node" => {
                let node_id = event.payload["node_id"].as_str().unwrap_or_default();
                let tenant_id = event.payload["tenant_id"]
                    .as_str()
                    .unwrap_or(&event.tenant_id);

                conn.execute(
                    "UPDATE memory_nodes SET deleted_at = CURRENT_TIMESTAMP WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
                    params![node_id, tenant_id],
                )?;
                conn.execute(
                    "UPDATE memory_edges SET deleted_at = CURRENT_TIMESTAMP WHERE (source_id = ? OR target_id = ?) AND tenant_id = ? AND deleted_at IS NULL",
                    params![node_id, node_id, tenant_id],
                )?;
            }
            "soft_delete_edge" => {
                let edge_id = event.payload["edge_id"].as_str().unwrap_or_default();
                let tenant_id = event.payload["tenant_id"]
                    .as_str()
                    .unwrap_or(&event.tenant_id);

                conn.execute(
                    "UPDATE memory_edges SET deleted_at = CURRENT_TIMESTAMP WHERE id = ? AND tenant_id = ? AND deleted_at IS NULL",
                    params![edge_id, tenant_id],
                )?;
            }
            other => {
                warn!("Unknown event kind '{}', skipping", other);
            }
        }
        Ok(())
    }

    pub fn start_tenant(&self, tenant_id: String) {
        let mut tenants = self.tenants.lock();
        if tenants.contains_key(&tenant_id) {
            return;
        }

        let initial_seq = Self::load_last_applied_seq(&self.writer.lock(), &tenant_id);

        let event_log = Arc::clone(&self.event_log);
        let writer = Arc::clone(&self.writer);
        let config = self.config.clone();
        let tid = tenant_id.clone();
        let tenants_map = Arc::clone(&self.tenants);
        let mut shutdown_rx = self.shutdown_rx.clone();

        let handle = tokio::spawn(async move {
            let mut current_seq = initial_seq;
            let mut events_since_checkpoint = 0u64;
            let mut last_checkpoint = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            let conn = writer.lock();
                            Self::save_last_applied_seq(&conn, &tid, current_seq);
                            info!("Projector for tenant {} shutting down at seq {}", tid, current_seq);
                            break;
                        }
                    }
                    _ = time::sleep(config.poll_interval) => {
                        match event_log.tail(&tid, current_seq, config.batch_size).await {
                            Ok(events) if !events.is_empty() => {
                                let conn = writer.lock();
                                for event in &events {
                                    if let Err(e) = Self::apply_event(&conn, event) {
                                        error!("Failed to apply event seq {} for tenant {}: {}", event.seq, tid, e);
                                        break;
                                    }
                                    current_seq = event.seq;
                                    events_since_checkpoint += 1;
                                }

                                if events_since_checkpoint > 0 {
                                    if let Some(state) = tenants_map.lock().get_mut(&tid) {
                                        state.last_applied_seq = current_seq;
                                    }
                                }

                                if last_checkpoint.elapsed() >= config.checkpoint_interval {
                                    Self::save_last_applied_seq(&conn, &tid, current_seq);
                                    events_since_checkpoint = 0;
                                    last_checkpoint = tokio::time::Instant::now();
                                }

                                debug!("Projector for tenant {} at seq {}", tid, current_seq);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!("Projector tail error for tenant {}: {}", tid, e);
                            }
                        }
                    }
                }
            }
        });

        tenants.insert(
            tenant_id,
            TenantProjectorState {
                last_applied_seq: initial_seq,
                _task_handle: handle,
            },
        );
    }

    pub fn last_applied_seq(&self, tenant_id: &str) -> Result<i64, ProjectorError> {
        let tenants = self.tenants.lock();
        tenants
            .get(tenant_id)
            .map(|s| s.last_applied_seq)
            .ok_or_else(|| ProjectorError::NotRunning(tenant_id.to_string()))
    }

    /// Check if all active tenants are within the lag threshold.
    pub async fn is_ready(&self) -> bool {
        let tenant_seqs: Vec<(String, i64)> = {
            let tenants = self.tenants.lock();
            tenants
                .iter()
                .map(|(tid, state)| (tid.clone(), state.last_applied_seq))
                .collect()
        };

        for (tenant_id, applied_seq) in tenant_seqs {
            match self.event_log.head_seq(&tenant_id).await {
                Ok(head) => {
                    if head - applied_seq > self.config.lag_threshold {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }

        true
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub fn active_tenant_count(&self) -> usize {
        self.tenants.lock().len()
    }
}
