//! Section 13.7: Policy Rollback
//!
//! Implements policy version history, rollback capability, and automatic
//! rollback on error rate thresholds.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Policy version manager with rollback support.
pub struct PolicyVersionManager {
    max_versions: usize,
    error_threshold: f64,
    versions: Arc<RwLock<HashMap<String, Vec<PolicyVersion>>>>,
    error_rates: Arc<RwLock<HashMap<String, ErrorMetrics>>>
}

/// Policy version with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyVersion {
    pub version_id: String,
    pub policy_id: String,
    pub policy_data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub version_number: u32,
    pub change_summary: String,
    pub is_active: bool
}

/// Error metrics for automatic rollback.
#[derive(Debug, Clone, Default)]
pub struct ErrorMetrics {
    pub total_requests: u64,
    pub error_requests: u64,
    pub error_rate: f64,
    pub last_calculated: Option<DateTime<Utc>>
}

/// Rollback result.
#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub success: bool,
    pub rolled_back_to_version: String,
    pub previous_version: String,
    pub timestamp: DateTime<Utc>,
    pub reason: String
}

/// Version diff result.
#[derive(Debug, Clone)]
pub struct VersionDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    pub unchanged: Vec<String>
}

impl PolicyVersionManager {
    /// Create new policy version manager.
    pub fn new(max_versions: usize, error_threshold: f64) -> Self {
        Self {
            max_versions,
            error_threshold,
            versions: Arc::new(RwLock::new(HashMap::new())),
            error_rates: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    /// Store new policy version (13.7.2).
    pub async fn store_version(
        &self,
        policy_id: &str,
        policy_data: serde_json::Value,
        created_by: &str,
        change_summary: &str
    ) -> Result<PolicyVersion, RollbackError> {
        let mut versions = self.versions.write().await;
        let policy_versions = versions.entry(policy_id.to_string()).or_default();

        // Deactivate current version
        for v in policy_versions.iter_mut() {
            v.is_active = false;
        }

        // Create new version
        let version_number = policy_versions.len() as u32 + 1;
        let version = PolicyVersion {
            version_id: format!("{}-v{}", policy_id, version_number),
            policy_id: policy_id.to_string(),
            policy_data,
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            version_number,
            change_summary: change_summary.to_string(),
            is_active: true
        };

        policy_versions.push(version.clone());

        // Trim to max versions
        if policy_versions.len() > self.max_versions {
            policy_versions.remove(0);
            // Renumber versions
            for (i, v) in policy_versions.iter_mut().enumerate() {
                v.version_number = (i + 1) as u32;
            }
        }

        info!(
            "Stored policy version {} for policy {} (version {})",
            version.version_id, policy_id, version_number
        );

        Ok(version)
    }

    /// Rollback to previous version (13.7.1).
    pub async fn rollback(
        &self,
        policy_id: &str,
        target_version: Option<u32>,
        reason: &str
    ) -> Result<RollbackResult, RollbackError> {
        let mut versions = self.versions.write().await;
        let policy_versions = versions
            .get_mut(policy_id)
            .ok_or_else(|| RollbackError::PolicyNotFound(policy_id.to_string()))?;

        if policy_versions.len() < 2 {
            return Err(RollbackError::NoPreviousVersion);
        }

        // Find current active version
        let current_idx = policy_versions
            .iter()
            .position(|v| v.is_active)
            .ok_or_else(|| RollbackError::NoActiveVersion)?;

        let current_version = policy_versions[current_idx].clone();

        // Determine target version
        let target_idx = match target_version {
            Some(vn) => policy_versions
                .iter()
                .position(|v| v.version_number == vn)
                .ok_or_else(|| RollbackError::VersionNotFound(vn))?,
            None => current_idx.saturating_sub(1)
        };

        let target = policy_versions[target_idx].clone();

        // Deactivate current, activate target
        policy_versions[current_idx].is_active = false;
        policy_versions[target_idx].is_active = true;

        let result = RollbackResult {
            success: true,
            rolled_back_to_version: target.version_id.clone(),
            previous_version: current_version.version_id.clone(),
            timestamp: Utc::now(),
            reason: reason.to_string()
        };

        info!(
            "Rolled back policy {} from version {} to version {}: {}",
            policy_id, current_version.version_number, target.version_number, reason
        );

        // 13.7.5: Log rollback in audit trail
        self.log_rollback_audit(&result, policy_id).await?;

        Ok(result)
    }

    /// Check for automatic rollback (13.7.3).
    pub async fn check_automatic_rollback(&self, policy_id: &str) -> Option<RollbackResult> {
        let error_rates = self.error_rates.read().await;
        let metrics = error_rates.get(policy_id)?;

        if metrics.error_rate > self.error_threshold {
            warn!(
                "Error rate {} exceeds threshold {} for policy {}. Triggering automatic rollback.",
                metrics.error_rate, self.error_threshold, policy_id
            );

            drop(error_rates);

            match self
                .rollback(
                    policy_id,
                    None,
                    &format!(
                        "Automatic rollback: error rate {:.2}% exceeded threshold {:.2}%",
                        metrics.error_rate * 100.0,
                        self.error_threshold * 100.0
                    )
                )
                .await
            {
                Ok(result) => Some(result),
                Err(e) => {
                    error!("Automatic rollback failed: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Record error for error rate calculation.
    pub async fn record_error(&self, policy_id: &str, has_error: bool) {
        let mut error_rates = self.error_rates.write().await;
        let metrics = error_rates.entry(policy_id.to_string()).or_default();

        metrics.total_requests += 1;
        if has_error {
            metrics.error_requests += 1;
        }

        metrics.error_rate = metrics.error_requests as f64 / metrics.total_requests as f64;
        metrics.last_calculated = Some(Utc::now());
    }

    /// Get version history for a policy.
    pub async fn get_version_history(&self, policy_id: &str) -> Vec<PolicyVersion> {
        self.versions
            .read()
            .await
            .get(policy_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get current active version.
    pub async fn get_active_version(&self, policy_id: &str) -> Option<PolicyVersion> {
        self.versions
            .read()
            .await
            .get(policy_id)
            .and_then(|versions| versions.iter().find(|v| v.is_active).cloned())
    }

    /// Diff two policy versions (13.7.4).
    pub async fn diff_versions(
        &self,
        policy_id: &str,
        version_a: u32,
        version_b: u32
    ) -> Result<VersionDiff, RollbackError> {
        let versions = self.versions.read().await;
        let policy_versions = versions
            .get(policy_id)
            .ok_or_else(|| RollbackError::PolicyNotFound(policy_id.to_string()))?;

        let v_a = policy_versions
            .iter()
            .find(|v| v.version_number == version_a)
            .ok_or_else(|| RollbackError::VersionNotFound(version_a))?;

        let v_b = policy_versions
            .iter()
            .find(|v| v.version_number == version_b)
            .ok_or_else(|| RollbackError::VersionNotFound(version_b))?;

        // Extract rule IDs from each version
        let rules_a: HashSet<String> = Self::extract_rule_ids(&v_a.policy_data);
        let rules_b: HashSet<String> = Self::extract_rule_ids(&v_b.policy_data);

        let added: Vec<String> = rules_b.difference(&rules_a).cloned().collect();
        let removed: Vec<String> = rules_a.difference(&rules_b).cloned().collect();
        let common: HashSet<String> = rules_a.intersection(&rules_b).cloned().collect();

        // For common rules, check if modified
        let mut modified = Vec::new();
        let mut unchanged = Vec::new();

        for rule_id in &common {
            if Self::rule_changed(&v_a.policy_data, &v_b.policy_data, rule_id) {
                modified.push(rule_id.clone());
            } else {
                unchanged.push(rule_id.clone());
            }
        }

        Ok(VersionDiff {
            added,
            removed,
            modified,
            unchanged
        })
    }

    /// Extract rule IDs from policy data.
    fn extract_rule_ids(policy_data: &serde_json::Value) -> HashSet<String> {
        let mut ids = HashSet::new();

        if let Some(rules) = policy_data.get("rules").and_then(|r| r.as_array()) {
            for rule in rules {
                if let Some(id) = rule.get("id").and_then(|i| i.as_str()) {
                    ids.insert(id.to_string());
                }
            }
        }

        ids
    }

    /// Check if a rule changed between versions.
    fn rule_changed(
        policy_a: &serde_json::Value,
        policy_b: &serde_json::Value,
        rule_id: &str
    ) -> bool {
        let rule_a = Self::find_rule(policy_a, rule_id);
        let rule_b = Self::find_rule(policy_b, rule_id);

        rule_a != rule_b
    }

    /// Find rule by ID in policy data.
    fn find_rule<'a>(
        policy_data: &'a serde_json::Value,
        rule_id: &str
    ) -> Option<&'a serde_json::Value> {
        policy_data
            .get("rules")?
            .as_array()?
            .iter()
            .find(|r| r.get("id")?.as_str()? == rule_id)
    }

    /// Log rollback in audit trail (13.7.5).
    async fn log_rollback_audit(
        &self,
        result: &RollbackResult,
        policy_id: &str
    ) -> Result<(), RollbackError> {
        // In real implementation: write to audit log
        info!(
            "AUDIT: Policy rollback - policy={}, from_version={}, to_version={}, reason={}",
            policy_id, result.previous_version, result.rolled_back_to_version, result.reason
        );
        Ok(())
    }
}

use std::collections::HashSet;

/// Rollback errors.
#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("Policy not found: {0}")]
    PolicyNotFound(String),

    #[error("No previous version available")]
    NoPreviousVersion,

    #[error("No active version found")]
    NoActiveVersion,

    #[error("Version {0} not found")]
    VersionNotFound(u32),

    #[error("Audit log error: {0}")]
    AuditError(String)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_storage_and_rollback() {
        let manager = PolicyVersionManager::new(10, 0.1);

        // Store versions
        let v1 = manager
            .store_version(
                "policy-1",
                serde_json::json!({"rules": [{"id": "rule-1", "action": "allow"}]}),
                "user-1",
                "Initial version"
            )
            .await
            .unwrap();

        let v2 = manager
            .store_version(
                "policy-1",
                serde_json::json!({"rules": [{"id": "rule-1", "action": "deny"}]}),
                "user-1",
                "Changed to deny"
            )
            .await
            .unwrap();

        assert_eq!(v2.version_number, 2);
        assert!(!v1.is_active);
        assert!(v2.is_active);

        // Rollback
        let rollback_result = manager
            .rollback("policy-1", None, "Test rollback")
            .await
            .unwrap();

        assert!(rollback_result.success);
        assert_eq!(rollback_result.rolled_back_to_version, v1.version_id);
    }

    #[tokio::test]
    async fn test_automatic_rollback() {
        let manager = PolicyVersionManager::new(10, 0.1);

        // Store versions
        manager
            .store_version(
                "policy-1",
                serde_json::json!({"version": 1}),
                "user-1",
                "Initial"
            )
            .await
            .unwrap();

        manager
            .store_version(
                "policy-1",
                serde_json::json!({"version": 2}),
                "user-1",
                "Updated"
            )
            .await
            .unwrap();

        // Record errors above threshold
        for _ in 0..20 {
            manager.record_error("policy-1", true).await;
        }

        // Check automatic rollback
        let rollback = manager.check_automatic_rollback("policy-1").await;
        assert!(rollback.is_some());
    }
}
