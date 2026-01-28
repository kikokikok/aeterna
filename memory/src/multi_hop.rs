use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone)]
pub struct MultiHopConfig {
    pub max_hop_depth: u32,
    pub hop_relevance_threshold: f32,
    pub max_query_budget: u32
}

impl Default for MultiHopConfig {
    fn default() -> Self {
        Self {
            max_hop_depth: 3,
            hop_relevance_threshold: 0.3,
            max_query_budget: 50
        }
    }
}

impl From<&config::ReasoningConfig> for MultiHopConfig {
    fn from(config: &config::ReasoningConfig) -> Self {
        Self {
            max_hop_depth: config.max_hop_depth,
            hop_relevance_threshold: config.hop_relevance_threshold,
            max_query_budget: config.max_query_budget
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationReason {
    MaxDepthReached,
    LowRelevance,
    QueryBudgetExhausted,
    NoMoreResults,
    Completed
}

pub struct MultiHopContext {
    config: MultiHopConfig,
    current_depth: AtomicU32,
    queries_executed: AtomicU32,
    paths_terminated_depth: AtomicU32,
    paths_terminated_relevance: AtomicU32,
    paths_terminated_budget: AtomicU32,
    telemetry: Arc<crate::telemetry::MemoryTelemetry>
}

impl MultiHopContext {
    pub fn new(config: MultiHopConfig, telemetry: Arc<crate::telemetry::MemoryTelemetry>) -> Self {
        Self {
            config,
            current_depth: AtomicU32::new(0),
            queries_executed: AtomicU32::new(0),
            paths_terminated_depth: AtomicU32::new(0),
            paths_terminated_relevance: AtomicU32::new(0),
            paths_terminated_budget: AtomicU32::new(0),
            telemetry
        }
    }

    pub fn can_continue(&self, relevance_score: f32) -> Result<(), TerminationReason> {
        let depth = self.current_depth.load(Ordering::SeqCst);
        let queries = self.queries_executed.load(Ordering::SeqCst);

        if depth >= self.config.max_hop_depth {
            self.paths_terminated_depth.fetch_add(1, Ordering::SeqCst);
            return Err(TerminationReason::MaxDepthReached);
        }

        if relevance_score < self.config.hop_relevance_threshold {
            self.paths_terminated_relevance
                .fetch_add(1, Ordering::SeqCst);
            return Err(TerminationReason::LowRelevance);
        }

        if queries >= self.config.max_query_budget {
            self.paths_terminated_budget.fetch_add(1, Ordering::SeqCst);
            return Err(TerminationReason::QueryBudgetExhausted);
        }

        Ok(())
    }

    pub fn record_hop(&self) {
        self.current_depth.fetch_add(1, Ordering::SeqCst);
    }

    pub fn record_query(&self) {
        self.queries_executed.fetch_add(1, Ordering::SeqCst);
    }

    pub fn current_depth(&self) -> u32 {
        self.current_depth.load(Ordering::SeqCst)
    }

    pub fn queries_executed(&self) -> u32 {
        self.queries_executed.load(Ordering::SeqCst)
    }

    pub fn max_depth(&self) -> u32 {
        self.config.max_hop_depth
    }

    pub fn query_budget(&self) -> u32 {
        self.config.max_query_budget
    }

    pub fn relevance_threshold(&self) -> f32 {
        self.config.hop_relevance_threshold
    }

    pub fn finalize(&self) -> MultiHopMetrics {
        let metrics = MultiHopMetrics {
            max_depth_reached: self.current_depth.load(Ordering::SeqCst),
            total_queries: self.queries_executed.load(Ordering::SeqCst),
            paths_terminated_depth: self.paths_terminated_depth.load(Ordering::SeqCst),
            paths_terminated_relevance: self.paths_terminated_relevance.load(Ordering::SeqCst),
            paths_terminated_budget: self.paths_terminated_budget.load(Ordering::SeqCst)
        };

        self.telemetry.record_multi_hop_metrics(&metrics);
        metrics
    }
}

#[derive(Debug, Clone, Default)]
pub struct MultiHopMetrics {
    pub max_depth_reached: u32,
    pub total_queries: u32,
    pub paths_terminated_depth: u32,
    pub paths_terminated_relevance: u32,
    pub paths_terminated_budget: u32
}

impl MultiHopMetrics {
    pub fn total_paths_terminated(&self) -> u32 {
        self.paths_terminated_depth + self.paths_terminated_relevance + self.paths_terminated_budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_telemetry() -> Arc<crate::telemetry::MemoryTelemetry> {
        Arc::new(crate::telemetry::MemoryTelemetry::new())
    }

    #[test]
    fn test_default_config() {
        let config = MultiHopConfig::default();
        assert_eq!(config.max_hop_depth, 3);
        assert!((config.hop_relevance_threshold - 0.3).abs() < f32::EPSILON);
        assert_eq!(config.max_query_budget, 50);
    }

    #[test]
    fn test_can_continue_at_max_depth() {
        let config = MultiHopConfig {
            max_hop_depth: 2,
            ..Default::default()
        };
        let ctx = MultiHopContext::new(config, test_telemetry());

        ctx.record_hop();
        assert!(ctx.can_continue(0.5).is_ok());

        ctx.record_hop();
        assert_eq!(
            ctx.can_continue(0.5),
            Err(TerminationReason::MaxDepthReached)
        );
    }

    #[test]
    fn test_can_continue_low_relevance() {
        let config = MultiHopConfig {
            hop_relevance_threshold: 0.5,
            ..Default::default()
        };
        let ctx = MultiHopContext::new(config, test_telemetry());

        assert!(ctx.can_continue(0.6).is_ok());
        assert_eq!(ctx.can_continue(0.4), Err(TerminationReason::LowRelevance));
    }

    #[test]
    fn test_can_continue_budget_exhausted() {
        let config = MultiHopConfig {
            max_query_budget: 3,
            ..Default::default()
        };
        let ctx = MultiHopContext::new(config, test_telemetry());

        ctx.record_query();
        ctx.record_query();
        assert!(ctx.can_continue(0.5).is_ok());

        ctx.record_query();
        assert_eq!(
            ctx.can_continue(0.5),
            Err(TerminationReason::QueryBudgetExhausted)
        );
    }

    #[test]
    fn test_metrics_finalize() {
        let config = MultiHopConfig {
            max_hop_depth: 2,
            hop_relevance_threshold: 0.5,
            max_query_budget: 10
        };
        let ctx = MultiHopContext::new(config, test_telemetry());

        ctx.record_hop();
        ctx.record_query();
        ctx.record_query();

        let _ = ctx.can_continue(0.3);
        let _ = ctx.can_continue(0.3);

        ctx.record_hop();
        let _ = ctx.can_continue(0.6);

        let metrics = ctx.finalize();
        assert_eq!(metrics.max_depth_reached, 2);
        assert_eq!(metrics.total_queries, 2);
        assert_eq!(metrics.paths_terminated_relevance, 2);
        assert_eq!(metrics.paths_terminated_depth, 1);
    }

    #[test]
    fn test_context_accessors() {
        let config = MultiHopConfig {
            max_hop_depth: 5,
            hop_relevance_threshold: 0.25,
            max_query_budget: 100
        };
        let ctx = MultiHopContext::new(config, test_telemetry());

        assert_eq!(ctx.max_depth(), 5);
        assert!((ctx.relevance_threshold() - 0.25).abs() < f32::EPSILON);
        assert_eq!(ctx.query_budget(), 100);
        assert_eq!(ctx.current_depth(), 0);
        assert_eq!(ctx.queries_executed(), 0);

        ctx.record_hop();
        ctx.record_query();

        assert_eq!(ctx.current_depth(), 1);
        assert_eq!(ctx.queries_executed(), 1);
    }

    #[test]
    fn test_total_paths_terminated() {
        let metrics = MultiHopMetrics {
            max_depth_reached: 3,
            total_queries: 10,
            paths_terminated_depth: 2,
            paths_terminated_relevance: 5,
            paths_terminated_budget: 1
        };
        assert_eq!(metrics.total_paths_terminated(), 8);
    }
}
