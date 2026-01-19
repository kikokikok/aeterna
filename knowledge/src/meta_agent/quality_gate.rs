use std::time::Instant;

use tokio::process::Command;
use tokio::time::{Duration, timeout};
use tracing::{info_span, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityGateType {
    Tests,
    Linter,
    Coverage,
}

impl std::fmt::Display for QualityGateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityGateType::Tests => write!(f, "tests"),
            QualityGateType::Linter => write!(f, "linter"),
            QualityGateType::Coverage => write!(f, "coverage"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub gate_type: QualityGateType,
    pub passed: bool,
    pub message: String,
    pub duration_ms: u64,
}

impl QualityGateResult {
    pub fn pass(gate_type: QualityGateType, message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            gate_type,
            passed: true,
            message: message.into(),
            duration_ms,
        }
    }

    pub fn fail(gate_type: QualityGateType, message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            gate_type,
            passed: false,
            message: message.into(),
            duration_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinterConfig {
    pub program: String,
    pub args: Vec<String>,
    pub timeout_secs: u64,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            program: "cargo".to_string(),
            args: vec![
                "clippy".to_string(),
                "--".to_string(),
                "-D".to_string(),
                "warnings".to_string(),
            ],
            timeout_secs: 120,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoverageConfig {
    pub program: String,
    pub args: Vec<String>,
    pub threshold_percent: f64,
    pub timeout_secs: u64,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            program: "cargo".to_string(),
            args: vec![
                "tarpaulin".to_string(),
                "--out".to_string(),
                "Json".to_string(),
            ],
            threshold_percent: 80.0,
            timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct QualityGateConfig {
    pub linter: Option<LinterConfig>,
    pub coverage: Option<CoverageConfig>,
    pub require_all_gates: bool,
}

impl QualityGateConfig {
    pub fn with_linter(mut self, config: LinterConfig) -> Self {
        self.linter = Some(config);
        self
    }

    pub fn with_coverage(mut self, config: CoverageConfig) -> Self {
        self.coverage = Some(config);
        self
    }

    pub fn require_all(mut self) -> Self {
        self.require_all_gates = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct QualityGateSummary {
    pub gates: Vec<QualityGateResult>,
    pub all_passed: bool,
    pub total_duration_ms: u64,
}

impl QualityGateSummary {
    pub fn from_results(gates: Vec<QualityGateResult>) -> Self {
        let all_passed = gates.iter().all(|g| g.passed);
        let total_duration_ms = gates.iter().map(|g| g.duration_ms).sum();
        Self {
            gates,
            all_passed,
            total_duration_ms,
        }
    }

    pub fn tests_passed(&self) -> bool {
        self.gates
            .iter()
            .find(|g| g.gate_type == QualityGateType::Tests)
            .map(|g| g.passed)
            .unwrap_or(false)
    }

    pub fn linter_passed(&self) -> Option<bool> {
        self.gates
            .iter()
            .find(|g| g.gate_type == QualityGateType::Linter)
            .map(|g| g.passed)
    }

    pub fn coverage_passed(&self) -> Option<bool> {
        self.gates
            .iter()
            .find(|g| g.gate_type == QualityGateType::Coverage)
            .map(|g| g.passed)
    }

    pub fn failed_gates(&self) -> Vec<&QualityGateResult> {
        self.gates.iter().filter(|g| !g.passed).collect()
    }

    pub fn format_summary(&self) -> String {
        let mut lines = vec!["Quality Gate Summary:".to_string()];
        for gate in &self.gates {
            let status = if gate.passed { "✓" } else { "✗" };
            lines.push(format!(
                "  {} {} - {} ({}ms)",
                status, gate.gate_type, gate.message, gate.duration_ms
            ));
        }
        lines.push(format!(
            "  Overall: {} (total: {}ms)",
            if self.all_passed { "PASSED" } else { "FAILED" },
            self.total_duration_ms
        ));
        lines.join("\n")
    }
}

pub struct QualityGateEvaluator {
    config: QualityGateConfig,
}

impl QualityGateEvaluator {
    pub fn new(config: QualityGateConfig) -> Self {
        Self { config }
    }

    pub fn mark_tests_result(&self, tests_passed: bool) -> QualityGateResult {
        if tests_passed {
            QualityGateResult::pass(QualityGateType::Tests, "All tests passed", 0)
        } else {
            QualityGateResult::fail(QualityGateType::Tests, "Tests failed", 0)
        }
    }

    pub async fn run_linter(&self) -> Option<QualityGateResult> {
        let linter_config = self.config.linter.as_ref()?;

        let span = info_span!(
            "quality_gate_linter",
            program = %linter_config.program,
            timeout_secs = linter_config.timeout_secs
        );
        let _guard = span.enter();

        let start = Instant::now();
        let mut cmd = Command::new(&linter_config.program);
        cmd.args(&linter_config.args);

        let result = timeout(
            Duration::from_secs(linter_config.timeout_secs),
            cmd.output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                if output.status.success() {
                    Some(QualityGateResult::pass(
                        QualityGateType::Linter,
                        "Linter passed with no warnings",
                        duration_ms,
                    ))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let message = if stderr.len() > 200 {
                        format!("Linter failed: {}...", &stderr[..200])
                    } else {
                        format!("Linter failed: {}", stderr)
                    };
                    Some(QualityGateResult::fail(
                        QualityGateType::Linter,
                        message,
                        duration_ms,
                    ))
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to run linter: {}", e);
                Some(QualityGateResult::fail(
                    QualityGateType::Linter,
                    format!("Failed to run linter: {e}"),
                    duration_ms,
                ))
            }
            Err(_) => Some(QualityGateResult::fail(
                QualityGateType::Linter,
                "Linter timed out",
                duration_ms,
            )),
        }
    }

    pub async fn run_coverage(&self) -> Option<QualityGateResult> {
        let coverage_config = self.config.coverage.as_ref()?;

        let span = info_span!(
            "quality_gate_coverage",
            program = %coverage_config.program,
            threshold = coverage_config.threshold_percent,
            timeout_secs = coverage_config.timeout_secs
        );
        let _guard = span.enter();

        let start = Instant::now();
        let mut cmd = Command::new(&coverage_config.program);
        cmd.args(&coverage_config.args);

        let result = timeout(
            Duration::from_secs(coverage_config.timeout_secs),
            cmd.output(),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let coverage = self.parse_coverage_output(&stdout);

                match coverage {
                    Some(pct) if pct >= coverage_config.threshold_percent => {
                        Some(QualityGateResult::pass(
                            QualityGateType::Coverage,
                            format!(
                                "Coverage {:.1}% >= {:.1}% threshold",
                                pct, coverage_config.threshold_percent
                            ),
                            duration_ms,
                        ))
                    }
                    Some(pct) => Some(QualityGateResult::fail(
                        QualityGateType::Coverage,
                        format!(
                            "Coverage {:.1}% < {:.1}% threshold",
                            pct, coverage_config.threshold_percent
                        ),
                        duration_ms,
                    )),
                    None => {
                        if output.status.success() {
                            Some(QualityGateResult::pass(
                                QualityGateType::Coverage,
                                "Coverage check passed (no percentage parsed)",
                                duration_ms,
                            ))
                        } else {
                            Some(QualityGateResult::fail(
                                QualityGateType::Coverage,
                                "Coverage check failed",
                                duration_ms,
                            ))
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to run coverage: {}", e);
                Some(QualityGateResult::fail(
                    QualityGateType::Coverage,
                    format!("Failed to run coverage: {e}"),
                    duration_ms,
                ))
            }
            Err(_) => Some(QualityGateResult::fail(
                QualityGateType::Coverage,
                "Coverage check timed out",
                duration_ms,
            )),
        }
    }

    fn parse_coverage_output(&self, output: &str) -> Option<f64> {
        // Try to parse coverage percentage from common formats:
        // - "Coverage: 85.5%"
        // - '"coverage": 85.5' (JSON)
        // - "85.50% coverage"
        for line in output.lines() {
            let line = line.trim();

            // JSON format: "coverage": 85.5
            if line.contains("\"coverage\"") || line.contains("\"line_coverage\"") {
                if let Some(idx) = line.find(':') {
                    let value_part = line[idx + 1..]
                        .trim()
                        .trim_matches(|c| c == ',' || c == '"');
                    if let Ok(pct) = value_part.parse::<f64>() {
                        return Some(pct);
                    }
                }
            }

            // "Coverage: 85.5%"
            if line.to_lowercase().contains("coverage") {
                for part in line.split_whitespace() {
                    let clean = part.trim_end_matches('%').trim_end_matches(',');
                    if let Ok(pct) = clean.parse::<f64>() {
                        return Some(pct);
                    }
                }
            }
        }

        None
    }

    pub async fn evaluate_all(&self, tests_passed: bool) -> QualityGateSummary {
        let span = info_span!("quality_gate_evaluate_all", tests_passed = tests_passed);
        let _guard = span.enter();

        let mut results = vec![self.mark_tests_result(tests_passed)];

        if let Some(linter_result) = self.run_linter().await {
            results.push(linter_result);
        }

        if let Some(coverage_result) = self.run_coverage().await {
            results.push(coverage_result);
        }

        QualityGateSummary::from_results(results)
    }

    pub fn can_commit(&self, summary: &QualityGateSummary) -> bool {
        if self.config.require_all_gates {
            summary.all_passed
        } else {
            summary.tests_passed()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_gate_result_pass() {
        let result = QualityGateResult::pass(QualityGateType::Tests, "All pass", 100);
        assert!(result.passed);
        assert_eq!(result.gate_type, QualityGateType::Tests);
        assert_eq!(result.duration_ms, 100);
    }

    #[test]
    fn test_quality_gate_result_fail() {
        let result = QualityGateResult::fail(QualityGateType::Linter, "Warnings found", 50);
        assert!(!result.passed);
        assert_eq!(result.gate_type, QualityGateType::Linter);
    }

    #[test]
    fn test_quality_gate_summary_all_passed() {
        let results = vec![
            QualityGateResult::pass(QualityGateType::Tests, "pass", 100),
            QualityGateResult::pass(QualityGateType::Linter, "pass", 50),
        ];
        let summary = QualityGateSummary::from_results(results);
        assert!(summary.all_passed);
        assert_eq!(summary.total_duration_ms, 150);
    }

    #[test]
    fn test_quality_gate_summary_some_failed() {
        let results = vec![
            QualityGateResult::pass(QualityGateType::Tests, "pass", 100),
            QualityGateResult::fail(QualityGateType::Linter, "fail", 50),
        ];
        let summary = QualityGateSummary::from_results(results);
        assert!(!summary.all_passed);
        assert!(summary.tests_passed());
        assert_eq!(summary.linter_passed(), Some(false));
    }

    #[test]
    fn test_quality_gate_summary_failed_gates() {
        let results = vec![
            QualityGateResult::pass(QualityGateType::Tests, "pass", 100),
            QualityGateResult::fail(QualityGateType::Linter, "fail", 50),
            QualityGateResult::fail(QualityGateType::Coverage, "low", 200),
        ];
        let summary = QualityGateSummary::from_results(results);
        let failed = summary.failed_gates();
        assert_eq!(failed.len(), 2);
    }

    #[test]
    fn test_evaluator_mark_tests_result() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let pass = evaluator.mark_tests_result(true);
        assert!(pass.passed);
        assert_eq!(pass.gate_type, QualityGateType::Tests);

        let fail = evaluator.mark_tests_result(false);
        assert!(!fail.passed);
    }

    #[test]
    fn test_can_commit_require_all() {
        let config = QualityGateConfig::default().require_all();
        let evaluator = QualityGateEvaluator::new(config);

        let all_pass = QualityGateSummary::from_results(vec![QualityGateResult::pass(
            QualityGateType::Tests,
            "pass",
            0,
        )]);
        assert!(evaluator.can_commit(&all_pass));

        let some_fail = QualityGateSummary::from_results(vec![
            QualityGateResult::pass(QualityGateType::Tests, "pass", 0),
            QualityGateResult::fail(QualityGateType::Linter, "fail", 0),
        ]);
        assert!(!evaluator.can_commit(&some_fail));
    }

    #[test]
    fn test_can_commit_tests_only() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let linter_fail = QualityGateSummary::from_results(vec![
            QualityGateResult::pass(QualityGateType::Tests, "pass", 0),
            QualityGateResult::fail(QualityGateType::Linter, "fail", 0),
        ]);
        assert!(evaluator.can_commit(&linter_fail));
    }

    #[test]
    fn test_parse_coverage_output_json() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let output = r#"{"coverage": 85.5, "files": 10}"#;
        assert_eq!(evaluator.parse_coverage_output(output), Some(85.5));
    }

    #[test]
    fn test_parse_coverage_output_text() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let output = "Coverage: 92.3%";
        assert_eq!(evaluator.parse_coverage_output(output), Some(92.3));
    }

    #[test]
    fn test_parse_coverage_output_no_match() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let output = "No coverage data";
        assert_eq!(evaluator.parse_coverage_output(output), None);
    }

    #[test]
    fn test_format_summary() {
        let results = vec![
            QualityGateResult::pass(QualityGateType::Tests, "All tests passed", 100),
            QualityGateResult::fail(QualityGateType::Linter, "2 warnings", 50),
        ];
        let summary = QualityGateSummary::from_results(results);
        let formatted = summary.format_summary();
        assert!(formatted.contains("Quality Gate Summary"));
        assert!(formatted.contains("✓ tests"));
        assert!(formatted.contains("✗ linter"));
        assert!(formatted.contains("FAILED"));
    }

    #[tokio::test]
    async fn test_run_linter_not_configured() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);
        let result = evaluator.run_linter().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_run_coverage_not_configured() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);
        let result = evaluator.run_coverage().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_run_linter_success() {
        let config = QualityGateConfig::default().with_linter(LinterConfig {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "exit 0".to_string()],
            timeout_secs: 2,
        });
        let evaluator = QualityGateEvaluator::new(config);
        let result = evaluator.run_linter().await.unwrap();
        assert!(result.passed);
        assert_eq!(result.gate_type, QualityGateType::Linter);
    }

    #[tokio::test]
    async fn test_run_linter_failure() {
        let config = QualityGateConfig::default().with_linter(LinterConfig {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "echo 'warning' >&2 && exit 1".to_string()],
            timeout_secs: 2,
        });
        let evaluator = QualityGateEvaluator::new(config);
        let result = evaluator.run_linter().await.unwrap();
        assert!(!result.passed);
        assert!(result.message.contains("warning"));
    }

    #[tokio::test]
    async fn test_evaluate_all_tests_only() {
        let config = QualityGateConfig::default();
        let evaluator = QualityGateEvaluator::new(config);

        let summary = evaluator.evaluate_all(true).await;
        assert!(summary.all_passed);
        assert_eq!(summary.gates.len(), 1);
        assert!(summary.tests_passed());
    }

    #[tokio::test]
    async fn test_evaluate_all_with_linter() {
        let config = QualityGateConfig::default().with_linter(LinterConfig {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "exit 0".to_string()],
            timeout_secs: 2,
        });
        let evaluator = QualityGateEvaluator::new(config);

        let summary = evaluator.evaluate_all(true).await;
        assert!(summary.all_passed);
        assert_eq!(summary.gates.len(), 2);
        assert!(summary.tests_passed());
        assert_eq!(summary.linter_passed(), Some(true));
    }
}
