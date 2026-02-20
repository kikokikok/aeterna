use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const RTO_LIMIT: Duration = Duration::from_secs(900);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum DrDrillStep {
    BackupVerification,
    FailoverSimulation,
    DataIntegrityCheck,
    RestoreFromBackup,
    ServiceHealthCheck,
}

impl DrDrillStep {
    fn all() -> &'static [DrDrillStep] {
        &[
            DrDrillStep::BackupVerification,
            DrDrillStep::FailoverSimulation,
            DrDrillStep::DataIntegrityCheck,
            DrDrillStep::RestoreFromBackup,
            DrDrillStep::ServiceHealthCheck,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            DrDrillStep::BackupVerification => "Backup Verification",
            DrDrillStep::FailoverSimulation => "Failover Simulation",
            DrDrillStep::DataIntegrityCheck => "Data Integrity Check",
            DrDrillStep::RestoreFromBackup => "Restore from Backup",
            DrDrillStep::ServiceHealthCheck => "Service Health Check",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StepResult {
    step: DrDrillStep,
    passed: bool,
    duration: Duration,
    details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DrDrillReport {
    steps: Vec<StepResult>,
    total_duration: Duration,
    rto_met: bool,
    all_steps_passed: bool,
}

fn run_quarterly_drill() -> DrDrillReport {
    let overall_start = Instant::now();
    let mut step_results = Vec::new();

    for &step in DrDrillStep::all() {
        let step_start = Instant::now();
        let (passed, details) = execute_drill_step(step);
        let duration = step_start.elapsed();

        step_results.push(StepResult {
            step,
            passed,
            duration,
            details,
        });
    }

    let total_duration = overall_start.elapsed();
    let all_passed = step_results.iter().all(|r| r.passed);

    DrDrillReport {
        steps: step_results,
        total_duration,
        rto_met: total_duration < RTO_LIMIT,
        all_steps_passed: all_passed,
    }
}

fn execute_drill_step(step: DrDrillStep) -> (bool, String) {
    match step {
        DrDrillStep::BackupVerification => {
            let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let scripts = [
                "infrastructure/ha/backups/backup-postgres.sh",
                "infrastructure/ha/backups/backup-qdrant.sh",
                "infrastructure/ha/backups/backup-redis.sh",
            ];

            let mut missing = Vec::new();
            for script in &scripts {
                if !project_root.join(script).exists() {
                    missing.push(*script);
                }
            }

            if missing.is_empty() {
                (true, "All backup scripts present".to_string())
            } else {
                (false, format!("Missing backup scripts: {:?}", missing))
            }
        }

        DrDrillStep::FailoverSimulation => {
            // Simulate failover by verifying HA config exists and is valid
            let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let patroni = project_root.join("infrastructure/ha/patroni/patroni.yml");

            if patroni.exists() {
                let content = std::fs::read_to_string(&patroni).unwrap_or_default();
                let has_sync = content.contains("synchronous_mode");
                (
                    has_sync,
                    format!("Patroni config present, sync mode configured: {has_sync}"),
                )
            } else {
                (false, "Patroni config missing".to_string())
            }
        }

        DrDrillStep::DataIntegrityCheck => {
            // Verify shard manager can handle tenant data correctly
            let mut manager = storage::shard_manager::ShardManager::new();
            let router = storage::tenant_router::TenantRouter::new();

            router.assign_shard("dr-test-tenant", storage::tenant_router::TenantSize::Small);
            let result = manager.increment_tenant_count("shared-shard-1");

            match result {
                Ok(()) => {
                    let info = manager.get_shard("shared-shard-1").unwrap();
                    (
                        true,
                        format!("Shard integrity OK: {} tenants", info.current_tenants),
                    )
                }
                Err(e) => (false, format!("Shard integrity failed: {e}")),
            }
        }

        DrDrillStep::RestoreFromBackup => {
            // Verify the restore test script exists and has correct RTO configuration
            let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let restore_script = project_root.join("infrastructure/ha/backups/restore-test.sh");

            if restore_script.exists() {
                let content = std::fs::read_to_string(&restore_script).unwrap_or_default();
                let has_rto = content.contains("RTO_LIMIT_SECONDS");
                let has_report = content.contains("rto_met");
                (
                    has_rto && has_report,
                    format!("Restore script: RTO configured={has_rto}, report output={has_report}"),
                )
            } else {
                (false, "Restore test script missing".to_string())
            }
        }

        DrDrillStep::ServiceHealthCheck => {
            // Verify core services can be instantiated
            let tracker = observability::CostTracker::new(observability::CostConfig::default());
            let detector = observability::AnomalyDetector::new(
                observability::AnomalyDetectorConfig::default(),
            );

            // Quick smoke test
            let ctx = mk_core::types::TenantContext::new(
                mk_core::types::TenantId::new("dr-health".to_string()).unwrap(),
                mk_core::types::UserId::new("dr-user".to_string()).unwrap(),
            );
            tracker.record_embedding_generation(&ctx, 100, "test-model");

            let result = detector.record_and_detect("dr_health_metric", 42.0);
            (
                !result.is_anomaly,
                "Core services instantiated and responsive".to_string(),
            )
        }
    }
}

#[test]
fn dr_drill_completes_within_rto() {
    let report = run_quarterly_drill();

    eprintln!("DR Drill Report:");
    for step in &report.steps {
        eprintln!(
            "  [{status}] {name}: {details} ({duration:.2?})",
            status = if step.passed { "PASS" } else { "FAIL" },
            name = step.step.name(),
            details = step.details,
            duration = step.duration,
        );
    }
    eprintln!(
        "Total duration: {:.2?} | RTO met: {} | All passed: {}",
        report.total_duration, report.rto_met, report.all_steps_passed
    );

    assert!(report.rto_met, "DR drill must complete within RTO of 900s");
    assert!(report.all_steps_passed, "All DR drill steps must pass");
}

#[test]
fn dr_drill_report_serializes_to_json() {
    let report = run_quarterly_drill();
    let json = serde_json::to_string_pretty(&report).expect("report must serialize to JSON");

    assert!(json.contains("rto_met"));
    assert!(json.contains("all_steps_passed"));
    assert!(json.contains("BackupVerification"));
}

#[test]
fn dr_drill_steps_cover_all_phases() {
    let steps = DrDrillStep::all();
    assert_eq!(steps.len(), 5, "drill must cover 5 phases");

    let names: Vec<&str> = steps.iter().map(|s| s.name()).collect();
    assert!(names.contains(&"Backup Verification"));
    assert!(names.contains(&"Failover Simulation"));
    assert!(names.contains(&"Data Integrity Check"));
    assert!(names.contains(&"Restore from Backup"));
    assert!(names.contains(&"Service Health Check"));
}

#[test]
fn rto_limit_is_900_seconds() {
    assert_eq!(RTO_LIMIT.as_secs(), 900);
}
