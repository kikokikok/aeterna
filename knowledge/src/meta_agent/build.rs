use std::sync::Arc;

use crate::context_architect::LlmClient;
use tracing::{Instrument, info_span};

use super::{BuildResult, HindsightLookup, HindsightStore, MetaAgentConfig, NoteLookup, NoteStore};

#[derive(Debug, Clone)]
pub struct BuildPhaseConfig {
    pub max_tokens: u32,
}

impl Default for BuildPhaseConfig {
    fn default() -> Self {
        Self { max_tokens: 1200 }
    }
}

#[derive(Debug, Clone)]
pub struct BuildPromptTemplate {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone)]
pub struct BuildPromptTemplates {
    pub default: BuildPromptTemplate,
}

impl Default for BuildPromptTemplates {
    fn default() -> Self {
        Self {
            default: BuildPromptTemplate {
                system: DEFAULT_SYSTEM.to_string(),
                user: DEFAULT_USER.to_string(),
            },
        }
    }
}

pub struct BuildPhase<C: LlmClient> {
    client: Arc<C>,
    config: BuildPhaseConfig,
    prompt_templates: BuildPromptTemplates,
    note_lookup: Option<Arc<dyn NoteLookup>>,
    hindsight_lookup: Option<Arc<dyn HindsightLookup>>,
    note_store: Option<Arc<dyn NoteStore>>,
    hindsight_store: Option<Arc<dyn HindsightStore>>,
    meta_config: MetaAgentConfig,
}

impl<C: LlmClient> BuildPhase<C> {
    pub fn new(client: Arc<C>, config: BuildPhaseConfig) -> Self {
        Self {
            client,
            config,
            prompt_templates: BuildPromptTemplates::default(),
            note_lookup: None,
            hindsight_lookup: None,
            note_store: None,
            hindsight_store: None,
            meta_config: MetaAgentConfig::default(),
        }
    }

    pub fn with_prompts(mut self, templates: BuildPromptTemplates) -> Self {
        self.prompt_templates = templates;
        self
    }

    pub fn with_note_lookup(mut self, lookup: Arc<dyn NoteLookup>) -> Self {
        self.note_lookup = Some(lookup);
        self
    }

    pub fn with_hindsight_lookup(mut self, lookup: Arc<dyn HindsightLookup>) -> Self {
        self.hindsight_lookup = Some(lookup);
        self
    }

    pub fn with_note_store(mut self, store: Arc<dyn NoteStore>) -> Self {
        self.note_store = Some(store);
        self
    }

    pub fn with_hindsight_store(mut self, store: Arc<dyn HindsightStore>) -> Self {
        self.hindsight_store = Some(store);
        self
    }

    pub fn with_meta_config(mut self, config: MetaAgentConfig) -> Self {
        self.meta_config = config;
        self
    }

    pub async fn execute(
        &self,
        requirements: &str,
        context: Option<&str>,
    ) -> Result<BuildResult, crate::context_architect::LlmError> {
        let span = info_span!(
            "build_phase",
            requirements_len = requirements.len(),
            has_context = context.is_some(),
            has_note_store = self.note_store.is_some(),
            has_hindsight_store = self.hindsight_store.is_some(),
            note_limit = self.meta_config.note_limit,
            hindsight_limit = self.meta_config.hindsight_limit
        );

        async move {
            let notes = match (&self.note_store, &self.note_lookup) {
                (Some(store), _) => {
                    store
                        .retrieve(requirements, self.meta_config.note_limit)
                        .await
                }
                (None, Some(lookup)) => {
                    lookup
                        .retrieve(requirements, self.meta_config.note_limit)
                        .await
                }
                _ => Vec::new(),
            };
            let hindsight = match (&self.hindsight_store, &self.hindsight_lookup) {
                (Some(store), _) => {
                    store
                        .retrieve(requirements, self.meta_config.hindsight_limit)
                        .await
                }
                (None, Some(lookup)) => {
                    lookup
                        .retrieve(requirements, self.meta_config.hindsight_limit)
                        .await
                }
                _ => Vec::new(),
            };

            let (system, user) = self.build_prompt(requirements, context, &notes, &hindsight);
            let response = self.client.complete_with_system(&system, &user).await?;

            Ok(BuildResult {
                output: response.clone(),
                notes,
                hindsight,
                tokens_used: estimate_tokens(&response),
            })
        }
        .instrument(span)
        .await
    }

    fn build_prompt(
        &self,
        requirements: &str,
        context: Option<&str>,
        notes: &[String],
        hindsight: &[String],
    ) -> (String, String) {
        let notes_block = format_list(notes);
        let hindsight_block = format_list(hindsight);
        let context_block = context.unwrap_or("None");
        let view_mode = format!("{:?}", self.meta_config.view_mode);

        let user = self
            .prompt_templates
            .default
            .user
            .replace("{requirements}", requirements)
            .replace("{context}", context_block)
            .replace("{notes}", &notes_block)
            .replace("{hindsight}", &hindsight_block)
            .replace("{view_mode}", &view_mode)
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

fn estimate_tokens(text: &str) -> u32 {
    let word_count = text.split_whitespace().count();
    let char_count = text.chars().count();
    let char_based = (char_count as f64 / 4.0).ceil() as u32;
    let word_based = (word_count as f64 * 1.3).ceil() as u32;
    char_based.max(word_based)
}

const DEFAULT_SYSTEM: &str = "You are a build assistant that generates code or instructions based \
                              on requirements. Use provided notes and hindsight to avoid known \
                              pitfalls.";

const DEFAULT_USER: &str = "Requirements:\n{requirements}\n\nContext:\n{context}\n\nNotes:\\
                            n{notes}\n\nHindsight:\n{hindsight}\n\nView mode: \
                            {view_mode}\n\nProvide the implementation plan or code changes. Keep \
                            it under {max_tokens} tokens.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_architect::LlmError;
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

    struct MockLookup;

    #[async_trait]
    impl NoteLookup for MockLookup {
        async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["note".to_string()]
        }
    }

    #[async_trait]
    impl HindsightLookup for MockLookup {
        async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["hindsight".to_string()]
        }
    }

    #[tokio::test]
    async fn test_build_phase_collects_notes() {
        let client = Arc::new(MockLlmClient::new(vec!["output".to_string()]));
        let build = BuildPhase::new(client, BuildPhaseConfig::default())
            .with_note_lookup(Arc::new(MockLookup))
            .with_hindsight_lookup(Arc::new(MockLookup));

        let result = build.execute("req", None).await.unwrap();
        assert_eq!(result.output, "output");
        assert_eq!(result.notes, vec!["note".to_string()]);
        assert_eq!(result.hindsight, vec!["hindsight".to_string()]);
    }

    struct MockNoteStore;

    #[async_trait]
    impl NoteStore for MockNoteStore {
        async fn add_note(&self, _note: crate::note_taking::GeneratedNote) {}

        async fn retrieve(&self, _query: &str, _limit: usize) -> Vec<String> {
            vec!["stored_note".to_string()]
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
    async fn test_build_phase_uses_note_store_over_lookup() {
        let client = Arc::new(MockLlmClient::new(vec!["output".to_string()]));
        let build = BuildPhase::new(client, BuildPhaseConfig::default())
            .with_note_lookup(Arc::new(MockLookup))
            .with_note_store(Arc::new(MockNoteStore))
            .with_hindsight_lookup(Arc::new(MockLookup));

        let result = build.execute("req", None).await.unwrap();
        assert_eq!(result.notes, vec!["stored_note".to_string()]);
        assert_eq!(result.hindsight, vec!["hindsight".to_string()]);
    }

    #[tokio::test]
    async fn test_build_phase_uses_hindsight_store_over_lookup() {
        let client = Arc::new(MockLlmClient::new(vec!["output".to_string()]));
        let build = BuildPhase::new(client, BuildPhaseConfig::default())
            .with_note_lookup(Arc::new(MockLookup))
            .with_hindsight_store(Arc::new(MockHindsightStore));

        let result = build.execute("req", None).await.unwrap();
        assert_eq!(result.notes, vec!["note".to_string()]);
        assert_eq!(result.hindsight, vec!["stored_hindsight".to_string()]);
    }

    #[tokio::test]
    async fn test_build_phase_uses_both_stores() {
        let client = Arc::new(MockLlmClient::new(vec!["output".to_string()]));
        let build = BuildPhase::new(client, BuildPhaseConfig::default())
            .with_note_store(Arc::new(MockNoteStore))
            .with_hindsight_store(Arc::new(MockHindsightStore));

        let result = build.execute("req", None).await.unwrap();
        assert_eq!(result.notes, vec!["stored_note".to_string()]);
        assert_eq!(result.hindsight, vec!["stored_hindsight".to_string()]);
    }

    #[tokio::test]
    async fn test_build_phase_with_view_mode() {
        use crate::meta_agent::ViewMode;

        let client = Arc::new(MockLlmClient::new(vec!["output".to_string()]));
        let config = MetaAgentConfig {
            view_mode: ViewMode::Ax,
            ..Default::default()
        };
        let build = BuildPhase::new(client, BuildPhaseConfig::default()).with_meta_config(config);

        let result = build.execute("req", None).await.unwrap();
        assert_eq!(result.output, "output");
    }
}
