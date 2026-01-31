use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{error, info};

use crate::persistence::ThreadRepository;

pub struct CleanupJob {
    repository: Arc<ThreadRepository>,
    interval: Duration
}

impl CleanupJob {
    pub fn new(repository: Arc<ThreadRepository>, interval_secs: u64) -> Self {
        Self {
            repository,
            interval: Duration::from_secs(interval_secs)
        }
    }

    pub async fn start(&self) {
        let mut interval = interval(self.interval);
        let repository = self.repository.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                match repository.delete_expired_threads().await {
                    Ok(count) => {
                        if count > 0 {
                            info!("Cleaned up {} expired threads", count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to clean up expired threads: {}", e);
                    }
                }
            }
        });
    }
}
