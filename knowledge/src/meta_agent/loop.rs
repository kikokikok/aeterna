use super::{
    BuildPhase, BuildResult, ImproveAction, ImprovePhase, ImproveResult, MetaAgentConfig,
    QualityGateConfig, QualityGateEvaluator, QualityGateSummary, TestCommand, TestPhase,
    TestResult, TestStatus, TimeBudget, TimeBudgetConfig, TimeBudgetExhaustedResult
};
use tracing::{Instrument, info_span, warn};

#[derive(Debug, Clone)]
pub struct MetaAgentLoopState {
    pub iterations: u32,
    pub last_build: Option<BuildResult>,
    pub last_test: Option<TestResult>,
    pub last_improve: Option<ImproveResult>
}

impl Default for MetaAgentLoopState {
    fn default() -> Self {
        Self {
            iterations: 0,
            last_build: None,
            last_test: None,
            last_improve: None
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetaAgentLoopStateExtended {
    pub iterations: u32,
    pub last_build: Option<BuildResult>,
    pub last_test: Option<TestResult>,
    pub last_improve: Option<ImproveResult>,
    pub quality_gates: Option<QualityGateSummary>
}

#[derive(Debug, Clone)]
pub enum MetaAgentLoopResult {
    Success {
        build: BuildResult,
        test: TestResult,
        iterations: u32
    },
    Failure {
        state: MetaAgentLoopState
    }
}

#[derive(Debug, Clone)]
pub enum MetaAgentLoopResultExtended {
    Success {
        build: BuildResult,
        test: TestResult,
        quality_gates: QualityGateSummary,
        iterations: u32
    },
    QualityGateFailure {
        build: BuildResult,
        test: TestResult,
        quality_gates: QualityGateSummary,
        iterations: u32
    },
    Failure {
        state: MetaAgentLoopStateExtended
    },
    TimeBudgetExhausted {
        exhausted: TimeBudgetExhaustedResult,
        state: MetaAgentLoopStateExtended
    }
}

impl MetaAgentLoopResultExtended {
    pub fn is_success(&self) -> bool {
        matches!(self, MetaAgentLoopResultExtended::Success { .. })
    }

    pub fn can_commit(&self) -> bool {
        matches!(self, MetaAgentLoopResultExtended::Success { quality_gates, .. } if quality_gates.all_passed)
    }

    pub fn quality_gates(&self) -> Option<&QualityGateSummary> {
        match self {
            MetaAgentLoopResultExtended::Success { quality_gates, .. } => Some(quality_gates),
            MetaAgentLoopResultExtended::QualityGateFailure { quality_gates, .. } => {
                Some(quality_gates)
            }
            MetaAgentLoopResultExtended::Failure { state } => state.quality_gates.as_ref(),
            MetaAgentLoopResultExtended::TimeBudgetExhausted { state, .. } => {
                state.quality_gates.as_ref()
            }
        }
    }
}

impl MetaAgentLoopResult {
    pub fn escalation_message(&self) -> Option<String> {
        match self {
            MetaAgentLoopResult::Failure { state } => state
                .last_improve
                .as_ref()
                .and_then(|improve| improve.escalation_message.clone()),
            _ => None
        }
    }
}

pub struct MetaAgentLoop<C: crate::context_architect::LlmClient> {
    build_phase: BuildPhase<C>,
    test_phase: TestPhase,
    improve_phase: ImprovePhase<C>,
    config: MetaAgentConfig
}

impl<C: crate::context_architect::LlmClient> MetaAgentLoop<C> {
    pub fn new(
        build_phase: BuildPhase<C>,
        test_phase: TestPhase,
        improve_phase: ImprovePhase<C>,
        config: MetaAgentConfig
    ) -> Self {
        Self {
            build_phase,
            test_phase,
            improve_phase,
            config
        }
    }

    pub async fn run(
        &self,
        requirements: &str,
        test_command: &TestCommand,
        context: Option<&str>
    ) -> Result<MetaAgentLoopResult, crate::context_architect::LlmError> {
        let span = info_span!(
            "meta_agent_loop",
            requirements_len = requirements.len(),
            has_context = context.is_some(),
            max_iterations = self.config.max_iterations
        );

        async move {
            let mut state = MetaAgentLoopState::default();

            while state.iterations < self.config.max_iterations {
                let iteration_span = info_span!(
                    "meta_agent_iteration",
                    iteration = state.iterations + 1,
                    max_iterations = self.config.max_iterations
                );

                let _guard = iteration_span.enter();

                let build = self.build_phase.execute(requirements, context).await?;
                let test = self.test_phase.execute(test_command).await;

                state.iterations += 1;
                state.last_build = Some(build.clone());
                state.last_test = Some(test.clone());

                if test.status == TestStatus::Pass {
                    return Ok(MetaAgentLoopResult::Success {
                        build,
                        test,
                        iterations: state.iterations
                    });
                }

                let improve = self.improve_phase.execute(&test).await?;
                state.last_improve = Some(improve.clone());

                if improve.action == ImproveAction::Escalate {
                    return Ok(MetaAgentLoopResult::Failure { state });
                }
            }

            Ok(MetaAgentLoopResult::Failure { state })
        }
        .instrument(span)
        .await
    }
}

pub struct MetaAgentLoopWithBudget<C: crate::context_architect::LlmClient> {
    build_phase: BuildPhase<C>,
    test_phase: TestPhase,
    improve_phase: ImprovePhase<C>,
    config: MetaAgentConfig,
    time_budget_config: TimeBudgetConfig,
    quality_gate_config: QualityGateConfig
}

impl<C: crate::context_architect::LlmClient> MetaAgentLoopWithBudget<C> {
    pub fn new(
        build_phase: BuildPhase<C>,
        test_phase: TestPhase,
        improve_phase: ImprovePhase<C>,
        config: MetaAgentConfig,
        time_budget_config: TimeBudgetConfig,
        quality_gate_config: QualityGateConfig
    ) -> Self {
        Self {
            build_phase,
            test_phase,
            improve_phase,
            config,
            time_budget_config,
            quality_gate_config
        }
    }

    pub async fn run(
        &self,
        requirements: &str,
        test_command: &TestCommand,
        context: Option<&str>
    ) -> Result<MetaAgentLoopResultExtended, crate::context_architect::LlmError> {
        let span = info_span!(
            "meta_agent_loop_with_budget",
            requirements_len = requirements.len(),
            has_context = context.is_some(),
            max_iterations = self.config.max_iterations,
            budget_secs = self.time_budget_config.total_duration.as_secs()
        );

        async move {
            let mut budget = TimeBudget::start(self.time_budget_config.clone());
            let quality_evaluator = QualityGateEvaluator::new(self.quality_gate_config.clone());

            let mut state = MetaAgentLoopStateExtended {
                iterations: 0,
                last_build: None,
                last_test: None,
                last_improve: None,
                quality_gates: None
            };

            while state.iterations < self.config.max_iterations {
                let budget_check = budget.check();
                if budget_check.is_exhausted() {
                    let exhausted =
                        TimeBudgetExhaustedResult::new(budget_check.elapsed, state.iterations);
                    return Ok(MetaAgentLoopResultExtended::TimeBudgetExhausted {
                        exhausted,
                        state
                    });
                }

                if budget_check.is_warning() {
                    warn!(
                        iterations = state.iterations,
                        remaining_secs = budget_check.remaining.as_secs(),
                        "Time budget warning - limiting remaining iterations"
                    );
                }

                let iteration_span = info_span!(
                    "meta_agent_iteration",
                    iteration = state.iterations + 1,
                    max_iterations = self.config.max_iterations,
                    budget_remaining_secs = budget.remaining().as_secs()
                );

                let _guard = iteration_span.enter();

                let build = self.build_phase.execute(requirements, context).await?;

                let budget_check = budget.check();
                if budget_check.is_exhausted() {
                    state.last_build = Some(build);
                    let exhausted =
                        TimeBudgetExhaustedResult::new(budget_check.elapsed, state.iterations)
                            .with_partial_results("Build completed, test not started");
                    return Ok(MetaAgentLoopResultExtended::TimeBudgetExhausted {
                        exhausted,
                        state
                    });
                }

                let test = self.test_phase.execute(test_command).await;

                state.iterations += 1;
                state.last_build = Some(build.clone());
                state.last_test = Some(test.clone());

                if test.status == TestStatus::Pass {
                    let quality_gates = quality_evaluator.evaluate_all(true).await;
                    state.quality_gates = Some(quality_gates.clone());

                    if quality_evaluator.can_commit(&quality_gates) {
                        return Ok(MetaAgentLoopResultExtended::Success {
                            build,
                            test,
                            quality_gates,
                            iterations: state.iterations
                        });
                    } else {
                        return Ok(MetaAgentLoopResultExtended::QualityGateFailure {
                            build,
                            test,
                            quality_gates,
                            iterations: state.iterations
                        });
                    }
                }

                let budget_check = budget.check();
                if budget_check.is_exhausted() {
                    let exhausted =
                        TimeBudgetExhaustedResult::new(budget_check.elapsed, state.iterations)
                            .with_partial_results("Tests failed, improve phase not started");
                    return Ok(MetaAgentLoopResultExtended::TimeBudgetExhausted {
                        exhausted,
                        state
                    });
                }

                let improve = self.improve_phase.execute(&test).await?;
                state.last_improve = Some(improve.clone());

                if improve.action == ImproveAction::Escalate {
                    return Ok(MetaAgentLoopResultExtended::Failure { state });
                }
            }

            Ok(MetaAgentLoopResultExtended::Failure { state })
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_architect::LlmError;
    use crate::meta_agent::{BuildPhaseConfig, ImprovePhaseConfig, TestPhaseConfig};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct MockLlmClient {
        responses: Mutex<Vec<String>>
    }

    impl MockLlmClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(responses)
            }
        }
    }

    #[async_trait]
    impl crate::context_architect::LlmClient for MockLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }

        async fn complete_with_system(
            &self,
            _system: &str,
            _user: &str
        ) -> Result<String, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }
    }

    #[tokio::test]
    async fn test_meta_agent_success() {
        let client = Arc::new(MockLlmClient::new(vec![
            "improve".to_string(),
            "build".to_string(),
        ]));
        let build_phase = BuildPhase::new(client.clone(), BuildPhaseConfig::default());
        let improve_phase = ImprovePhase::new(client, ImprovePhaseConfig::default());
        let test_phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });

        let loop_runner = MetaAgentLoop::new(
            build_phase,
            test_phase,
            improve_phase,
            MetaAgentConfig {
                max_iterations: 1,
                ..Default::default()
            }
        );

        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 0".to_string()]);
        let result = loop_runner.run("req", &command, None).await.unwrap();

        match result {
            MetaAgentLoopResult::Success { iterations, .. } => {
                assert_eq!(iterations, 1);
            }
            _ => panic!("Expected success")
        }
    }

    #[tokio::test]
    async fn test_loop_with_budget_success() {
        let client = Arc::new(MockLlmClient::new(vec!["build".to_string()]));
        let build_phase = BuildPhase::new(client.clone(), BuildPhaseConfig::default());
        let improve_phase = ImprovePhase::new(client, ImprovePhaseConfig::default());
        let test_phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });

        let loop_runner = MetaAgentLoopWithBudget::new(
            build_phase,
            test_phase,
            improve_phase,
            MetaAgentConfig {
                max_iterations: 3,
                ..Default::default()
            },
            TimeBudgetConfig::default().with_duration_secs(60),
            QualityGateConfig::default()
        );

        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 0".to_string()]);
        let result = loop_runner.run("req", &command, None).await.unwrap();

        assert!(result.is_success());
        assert!(result.can_commit());
        assert!(result.quality_gates().is_some());
    }

    #[tokio::test]
    async fn test_loop_with_budget_quality_gate_failure() {
        let client = Arc::new(MockLlmClient::new(vec!["build".to_string()]));
        let build_phase = BuildPhase::new(client.clone(), BuildPhaseConfig::default());
        let improve_phase = ImprovePhase::new(client, ImprovePhaseConfig::default());
        let test_phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });

        let loop_runner = MetaAgentLoopWithBudget::new(
            build_phase,
            test_phase,
            improve_phase,
            MetaAgentConfig {
                max_iterations: 3,
                ..Default::default()
            },
            TimeBudgetConfig::default().with_duration_secs(60),
            QualityGateConfig::default()
                .with_linter(super::super::LinterConfig {
                    program: "sh".to_string(),
                    args: vec!["-c".to_string(), "exit 1".to_string()],
                    timeout_secs: 2
                })
                .require_all()
        );

        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 0".to_string()]);
        let result = loop_runner.run("req", &command, None).await.unwrap();

        assert!(!result.can_commit());
        match result {
            MetaAgentLoopResultExtended::QualityGateFailure { quality_gates, .. } => {
                assert!(!quality_gates.all_passed);
                assert!(quality_gates.tests_passed());
                assert_eq!(quality_gates.linter_passed(), Some(false));
            }
            _ => panic!("Expected QualityGateFailure")
        }
    }

    #[tokio::test]
    async fn test_loop_with_budget_time_exhausted() {
        let client = Arc::new(MockLlmClient::new(vec!["build".to_string()]));
        let build_phase = BuildPhase::new(client.clone(), BuildPhaseConfig::default());
        let improve_phase = ImprovePhase::new(client, ImprovePhaseConfig::default());
        let test_phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });

        let loop_runner = MetaAgentLoopWithBudget::new(
            build_phase,
            test_phase,
            improve_phase,
            MetaAgentConfig {
                max_iterations: 10,
                ..Default::default()
            },
            TimeBudgetConfig::default().with_duration_secs(0),
            QualityGateConfig::default()
        );

        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 0".to_string()]);
        let result = loop_runner.run("req", &command, None).await.unwrap();

        match result {
            MetaAgentLoopResultExtended::TimeBudgetExhausted { exhausted, .. } => {
                assert_eq!(exhausted.iterations_completed, 0);
            }
            _ => panic!("Expected TimeBudgetExhausted")
        }
    }

    #[tokio::test]
    async fn test_result_extended_helpers() {
        let quality_gates =
            QualityGateSummary::from_results(vec![super::super::QualityGateResult::pass(
                super::super::QualityGateType::Tests,
                "pass",
                0
            )]);

        let success = MetaAgentLoopResultExtended::Success {
            build: BuildResult {
                output: "out".to_string(),
                notes: vec![],
                hindsight: vec![],
                tokens_used: 0
            },
            test: TestResult {
                status: TestStatus::Pass,
                output: "pass".to_string(),
                duration_ms: 100
            },
            quality_gates: quality_gates.clone(),
            iterations: 1
        };

        assert!(success.is_success());
        assert!(success.can_commit());
        assert!(success.quality_gates().is_some());
    }
}
