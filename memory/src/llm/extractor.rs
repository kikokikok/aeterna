use mk_core::traits::LlmService;
use mk_core::types::MemoryEntry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    pub label: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelation {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<ExtractedEntity>,
    pub relations: Vec<ExtractedRelation>,
}

pub struct EntityExtractor {
    llm: Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
}

impl EntityExtractor {
    pub fn new(
        llm: Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>> + Send + Sync>,
    ) -> Self {
        Self { llm }
    }

    pub async fn extract(
        &self,
        entry: &MemoryEntry,
    ) -> Result<ExtractionResult, Box<dyn std::error::Error + Send + Sync>> {
        let prompt = format!(
            "Extract entities and relationships from the following memory content in JSON \
             format.\nContent: {}\n\nExpected JSON structure:\n{{\n\"entities\": [{{ \"name\": \
             \"entity name\", \"label\": \"category\", \"properties\": {{}} }}],\n\"relations\": \
             [{{ \"source\": \"entity A\", \"target\": \"entity B\", \"relation\": \"relationship \
             type\", \"properties\": {{}} }}]\n}}",
            entry.content
        );

        let response = self.llm.generate(&prompt).await?;

        let json_start = response
            .find('{')
            .ok_or("No JSON object found in response")?;
        let json_end = response
            .rfind('}')
            .ok_or("No JSON object found in response")?
            + 1;
        let json_str = &response[json_start..json_end];

        let result: ExtractionResult = serde_json::from_str(json_str)?;
        Ok(result)
    }
}
