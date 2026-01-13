#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use memory::reasoning::ReflectiveReasoner;
    use mk_core::types::{ReasoningStrategy, ReasoningTrace};

    struct MockReasoner;

    #[async_trait]
    impl ReflectiveReasoner for MockReasoner {
        async fn reason(
            &self,
            query: &str,
            _context: Option<&str>,
        ) -> anyhow::Result<ReasoningTrace> {
            Ok(ReasoningTrace {
                strategy: ReasoningStrategy::Targeted,
                thought_process: format!("Mock reasoning for: {}", query),
                refined_query: Some(query.to_string()),
                start_time: Utc::now(),
                end_time: Utc::now(),
                metadata: std::collections::HashMap::new(),
            })
        }
    }

    #[tokio::test]
    async fn test_mock_reasoner() {
        let reasoner = MockReasoner;
        let trace = reasoner.reason("test query", None).await.unwrap();
        assert_eq!(trace.strategy, ReasoningStrategy::Targeted);
        assert!(trace.thought_process.contains("test query"));
    }
}
