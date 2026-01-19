use std::time::Instant;

use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::info_span;

use super::{TestCommand, TestResult, TestStatus};

#[derive(Debug, Clone)]
pub struct TestPhaseConfig {
    pub timeout_secs: u64,
}

impl Default for TestPhaseConfig {
    fn default() -> Self {
        Self { timeout_secs: 300 }
    }
}

pub struct TestPhase {
    config: TestPhaseConfig,
}

impl TestPhase {
    pub fn new(config: TestPhaseConfig) -> Self {
        Self { config }
    }

    pub async fn execute(&self, command: &TestCommand) -> TestResult {
        let span = info_span!(
            "test_phase",
            program = %command.program,
            args_count = command.args.len(),
            timeout_secs = self.config.timeout_secs
        );

        let _guard = span.enter();

        let start = Instant::now();
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args);

        let output =
            match timeout(Duration::from_secs(self.config.timeout_secs), cmd.output()).await {
                Ok(result) => match result {
                    Ok(out) => out,
                    Err(err) => {
                        return TestResult {
                            status: TestStatus::Fail,
                            output: format!("Failed to run tests: {err}"),
                            duration_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                },
                Err(_) => {
                    return TestResult {
                        status: TestStatus::Timeout,
                        output: "Test run timed out".to_string(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };

        let duration_ms = start.elapsed().as_millis() as u64;
        let status = if output.status.success() {
            TestStatus::Pass
        } else {
            TestStatus::Fail
        };

        TestResult {
            status,
            output: String::from_utf8_lossy(&output.stdout).to_string()
                + &String::from_utf8_lossy(&output.stderr),
            duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_phase_pass() {
        let phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });
        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 0".to_string()]);

        let result = phase.execute(&command).await;
        assert_eq!(result.status, TestStatus::Pass);
    }

    #[tokio::test]
    async fn test_test_phase_fail() {
        let phase = TestPhase::new(TestPhaseConfig { timeout_secs: 2 });
        let command = TestCommand::new("sh", vec!["-c".to_string(), "exit 1".to_string()]);

        let result = phase.execute(&command).await;
        assert_eq!(result.status, TestStatus::Fail);
    }
}
