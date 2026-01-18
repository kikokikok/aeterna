use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use mk_core::types::{MemoryLayer, SummaryConfig, SummaryDepth};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct TriggerMonitorConfig {
    pub default_check_interval_secs: u64,
    pub enable_time_based_triggers: bool,
    pub enable_change_count_triggers: bool,
    pub enable_hash_based_triggers: bool
}

impl Default for TriggerMonitorConfig {
    fn default() -> Self {
        Self {
            default_check_interval_secs: 300,
            enable_time_based_triggers: true,
            enable_change_count_triggers: true,
            enable_hash_based_triggers: true
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerReason {
    TimeThresholdExceeded {
        age_seconds: u64,
        threshold_seconds: u64
    },
    ChangeCountExceeded {
        change_count: u32,
        threshold: u32
    },
    SourceHashChanged {
        previous_hash: String,
        new_hash: String
    },
    NoExistingSummary,
    ManualRefresh,
    ParentSummaryInvalidated {
        parent_layer: MemoryLayer
    }
}

#[derive(Debug, Clone)]
pub struct TriggerResult {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub depth: SummaryDepth,
    pub reason: TriggerReason,
    pub triggered_at: i64
}

#[derive(Debug, Clone)]
pub struct EntryState {
    pub entry_id: String,
    pub layer: MemoryLayer,
    pub source_hash: String,
    pub change_count: u32,
    pub summaries: HashMap<SummaryDepth, SummaryState>,
    pub last_modified_at: i64
}

#[derive(Debug, Clone)]
pub struct SummaryState {
    pub content_hash: String,
    pub generated_at: i64,
    pub is_stale: bool
}

pub struct SummaryTriggerMonitor {
    config: TriggerMonitorConfig,
    layer_configs: RwLock<HashMap<MemoryLayer, SummaryConfig>>,
    entry_states: RwLock<HashMap<String, EntryState>>
}

impl SummaryTriggerMonitor {
    pub fn new(config: TriggerMonitorConfig) -> Self {
        Self {
            config,
            layer_configs: RwLock::new(HashMap::new()),
            entry_states: RwLock::new(HashMap::new())
        }
    }

    pub async fn register_layer_config(&self, config: SummaryConfig) {
        let mut configs = self.layer_configs.write().await;
        configs.insert(config.layer, config);
    }

    pub async fn get_layer_config(&self, layer: MemoryLayer) -> Option<SummaryConfig> {
        let configs = self.layer_configs.read().await;
        configs.get(&layer).cloned()
    }

    pub async fn record_change(&self, entry_id: &str, layer: MemoryLayer, new_source_hash: &str) {
        let mut states = self.entry_states.write().await;
        let now = current_timestamp();

        let entry = states
            .entry(entry_id.to_string())
            .or_insert_with(|| EntryState {
                entry_id: entry_id.to_string(),
                layer,
                source_hash: new_source_hash.to_string(),
                change_count: 0,
                summaries: HashMap::new(),
                last_modified_at: now
            });

        entry.change_count += 1;
        entry.source_hash = new_source_hash.to_string();
        entry.last_modified_at = now;
    }

    pub async fn record_summary_generated(
        &self,
        entry_id: &str,
        layer: MemoryLayer,
        depth: SummaryDepth,
        content_hash: &str,
        source_hash: &str
    ) {
        let mut states = self.entry_states.write().await;
        let now = current_timestamp();

        let entry = states
            .entry(entry_id.to_string())
            .or_insert_with(|| EntryState {
                entry_id: entry_id.to_string(),
                layer,
                source_hash: source_hash.to_string(),
                change_count: 0,
                summaries: HashMap::new(),
                last_modified_at: now
            });

        entry.summaries.insert(
            depth,
            SummaryState {
                content_hash: content_hash.to_string(),
                generated_at: now,
                is_stale: false
            }
        );

        entry.change_count = 0;
        entry.source_hash = source_hash.to_string();
    }

    pub async fn mark_stale(&self, entry_id: &str, depth: SummaryDepth) {
        let mut states = self.entry_states.write().await;
        if let Some(entry) = states.get_mut(entry_id) {
            if let Some(summary) = entry.summaries.get_mut(&depth) {
                summary.is_stale = true;
            }
        }
    }

    pub async fn should_update_summary(
        &self,
        entry_id: &str,
        layer: MemoryLayer,
        depth: SummaryDepth,
        current_source_hash: &str
    ) -> Option<TriggerResult> {
        let configs = self.layer_configs.read().await;
        let config = configs.get(&layer)?;

        if !config.depths.contains(&depth) {
            return None;
        }

        let states = self.entry_states.read().await;
        let now = current_timestamp();

        match states.get(entry_id) {
            None => Some(TriggerResult {
                entry_id: entry_id.to_string(),
                layer,
                depth,
                reason: TriggerReason::NoExistingSummary,
                triggered_at: now
            }),
            Some(entry) => self.check_entry_triggers(entry, depth, current_source_hash, config, now)
        }
    }

    pub async fn check_all_entries(&self) -> Vec<TriggerResult> {
        let configs = self.layer_configs.read().await;
        let states = self.entry_states.read().await;
        let now = current_timestamp();
        let mut results = Vec::new();

        for (_, entry) in states.iter() {
            if let Some(config) = configs.get(&entry.layer) {
                for depth in &config.depths {
                    if let Some(trigger) =
                        self.check_entry_triggers(entry, *depth, &entry.source_hash, config, now)
                    {
                        results.push(trigger);
                    }
                }
            }
        }

        results
    }

    pub async fn get_entries_needing_update(&self, layer: MemoryLayer) -> Vec<TriggerResult> {
        let configs = self.layer_configs.read().await;
        let config = match configs.get(&layer) {
            Some(c) => c,
            None => return Vec::new()
        };

        let states = self.entry_states.read().await;
        let now = current_timestamp();
        let mut results = Vec::new();

        for (_, entry) in states.iter() {
            if entry.layer != layer {
                continue;
            }

            for depth in &config.depths {
                if let Some(trigger) =
                    self.check_entry_triggers(entry, *depth, &entry.source_hash, config, now)
                {
                    results.push(trigger);
                }
            }
        }

        results
    }

    fn check_entry_triggers(
        &self,
        entry: &EntryState,
        depth: SummaryDepth,
        current_source_hash: &str,
        config: &SummaryConfig,
        now: i64
    ) -> Option<TriggerResult> {
        match entry.summaries.get(&depth) {
            None => Some(TriggerResult {
                entry_id: entry.entry_id.clone(),
                layer: entry.layer,
                depth,
                reason: TriggerReason::NoExistingSummary,
                triggered_at: now
            }),
            Some(summary) => {
                if summary.is_stale {
                    return Some(TriggerResult {
                        entry_id: entry.entry_id.clone(),
                        layer: entry.layer,
                        depth,
                        reason: TriggerReason::ManualRefresh,
                        triggered_at: now
                    });
                }

                if self.config.enable_hash_based_triggers
                    && entry.source_hash != current_source_hash
                {
                    return Some(TriggerResult {
                        entry_id: entry.entry_id.clone(),
                        layer: entry.layer,
                        depth,
                        reason: TriggerReason::SourceHashChanged {
                            previous_hash: entry.source_hash.clone(),
                            new_hash: current_source_hash.to_string()
                        },
                        triggered_at: now
                    });
                }

                if self.config.enable_time_based_triggers {
                    if let Some(interval) = config.update_interval_secs {
                        let age = (now - summary.generated_at) as u64;
                        if age >= interval {
                            return Some(TriggerResult {
                                entry_id: entry.entry_id.clone(),
                                layer: entry.layer,
                                depth,
                                reason: TriggerReason::TimeThresholdExceeded {
                                    age_seconds: age,
                                    threshold_seconds: interval
                                },
                                triggered_at: now
                            });
                        }
                    }
                }

                if self.config.enable_change_count_triggers {
                    if let Some(threshold) = config.update_on_changes {
                        if entry.change_count >= threshold {
                            return Some(TriggerResult {
                                entry_id: entry.entry_id.clone(),
                                layer: entry.layer,
                                depth,
                                reason: TriggerReason::ChangeCountExceeded {
                                    change_count: entry.change_count,
                                    threshold
                                },
                                triggered_at: now
                            });
                        }
                    }
                }

                None
            }
        }
    }

    pub async fn clear_entry(&self, entry_id: &str) {
        let mut states = self.entry_states.write().await;
        states.remove(entry_id);
    }

    pub async fn entry_count(&self) -> usize {
        let states = self.entry_states.read().await;
        states.len()
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SummaryConfig {
        SummaryConfig {
            layer: MemoryLayer::Session,
            update_interval_secs: Some(60),
            update_on_changes: Some(5),
            skip_if_unchanged: true,
            personalized: false,
            depths: vec![SummaryDepth::Sentence, SummaryDepth::Paragraph]
        }
    }

    #[tokio::test]
    async fn test_new_entry_triggers_no_summary() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash1"
            )
            .await;

        assert!(result.is_some());
        let trigger = result.unwrap();
        assert_eq!(trigger.entry_id, "entry1");
        assert!(matches!(trigger.reason, TriggerReason::NoExistingSummary));
    }

    #[tokio::test]
    async fn test_no_trigger_for_unconfigured_layer() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash1"
            )
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_no_trigger_for_unconfigured_depth() {
        let mut config = test_config();
        config.depths = vec![SummaryDepth::Detailed];

        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(config).await;

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash1"
            )
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_hash_change_triggers_update() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "old_source_hash"
            )
            .await;

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "new_source_hash"
            )
            .await;

        assert!(result.is_some());
        let trigger = result.unwrap();
        assert!(matches!(
            trigger.reason,
            TriggerReason::SourceHashChanged { .. }
        ));
    }

    #[tokio::test]
    async fn test_change_count_triggers_update() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "source_hash"
            )
            .await;

        for _ in 0..5 {
            monitor
                .record_change("entry1", MemoryLayer::Session, "source_hash")
                .await;
        }

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "source_hash"
            )
            .await;

        assert!(result.is_some());
        let trigger = result.unwrap();
        assert!(matches!(
            trigger.reason,
            TriggerReason::ChangeCountExceeded {
                change_count: 5,
                threshold: 5
            }
        ));
    }

    #[tokio::test]
    async fn test_stale_summary_triggers_update() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "source_hash"
            )
            .await;

        monitor.mark_stale("entry1", SummaryDepth::Sentence).await;

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "source_hash"
            )
            .await;

        assert!(result.is_some());
        let trigger = result.unwrap();
        assert!(matches!(trigger.reason, TriggerReason::ManualRefresh));
    }

    #[tokio::test]
    async fn test_no_trigger_when_up_to_date() {
        let mut config = test_config();
        config.update_interval_secs = None;
        config.update_on_changes = None;

        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(config).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "source_hash"
            )
            .await;

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "source_hash"
            )
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_check_all_entries() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash1",
                "src1"
            )
            .await;
        monitor
            .record_summary_generated(
                "entry2",
                MemoryLayer::Session,
                SummaryDepth::Paragraph,
                "hash2",
                "src2"
            )
            .await;

        monitor.mark_stale("entry1", SummaryDepth::Sentence).await;
        monitor.mark_stale("entry2", SummaryDepth::Paragraph).await;

        let results = monitor.check_all_entries().await;

        assert!(results.len() >= 2);
    }

    #[tokio::test]
    async fn test_get_entries_needing_update() {
        let mut session_config = test_config();
        session_config.depths = vec![SummaryDepth::Sentence];

        let mut project_config = test_config();
        project_config.layer = MemoryLayer::Project;
        project_config.depths = vec![SummaryDepth::Sentence];

        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(session_config).await;
        monitor.register_layer_config(project_config).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash1",
                "src1"
            )
            .await;
        monitor
            .record_summary_generated(
                "entry2",
                MemoryLayer::Project,
                SummaryDepth::Sentence,
                "hash2",
                "src2"
            )
            .await;

        monitor.mark_stale("entry1", SummaryDepth::Sentence).await;
        monitor.mark_stale("entry2", SummaryDepth::Sentence).await;

        let session_results = monitor
            .get_entries_needing_update(MemoryLayer::Session)
            .await;
        let project_results = monitor
            .get_entries_needing_update(MemoryLayer::Project)
            .await;

        assert_eq!(session_results.len(), 1);
        assert_eq!(session_results[0].entry_id, "entry1");

        assert_eq!(project_results.len(), 1);
        assert_eq!(project_results[0].entry_id, "entry2");
    }

    #[tokio::test]
    async fn test_clear_entry() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "hash",
                "src"
            )
            .await;

        assert_eq!(monitor.entry_count().await, 1);

        monitor.clear_entry("entry1").await;

        assert_eq!(monitor.entry_count().await, 0);
    }

    #[tokio::test]
    async fn test_disabled_triggers() {
        let config = TriggerMonitorConfig {
            enable_time_based_triggers: false,
            enable_change_count_triggers: false,
            enable_hash_based_triggers: false,
            ..Default::default()
        };

        let monitor = SummaryTriggerMonitor::new(config);
        monitor.register_layer_config(test_config()).await;

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "old_hash"
            )
            .await;

        for _ in 0..10 {
            monitor
                .record_change("entry1", MemoryLayer::Session, "new_hash")
                .await;
        }

        let result = monitor
            .should_update_summary(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "new_hash"
            )
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_record_change_increments_count() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());

        monitor
            .record_change("entry1", MemoryLayer::Session, "hash1")
            .await;
        monitor
            .record_change("entry1", MemoryLayer::Session, "hash1")
            .await;
        monitor
            .record_change("entry1", MemoryLayer::Session, "hash1")
            .await;

        let states = monitor.entry_states.read().await;
        let entry = states.get("entry1").unwrap();
        assert_eq!(entry.change_count, 3);
    }

    #[tokio::test]
    async fn test_summary_generated_resets_change_count() {
        let monitor = SummaryTriggerMonitor::new(TriggerMonitorConfig::default());

        monitor
            .record_change("entry1", MemoryLayer::Session, "hash1")
            .await;
        monitor
            .record_change("entry1", MemoryLayer::Session, "hash1")
            .await;

        {
            let states = monitor.entry_states.read().await;
            let entry = states.get("entry1").unwrap();
            assert_eq!(entry.change_count, 2);
        }

        monitor
            .record_summary_generated(
                "entry1",
                MemoryLayer::Session,
                SummaryDepth::Sentence,
                "summary_hash",
                "hash1"
            )
            .await;

        let states = monitor.entry_states.read().await;
        let entry = states.get("entry1").unwrap();
        assert_eq!(entry.change_count, 0);
    }
}
