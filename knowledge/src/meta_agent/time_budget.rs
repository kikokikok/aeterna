use std::time::{Duration, Instant};

use tracing::{info_span, warn};

#[derive(Debug, Clone)]
pub struct TimeBudgetConfig {
    pub total_duration: Duration,
    pub warning_threshold_percent: f64,
}

impl Default for TimeBudgetConfig {
    fn default() -> Self {
        Self {
            total_duration: Duration::from_secs(300),
            warning_threshold_percent: 75.0,
        }
    }
}

impl TimeBudgetConfig {
    pub fn with_duration_secs(mut self, secs: u64) -> Self {
        self.total_duration = Duration::from_secs(secs);
        self
    }

    pub fn with_warning_threshold(mut self, percent: f64) -> Self {
        self.warning_threshold_percent = percent.clamp(0.0, 100.0);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetStatus {
    Available,
    Warning,
    Exhausted,
}

#[derive(Debug, Clone)]
pub struct BudgetCheck {
    pub status: BudgetStatus,
    pub elapsed: Duration,
    pub remaining: Duration,
    pub percent_used: f64,
}

impl BudgetCheck {
    pub fn is_available(&self) -> bool {
        self.status != BudgetStatus::Exhausted
    }

    pub fn is_warning(&self) -> bool {
        self.status == BudgetStatus::Warning
    }

    pub fn is_exhausted(&self) -> bool {
        self.status == BudgetStatus::Exhausted
    }
}

pub struct TimeBudget {
    config: TimeBudgetConfig,
    start: Instant,
    warning_logged: bool,
}

impl TimeBudget {
    pub fn start(config: TimeBudgetConfig) -> Self {
        let span = info_span!(
            "time_budget_start",
            total_secs = config.total_duration.as_secs(),
            warning_threshold = config.warning_threshold_percent
        );
        let _guard = span.enter();

        Self {
            config,
            start: Instant::now(),
            warning_logged: false,
        }
    }

    pub fn start_with_default() -> Self {
        Self::start(TimeBudgetConfig::default())
    }

    pub fn check(&mut self) -> BudgetCheck {
        let elapsed = self.start.elapsed();
        let total = self.config.total_duration;

        if elapsed >= total {
            return BudgetCheck {
                status: BudgetStatus::Exhausted,
                elapsed,
                remaining: Duration::ZERO,
                percent_used: 100.0,
            };
        }

        let remaining = total - elapsed;
        let percent_used = (elapsed.as_secs_f64() / total.as_secs_f64()) * 100.0;

        let status = if percent_used >= self.config.warning_threshold_percent {
            if !self.warning_logged {
                warn!(
                    elapsed_secs = elapsed.as_secs(),
                    remaining_secs = remaining.as_secs(),
                    percent_used = format!("{:.1}", percent_used),
                    "Time budget warning threshold reached"
                );
                self.warning_logged = true;
            }
            BudgetStatus::Warning
        } else {
            BudgetStatus::Available
        };

        BudgetCheck {
            status,
            elapsed,
            remaining,
            percent_used,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn remaining(&self) -> Duration {
        let elapsed = self.start.elapsed();
        if elapsed >= self.config.total_duration {
            Duration::ZERO
        } else {
            self.config.total_duration - elapsed
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.start.elapsed() >= self.config.total_duration
    }

    pub fn percent_used(&self) -> f64 {
        let elapsed = self.start.elapsed();
        (elapsed.as_secs_f64() / self.config.total_duration.as_secs_f64()) * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct TimeBudgetExhaustedResult {
    pub elapsed: Duration,
    pub iterations_completed: u32,
    pub partial_results: Option<String>,
}

impl TimeBudgetExhaustedResult {
    pub fn new(elapsed: Duration, iterations_completed: u32) -> Self {
        Self {
            elapsed,
            iterations_completed,
            partial_results: None,
        }
    }

    pub fn with_partial_results(mut self, results: impl Into<String>) -> Self {
        self.partial_results = Some(results.into());
        self
    }

    pub fn format_message(&self) -> String {
        let mut msg = format!(
            "Time budget exhausted after {:.1}s ({} iterations completed)",
            self.elapsed.as_secs_f64(),
            self.iterations_completed
        );
        if let Some(ref partial) = self.partial_results {
            msg.push_str(&format!("\nPartial results: {}", partial));
        }
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_time_budget_config_default() {
        let config = TimeBudgetConfig::default();
        assert_eq!(config.total_duration, Duration::from_secs(300));
        assert_eq!(config.warning_threshold_percent, 75.0);
    }

    #[test]
    fn test_time_budget_config_builder() {
        let config = TimeBudgetConfig::default()
            .with_duration_secs(60)
            .with_warning_threshold(80.0);
        assert_eq!(config.total_duration, Duration::from_secs(60));
        assert_eq!(config.warning_threshold_percent, 80.0);
    }

    #[test]
    fn test_time_budget_config_clamp_threshold() {
        let config = TimeBudgetConfig::default().with_warning_threshold(150.0);
        assert_eq!(config.warning_threshold_percent, 100.0);

        let config = TimeBudgetConfig::default().with_warning_threshold(-10.0);
        assert_eq!(config.warning_threshold_percent, 0.0);
    }

    #[test]
    fn test_time_budget_fresh_start() {
        let mut budget = TimeBudget::start(TimeBudgetConfig::default().with_duration_secs(60));
        let check = budget.check();

        assert_eq!(check.status, BudgetStatus::Available);
        assert!(check.percent_used < 1.0);
        assert!(check.remaining.as_secs() >= 59);
    }

    #[test]
    fn test_time_budget_exhausted() {
        let config = TimeBudgetConfig::default().with_duration_secs(0);
        let mut budget = TimeBudget::start(config);

        thread::sleep(Duration::from_millis(10));
        let check = budget.check();

        assert_eq!(check.status, BudgetStatus::Exhausted);
        assert!(check.is_exhausted());
        assert_eq!(check.remaining, Duration::ZERO);
        assert!(check.percent_used >= 100.0);
    }

    #[test]
    fn test_budget_check_methods() {
        let available = BudgetCheck {
            status: BudgetStatus::Available,
            elapsed: Duration::from_secs(10),
            remaining: Duration::from_secs(50),
            percent_used: 16.67,
        };
        assert!(available.is_available());
        assert!(!available.is_warning());
        assert!(!available.is_exhausted());

        let warning = BudgetCheck {
            status: BudgetStatus::Warning,
            elapsed: Duration::from_secs(45),
            remaining: Duration::from_secs(15),
            percent_used: 75.0,
        };
        assert!(warning.is_available());
        assert!(warning.is_warning());
        assert!(!warning.is_exhausted());

        let exhausted = BudgetCheck {
            status: BudgetStatus::Exhausted,
            elapsed: Duration::from_secs(60),
            remaining: Duration::ZERO,
            percent_used: 100.0,
        };
        assert!(!exhausted.is_available());
        assert!(!exhausted.is_warning());
        assert!(exhausted.is_exhausted());
    }

    #[test]
    fn test_time_budget_is_exhausted() {
        let budget = TimeBudget::start(TimeBudgetConfig::default().with_duration_secs(0));
        thread::sleep(Duration::from_millis(5));
        assert!(budget.is_exhausted());
    }

    #[test]
    fn test_time_budget_percent_used() {
        let budget = TimeBudget::start(TimeBudgetConfig::default().with_duration_secs(100));
        let percent = budget.percent_used();
        assert!(percent < 1.0);
    }

    #[test]
    fn test_time_budget_remaining() {
        let budget = TimeBudget::start(TimeBudgetConfig::default().with_duration_secs(60));
        let remaining = budget.remaining();
        assert!(remaining.as_secs() >= 59);
    }

    #[test]
    fn test_time_budget_remaining_exhausted() {
        let budget = TimeBudget::start(TimeBudgetConfig::default().with_duration_secs(0));
        thread::sleep(Duration::from_millis(5));
        assert_eq!(budget.remaining(), Duration::ZERO);
    }

    #[test]
    fn test_time_budget_exhausted_result() {
        let result = TimeBudgetExhaustedResult::new(Duration::from_secs(120), 2);
        assert_eq!(result.iterations_completed, 2);
        assert!(result.partial_results.is_none());

        let msg = result.format_message();
        assert!(msg.contains("120"));
        assert!(msg.contains("2 iterations"));
    }

    #[test]
    fn test_time_budget_exhausted_result_with_partial() {
        let result = TimeBudgetExhaustedResult::new(Duration::from_secs(60), 1)
            .with_partial_results("partial build output");

        let msg = result.format_message();
        assert!(msg.contains("partial build output"));
    }

    #[test]
    fn test_warning_logged_once() {
        let config = TimeBudgetConfig::default()
            .with_duration_secs(1)
            .with_warning_threshold(1.0);
        let mut budget = TimeBudget::start(config);

        thread::sleep(Duration::from_millis(50));
        let check1 = budget.check();
        assert!(check1.is_warning() || check1.is_exhausted());

        let check2 = budget.check();
        assert!(check2.is_warning() || check2.is_exhausted());
    }
}
