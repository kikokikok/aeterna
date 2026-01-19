use std::sync::Arc;

use crate::context_architect::LlmClient;
use tracing::{Instrument, info_span};

use super::{HindsightLookup, HindsightStore, ImproveAction, ImproveResult, TestResult};

#[derive(Debug, Clone)]
pub struct ImprovePhaseConfig {
    pub max_tokens: u32,
}

impl Default for ImprovePhaseConfig {
    fn default() -> Self {
        Self { max_tokens: 800 }
    }
}

#[derive(Debug, Clone)]
pub struct ImprovePromptTemplate {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone)]
pub struct ImprovePromptTemplates {
    pub default: ImprovePromptTemplate,
}

impl Default for ImprovePromptTemplates {
    fn default() -> Self {
        Self {
            default: ImprovePromptTemplate {
                system: DEFAULT_SYSTEM.to_string(),
                user: DEFAULT_USER.to_string(),
            },
        }
    }
}

pub struct ImprovePhase<C: LlmClient> {
    client: Arc<C>,
    config: ImprovePhaseConfig,
    prompt_templates: ImprovePromptTemplates,
    hindsight_lookup: Option<Arc<dyn HindsightLookup>>,
    hindsight_store: Option<Arc<dyn HindsightStore>>,
}

impl<C: LlmClient> ImprovePhase<C> {
    pub fn new(client: Arc<C>, config: ImprovePhaseConfig) -> Self {
        Self {
            client,
            config,
            prompt_templates: ImprovePromptTemplates::default(),
            hindsight_lookup: None,
            hindsight_store: None,
        }
    }

    pub fn with_prompts(mut self, templates: ImprovePromptTemplates) -> Self {
        self.prompt_templates = templates;
        self
    }

    pub fn with_hindsight_lookup(mut self, lookup: Arc<dyn HindsightLookup>) -> Self {
        self.hindsight_lookup = Some(lookup);
        self
    }

    pub fn with_hindsight_store(mut self, store: Arc<dyn HindsightStore>) -> Self {
        self.hindsight_store = Some(store);
        self
    }

    pub async fn execute(
        &self,
        test_result: &TestResult,
    ) -> Result<ImproveResult, crate::context_architect::LlmError> {
        let span = info_span!(
            "improve_phase",
            test_status = ?test_result.status,
            test_output_len = test_result.output.len(),
            test_duration_ms = test_result.duration_ms,
            has_hindsight_store = self.hindsight_store.is_some(),
            has_hindsight_lookup = self.hindsight_lookup.is_some()
        );

        async move {
            let hindsight = match (&self.hindsight_store, &self.hindsight_lookup) {
                (Some(store), _) => store.retrieve(&test_result.output, 5).await,
                (None, Some(lookup)) => lookup.retrieve(&test_result.output, 5).await,
                _ => Vec::new(),
            };

            let (system, user) = self.build_prompt(test_result, &hindsight);
            let response = self.client.complete_with_system(&system, &user).await?;

            let action = if response.to_lowercase().contains("escalate") {
                ImproveAction::Escalate
            } else {
                ImproveAction::Retry
            };

            let escalation_message = if action == ImproveAction::Escalate {
                Some(format!(
                    "Escalation recommended based on test output:\n{}",
                    test_result.output
                ))
            } else {
                None
            };

            Ok(ImproveResult {
                analysis: response,
                action,
                escalation_message,
            })
        }
        .instrument(span)
        .await
    }

    fn build_prompt(&self, test_result: &TestResult, hindsight: &[String]) -> (String, String) {
        let hindsight_block = format_list(hindsight);
        let user = self
            .prompt_templates
            .default
            .user
            .replace("{test_output}", &test_result.output)
            .replace("{hindsight}", &hindsight_block)
            .replace("{max_tokens}", &self.config.max_tokens.to_string());
        (self.prompt_templates.default.system.clone(), user)
    }
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        "None".to_string()
    } else {
        let mut out = String::new();
        for item in items {
            out.push_str("- ");
            out.push_str(item);
            out.push('\n');
        }
        out.trim_end().to_string()
    }
}

const DEFAULT_SYSTEM: &str = "You are an improvement assistant. Analyze failures and propose \
                              fixes based on test output and hindsight.";

const DEFAULT_USER: &str = "Test output:\n{test_output}\n\nHindsight:\n{hindsight}\n\nProvide a \
                            concise analysis and next action. Keep it under {max_tokens} tokens.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_architect::LlmError;
    use crate::meta_agent::TestStatus;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockLlmClient {
        responses: Mutex<Vec<String>>,
    }

    impl MockLlmClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }

        async fn complete_with_system(
            &self,
            _system: &str,
            _user: &str,
        ) -> Result<String, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }
    }

    #[tokio::test]
    async fn test_improve_phase_retry() {
        let client = Arc::new(MockLlmClient::new(vec!["Retry with fix".to_string()]));
        let phase = ImprovePhase::new(client, ImprovePhaseConfig::default());
        let result = phase
            .execute(&TestResult {
                status: TestStatus::Fail,
                output: "failed".to_string(),
                duration_ms: 1,
            })
            .await
            .unwrap();

        assert_eq!(result.action, ImproveAction::Retry);
        assert!(result.escalation_message.is_none());
    }

    struct MockHindsightLookup;

    #[async_trait]
    impl HindsightLookup for MockHindsightLookup {
        async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["past_hindsight".to_string()]
        }
    }

    struct MockHindsightStore;

    #[async_trait]
    impl HindsightStore for MockHindsightStore {
        async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["stored_hindsight".to_string()]
        }
    }

    #[tokio::test]
    async fn test_improve_phase_uses_hindsight_store_over_lookup() {
        let client = Arc::new(MockLlmClient::new(vec!["Retry".to_string()]));
        let phase = ImprovePhase::new(client, ImprovePhaseConfig::default())
            .with_hindsight_lookup(Arc::new(MockHindsightLookup))
            .with_hindsight_store(Arc::new(MockHindsightStore));

        let result = phase
            .execute(&TestResult {
                status: TestStatus::Fail,
                output: "error".to_string(),
                duration_ms: 1,
            })
            .await
            .unwrap();

        assert_eq!(result.action, ImproveAction::Retry);
    }

    #[tokio::test]
    async fn test_improve_phase_escalates() {
        let client = Arc::new(MockLlmClient::new(vec![
            "Unable to fix. Escalate to human.".to_string(),
        ]));
        let phase = ImprovePhase::new(client, ImprovePhaseConfig::default());

        let result = phase
            .execute(&TestResult {
                status: TestStatus::Fail,
                output: "critical error".to_string(),
                duration_ms: 1,
            })
            .await
            .unwrap();

        assert_eq!(result.action, ImproveAction::Escalate);
        assert!(result.escalation_message.is_some());
    }
}
