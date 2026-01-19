use mk_core::types::{ErrorSignature, HindsightNote, Resolution};
use storage::postgres::{PostgresBackend, PostgresError};

use super::{
    BuildResult, MetaAgentFailureReport, MetaAgentLoopResult, MetaAgentSuccessReport,
    MetaAgentTelemetry, TestResult,
};

#[derive(Debug, Clone)]
pub struct ResultHandlingConfig {
    pub commit_message_template: String,
    pub pr_hint_template: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ResultHandlingError {
    #[error("Storage error: {0}")]
    Storage(#[from] PostgresError),

    #[error("Missing failure details")]
    MissingFailureDetails,
}

impl Default for ResultHandlingConfig {
    fn default() -> Self {
        Self {
            commit_message_template: "chore: apply changes for {summary}".to_string(),
            pr_hint_template: "Create PR for {summary}".to_string(),
        }
    }
}

pub struct ResultHandler {
    config: ResultHandlingConfig,
    telemetry: crate::meta_agent::MetaAgentTelemetrySink,
    storage: Option<std::sync::Arc<PostgresBackend>>,
}

#[derive(Debug, Clone)]
pub struct FailureContext {
    pub tenant_id: String,
    pub signature: ErrorSignature,
    pub resolutions: Vec<Resolution>,
}

#[derive(Debug, Clone)]
pub enum ResultHandlingOutcome {
    Success(MetaAgentSuccessReport),
    Failure {
        report: MetaAgentFailureReport,
        hindsight: Option<HindsightNote>,
    },
}

impl ResultHandler {
    pub fn new(config: ResultHandlingConfig) -> Self {
        Self {
            config,
            telemetry: crate::meta_agent::MetaAgentTelemetrySink,
            storage: None,
        }
    }

    pub fn with_storage(mut self, storage: std::sync::Arc<PostgresBackend>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn handle_success(&self, build: &BuildResult, test: &TestResult) -> MetaAgentSuccessReport {
        self.handle_success_with_iterations(build, test, 1)
    }

    pub fn handle_success_with_iterations(
        &self,
        build: &BuildResult,
        test: &TestResult,
        iterations: u32,
    ) -> MetaAgentSuccessReport {
        let summary = summarize(build, test, iterations);
        let commit_message_hint = self
            .config
            .commit_message_template
            .replace("{summary}", &summary);
        let pr_hint = self.config.pr_hint_template.replace("{summary}", &summary);

        MetaAgentSuccessReport {
            summary,
            commit_message_hint,
            pr_hint,
        }
    }

    pub fn handle_failure(&self, result: &MetaAgentLoopResult) -> MetaAgentFailureReport {
        let summary = failure_summary(result);
        let detailed_report = match result {
            MetaAgentLoopResult::Failure { state } => {
                let build = state.last_build.as_ref();
                let test = state.last_test.as_ref();
                let improve = state.last_improve.as_ref();
                let mut report = String::new();
                report.push_str("Iterations: ");
                report.push_str(&state.iterations.to_string());
                report.push_str("\n\n");
                if let Some(build) = build {
                    report.push_str("Build output:\n");
                    report.push_str(&build.output);
                    report.push_str("\n\n");
                }
                if let Some(test) = test {
                    report.push_str("Test output:\n");
                    report.push_str(&test.output);
                    report.push_str("\n\n");
                }
                if let Some(improve) = improve {
                    report.push_str("Improve analysis:\n");
                    report.push_str(&improve.analysis);
                }
                report
            }
            _ => String::new(),
        };

        MetaAgentFailureReport {
            summary,
            detailed_report,
        }
    }

    pub fn record_telemetry(&self, telemetry: MetaAgentTelemetry) {
        self.telemetry.record(&telemetry);
    }

    pub async fn store_failure_hindsight(
        &self,
        tenant_id: &str,
        signature: ErrorSignature,
        resolutions: Vec<Resolution>,
        report: &MetaAgentFailureReport,
    ) -> Result<HindsightNote, ResultHandlingError> {
        let note = self.build_failure_hindsight(signature, resolutions, report);
        let storage = self
            .storage
            .as_ref()
            .ok_or(ResultHandlingError::MissingFailureDetails)?;
        storage.create_hindsight_note(tenant_id, &note).await?;
        Ok(note)
    }

    pub fn emit_telemetry(&self, result: &MetaAgentLoopResult) {
        match result {
            MetaAgentLoopResult::Success { iterations, .. } => {
                self.telemetry.record(&MetaAgentTelemetry {
                    iterations: *iterations,
                    success: true,
                });
            }
            MetaAgentLoopResult::Failure { state } => {
                self.telemetry.record(&MetaAgentTelemetry {
                    iterations: state.iterations,
                    success: false,
                });
            }
        }
    }

    pub async fn handle_result(
        &self,
        result: &MetaAgentLoopResult,
        failure: Option<FailureContext>,
    ) -> Result<ResultHandlingOutcome, ResultHandlingError> {
        self.emit_telemetry(result);
        match result {
            MetaAgentLoopResult::Success {
                build,
                test,
                iterations,
            } => {
                let report = self.handle_success_with_iterations(build, test, *iterations);
                Ok(ResultHandlingOutcome::Success(report))
            }
            MetaAgentLoopResult::Failure { .. } => {
                let report = self.handle_failure(result);
                let hindsight = if let Some(failure) = failure {
                    Some(
                        self.store_failure_hindsight(
                            &failure.tenant_id,
                            failure.signature,
                            failure.resolutions,
                            &report,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                Ok(ResultHandlingOutcome::Failure { report, hindsight })
            }
        }
    }

    pub fn build_failure_hindsight(
        &self,
        signature: ErrorSignature,
        resolutions: Vec<mk_core::types::Resolution>,
        report: &MetaAgentFailureReport,
    ) -> HindsightNote {
        let now = chrono::Utc::now().timestamp();
        HindsightNote {
            id: uuid::Uuid::new_v4().to_string(),
            error_signature: signature,
            resolutions,
            content: report.detailed_report.clone(),
            tags: vec!["meta-agent".to_string(), "failure".to_string()],
            created_at: now,
            updated_at: now,
        }
    }
}

fn summarize(build: &BuildResult, test: &TestResult, iterations: u32) -> String {
    let mut summary = String::new();
    summary.push_str("iterations: ");
    summary.push_str(&iterations.to_string());
    summary.push_str(", status: ");
    summary.push_str(match test.status {
        super::TestStatus::Pass => "pass",
        super::TestStatus::Fail => "fail",
        super::TestStatus::Timeout => "timeout",
    });
    if !build.notes.is_empty() {
        summary.push_str(", notes");
    }
    summary
}

fn failure_summary(result: &MetaAgentLoopResult) -> String {
    match result {
        MetaAgentLoopResult::Failure { state } => {
            format!("iterations: {}, failed", state.iterations)
        }
        _ => "".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::types::{ErrorSignature, Resolution};

    #[test]
    fn test_success_report() {
        let handler = ResultHandler::new(ResultHandlingConfig::default());
        let build = BuildResult {
            output: "done".to_string(),
            notes: vec!["note".to_string()],
            hindsight: vec![],
            tokens_used: 10,
        };
        let test = TestResult {
            status: super::super::TestStatus::Pass,
            output: "ok".to_string(),
            duration_ms: 1,
        };
        let report = handler.handle_success_with_iterations(&build, &test, 2);
        assert!(report.commit_message_hint.contains("iterations: 2"));
        assert!(report.pr_hint.contains("iterations: 2"));
    }

    #[test]
    fn test_failure_report() {
        let handler = ResultHandler::new(ResultHandlingConfig::default());
        let mut state = super::super::MetaAgentLoopState::default();
        state.iterations = 2;
        state.last_build = Some(BuildResult {
            output: "build".to_string(),
            notes: vec![],
            hindsight: vec![],
            tokens_used: 1,
        });
        state.last_test = Some(TestResult {
            status: super::super::TestStatus::Fail,
            output: "fail".to_string(),
            duration_ms: 1,
        });
        let result = MetaAgentLoopResult::Failure { state };
        let report = handler.handle_failure(&result);
        assert!(report.detailed_report.contains("Build output"));
    }

    #[test]
    fn test_failure_hindsight_note() {
        let handler = ResultHandler::new(ResultHandlingConfig::default());
        let signature = ErrorSignature {
            error_type: "BuildError".to_string(),
            message_pattern: "fail".to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: None,
        };
        let report = MetaAgentFailureReport {
            summary: "failed".to_string(),
            detailed_report: "details".to_string(),
        };
        let note = handler.build_failure_hindsight(
            signature,
            vec![Resolution {
                id: "r".to_string(),
                error_signature_id: "e".to_string(),
                description: "fix".to_string(),
                changes: vec![],
                success_rate: 0.0,
                application_count: 0,
                last_success_at: 0,
            }],
            &report,
        );
        assert!(note.content.contains("details"));
    }

    #[tokio::test]
    async fn test_handle_result_missing_failure_context() {
        let handler = ResultHandler::new(ResultHandlingConfig::default());
        let state = super::super::MetaAgentLoopState::default();
        let result = MetaAgentLoopResult::Failure { state };
        let outcome = handler.handle_result(&result, None).await.unwrap();
        match outcome {
            ResultHandlingOutcome::Failure { report, hindsight } => {
                assert!(report.summary.contains("failed"));
                assert!(hindsight.is_none());
            }
            _ => panic!("Expected failure outcome"),
        }
    }

    #[test]
    fn test_emit_telemetry_success() {
        let handler = ResultHandler::new(ResultHandlingConfig::default());
        let build = BuildResult {
            output: "done".to_string(),
            notes: vec![],
            hindsight: vec![],
            tokens_used: 1,
        };
        let test = TestResult {
            status: super::super::TestStatus::Pass,
            output: "ok".to_string(),
            duration_ms: 1,
        };
        let result = MetaAgentLoopResult::Success {
            build,
            test,
            iterations: 1,
        };
        handler.emit_telemetry(&result);
    }
}
