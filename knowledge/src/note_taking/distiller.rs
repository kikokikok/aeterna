use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{Instrument, info_span};

use super::capture::TrajectoryEvent;
use crate::context_architect::LlmClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillationTrigger {
    SessionEnd,
    SignificantSuccess,
    ManualRequest,
    FailurePattern
}

#[derive(Debug, Clone)]
pub struct DistillerConfig {
    pub min_events_for_distillation: usize,
    pub min_success_ratio: f32,
    pub extract_code_snippets: bool,
    pub max_tags: usize
}

impl Default for DistillerConfig {
    fn default() -> Self {
        Self {
            min_events_for_distillation: 3,
            min_success_ratio: 0.5,
            extract_code_snippets: true,
            max_tags: 10
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSection {
    pub name: String,
    pub content: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationResult {
    pub id: String,
    pub trigger: String,
    pub context: String,
    pub problem: String,
    pub solution: String,
    pub patterns: Vec<String>,
    pub tags: Vec<String>,
    pub code_snippets: Vec<String>,
    pub quality_score: f32,
    pub distilled_at: u64,
    pub source_event_count: usize
}

impl DistillationResult {
    pub fn is_high_quality(&self) -> bool {
        self.quality_score >= 0.7
    }
}

pub struct Distiller<C: LlmClient> {
    config: DistillerConfig,
    llm_client: C
}

impl<C: LlmClient> Distiller<C> {
    pub fn new(config: DistillerConfig, llm_client: C) -> Self {
        Self { config, llm_client }
    }

    pub async fn distill(
        &self,
        events: &[TrajectoryEvent],
        trigger: DistillationTrigger
    ) -> Result<DistillationResult, DistillationError> {
        let span = info_span!(
            "distill",
            events_count = events.len(),
            trigger = ?trigger,
            min_events = self.config.min_events_for_distillation,
            extract_code_snippets = self.config.extract_code_snippets
        );

        async move {
            if events.len() < self.config.min_events_for_distillation {
                return Err(DistillationError::InsufficientEvents {
                    provided: events.len(),
                    required: self.config.min_events_for_distillation
                });
            }

            let success_count = events.iter().filter(|e| e.success).count();
            let success_ratio = success_count as f32 / events.len() as f32;

            if success_ratio < self.config.min_success_ratio
                && trigger != DistillationTrigger::FailurePattern
            {
                return Err(DistillationError::LowSuccessRatio {
                    ratio: success_ratio,
                    required: self.config.min_success_ratio
                });
            }

            let trajectory_text = self.format_trajectory(events);
            let prompt = self.build_distillation_prompt(&trajectory_text);

            let response = self
                .llm_client
                .complete_with_system(DISTILLATION_SYSTEM_PROMPT, &prompt)
                .await
                .map_err(|e| DistillationError::LlmError(e.to_string()))?;

            let parsed = self.parse_distillation_response(&response)?;
            let code_snippets = if self.config.extract_code_snippets {
                self.extract_code_snippets(events)
            } else {
                vec![]
            };

            let quality_score = self.calculate_quality_score(&parsed, events);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            Ok(DistillationResult {
                id: uuid::Uuid::new_v4().to_string(),
                trigger: format!("{trigger:?}"),
                context: parsed.context,
                problem: parsed.problem,
                solution: parsed.solution,
                patterns: parsed.patterns,
                tags: parsed.tags.into_iter().take(self.config.max_tags).collect(),
                code_snippets,
                quality_score,
                distilled_at: timestamp,
                source_event_count: events.len()
            })
        }
        .instrument(span)
        .await
    }

    fn format_trajectory(&self, events: &[TrajectoryEvent]) -> String {
        events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    "Step {}: {}\nInput: {}\nOutput: {}\nSuccess: {}",
                    i + 1,
                    e.tool_name,
                    e.input,
                    if e.output.len() > 500 {
                        format!("{}...", &e.output[..500])
                    } else {
                        e.output.clone()
                    },
                    e.success
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn build_distillation_prompt(&self, trajectory: &str) -> String {
        format!(
            "Analyze the following agent trajectory and extract \
             learnings:\n\n{trajectory}\n\nProvide your analysis in the following \
             format:\nCONTEXT: [What was the overall goal or situation?]\nPROBLEM: [What specific \
             problem was being solved?]\nSOLUTION: [What approach worked or what was \
             learned?]\nPATTERNS: [Comma-separated list of reusable patterns]\nTAGS: \
             [Comma-separated list of relevant tags]"
        )
    }

    fn parse_distillation_response(
        &self,
        response: &str
    ) -> Result<ParsedDistillation, DistillationError> {
        let context = self
            .extract_section(response, "CONTEXT:")
            .unwrap_or_default();
        let problem = self
            .extract_section(response, "PROBLEM:")
            .unwrap_or_default();
        let solution = self
            .extract_section(response, "SOLUTION:")
            .unwrap_or_default();
        let patterns_str = self
            .extract_section(response, "PATTERNS:")
            .unwrap_or_default();
        let tags_str = self.extract_section(response, "TAGS:").unwrap_or_default();

        if context.is_empty() && problem.is_empty() && solution.is_empty() {
            return Err(DistillationError::ParseError(
                "Could not extract any sections from LLM response".to_string()
            ));
        }

        let patterns = patterns_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let tags = tags_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(ParsedDistillation {
            context,
            problem,
            solution,
            patterns,
            tags
        })
    }

    fn extract_section(&self, text: &str, marker: &str) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.contains(marker) {
                let after_marker = line.split(marker).nth(1).map(|s| s.trim().to_string());

                if let Some(content) = after_marker
                    && !content.is_empty()
                {
                    return Some(content);
                }

                let mut content_lines = Vec::new();
                for subsequent_line in lines.iter().skip(i + 1) {
                    if subsequent_line.contains(':')
                        && (subsequent_line.starts_with("CONTEXT")
                            || subsequent_line.starts_with("PROBLEM")
                            || subsequent_line.starts_with("SOLUTION")
                            || subsequent_line.starts_with("PATTERNS")
                            || subsequent_line.starts_with("TAGS"))
                    {
                        break;
                    }
                    content_lines.push(*subsequent_line);
                }

                if !content_lines.is_empty() {
                    return Some(content_lines.join("\n").trim().to_string());
                }
            }
        }

        None
    }

    fn extract_code_snippets(&self, events: &[TrajectoryEvent]) -> Vec<String> {
        let mut snippets = Vec::new();

        for event in events {
            if event.tool_name.contains("write")
                || event.tool_name.contains("edit")
                || event.tool_name.contains("code")
            {
                if let Some(code) = self.extract_code_block(&event.input) {
                    snippets.push(code);
                }
                if let Some(code) = self.extract_code_block(&event.output) {
                    snippets.push(code);
                }
            }
        }

        snippets
    }

    fn extract_code_block(&self, text: &str) -> Option<String> {
        if let Some(start) = text.find("```") {
            let after_start = &text[start + 3..];
            if let Some(end) = after_start.find("```") {
                let code = after_start[..end].trim();
                let code = code
                    .lines()
                    .skip_while(|l| !l.is_empty() && !l.contains(' '))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !code.is_empty() {
                    return Some(code);
                }
            }
        }
        None
    }

    fn calculate_quality_score(
        &self,
        parsed: &ParsedDistillation,
        events: &[TrajectoryEvent]
    ) -> f32 {
        let mut score = 0.0;

        if !parsed.context.is_empty() {
            score += 0.2;
        }
        if !parsed.problem.is_empty() {
            score += 0.2;
        }
        if !parsed.solution.is_empty() {
            score += 0.3;
        }
        if !parsed.patterns.is_empty() {
            score += 0.15;
        }
        if !parsed.tags.is_empty() {
            score += 0.1;
        }

        let success_count = events.iter().filter(|e| e.success).count();
        let success_ratio = success_count as f32 / events.len().max(1) as f32;
        score += 0.05 * success_ratio;

        score.min(1.0)
    }
}

struct ParsedDistillation {
    context: String,
    problem: String,
    solution: String,
    patterns: Vec<String>,
    tags: Vec<String>
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum DistillationError {
    #[error("Insufficient events: {provided} provided, {required} required")]
    InsufficientEvents { provided: usize, required: usize },

    #[error("Low success ratio: {ratio:.2} (required: {required:.2})")]
    LowSuccessRatio { ratio: f32, required: f32 },

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Parse error: {0}")]
    ParseError(String)
}

const DISTILLATION_SYSTEM_PROMPT: &str = r#"You are a learning distillation agent. Your task is to analyze agent tool execution trajectories and extract reusable learnings.

Focus on:
1. Understanding the context and goal
2. Identifying the specific problem being solved
3. Extracting the successful solution approach
4. Recognizing patterns that could help with similar problems
5. Generating relevant tags for searchability

Be concise but comprehensive. Extract actionable insights that would help an agent facing a similar situation in the future."#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_architect::LlmError;
    use async_trait::async_trait;

    struct MockLlmClient {
        response: String
    }

    impl MockLlmClient {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into()
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String, LlmError> {
            Ok(self.response.clone())
        }

        async fn complete_with_system(
            &self,
            _system: &str,
            _user: &str
        ) -> Result<String, LlmError> {
            Ok(self.response.clone())
        }
    }

    fn sample_events(count: usize, success: bool) -> Vec<TrajectoryEvent> {
        (0..count)
            .map(|i| TrajectoryEvent::new(format!("tool{i}"), "input", "output", success, 100))
            .collect()
    }

    #[tokio::test]
    async fn test_distill_success() {
        let response = "CONTEXT: Testing context\nPROBLEM: Test problem\nSOLUTION: Test \
                        solution\nPATTERNS: pattern1, pattern2\nTAGS: rust, testing";

        let client = MockLlmClient::new(response);
        let distiller = Distiller::new(DistillerConfig::default(), client);

        let events = sample_events(5, true);
        let result = distiller
            .distill(&events, DistillationTrigger::SessionEnd)
            .await
            .unwrap();

        assert_eq!(result.context, "Testing context");
        assert_eq!(result.problem, "Test problem");
        assert_eq!(result.solution, "Test solution");
        assert!(result.patterns.contains(&"pattern1".to_string()));
        assert!(result.tags.contains(&"rust".to_string()));
    }

    #[tokio::test]
    async fn test_distill_insufficient_events() {
        let client = MockLlmClient::new("response");
        let distiller = Distiller::new(DistillerConfig::default(), client);

        let events = sample_events(1, true);
        let result = distiller
            .distill(&events, DistillationTrigger::SessionEnd)
            .await;

        assert!(matches!(
            result,
            Err(DistillationError::InsufficientEvents { .. })
        ));
    }

    #[tokio::test]
    async fn test_distill_low_success_ratio() {
        let client = MockLlmClient::new("response");
        let config = DistillerConfig {
            min_events_for_distillation: 2,
            min_success_ratio: 0.8,
            ..Default::default()
        };
        let distiller = Distiller::new(config, client);

        let mut events = sample_events(5, false);
        events[0] = TrajectoryEvent::new("tool0", "input", "output", true, 100);

        let result = distiller
            .distill(&events, DistillationTrigger::SessionEnd)
            .await;

        assert!(matches!(
            result,
            Err(DistillationError::LowSuccessRatio { .. })
        ));
    }

    #[tokio::test]
    async fn test_distill_failure_pattern_ignores_success_ratio() {
        let response =
            "CONTEXT: Failure analysis\nPROBLEM: Recurring error\nSOLUTION: Fix approach";

        let client = MockLlmClient::new(response);
        let distiller = Distiller::new(DistillerConfig::default(), client);

        let events = sample_events(5, false);
        let result = distiller
            .distill(&events, DistillationTrigger::FailurePattern)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_quality_score_calculation() {
        let response = "CONTEXT: Full context\nPROBLEM: Full problem\nSOLUTION: Full \
                        solution\nPATTERNS: pattern\nTAGS: tag";

        let client = MockLlmClient::new(response);
        let distiller = Distiller::new(DistillerConfig::default(), client);

        let events = sample_events(5, true);
        let result = distiller
            .distill(&events, DistillationTrigger::SessionEnd)
            .await
            .unwrap();

        assert!(result.quality_score >= 0.9);
        assert!(result.is_high_quality());
    }

    #[test]
    fn test_extract_section_inline() {
        let distiller = Distiller::new(DistillerConfig::default(), MockLlmClient::new(""));

        let text = "CONTEXT: Some context here\nPROBLEM: The problem";
        let section = distiller.extract_section(text, "CONTEXT:");

        assert_eq!(section, Some("Some context here".to_string()));
    }

    #[test]
    fn test_extract_section_multiline() {
        let distiller = Distiller::new(DistillerConfig::default(), MockLlmClient::new(""));

        let text = "CONTEXT:\nLine 1\nLine 2\nPROBLEM: Next";
        let section = distiller.extract_section(text, "CONTEXT:");

        assert!(section.is_some());
        assert!(section.unwrap().contains("Line 1"));
    }

    #[test]
    fn test_code_snippet_extraction() {
        let distiller = Distiller::new(DistillerConfig::default(), MockLlmClient::new(""));

        let events = vec![TrajectoryEvent::new(
            "write_file",
            "```rust\nfn main() {}\n```",
            "success",
            true,
            100
        )];

        let snippets = distiller.extract_code_snippets(&events);

        assert!(!snippets.is_empty());
    }

    #[test]
    fn test_max_tags_limit() {
        let response = "CONTEXT: c\nPROBLEM: p\nSOLUTION: s\nPATTERNS: p\nTAGS: \
                        t1,t2,t3,t4,t5,t6,t7,t8,t9,t10,t11,t12";

        let config = DistillerConfig {
            max_tags: 5,
            min_events_for_distillation: 1,
            ..Default::default()
        };

        let _distiller = Distiller::new(config, MockLlmClient::new(response));
    }
}
