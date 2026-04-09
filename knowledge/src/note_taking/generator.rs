use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::distiller::DistillationResult;
use crate::context_architect::ViewMode;

#[derive(Debug, Clone)]
pub struct NoteGeneratorConfig {
    pub include_code_snippets: bool,
    pub include_metadata: bool,
    pub max_pattern_length: usize,
}

impl Default for NoteGeneratorConfig {
    fn default() -> Self {
        Self {
            include_code_snippets: true,
            include_metadata: true,
            max_pattern_length: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedNote {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub source_distillation_id: String,
    pub created_at: u64,
    pub quality_score: f32,
}

impl GeneratedNote {
    pub fn frontmatter(&self) -> String {
        format!(
            "---\ntitle: \"{}\"\ntags: [{}]\ncreated: {}\nquality: {:.2}\n---",
            self.title,
            self.tags
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(", "),
            self.created_at,
            self.quality_score
        )
    }

    pub fn full_markdown(&self) -> String {
        format!("{}\n\n{}", self.frontmatter(), self.content)
    }
}

pub struct NoteTemplate {
    pub sections: Vec<NoteSection>,
}

impl Default for NoteTemplate {
    fn default() -> Self {
        Self {
            sections: vec![
                NoteSection::Context,
                NoteSection::Problem,
                NoteSection::Solution,
                NoteSection::Patterns,
                NoteSection::CodeSnippets,
                NoteSection::Metadata,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSection {
    Context,
    Problem,
    Solution,
    Patterns,
    CodeSnippets,
    Metadata,
}

pub struct NoteGenerator {
    config: NoteGeneratorConfig,
    template: NoteTemplate,
}

impl NoteGeneratorConfig {
    pub fn for_view_mode(view_mode: ViewMode) -> Self {
        match view_mode {
            ViewMode::Ax => Self {
                include_code_snippets: false,
                include_metadata: false,
                max_pattern_length: 200,
            },
            ViewMode::Ux => Self {
                include_code_snippets: false,
                include_metadata: true,
                max_pattern_length: 300,
            },
            ViewMode::Dx => Self::default(),
        }
    }
}

impl NoteTemplate {
    pub fn for_view_mode(view_mode: ViewMode) -> Self {
        let sections = match view_mode {
            ViewMode::Ax => vec![NoteSection::Problem, NoteSection::Solution],
            ViewMode::Ux => vec![
                NoteSection::Context,
                NoteSection::Problem,
                NoteSection::Solution,
                NoteSection::Patterns,
                NoteSection::Metadata,
            ],
            ViewMode::Dx => vec![
                NoteSection::Context,
                NoteSection::Problem,
                NoteSection::Solution,
                NoteSection::Patterns,
                NoteSection::CodeSnippets,
                NoteSection::Metadata,
            ],
        };
        Self { sections }
    }
}

impl NoteGenerator {
    pub fn new(config: NoteGeneratorConfig) -> Self {
        Self {
            config,
            template: NoteTemplate::default(),
        }
    }

    pub fn with_template(mut self, template: NoteTemplate) -> Self {
        self.template = template;
        self
    }

    pub fn generate(&self, distillation: &DistillationResult) -> GeneratedNote {
        let title = self.generate_title(distillation);
        let content = self.generate_content(distillation);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        GeneratedNote {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            content,
            tags: distillation.tags.clone(),
            source_distillation_id: distillation.id.clone(),
            created_at: timestamp,
            quality_score: distillation.quality_score,
        }
    }

    pub fn generate_for_view(
        &self,
        distillation: &DistillationResult,
        view_mode: ViewMode,
    ) -> GeneratedNote {
        let template = NoteTemplate::for_view_mode(view_mode);
        let config = NoteGeneratorConfig::for_view_mode(view_mode);
        NoteGenerator::new(config)
            .with_template(template)
            .generate(distillation)
    }

    fn generate_title(&self, distillation: &DistillationResult) -> String {
        if !distillation.problem.is_empty() {
            let problem = &distillation.problem;
            if problem.len() <= 60 {
                return problem.clone();
            }
            return format!("{}...", &problem[..57]);
        }

        if !distillation.context.is_empty() {
            let context = &distillation.context;
            if context.len() <= 60 {
                return context.clone();
            }
            return format!("{}...", &context[..57]);
        }

        format!("Note from {}", distillation.trigger)
    }

    fn generate_content(&self, distillation: &DistillationResult) -> String {
        let mut sections = Vec::new();

        for section in &self.template.sections {
            if let Some(content) = self.render_section(*section, distillation) {
                sections.push(content);
            }
        }

        sections.join("\n\n")
    }

    fn render_section(
        &self,
        section: NoteSection,
        distillation: &DistillationResult,
    ) -> Option<String> {
        match section {
            NoteSection::Context => {
                if distillation.context.is_empty() {
                    None
                } else {
                    Some(format!("## Context\n\n{}", distillation.context))
                }
            }
            NoteSection::Problem => {
                if distillation.problem.is_empty() {
                    None
                } else {
                    Some(format!("## Problem\n\n{}", distillation.problem))
                }
            }
            NoteSection::Solution => {
                if distillation.solution.is_empty() {
                    None
                } else {
                    Some(format!("## Solution\n\n{}", distillation.solution))
                }
            }
            NoteSection::Patterns => {
                if distillation.patterns.is_empty() {
                    None
                } else {
                    let patterns: Vec<_> = distillation
                        .patterns
                        .iter()
                        .map(|p| {
                            if p.len() > self.config.max_pattern_length {
                                format!("- {}...", &p[..self.config.max_pattern_length - 3])
                            } else {
                                format!("- {p}")
                            }
                        })
                        .collect();
                    Some(format!("## Patterns\n\n{}", patterns.join("\n")))
                }
            }
            NoteSection::CodeSnippets => {
                if !self.config.include_code_snippets || distillation.code_snippets.is_empty() {
                    None
                } else {
                    let snippets: Vec<_> = distillation
                        .code_snippets
                        .iter()
                        .map(|s| format!("```\n{s}\n```"))
                        .collect();
                    Some(format!("## Code Examples\n\n{}", snippets.join("\n\n")))
                }
            }
            NoteSection::Metadata => {
                if !self.config.include_metadata {
                    None
                } else {
                    Some(format!(
                        "## Metadata\n\n- Trigger: {}\n- Source Events: {}\n- Quality Score: {:.2}",
                        distillation.trigger,
                        distillation.source_event_count,
                        distillation.quality_score
                    ))
                }
            }
        }
    }

    pub fn generate_batch(&self, distillations: &[DistillationResult]) -> Vec<GeneratedNote> {
        distillations.iter().map(|d| self.generate(d)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_distillation() -> DistillationResult {
        DistillationResult {
            id: "dist-123".to_string(),
            trigger: "SessionEnd".to_string(),
            context: "Working on Rust project".to_string(),
            problem: "How to handle async errors".to_string(),
            solution: "Use anyhow crate with ? operator".to_string(),
            patterns: vec!["Error handling pattern".to_string()],
            tags: vec!["rust".to_string(), "async".to_string()],
            code_snippets: vec!["fn main() -> Result<()> {}".to_string()],
            quality_score: 0.85,
            distilled_at: 1234567890,
            source_event_count: 5,
        }
    }

    #[test]
    fn test_generate_note() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert_eq!(note.title, "How to handle async errors");
        assert!(note.content.contains("## Context"));
        assert!(note.content.contains("## Solution"));
        assert!(!note.id.is_empty());
    }

    #[test]
    fn test_generate_title_from_problem() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert_eq!(note.title, distillation.problem);
    }

    #[test]
    fn test_generate_title_truncation() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let mut distillation = sample_distillation();
        distillation.problem = "A".repeat(100);

        let note = generator.generate(&distillation);

        assert!(note.title.len() <= 60);
        assert!(note.title.ends_with("..."));
    }

    #[test]
    fn test_generate_title_fallback_to_context() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let mut distillation = sample_distillation();
        distillation.problem = String::new();

        let note = generator.generate(&distillation);

        assert_eq!(note.title, distillation.context);
    }

    #[test]
    fn test_content_includes_all_sections() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert!(note.content.contains("## Context"));
        assert!(note.content.contains("## Problem"));
        assert!(note.content.contains("## Solution"));
        assert!(note.content.contains("## Patterns"));
        assert!(note.content.contains("## Code Examples"));
        assert!(note.content.contains("## Metadata"));
    }

    #[test]
    fn test_excludes_empty_sections() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let mut distillation = sample_distillation();
        distillation.patterns = vec![];

        let note = generator.generate(&distillation);

        assert!(!note.content.contains("## Patterns"));
    }

    #[test]
    fn test_config_excludes_code_snippets() {
        let config = NoteGeneratorConfig {
            include_code_snippets: false,
            ..Default::default()
        };
        let generator = NoteGenerator::new(config);
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert!(!note.content.contains("## Code Examples"));
    }

    #[test]
    fn test_config_excludes_metadata() {
        let config = NoteGeneratorConfig {
            include_metadata: false,
            ..Default::default()
        };
        let generator = NoteGenerator::new(config);
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert!(!note.content.contains("## Metadata"));
    }

    #[test]
    fn test_frontmatter_generation() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);
        let frontmatter = note.frontmatter();

        assert!(frontmatter.starts_with("---"));
        assert!(frontmatter.contains("title:"));
        assert!(frontmatter.contains("tags:"));
        assert!(frontmatter.contains("quality:"));
    }

    #[test]
    fn test_full_markdown() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);
        let markdown = note.full_markdown();

        assert!(markdown.starts_with("---"));
        assert!(markdown.contains("## Context"));
    }

    #[test]
    fn test_batch_generation() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillations = vec![sample_distillation(), sample_distillation()];

        let notes = generator.generate_batch(&distillations);

        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn test_custom_template() {
        let template = NoteTemplate {
            sections: vec![NoteSection::Problem, NoteSection::Solution],
        };
        let generator = NoteGenerator::new(NoteGeneratorConfig::default()).with_template(template);
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert!(note.content.contains("## Problem"));
        assert!(note.content.contains("## Solution"));
        assert!(!note.content.contains("## Context"));
        assert!(!note.content.contains("## Metadata"));
    }

    #[test]
    fn test_view_mode_ax_limits_sections() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate_for_view(&distillation, ViewMode::Ax);

        assert!(note.content.contains("## Problem"));
        assert!(note.content.contains("## Solution"));
        assert!(!note.content.contains("## Context"));
        assert!(!note.content.contains("## Metadata"));
    }

    #[test]
    fn test_view_mode_ux_includes_context() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate_for_view(&distillation, ViewMode::Ux);

        assert!(note.content.contains("## Context"));
        assert!(note.content.contains("## Problem"));
        assert!(note.content.contains("## Metadata"));
        assert!(!note.content.contains("## Code Examples"));
    }

    #[test]
    fn test_pattern_length_truncation() {
        let config = NoteGeneratorConfig {
            max_pattern_length: 20,
            ..Default::default()
        };
        let generator = NoteGenerator::new(config);
        let mut distillation = sample_distillation();
        distillation.patterns = vec!["A".repeat(50)];

        let note = generator.generate(&distillation);

        assert!(note.content.contains("..."));
    }

    #[test]
    fn test_tags_preserved() {
        let generator = NoteGenerator::new(NoteGeneratorConfig::default());
        let distillation = sample_distillation();

        let note = generator.generate(&distillation);

        assert_eq!(note.tags, distillation.tags);
    }
}
