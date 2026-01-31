//! Section 13.8: LLM Translation Determinism
//!
//! Implements prompt caching, few-shot template library, and deterministic
//! translation for common policy patterns.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Deterministic LLM translator with caching.
pub struct DeterministicTranslator {
    cache: Arc<Mutex<LruCache<String, TranslationCacheEntry>>>,
    templates: HashMap<String, TranslationTemplate>,
    cache_ttl: Duration,
    confidence_threshold: f64
}

/// Cached translation entry.
#[derive(Debug, Clone)]
struct TranslationCacheEntry {
    result: TranslationResult,
    cached_at: Instant
}

/// Translation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub cedar_policy: String,
    pub method: TranslationMethod,
    pub confidence: f64,
    pub template_used: Option<String>,
    pub raw_llm_output: Option<String>
}

/// Translation method used.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TranslationMethod {
    CacheHit,
    TemplateMatch,
    FewShotPrompt,
    LLMGenerated
}

/// Translation template.
#[derive(Debug, Clone)]
pub struct TranslationTemplate {
    pub pattern: String,
    pub cedar_template: String,
    pub description: String,
    pub coverage_category: String
}

/// Translation metrics.
#[derive(Debug, Clone, Default)]
pub struct TranslationMetrics {
    pub cache_hits: u64,
    pub template_matches: u64,
    pub few_shot_uses: u64,
    pub llm_generations: u64,
    pub total_requests: u64,
    pub cache_hit_rate: f64,
    pub template_coverage: f64,
    pub average_confidence: f64
}

impl DeterministicTranslator {
    /// Create new deterministic translator.
    pub fn new(cache_size: usize, cache_ttl_hours: u64) -> Self {
        let mut translator = Self {
            cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
            templates: HashMap::new(),
            cache_ttl: Duration::from_secs(cache_ttl_hours * 3600),
            confidence_threshold: 0.8
        };

        // 13.8.2: Initialize few-shot template library
        translator.initialize_templates();

        translator
    }

    /// Initialize template library for common patterns (13.8.2).
    fn initialize_templates(&mut self) {
        let templates = vec![
            TranslationTemplate {
                pattern: r"(?i)block\s+all\s+external\s+requests".to_string(),
                cedar_template: r#"forbid (principal, action, resource) when { resource has scheme && resource.scheme == "https" } unless { principal.is_internal }"#.to_string(),
                description: "Block external HTTP requests".to_string(),
                coverage_category: "security".to_string()
            },
            TranslationTemplate {
                pattern: r"(?i)require\s+authentication".to_string(),
                cedar_template: r#"forbid (principal, action, resource) unless { principal.is_authenticated }"#.to_string(),
                description: "Require user authentication".to_string(),
                coverage_category: "auth".to_string()
            },
            TranslationTemplate {
                pattern: r"(?i)only\s+allow\s+admin".to_string(),
                cedar_template: r#"permit (principal, action, resource) when { principal.role == "admin" }"#.to_string(),
                description: "Admin-only access".to_string(),
                coverage_category: "authorization".to_string()
            },
            TranslationTemplate {
                pattern: r"(?i)block\s+eval\s*\(".to_string(),
                cedar_template: r#"forbid (principal, action, resource) when { resource has code && resource.code.contains("eval(") }"#.to_string(),
                description: "Block eval() function calls".to_string(),
                coverage_category: "security".to_string()
            },
            TranslationTemplate {
                pattern: r"(?i)require\s+readme".to_string(),
                cedar_template: r#"permit (principal, action, resource) when { resource has files && resource.files.contains("README.md") }"#.to_string(),
                description: "Require README file".to_string(),
                coverage_category: "standards".to_string()
            }
        ];

        for (i, template) in templates.into_iter().enumerate() {
            self.templates.insert(format!("tpl-{}", i), template);
        }

        info!("Initialized {} translation templates", self.templates.len());
    }

    /// Translate natural language to Cedar (13.8.3).
    pub fn translate(&self, natural_language: &str) -> TranslationResult {
        let start = Instant::now();

        // 13.8.1: Check cache first
        if let Some(cached) = self.check_cache(natural_language) {
            debug!("Cache hit for: {}", natural_language);
            return cached;
        }

        // Try template match
        if let Some(template_result) = self.try_template_match(natural_language) {
            let result = TranslationResult {
                cedar_policy: template_result,
                method: TranslationMethod::TemplateMatch,
                confidence: 0.95,
                template_used: Some("matched".to_string()),
                raw_llm_output: None
            };

            self.cache_result(natural_language, result.clone());
            return result;
        }

        // Use few-shot prompting (13.8.2)
        let few_shot_result = self.few_shot_translate(natural_language);

        if few_shot_result.confidence >= self.confidence_threshold {
            self.cache_result(natural_language, few_shot_result.clone());
            return few_shot_result;
        }

        // Fall back to LLM generation
        let llm_result = self.llm_translate(natural_language);

        // 13.8.4: Log translation method and confidence
        info!(
            "Translation: method={:?}, confidence={:.2}, input={}",
            llm_result.method, llm_result.confidence, natural_language
        );

        self.cache_result(natural_language, llm_result.clone());

        llm_result
    }

    /// Check cache for existing translation (13.8.1).
    fn check_cache(&self, input: &str) -> Option<TranslationResult> {
        let mut cache = self.cache.lock();

        if let Some(entry) = cache.get(input) {
            if entry.cached_at.elapsed() < self.cache_ttl {
                return Some(entry.result.clone());
            }
        }

        None
    }

    /// Cache translation result.
    fn cache_result(&self, input: &str, result: TranslationResult) {
        let mut cache = self.cache.lock();
        cache.put(
            input.to_string(),
            TranslationCacheEntry {
                result,
                cached_at: Instant::now()
            }
        );
    }

    /// Try to match input against templates.
    fn try_template_match(&self, input: &str) -> Option<String> {
        for (id, template) in &self.templates {
            if let Ok(regex) = regex::Regex::new(&template.pattern) {
                if regex.is_match(input) {
                    debug!("Template {} matched for: {}", id, input);
                    return Some(template.cedar_template.clone());
                }
            }
        }

        None
    }

    /// Few-shot translation using examples.
    fn few_shot_translate(&self, input: &str) -> TranslationResult {
        // In real implementation: call LLM with few-shot examples
        // For now, return a simulated result
        TranslationResult {
            cedar_policy: format!(
                "// Generated from: {}\npermit (principal, action, resource);",
                input
            ),
            method: TranslationMethod::FewShotPrompt,
            confidence: 0.85,
            template_used: None,
            raw_llm_output: None
        }
    }

    /// LLM translation.
    fn llm_translate(&self, input: &str) -> TranslationResult {
        // In real implementation: call LLM API
        TranslationResult {
            cedar_policy: format!(
                "// Generated from: {}\npermit (principal, action, resource);",
                input
            ),
            method: TranslationMethod::LLMGenerated,
            confidence: 0.75,
            template_used: None,
            raw_llm_output: None
        }
    }

    /// Get translation metrics.
    pub fn metrics(&self) -> TranslationMetrics {
        // In real implementation: track actual metrics
        TranslationMetrics::default()
    }

    /// Calculate template coverage.
    pub fn template_coverage(&self) -> f64 {
        // 13.8.2: Target 80% coverage
        let _total_categories = 5; // security, auth, authorization, standards, etc.
        let _implemented_categories = self
            .templates
            .values()
            .map(|t| &t.coverage_category)
            .collect::<std::collections::HashSet<_>>()
            .len();

        // Simplified calculation
        self.templates.len() as f64 / 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_matching() {
        let translator = DeterministicTranslator::new(100, 24);

        // Should match template
        let result = translator.translate("Block all external requests");
        assert_eq!(result.method, TranslationMethod::TemplateMatch);
        assert!(result.confidence >= 0.9);

        // Should match another template
        let result2 = translator.translate("Require authentication");
        assert_eq!(result2.method, TranslationMethod::TemplateMatch);
    }

    #[test]
    fn test_cache_hit() {
        let translator = DeterministicTranslator::new(100, 24);

        let input = "Require readme file";

        // First call
        let result1 = translator.translate(input);

        // Second call should hit cache
        let result2 = translator.translate(input);

        assert_eq!(result2.method, TranslationMethod::CacheHit);
    }
}
