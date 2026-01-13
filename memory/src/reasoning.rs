use async_trait::async_trait;
use chrono::Utc;
use mk_core::traits::LlmService;
use mk_core::types::{ReasoningStrategy, ReasoningTrace};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReasoningPlan {
    pub strategy: ReasoningStrategy,
    pub refined_query: String,
    pub reasoning: String,
}

#[async_trait]
pub trait ReflectiveReasoner: Send + Sync {
    async fn reason(
        &self,
        query: &str,
        context_summary: Option<&str>,
    ) -> anyhow::Result<ReasoningTrace>;
}

pub struct DefaultReflectiveReasoner {
    llm: Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>>>,
}

impl DefaultReflectiveReasoner {
    pub fn new(llm: Arc<dyn LlmService<Error = Box<dyn std::error::Error + Send + Sync>>>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl ReflectiveReasoner for DefaultReflectiveReasoner {
    async fn reason(
        &self,
        query: &str,
        context_summary: Option<&str>,
    ) -> anyhow::Result<ReasoningTrace> {
        let start_time = Utc::now();

        let prompt = format!(
            "Given the following user query and optional context summary, determine the best retrieval strategy \
            and refine the query for vector search.\n\n\
            Query: {}\n\
            Context Summary: {}\n\n\
            Return your response in JSON format:\n\
            {{\n\
              \"strategy\": \"exhaustive\" | \"targeted\" | \"semanticOnly\",\n\
              \"refined_query\": \"...\",\n\
              \"reasoning\": \"...\"\n\
            }}",
            query,
            context_summary.unwrap_or("None")
        );

        let response = self
            .llm
            .generate(&prompt)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        // Extract JSON if needed
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };

        let plan: ReasoningPlan = serde_json::from_str(json_str)?;

        Ok(ReasoningTrace {
            strategy: plan.strategy,
            thought_process: plan.reasoning,
            refined_query: Some(plan.refined_query),
            start_time,
            end_time: Utc::now(),
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::mock::MockLlmService;

    #[tokio::test]
    async fn test_reasoning_logic() {
        let mut mock_llm = MockLlmService::new();
        let plan_json = "{\"strategy\": \"exhaustive\", \"refined_query\": \"deep search for memory-r1 architecture\", \"reasoning\": \"The query is specific and complex, requiring cross-layer verification.\"}";
        mock_llm.set_response(plan_json).await;

        let reasoner = DefaultReflectiveReasoner::new(Arc::new(mock_llm));
        let trace = reasoner.reason("memory-r1", None).await.unwrap();

        assert_eq!(trace.strategy, ReasoningStrategy::Exhaustive);
        assert_eq!(
            trace.refined_query.unwrap(),
            "deep search for memory-r1 architecture"
        );
        assert!(trace.thought_process.contains("complex"));
    }
}
