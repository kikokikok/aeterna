use mk_core::types::SummaryDepth;

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub system: String,
    pub user: String
}

#[derive(Debug, Clone)]
pub struct PromptTemplates {
    pub sentence: PromptTemplate,
    pub paragraph: PromptTemplate,
    pub detailed: PromptTemplate
}

impl Default for PromptTemplates {
    fn default() -> Self {
        Self {
            sentence: PromptTemplate {
                system: SENTENCE_SYSTEM.to_string(),
                user: SENTENCE_USER.to_string()
            },
            paragraph: PromptTemplate {
                system: PARAGRAPH_SYSTEM.to_string(),
                user: PARAGRAPH_USER.to_string()
            },
            detailed: PromptTemplate {
                system: DETAILED_SYSTEM.to_string(),
                user: DETAILED_USER.to_string()
            }
        }
    }
}

impl PromptTemplates {
    pub fn get_template(&self, depth: SummaryDepth) -> &PromptTemplate {
        match depth {
            SummaryDepth::Sentence => &self.sentence,
            SummaryDepth::Paragraph => &self.paragraph,
            SummaryDepth::Detailed => &self.detailed
        }
    }

    pub fn build_prompt(
        &self,
        content: &str,
        depth: SummaryDepth,
        context: Option<&str>,
        personalization: Option<&str>,
        max_tokens: u32
    ) -> (String, String) {
        let template = self.get_template(depth);

        let system = self.build_system_prompt(&template.system, personalization);
        let user = self.build_user_prompt(&template.user, content, context, max_tokens);

        (system, user)
    }

    fn build_system_prompt(&self, base: &str, personalization: Option<&str>) -> String {
        match personalization {
            Some(ctx) => format!("{base}\n\nPersonalization context: {ctx}"),
            None => base.to_string()
        }
    }

    fn build_user_prompt(
        &self,
        base: &str,
        content: &str,
        context: Option<&str>,
        max_tokens: u32
    ) -> String {
        let context_section = context
            .map(|c| format!("Context: {c}\n\n"))
            .unwrap_or_default();

        base.replace("{content}", content)
            .replace("{context}", &context_section)
            .replace("{max_tokens}", &max_tokens.to_string())
    }
}

const SENTENCE_SYSTEM: &str = "\
You are a precise summarization assistant. Your task is to distill content into exactly ONE clear, \
                               informative sentence. Focus on the most critical information. \
                               Never use phrases like 'This document describes...' or 'The \
                               content covers...'. State the key point directly.";

const SENTENCE_USER: &str = "\
{context}Summarize the following content in exactly ONE sentence (maximum {max_tokens} tokens):

{content}";

const PARAGRAPH_SYSTEM: &str = "\
You are a summarization assistant specializing in concise paragraph summaries. Your task is to \
                                capture all key points in a single well-structured paragraph. \
                                Maintain logical flow and ensure completeness. Avoid introductory \
                                phrases and filler content.";

const PARAGRAPH_USER: &str = "\
{context}Summarize the following content in ONE paragraph (maximum {max_tokens} tokens). Include \
                              all significant points while maintaining clarity:

{content}";

const DETAILED_SYSTEM: &str = "\
You are a comprehensive summarization assistant. Your task is to create detailed summaries that \
                               preserve important nuances, context, and relationships within the \
                               content. Use clear structure with bullet points if appropriate. \
                               Maintain the original meaning while being concise.";

const DETAILED_USER: &str = "\
{context}Create a detailed summary of the following content (maximum {max_tokens} tokens). \
                             Preserve key details, relationships, and nuances:

{content}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_templates_exist() {
        let templates = PromptTemplates::default();

        assert!(!templates.sentence.system.is_empty());
        assert!(!templates.sentence.user.is_empty());
        assert!(!templates.paragraph.system.is_empty());
        assert!(!templates.paragraph.user.is_empty());
        assert!(!templates.detailed.system.is_empty());
        assert!(!templates.detailed.user.is_empty());
    }

    #[test]
    fn test_get_template_by_depth() {
        let templates = PromptTemplates::default();

        let sentence = templates.get_template(SummaryDepth::Sentence);
        let paragraph = templates.get_template(SummaryDepth::Paragraph);
        let detailed = templates.get_template(SummaryDepth::Detailed);

        assert!(sentence.system.contains("ONE"));
        assert!(paragraph.system.contains("paragraph"));
        assert!(detailed.system.contains("detailed"));
    }

    #[test]
    fn test_build_prompt_basic() {
        let templates = PromptTemplates::default();
        let content = "Test content here";

        let (system, user) =
            templates.build_prompt(content, SummaryDepth::Sentence, None, None, 50);

        assert!(!system.is_empty());
        assert!(user.contains("Test content here"));
        assert!(user.contains("50"));
    }

    #[test]
    fn test_build_prompt_with_context() {
        let templates = PromptTemplates::default();
        let content = "Test content";
        let context = "Technical documentation";

        let (_, user) =
            templates.build_prompt(content, SummaryDepth::Paragraph, Some(context), None, 200);

        assert!(user.contains("Context: Technical documentation"));
        assert!(user.contains("Test content"));
    }

    #[test]
    fn test_build_prompt_with_personalization() {
        let templates = PromptTemplates::default();
        let content = "Test content";
        let personalization = "developer audience";

        let (system, _) = templates.build_prompt(
            content,
            SummaryDepth::Detailed,
            None,
            Some(personalization),
            500
        );

        assert!(system.contains("Personalization context: developer audience"));
    }

    #[test]
    fn test_build_prompt_with_all_options() {
        let templates = PromptTemplates::default();
        let content = "Complex technical content";
        let context = "API documentation";
        let personalization = "senior engineers";

        let (system, user) = templates.build_prompt(
            content,
            SummaryDepth::Detailed,
            Some(context),
            Some(personalization),
            500
        );

        assert!(system.contains("senior engineers"));
        assert!(user.contains("API documentation"));
        assert!(user.contains("Complex technical content"));
        assert!(user.contains("500"));
    }

    #[test]
    fn test_sentence_template_constraints() {
        let templates = PromptTemplates::default();
        let template = templates.get_template(SummaryDepth::Sentence);

        assert!(template.system.contains("ONE"));
        assert!(template.system.contains("sentence"));
        assert!(template.user.contains("{content}"));
        assert!(template.user.contains("{max_tokens}"));
    }

    #[test]
    fn test_paragraph_template_constraints() {
        let templates = PromptTemplates::default();
        let template = templates.get_template(SummaryDepth::Paragraph);

        assert!(template.system.contains("paragraph"));
        assert!(template.user.contains("{content}"));
    }

    #[test]
    fn test_detailed_template_constraints() {
        let templates = PromptTemplates::default();
        let template = templates.get_template(SummaryDepth::Detailed);

        assert!(template.system.contains("detailed"));
        assert!(template.system.contains("comprehensive"));
    }

    #[test]
    fn test_custom_templates() {
        let custom = PromptTemplates {
            sentence: PromptTemplate {
                system: "Custom system".to_string(),
                user: "Custom user: {content}".to_string()
            },
            paragraph: PromptTemplate {
                system: "Para system".to_string(),
                user: "Para user: {content}".to_string()
            },
            detailed: PromptTemplate {
                system: "Detail system".to_string(),
                user: "Detail user: {content}".to_string()
            }
        };

        let (system, user) = custom.build_prompt("Test", SummaryDepth::Sentence, None, None, 50);

        assert_eq!(system, "Custom system");
        assert!(user.contains("Custom user"));
    }
}
