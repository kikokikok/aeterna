use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mk_core::types::{ErrorSignature, HindsightNote, Resolution};
use storage::postgres::{PostgresBackend, PostgresError};
use tracing::{Instrument, info_span};

use crate::context_architect::{LlmClient, LlmError, ViewMode};

#[derive(Debug, Clone)]
pub enum HindsightNoteGenerationMode {
    Single,
    DraftAndRefine
}

#[derive(Debug, Clone)]
pub struct HindsightNoteGeneratorConfig {
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub max_tokens: u32,
    pub mode: HindsightNoteGenerationMode
}

impl Default for HindsightNoteGeneratorConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            retry_delay_ms: 500,
            max_tokens: 600,
            mode: HindsightNoteGenerationMode::Single
        }
    }
}

#[derive(Debug, Clone)]
pub struct HindsightNoteRequest {
    pub error_signature: ErrorSignature,
    pub resolutions: Vec<Resolution>,
    pub context: Option<String>,
    pub tags: Vec<String>,
    pub view_mode: ViewMode
}

impl HindsightNoteRequest {
    pub fn new(
        error_signature: ErrorSignature,
        resolutions: Vec<Resolution>,
        context: Option<String>,
        tags: Vec<String>,
        view_mode: ViewMode
    ) -> Self {
        Self {
            error_signature,
            resolutions,
            context,
            tags,
            view_mode
        }
    }
}

#[derive(Debug, Clone)]
pub struct HindsightNoteResult {
    pub note: HindsightNote,
    pub tokens_used: u32
}

#[derive(Debug, thiserror::Error)]
pub enum NoteGenerationError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("Storage error: {0}")]
    Storage(#[from] PostgresError),

    #[error("Storage not configured")]
    StorageNotConfigured,

    #[error("Empty error message pattern")]
    EmptyMessagePattern,

    #[error("Empty LLM response")]
    EmptyResponse
}

#[derive(Debug, Clone)]
pub struct HindsightPromptTemplate {
    pub system: String,
    pub user: String
}

#[derive(Debug, Clone)]
pub struct HindsightPromptTemplates {
    pub default: HindsightPromptTemplate,
    pub draft: HindsightPromptTemplate,
    pub refine: HindsightPromptTemplate
}

impl Default for HindsightPromptTemplates {
    fn default() -> Self {
        Self {
            default: HindsightPromptTemplate {
                system: DEFAULT_SYSTEM.to_string(),
                user: DEFAULT_USER.to_string()
            },
            draft: HindsightPromptTemplate {
                system: DRAFT_SYSTEM.to_string(),
                user: DRAFT_USER.to_string()
            },
            refine: HindsightPromptTemplate {
                system: REFINE_SYSTEM.to_string(),
                user: REFINE_USER.to_string()
            }
        }
    }
}

impl HindsightPromptTemplates {
    pub fn build_prompt(
        &self,
        signature: &ErrorSignature,
        resolutions: &[Resolution],
        context: Option<&str>,
        max_tokens: u32,
        view_mode: ViewMode
    ) -> (String, String) {
        self.build_template(
            &self.default,
            signature,
            resolutions,
            context,
            max_tokens,
            None,
            view_mode
        )
    }

    pub fn build_draft_prompt(
        &self,
        signature: &ErrorSignature,
        resolutions: &[Resolution],
        context: Option<&str>,
        max_tokens: u32,
        view_mode: ViewMode
    ) -> (String, String) {
        self.build_template(
            &self.draft,
            signature,
            resolutions,
            context,
            max_tokens,
            None,
            view_mode
        )
    }

    pub fn build_refine_prompt(
        &self,
        signature: &ErrorSignature,
        resolutions: &[Resolution],
        context: Option<&str>,
        draft: &str,
        max_tokens: u32,
        view_mode: ViewMode
    ) -> (String, String) {
        self.build_template(
            &self.refine,
            signature,
            resolutions,
            context,
            max_tokens,
            Some(draft),
            view_mode
        )
    }

    fn build_template(
        &self,
        template: &HindsightPromptTemplate,
        signature: &ErrorSignature,
        resolutions: &[Resolution],
        context: Option<&str>,
        max_tokens: u32,
        draft: Option<&str>,
        view_mode: ViewMode
    ) -> (String, String) {
        let context_text = context.unwrap_or("None");
        let stack_patterns = format_list(&signature.stack_patterns);
        let context_patterns = format_list(&signature.context_patterns);
        let resolution_list = format_resolutions(resolutions);
        let draft_text = draft.unwrap_or("None");
        let view_mode_text = view_mode_label(view_mode);

        let user = template
            .user
            .replace("{error_type}", &signature.error_type)
            .replace("{message_pattern}", &signature.message_pattern)
            .replace("{stack_patterns}", &stack_patterns)
            .replace("{context_patterns}", &context_patterns)
            .replace("{resolutions}", &resolution_list)
            .replace("{context}", context_text)
            .replace("{draft}", draft_text)
            .replace("{view_mode}", view_mode_text)
            .replace("{max_tokens}", &max_tokens.to_string());

        (template.system.clone(), user)
    }
}

pub struct HindsightNoteGenerator<C: LlmClient> {
    client: Arc<C>,
    config: HindsightNoteGeneratorConfig,
    prompts: HindsightPromptTemplates,
    storage: Option<Arc<PostgresBackend>>
}

impl<C: LlmClient> HindsightNoteGenerator<C> {
    pub fn new(client: Arc<C>, config: HindsightNoteGeneratorConfig) -> Self {
        Self {
            client,
            config,
            prompts: HindsightPromptTemplates::default(),
            storage: None
        }
    }

    pub fn with_prompts(mut self, prompts: HindsightPromptTemplates) -> Self {
        self.prompts = prompts;
        self
    }

    pub fn with_storage(mut self, storage: Arc<PostgresBackend>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub async fn generate_note(
        &self,
        request: &HindsightNoteRequest
    ) -> Result<HindsightNoteResult, NoteGenerationError> {
        let span = info_span!(
            "generate_hindsight_note",
            error_type = %request.error_signature.error_type,
            mode = ?self.config.mode,
            has_context = request.context.is_some(),
            resolutions_count = request.resolutions.len(),
            tags_count = request.tags.len()
        );

        async move {
            if request.error_signature.message_pattern.trim().is_empty() {
                return Err(NoteGenerationError::EmptyMessagePattern);
            }

            let content = match self.config.mode {
                HindsightNoteGenerationMode::Single => {
                    let (system, user) = self.prompts.build_prompt(
                        &request.error_signature,
                        &request.resolutions,
                        request.context.as_deref(),
                        self.config.max_tokens,
                        request.view_mode
                    );
                    self.call_llm_with_retry(&system, &user).await?
                }
                HindsightNoteGenerationMode::DraftAndRefine => {
                    let (system, user) = self.prompts.build_draft_prompt(
                        &request.error_signature,
                        &request.resolutions,
                        request.context.as_deref(),
                        self.config.max_tokens,
                        request.view_mode
                    );
                    let draft = self.call_llm_with_retry(&system, &user).await?;
                    let draft = draft.trim();
                    if draft.is_empty() {
                        return Err(NoteGenerationError::EmptyResponse);
                    }
                    let (system, user) = self.prompts.build_refine_prompt(
                        &request.error_signature,
                        &request.resolutions,
                        request.context.as_deref(),
                        draft,
                        self.config.max_tokens,
                        request.view_mode
                    );
                    self.call_llm_with_retry(&system, &user).await?
                }
            };

            let response = content.trim().to_string();
            if response.is_empty() {
                return Err(NoteGenerationError::EmptyResponse);
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let mut tags = request.tags.clone();
            tags.extend(derive_tags(&request.error_signature, &request.context));
            tags.sort();
            tags.dedup();

            let note = HindsightNote {
                id: uuid::Uuid::new_v4().to_string(),
                error_signature: request.error_signature.clone(),
                resolutions: request.resolutions.clone(),
                content: response.clone(),
                tags,
                created_at: now,
                updated_at: now
            };

            Ok(HindsightNoteResult {
                note,
                tokens_used: estimate_tokens(&response)
            })
        }
        .instrument(span)
        .await
    }

    pub async fn generate_and_store(
        &self,
        tenant_id: &str,
        request: &HindsightNoteRequest
    ) -> Result<HindsightNoteResult, NoteGenerationError> {
        let span = info_span!(
            "generate_and_store_hindsight_note",
            tenant_id,
            error_type = %request.error_signature.error_type,
            has_storage = self.storage.is_some()
        );

        async move {
            let storage = self
                .storage
                .as_ref()
                .ok_or(NoteGenerationError::StorageNotConfigured)?;

            let result = self.generate_note(request).await?;
            storage
                .create_hindsight_note(tenant_id, &result.note)
                .await?;
            Ok(result)
        }
        .instrument(span)
        .await
    }

    async fn call_llm_with_retry(
        &self,
        system: &str,
        user: &str
    ) -> Result<String, NoteGenerationError> {
        let mut attempt = 0;
        loop {
            match self.client.complete_with_system(system, user).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if attempt >= self.config.max_retries {
                        return Err(NoteGenerationError::Llm(err));
                    }
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                }
            }
        }
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

fn format_resolutions(resolutions: &[Resolution]) -> String {
    if resolutions.is_empty() {
        return "None".to_string();
    }

    let mut out = String::new();
    for resolution in resolutions {
        let line = format!(
            "- {} (success_rate: {:.2}, applications: {})",
            resolution.description, resolution.success_rate, resolution.application_count
        );
        out.push_str(&line);
        out.push('\n');
    }
    out.trim_end().to_string()
}

pub fn estimate_tokens(text: &str) -> u32 {
    let word_count = text.split_whitespace().count();
    let char_count = text.chars().count();

    let char_based = (char_count as f64 / 4.0).ceil() as u32;
    let word_based = (word_count as f64 * 1.3).ceil() as u32;

    char_based.max(word_based)
}

fn derive_tags(signature: &ErrorSignature, context: &Option<String>) -> Vec<String> {
    let mut tags = vec![signature.error_type.to_lowercase()];

    for pattern in &signature.context_patterns {
        if let Some((key, value)) = pattern.split_once(':') {
            if key == "tool" || key == "op" || key == "file" {
                tags.push(value.to_lowercase());
            }
        }
    }

    if let Some(ctx) = context {
        if ctx.to_lowercase().contains("test") {
            tags.push("tests".to_string());
        }
    }

    tags
}

const DEFAULT_SYSTEM: &str = "You are a hindsight learning assistant. Produce a concise markdown \
                              note that captures the error pattern, root cause, resolution, and \
                              prevention guidance. Be factual and avoid speculation.";

const DEFAULT_USER: &str =
    "Error type: {error_type}\nMessage pattern: {message_pattern}\n\nStack \
     patterns:\n{stack_patterns}\n\nContext patterns:\n{context_patterns}\n\nKnown \
     resolutions:\n{resolutions}\n\nAdditional context:\n{context}\n\nView mode: \
     {view_mode}\n\nWrite a markdown note with these sections:\n- Summary\n- Root Cause\n- \
     Resolution\n- Prevention\n\nKeep it under {max_tokens} tokens.";

const DRAFT_SYSTEM: &str = "You are a hindsight learning assistant. Draft a concise markdown note \
                            that captures the error pattern, root cause, resolution, and \
                            prevention guidance. Be factual and avoid speculation.";

const DRAFT_USER: &str =
    "Error type: {error_type}\nMessage pattern: {message_pattern}\n\nStack \
     patterns:\n{stack_patterns}\n\nContext patterns:\n{context_patterns}\n\nKnown \
     resolutions:\n{resolutions}\n\nAdditional context:\n{context}\n\nView mode: \
     {view_mode}\n\nDraft a markdown note with these sections:\n- Summary\n- Root Cause\n- \
     Resolution\n- Prevention\n\nKeep it under {max_tokens} tokens.";

const REFINE_SYSTEM: &str = "You are a hindsight learning assistant. Refine the draft into a \
                             clear, well-structured markdown note without adding speculative \
                             details.";

const REFINE_USER: &str = "Error type: {error_type}\nMessage pattern: {message_pattern}\n\nStack \
                           patterns:\n{stack_patterns}\n\nContext \
                           patterns:\n{context_patterns}\n\nKnown \
                           resolutions:\n{resolutions}\n\nAdditional context:\n{context}\n\nView \
                           mode: {view_mode}\n\nDraft:\n{draft}\n\nRefine the draft into a \
                           markdown note with these sections:\n- Summary\n- Root Cause\n- \
                           Resolution\n- Prevention\n\nKeep it under {max_tokens} tokens.";

fn view_mode_label(view_mode: ViewMode) -> &'static str {
    match view_mode {
        ViewMode::Ax => "AX",
        ViewMode::Ux => "UX",
        ViewMode::Dx => "DX"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

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

    #[async_trait::async_trait]
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
            _user: &str
        ) -> Result<String, LlmError> {
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }
    }

    struct RecordingLlmClient {
        responses: Mutex<Vec<String>>,
        calls: Mutex<Vec<(String, String)>>
    }

    impl RecordingLlmClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new())
            }
        }

        fn calls(&self) -> Vec<(String, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LlmClient for RecordingLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String, LlmError> {
            Err(LlmError::InvalidResponse("Unsupported".into()))
        }

        async fn complete_with_system(&self, system: &str, user: &str) -> Result<String, LlmError> {
            self.calls
                .lock()
                .unwrap()
                .push((system.to_string(), user.to_string()));
            let mut responses = self.responses.lock().unwrap();
            responses
                .pop()
                .ok_or_else(|| LlmError::InvalidResponse("No mock response".into()))
        }
    }

    fn sample_signature() -> ErrorSignature {
        ErrorSignature {
            error_type: "TypeError".to_string(),
            message_pattern: "cannot read property".to_string(),
            stack_patterns: vec!["at foo (src/lib.rs:10)".to_string()],
            context_patterns: vec!["tool:cargo_test".to_string()],
            embedding: None
        }
    }

    fn sample_resolution() -> Resolution {
        Resolution {
            id: "r1".to_string(),
            error_signature_id: "e1".to_string(),
            description: "Add null guard".to_string(),
            changes: vec![],
            success_rate: 0.9,
            application_count: 3,
            last_success_at: 0
        }
    }

    #[tokio::test]
    async fn test_generate_note_success() {
        let mock = Arc::new(MockLlmClient::new(vec![
            "## Summary\nSomething broke.".to_string(),
        ]));
        let generator = HindsightNoteGenerator::new(mock, HindsightNoteGeneratorConfig::default());

        let request = HindsightNoteRequest::new(
            sample_signature(),
            vec![sample_resolution()],
            Some("running tests".to_string()),
            vec!["rust".to_string()],
            ViewMode::Dx
        );

        let result = generator.generate_note(&request).await.unwrap();

        assert!(!result.note.id.is_empty());
        assert!(result.note.content.contains("Summary"));
        assert!(result.note.tags.contains(&"rust".to_string()));
        assert!(result.note.tags.contains(&"typeerror".to_string()));
        assert!(result.note.tags.contains(&"tests".to_string()));
        assert!(result.tokens_used > 0);
    }

    #[tokio::test]
    async fn test_generate_note_refine() {
        let mock = Arc::new(RecordingLlmClient::new(vec![
            "## Summary\nFinal note.".to_string(),
            "## Summary\nDraft note.".to_string(),
        ]));
        let config = HindsightNoteGeneratorConfig {
            mode: HindsightNoteGenerationMode::DraftAndRefine,
            ..Default::default()
        };
        let generator = HindsightNoteGenerator::new(mock.clone(), config);

        let request = HindsightNoteRequest::new(
            sample_signature(),
            vec![sample_resolution()],
            Some("running tests".to_string()),
            vec!["rust".to_string()],
            ViewMode::Dx
        );

        let result = generator.generate_note(&request).await.unwrap();

        assert!(result.note.content.contains("Final"));
        let calls = mock.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[1].1.contains("Draft:\n## Summary\nDraft note."));
    }

    #[tokio::test]
    async fn test_empty_message_pattern_error() {
        let mock = Arc::new(MockLlmClient::new(vec!["x".to_string()]));
        let generator = HindsightNoteGenerator::new(mock, HindsightNoteGeneratorConfig::default());

        let request = HindsightNoteRequest::new(
            ErrorSignature {
                error_type: "TypeError".to_string(),
                message_pattern: "".to_string(),
                stack_patterns: vec![],
                context_patterns: vec![],
                embedding: None
            },
            vec![],
            None,
            vec![],
            ViewMode::Dx
        );

        let result = generator.generate_note(&request).await;
        assert!(matches!(
            result,
            Err(NoteGenerationError::EmptyMessagePattern)
        ));
    }

    #[tokio::test]
    async fn test_empty_response_error() {
        let mock = Arc::new(MockLlmClient::new(vec!["   ".to_string()]));
        let generator = HindsightNoteGenerator::new(mock, HindsightNoteGeneratorConfig::default());

        let request =
            HindsightNoteRequest::new(sample_signature(), vec![], None, vec![], ViewMode::Dx);

        let result = generator.generate_note(&request).await;
        assert!(matches!(result, Err(NoteGenerationError::EmptyResponse)));
    }

    #[test]
    fn test_build_prompt_includes_context() {
        let templates = HindsightPromptTemplates::default();
        let signature = sample_signature();
        let (system, user) = templates.build_prompt(
            &signature,
            &[sample_resolution()],
            Some("extra context"),
            400,
            ViewMode::Dx
        );

        assert!(system.contains("hindsight"));
        assert!(user.contains("extra context"));
        assert!(user.contains("400"));
        assert!(user.contains("View mode: DX"));
        assert_eq!(view_mode_label(ViewMode::Ux), "UX");
    }

    #[test]
    fn test_format_list_empty() {
        assert_eq!(format_list(&[]), "None");
    }

    #[test]
    fn test_format_resolutions_empty() {
        assert_eq!(format_resolutions(&[]), "None");
    }

    #[test]
    fn test_derive_tags_from_context() {
        let signature = sample_signature();
        let tags = derive_tags(&signature, &Some("run tests".to_string()));
        assert!(tags.contains(&"typeerror".to_string()));
        assert!(tags.contains(&"cargo_test".to_string()));
        assert!(tags.contains(&"tests".to_string()));
    }
}
