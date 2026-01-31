use crate::config::IdpSyncConfig;
use crate::error::{IdpSyncError, IdpSyncResult};
use crate::sync::{IdpSyncService, SyncReport};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

pub struct SyncScheduler {
    scheduler: JobScheduler,
    sync_service: Arc<IdpSyncService>,
    last_report: Arc<RwLock<Option<SyncReport>>>
}

impl SyncScheduler {
    pub async fn new(sync_service: IdpSyncService, config: &IdpSyncConfig) -> IdpSyncResult<Self> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| IdpSyncError::SchedulerError(e.to_string()))?;

        let sync_service = Arc::new(sync_service);
        let last_report = Arc::new(RwLock::new(None));

        let cron_expression = format!("0 */{} * * * *", config.sync_interval_seconds / 60);

        let service_clone = sync_service.clone();
        let report_clone = last_report.clone();

        let job = Job::new_async(cron_expression.as_str(), move |_uuid, _lock| {
            let service = service_clone.clone();
            let report = report_clone.clone();
            Box::pin(async move {
                info!("Starting scheduled IdP sync");
                match service.sync_all().await {
                    Ok(sync_report) => {
                        info!(
                            users_created = sync_report.users_created,
                            users_updated = sync_report.users_updated,
                            "Scheduled sync completed"
                        );
                        let mut guard = report.write().await;
                        *guard = Some(sync_report);
                    }
                    Err(e) => {
                        error!(error = %e, "Scheduled sync failed");
                    }
                }
            })
        })
        .map_err(|e| IdpSyncError::SchedulerError(e.to_string()))?;

        scheduler
            .add(job)
            .await
            .map_err(|e| IdpSyncError::SchedulerError(e.to_string()))?;

        Ok(Self {
            scheduler,
            sync_service,
            last_report
        })
    }

    pub async fn start(&self) -> IdpSyncResult<()> {
        self.scheduler
            .start()
            .await
            .map_err(|e| IdpSyncError::SchedulerError(e.to_string()))?;
        info!("IdP sync scheduler started");
        Ok(())
    }

    pub async fn stop(&mut self) -> IdpSyncResult<()> {
        self.scheduler
            .shutdown()
            .await
            .map_err(|e| IdpSyncError::SchedulerError(e.to_string()))?;
        info!("IdP sync scheduler stopped");
        Ok(())
    }

    pub async fn run_now(&self) -> IdpSyncResult<SyncReport> {
        let report = self.sync_service.sync_all().await?;
        let mut guard = self.last_report.write().await;
        *guard = Some(report.clone());
        Ok(report)
    }

    pub async fn last_report(&self) -> Option<SyncReport> {
        self.last_report.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cron_expression_generation() {
        let interval_seconds = 300u64;
        let cron = format!("0 */{} * * * *", interval_seconds / 60);
        assert_eq!(cron, "0 */5 * * * *");
    }
}
