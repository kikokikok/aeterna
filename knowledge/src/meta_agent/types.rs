#[derive(Debug, Clone)]
pub struct BuildResult {
    pub output: String,
    pub notes: Vec<String>,
    pub hindsight: Vec<String>,
    pub tokens_used: u32,
}

#[derive(Debug, Clone)]
pub struct MetaAgentSuccessReport {
    pub summary: String,
    pub commit_message_hint: String,
    pub pr_hint: String,
}

#[derive(Debug, Clone)]
pub struct MetaAgentFailureReport {
    pub summary: String,
    pub detailed_report: String,
}

#[derive(Debug, Clone)]
pub struct MetaAgentTelemetry {
    pub iterations: u32,
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Pass,
    Fail,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub status: TestStatus,
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImproveAction {
    Retry,
    Escalate,
}

#[derive(Debug, Clone)]
pub struct ImproveResult {
    pub analysis: String,
    pub action: ImproveAction,
    pub escalation_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetaAgentConfig {
    pub max_iterations: u32,
    pub note_limit: usize,
    pub hindsight_limit: usize,
    pub view_mode: crate::context_architect::ViewMode,
}

impl Default for MetaAgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            note_limit: 5,
            hindsight_limit: 5,
            view_mode: crate::context_architect::ViewMode::Ax,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl TestCommand {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}
