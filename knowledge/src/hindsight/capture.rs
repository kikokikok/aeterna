use mk_core::traits::EmbeddingService;
use mk_core::types::ErrorSignature;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::info_span;

#[derive(Debug, Clone)]
pub struct ErrorCaptureConfig {
    pub max_stack_patterns: usize,
    pub max_context_patterns: usize,
    pub normalize_line_numbers: bool,
    pub normalize_hex: bool,
    pub normalize_uuid: bool,
    pub normalize_timestamps: bool,
}

impl Default for ErrorCaptureConfig {
    fn default() -> Self {
        Self {
            max_stack_patterns: 8,
            max_context_patterns: 8,
            normalize_line_numbers: true,
            normalize_hex: true,
            normalize_uuid: true,
            normalize_timestamps: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub tool_name: Option<String>,
    pub operation: Option<String>,
    pub file_path: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            tool_name: None,
            operation: None,
            file_path: None,
            metadata: HashMap::new(),
        }
    }
}

impl ErrorContext {
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool_name = Some(tool.into());
        self
    }

    pub fn with_operation(mut self, op: impl Into<String>) -> Self {
        self.operation = Some(op.into());
        self
    }

    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct CapturedError {
    pub signature: ErrorSignature,
    pub raw_message: String,
    pub raw_stack: Vec<String>,
    pub context: ErrorContext,
}

pub trait ErrorNormalizer: Send + Sync {
    fn normalize_message(&self, message: &str) -> String;
    fn normalize_stack_line(&self, line: &str) -> String;
}

#[derive(Debug, Clone)]
pub struct DefaultErrorNormalizer {
    cfg: ErrorCaptureConfig,
}

impl DefaultErrorNormalizer {
    pub fn new(cfg: ErrorCaptureConfig) -> Self {
        Self { cfg }
    }
}

impl ErrorNormalizer for DefaultErrorNormalizer {
    fn normalize_message(&self, message: &str) -> String {
        normalize_text(message, &self.cfg)
    }

    fn normalize_stack_line(&self, line: &str) -> String {
        normalize_text(line, &self.cfg)
    }
}

#[derive(Debug, Clone)]
pub struct ErrorCapture<N> {
    normalizer: N,
    cfg: ErrorCaptureConfig,
}

impl ErrorCapture<DefaultErrorNormalizer> {
    pub fn new(cfg: ErrorCaptureConfig) -> Self {
        let normalizer = DefaultErrorNormalizer::new(cfg.clone());
        Self { normalizer, cfg }
    }
}

impl<N: ErrorNormalizer> ErrorCapture<N> {
    pub fn with_normalizer(cfg: ErrorCaptureConfig, normalizer: N) -> Self {
        Self { cfg, normalizer }
    }

    pub fn capture(
        &self,
        error_type: impl Into<String>,
        message: &str,
        stack: &[String],
        context: ErrorContext,
    ) -> CapturedError {
        let error_type_str = error_type.into();
        let _span = info_span!(
            "capture_error",
            error_type = %error_type_str,
            message_len = message.len(),
            stack_len = stack.len(),
            has_tool = context.tool_name.is_some(),
            has_file = context.file_path.is_some(),
            metadata_count = context.metadata.len()
        )
        .entered();

        let error_type = error_type_str;

        let normalized_message = self.normalizer.normalize_message(message);
        let mut stack_patterns: Vec<String> = stack
            .iter()
            .map(|l| self.normalizer.normalize_stack_line(l))
            .filter(|l| !l.trim().is_empty())
            .collect();

        if stack_patterns.len() > self.cfg.max_stack_patterns {
            stack_patterns.truncate(self.cfg.max_stack_patterns);
        }

        let mut context_patterns = Vec::new();
        if let Some(ref tool) = context.tool_name {
            context_patterns.push(format!("tool:{tool}"));
        }
        if let Some(ref op) = context.operation {
            context_patterns.push(format!("op:{op}"));
        }
        if let Some(ref file) = context.file_path {
            context_patterns.push(format!("file:{file}"));
        }

        for (k, v) in &context.metadata {
            if context_patterns.len() >= self.cfg.max_context_patterns {
                break;
            }
            context_patterns.push(format!("meta:{k}={v}"));
        }

        CapturedError {
            signature: ErrorSignature {
                error_type,
                message_pattern: normalized_message,
                stack_patterns,
                context_patterns,
                embedding: None,
            },
            raw_message: message.to_string(),
            raw_stack: stack.to_vec(),
            context,
        }
    }

    pub async fn capture_with_embedding<E: std::error::Error + Send + Sync + 'static>(
        &self,
        error_type: impl Into<String>,
        message: &str,
        stack: &[String],
        context: ErrorContext,
        embedder: Option<&Arc<dyn EmbeddingService<Error = E>>>,
    ) -> CapturedError {
        let error_type_str = error_type.into();
        let span = info_span!(
            "capture_error_with_embedding",
            error_type = %error_type_str,
            has_embedder = embedder.is_some()
        );

        let _guard = span.enter();

        let mut captured = self.capture(error_type_str, message, stack, context);
        if let Some(service) = embedder {
            let text = format!(
                "{}\n{}\n{}",
                captured.signature.message_pattern,
                captured.signature.stack_patterns.join("\n"),
                captured.signature.context_patterns.join("\n")
            );
            captured.signature.embedding = service.embed(&text).await.ok();
        }
        captured
    }

    pub fn should_deduplicate(
        &self,
        candidate: &ErrorSignature,
        existing: &[ErrorSignature],
        threshold: f32,
    ) -> bool {
        existing.iter().any(|sig| {
            if sig.error_type != candidate.error_type {
                return false;
            }

            let msg_sim = jaccard_similarity(
                &tokenize(&candidate.message_pattern),
                &tokenize(&sig.message_pattern),
            );
            let ctx_sim = jaccard_similarity(&candidate.context_patterns, &sig.context_patterns);
            let stack_sim = jaccard_similarity(&candidate.stack_patterns, &sig.stack_patterns);

            let mut score = msg_sim * 0.6 + ctx_sim * 0.2 + stack_sim * 0.2;

            if let (Some(a), Some(b)) = (candidate.embedding.as_ref(), sig.embedding.as_ref()) {
                let sim = cosine_similarity(a, b);
                score = score.max(sim);
            }

            score >= threshold
        })
    }
}

fn normalize_text(input: &str, cfg: &ErrorCaptureConfig) -> String {
    let mut out = input.to_string();

    if cfg.normalize_uuid {
        out = replace_uuid_like(&out);
    }
    if cfg.normalize_hex {
        out = replace_hex_like(&out);
    }
    if cfg.normalize_line_numbers {
        out = replace_line_numbers(&out);
    }
    if cfg.normalize_timestamps {
        out = replace_timestamps(&out);
    }

    out
}

fn replace_uuid_like(input: &str) -> String {
    let re =
        regex::Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b");
    match re {
        Ok(re) => re.replace_all(input, "<uuid>").to_string(),
        Err(_) => input.to_string(),
    }
}

fn replace_hex_like(input: &str) -> String {
    let re = regex::Regex::new(r"(?i)\b0x[0-9a-f]+\b");
    match re {
        Ok(re) => re.replace_all(input, "<hex>").to_string(),
        Err(_) => input.to_string(),
    }
}

fn replace_line_numbers(input: &str) -> String {
    let re = regex::Regex::new(r"\bline\s+\d+\b");
    match re {
        Ok(re) => re.replace_all(input, "line <n>").to_string(),
        Err(_) => input.to_string(),
    }
}

fn replace_timestamps(input: &str) -> String {
    let re = regex::Regex::new(r"\b\d{4}-\d{2}-\d{2}[T\s]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?\b");
    match re {
        Ok(re) => re.replace_all(input, "<ts>").to_string(),
        Err(_) => input.to_string(),
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_set: HashSet<_> = a.iter().collect();
    let b_set: HashSet<_> = b.iter().collect();

    let intersection = a_set.intersection(&b_set).count() as f32;
    let union = a_set.union(&b_set).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_basic() {
        let cap = ErrorCapture::new(ErrorCaptureConfig::default());
        let stack = vec!["at foo (src/lib.rs:10)".to_string()];
        let ctx = ErrorContext::default().with_tool("cargo_test");

        let captured = cap.capture("TypeError", "Something failed", &stack, ctx);

        assert_eq!(captured.signature.error_type, "TypeError");
        assert_eq!(captured.signature.message_pattern, "Something failed");
        assert_eq!(captured.signature.stack_patterns.len(), 1);
        assert!(
            captured
                .signature
                .context_patterns
                .iter()
                .any(|p| p.contains("tool:"))
        );
    }

    #[test]
    fn test_normalize_uuid_and_hex() {
        let cfg = ErrorCaptureConfig::default();
        let cap = ErrorCapture::new(cfg);
        let msg = "Failed request id 550e8400-e29b-41d4-a716-446655440000 ptr 0xDEADBEEF";

        let captured = cap.capture("Err", msg, &[], ErrorContext::default());

        assert!(captured.signature.message_pattern.contains("<uuid>"));
        assert!(captured.signature.message_pattern.contains("<hex>"));
    }

    #[test]
    fn test_normalize_line_numbers() {
        let cfg = ErrorCaptureConfig::default();
        let cap = ErrorCapture::new(cfg);
        let msg = "Parse error at line 123";

        let captured = cap.capture("ParseError", msg, &[], ErrorContext::default());

        assert_eq!(
            captured.signature.message_pattern,
            "Parse error at line <n>"
        );
    }

    #[test]
    fn test_normalize_timestamps() {
        let cfg = ErrorCaptureConfig::default();
        let cap = ErrorCapture::new(cfg);
        let msg = "Event at 2025-01-10T12:13:14Z failed";

        let captured = cap.capture("EventError", msg, &[], ErrorContext::default());

        assert!(captured.signature.message_pattern.contains("<ts>"));
    }

    #[test]
    fn test_stack_truncation() {
        let cfg = ErrorCaptureConfig {
            max_stack_patterns: 2,
            ..Default::default()
        };
        let cap = ErrorCapture::new(cfg);
        let stack = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        let captured = cap.capture("Err", "msg", &stack, ErrorContext::default());

        assert_eq!(captured.signature.stack_patterns.len(), 2);
    }

    #[test]
    fn test_context_pattern_limit() {
        let cfg = ErrorCaptureConfig {
            max_context_patterns: 2,
            ..Default::default()
        };
        let cap = ErrorCapture::new(cfg);
        let ctx = ErrorContext::default()
            .with_tool("t")
            .with_operation("o")
            .with_meta("k1", "v1");

        let captured = cap.capture("Err", "msg", &[], ctx);

        assert_eq!(captured.signature.context_patterns.len(), 2);
    }

    #[test]
    fn test_deduplication_match() {
        let cap = ErrorCapture::new(ErrorCaptureConfig::default());
        let existing = vec![ErrorSignature {
            error_type: "TypeError".to_string(),
            message_pattern: "cannot read property".to_string(),
            stack_patterns: vec!["at foo".to_string()],
            context_patterns: vec!["tool:cargo_test".to_string()],
            embedding: None,
        }];

        let candidate = ErrorSignature {
            error_type: "TypeError".to_string(),
            message_pattern: "cannot read property".to_string(),
            stack_patterns: vec!["at foo".to_string()],
            context_patterns: vec!["tool:cargo_test".to_string()],
            embedding: None,
        };

        assert!(cap.should_deduplicate(&candidate, &existing, 0.6));
    }

    #[test]
    fn test_deduplication_no_match() {
        let cap = ErrorCapture::new(ErrorCaptureConfig::default());
        let existing = vec![ErrorSignature {
            error_type: "TypeError".to_string(),
            message_pattern: "cannot read property".to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: None,
        }];

        let candidate = ErrorSignature {
            error_type: "TypeError".to_string(),
            message_pattern: "different error".to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: None,
        };

        assert!(!cap.should_deduplicate(&candidate, &existing, 0.8));
    }
}
